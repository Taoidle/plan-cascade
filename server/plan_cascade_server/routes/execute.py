"""
Execution Routes

Provides endpoints for task execution and PRD management.
Integrates with Plan Cascade core for real orchestration.
"""

import asyncio
import json
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, BackgroundTasks, HTTPException, Request
from pydantic import BaseModel, Field

from ..state import ExecutionStatus, StoryStatus

router = APIRouter()

# Store for background task cancellation
_cancel_flags: Dict[str, asyncio.Event] = {}


class ExecuteRequest(BaseModel):
    """Request model for task execution."""
    description: str = Field(..., description="Task description")
    mode: str = Field(default="simple", description="Execution mode: simple or expert")
    project_path: Optional[str] = Field(default=None, description="Project path")
    use_worktree: bool = Field(default=False, description="Use git worktree")
    strategy: Optional[str] = Field(default=None, description="Execution strategy override")
    prd: Optional[Dict[str, Any]] = Field(default=None, description="PRD for expert mode")


class ExecuteResponse(BaseModel):
    """Response model for task execution."""
    task_id: str
    status: str
    message: str


class PRDRequest(BaseModel):
    """Request model for PRD generation."""
    description: str = Field(..., description="Task description for PRD generation")
    context: Optional[str] = Field(default=None, description="Additional context")


class PRDResponse(BaseModel):
    """Response model for PRD generation."""
    prd_path: str
    stories: List[Dict[str, Any]]
    metadata: Optional[Dict[str, Any]] = None


class PRDUpdateRequest(BaseModel):
    """Request model for updating PRD."""
    stories: Optional[List[Dict[str, Any]]] = Field(default=None, description="Updated stories list")
    metadata: Optional[Dict[str, Any]] = Field(default=None, description="Updated metadata")


async def _run_execution(
    app_state: Any,
    task_id: str,
    description: str,
    mode: str,
    project_path: Optional[str],
    use_worktree: bool,
    strategy: Optional[str] = None,
    prd: Optional[Dict[str, Any]] = None,
    cancel_event: Optional[asyncio.Event] = None,
) -> None:
    """
    Background task to run the actual execution.

    Integrates with Plan Cascade core SimpleWorkflow and ExpertWorkflow.
    """
    try:
        from plan_cascade.backends.builtin import BuiltinAgent
        from plan_cascade.core.simple_workflow import SimpleWorkflow, ProgressEvent

        # Determine project path
        path = Path(project_path) if project_path else Path.cwd()

        # Create progress callback that emits WebSocket events
        async def on_progress(event: ProgressEvent) -> None:
            # Check for cancellation
            if cancel_event and cancel_event.is_set():
                raise asyncio.CancelledError("Execution cancelled by user")

            event_data = {
                "task_id": task_id,
                **event.data
            }
            await app_state.emit_event(event.type, event_data)

            # Update story status in app state if applicable
            if event.type == "story_started":
                story_id = event.data.get("story_id", "")
                title = event.data.get("title", "")
                app_state.execution.stories.append(
                    StoryStatus(id=story_id, title=title, status="in_progress")
                )
                app_state.execution.current_story_id = story_id
            elif event.type == "story_completed":
                story_id = event.data.get("story_id", "")
                for story in app_state.execution.stories:
                    if story.id == story_id:
                        story.status = "completed"
                        story.progress = 100.0
                        story.completed_at = datetime.utcnow()
                        break
            elif event.type == "story_failed":
                story_id = event.data.get("story_id", "")
                error = event.data.get("error", "Unknown error")
                for story in app_state.execution.stories:
                    if story.id == story_id:
                        story.status = "failed"
                        story.error = error
                        break

        # Create backend
        backend = BuiltinAgent(project_path=path)

        if mode == "simple":
            # Simple mode: use SimpleWorkflow
            workflow = SimpleWorkflow(
                backend=backend,
                project_path=path,
                on_progress=on_progress,
                use_llm_strategy=True
            )

            result = await workflow.run(
                description=description,
                context=""
            )
        else:
            # Expert mode: use provided PRD or generate one
            from plan_cascade.core.expert_workflow import ExpertWorkflow

            workflow = ExpertWorkflow(
                backend=backend,
                project_path=path,
                on_progress=on_progress
            )

            if prd:
                result = await workflow.run_with_prd(prd)
            else:
                result = await workflow.run(description=description)

        # Update final state
        if result.success:
            app_state.execution.status = ExecutionStatus.COMPLETED
            app_state.execution.completed_at = datetime.utcnow()
            await app_state.emit_event("execution_completed", {
                "task_id": task_id,
                "success": True,
                "stories_completed": result.stories_completed,
                "stories_total": result.stories_total,
                "duration_seconds": result.duration_seconds,
            })
        else:
            app_state.execution.status = ExecutionStatus.FAILED
            app_state.execution.error = result.error
            app_state.execution.completed_at = datetime.utcnow()
            await app_state.emit_event("execution_failed", {
                "task_id": task_id,
                "success": False,
                "error": result.error,
                "stories_completed": result.stories_completed,
                "stories_total": result.stories_total,
            })

    except asyncio.CancelledError:
        app_state.execution.status = ExecutionStatus.IDLE
        app_state.execution.error = "Cancelled by user"
        app_state.execution.completed_at = datetime.utcnow()
        await app_state.emit_event("execution_cancelled", {
            "task_id": task_id,
            "reason": "user_request",
        })
    except ImportError as e:
        # Plan cascade core not available - fall back to simulation
        app_state.execution.status = ExecutionStatus.FAILED
        app_state.execution.error = f"Plan Cascade core not available: {e}"
        app_state.execution.completed_at = datetime.utcnow()
        await app_state.emit_event("execution_failed", {
            "task_id": task_id,
            "success": False,
            "error": f"Plan Cascade core not available: {e}",
        })
    except Exception as e:
        app_state.execution.status = ExecutionStatus.FAILED
        app_state.execution.error = str(e)
        app_state.execution.completed_at = datetime.utcnow()
        await app_state.emit_event("execution_failed", {
            "task_id": task_id,
            "success": False,
            "error": str(e),
        })
    finally:
        # Clean up cancel flag
        if task_id in _cancel_flags:
            del _cancel_flags[task_id]


@router.post("/execute", response_model=ExecuteResponse)
async def execute_task(
    request: Request,
    body: ExecuteRequest,
    background_tasks: BackgroundTasks
) -> ExecuteResponse:
    """
    Start task execution.

    Initiates task execution based on the provided description and mode.
    Progress updates are sent via WebSocket.

    In simple mode, the strategy is automatically determined.
    In expert mode, a PRD can be provided for execution.
    """
    app_state = request.app.state.app_state

    # Check if already running
    if app_state.execution.status == ExecutionStatus.RUNNING:
        raise HTTPException(
            status_code=409,
            detail="A task is already running. Please wait or cancel it first."
        )

    # Generate a task ID
    task_id = f"task_{datetime.utcnow().strftime('%Y%m%d_%H%M%S')}"

    # Create cancellation event
    cancel_event = asyncio.Event()
    _cancel_flags[task_id] = cancel_event

    # Reset execution state
    app_state.execution.status = ExecutionStatus.RUNNING
    app_state.execution.task_description = body.description
    app_state.execution.started_at = datetime.utcnow()
    app_state.execution.completed_at = None
    app_state.execution.error = None
    app_state.execution.stories = []
    app_state.execution.current_story_id = None

    # Broadcast status update
    await app_state.emit_event("execution_started", {
        "task_id": task_id,
        "description": body.description,
        "mode": body.mode,
    })

    # Start execution in background
    background_tasks.add_task(
        _run_execution,
        app_state,
        task_id,
        body.description,
        body.mode,
        body.project_path,
        body.use_worktree,
        body.strategy,
        body.prd,
        cancel_event,
    )

    return ExecuteResponse(
        task_id=task_id,
        status="started",
        message=f"Task execution started in {body.mode} mode"
    )


@router.post("/execute/cancel")
async def cancel_execution(request: Request) -> Dict[str, Any]:
    """
    Cancel the current execution.

    Attempts to gracefully cancel the running task.
    """
    app_state = request.app.state.app_state

    if app_state.execution.status != ExecutionStatus.RUNNING:
        raise HTTPException(
            status_code=400,
            detail="No task is currently running"
        )

    # Find and set the cancel flag
    for task_id, cancel_event in _cancel_flags.items():
        cancel_event.set()

    # Update status
    app_state.execution.status = ExecutionStatus.IDLE
    app_state.execution.error = "Cancelled by user"

    # Broadcast cancellation
    await app_state.emit_event("execution_cancelled", {
        "reason": "user_request"
    })

    return {"status": "cancelled", "message": "Task execution cancelled"}


@router.post("/execute/pause")
async def pause_execution(request: Request) -> Dict[str, Any]:
    """
    Pause the current execution.

    Pauses execution after the current story completes.
    """
    app_state = request.app.state.app_state

    if app_state.execution.status != ExecutionStatus.RUNNING:
        raise HTTPException(
            status_code=400,
            detail="No task is currently running"
        )

    # Update status to paused
    app_state.execution.status = ExecutionStatus.PAUSED

    # Broadcast pause event
    await app_state.emit_event("execution_paused", {
        "current_story_id": app_state.execution.current_story_id
    })

    return {"status": "paused", "message": "Execution will pause after current story"}


@router.post("/execute/resume")
async def resume_execution(request: Request) -> Dict[str, Any]:
    """
    Resume a paused execution.
    """
    app_state = request.app.state.app_state

    if app_state.execution.status != ExecutionStatus.PAUSED:
        raise HTTPException(
            status_code=400,
            detail="No task is currently paused"
        )

    # Update status to running
    app_state.execution.status = ExecutionStatus.RUNNING

    # Broadcast resume event
    await app_state.emit_event("execution_resumed", {
        "current_story_id": app_state.execution.current_story_id
    })

    return {"status": "running", "message": "Execution resumed"}


@router.post("/prd/generate", response_model=PRDResponse)
async def generate_prd(request: Request, body: PRDRequest) -> PRDResponse:
    """
    Generate a PRD from task description.

    Creates a structured PRD with user stories based on the description.
    Uses the PRDGenerator from plan_cascade core.
    """
    app_state = request.app.state.app_state

    try:
        from plan_cascade.core.prd_generator import PRDGenerator
        from plan_cascade.backends.builtin import BuiltinAgent

        # Create generator
        project_path = Path.cwd()
        generator = PRDGenerator(project_root=project_path)

        # Get backend for LLM access
        backend = BuiltinAgent(project_path=project_path)
        llm = backend.get_llm()

        # Generate PRD using LLM
        prompt = f"""Generate a PRD (Product Requirements Document) for the following task.

## Task Description
{body.description}

## Context
{body.context or "No additional context."}

## Requirements
Create a JSON PRD with the following structure:
```json
{{
    "metadata": {{
        "version": "1.0.0",
        "title": "<brief title>",
        "description": "<brief description>"
    }},
    "goal": "<main goal>",
    "stories": [
        {{
            "id": "story-001",
            "title": "<story title>",
            "description": "<detailed description>",
            "priority": "high" | "medium" | "low",
            "dependencies": [],
            "status": "pending",
            "acceptance_criteria": ["<criterion 1>", "<criterion 2>"]
        }}
    ]
}}
```

Guidelines:
- Break the task into 2-6 user stories
- Order stories by dependency (foundational first)
- Include clear acceptance criteria
- Use high priority for blocking/critical stories

Return ONLY the JSON, no additional text."""

        import re
        response = await llm.complete([{"role": "user", "content": prompt}])

        # Parse JSON from response
        json_match = re.search(r'\{[\s\S]*\}', response.content)
        if not json_match:
            raise ValueError("No JSON found in LLM response")

        prd = json.loads(json_match.group())

        # Save PRD to file
        prd_path = project_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f, indent=2)

        # Store in app state
        app_state.execution.prd_path = str(prd_path)

        return PRDResponse(
            prd_path=str(prd_path),
            stories=prd.get("stories", []),
            metadata=prd.get("metadata", {}),
        )

    except ImportError as e:
        # Fall back to basic PRD structure
        basic_prd = generator.generate_prd(body.description)
        prd_path = project_path / "prd.json"

        with open(prd_path, "w") as f:
            json.dump(basic_prd, f, indent=2)

        return PRDResponse(
            prd_path=str(prd_path),
            stories=basic_prd.get("stories", []),
            metadata=basic_prd.get("metadata", {}),
        )

    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"Failed to generate PRD: {str(e)}"
        )


@router.get("/prd")
async def get_prd(request: Request) -> Dict[str, Any]:
    """
    Get the current PRD.
    """
    app_state = request.app.state.app_state

    if not app_state.execution.prd_path:
        raise HTTPException(
            status_code=404,
            detail="No PRD has been generated"
        )

    try:
        with open(app_state.execution.prd_path, "r") as f:
            prd = json.load(f)
        return prd
    except FileNotFoundError:
        raise HTTPException(
            status_code=404,
            detail="PRD file not found"
        )


@router.put("/prd")
async def update_prd(request: Request, body: PRDUpdateRequest) -> Dict[str, Any]:
    """
    Update the current PRD.
    """
    app_state = request.app.state.app_state

    if not app_state.execution.prd_path:
        raise HTTPException(
            status_code=404,
            detail="No PRD has been generated"
        )

    try:
        # Read current PRD
        with open(app_state.execution.prd_path, "r") as f:
            prd = json.load(f)

        # Update fields
        if body.stories is not None:
            prd["stories"] = body.stories
        if body.metadata is not None:
            prd["metadata"] = {**prd.get("metadata", {}), **body.metadata}

        # Save updated PRD
        with open(app_state.execution.prd_path, "w") as f:
            json.dump(prd, f, indent=2)

        return {"status": "updated", "prd": prd}

    except FileNotFoundError:
        raise HTTPException(
            status_code=404,
            detail="PRD file not found"
        )


@router.delete("/prd")
async def delete_prd(request: Request) -> Dict[str, Any]:
    """
    Delete the current PRD.
    """
    app_state = request.app.state.app_state

    if not app_state.execution.prd_path:
        raise HTTPException(
            status_code=404,
            detail="No PRD has been generated"
        )

    try:
        import os
        os.remove(app_state.execution.prd_path)
        app_state.execution.prd_path = None
        return {"status": "deleted", "message": "PRD deleted successfully"}
    except FileNotFoundError:
        app_state.execution.prd_path = None
        return {"status": "deleted", "message": "PRD was already deleted"}


@router.post("/prd/approve")
async def approve_prd(
    request: Request,
    background_tasks: BackgroundTasks
) -> Dict[str, Any]:
    """
    Approve the generated PRD and start execution.
    """
    app_state = request.app.state.app_state

    if not app_state.execution.prd_path:
        raise HTTPException(
            status_code=400,
            detail="No PRD has been generated"
        )

    if app_state.execution.status == ExecutionStatus.RUNNING:
        raise HTTPException(
            status_code=409,
            detail="A task is already running"
        )

    # Read PRD
    try:
        with open(app_state.execution.prd_path, "r") as f:
            prd = json.load(f)
    except FileNotFoundError:
        raise HTTPException(
            status_code=404,
            detail="PRD file not found"
        )

    # Generate task ID
    task_id = f"task_{datetime.utcnow().strftime('%Y%m%d_%H%M%S')}"

    # Create cancellation event
    cancel_event = asyncio.Event()
    _cancel_flags[task_id] = cancel_event

    # Start execution with the PRD
    app_state.execution.status = ExecutionStatus.RUNNING
    app_state.execution.started_at = datetime.utcnow()
    app_state.execution.stories = []

    await app_state.emit_event("prd_approved", {
        "task_id": task_id,
        "prd_path": app_state.execution.prd_path
    })

    # Start execution in background
    background_tasks.add_task(
        _run_execution,
        app_state,
        task_id,
        prd.get("goal", "Execute PRD"),
        "expert",
        None,
        False,
        None,
        prd,
        cancel_event,
    )

    return {"status": "approved", "task_id": task_id, "message": "PRD approved, execution started"}
