"""
Execution Routes

Provides endpoints for task execution and PRD management.
"""

from datetime import datetime
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, Field

from ..state import ExecutionStatus, StoryStatus

router = APIRouter()


class ExecuteRequest(BaseModel):
    """Request model for task execution."""
    description: str = Field(..., description="Task description")
    mode: str = Field(default="simple", description="Execution mode: simple or expert")
    project_path: Optional[str] = Field(default=None, description="Project path")
    use_worktree: bool = Field(default=False, description="Use git worktree")


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


@router.post("/execute", response_model=ExecuteResponse)
async def execute_task(request: Request, body: ExecuteRequest) -> ExecuteResponse:
    """
    Start task execution.

    Initiates task execution based on the provided description and mode.
    Progress updates are sent via WebSocket.
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

    # Update execution state
    app_state.execution.status = ExecutionStatus.RUNNING
    app_state.execution.task_description = body.description
    app_state.execution.started_at = datetime.utcnow()
    app_state.execution.error = None

    # Broadcast status update
    await app_state.emit_event("execution_started", {
        "task_id": task_id,
        "description": body.description,
        "mode": body.mode,
    })

    # TODO: Actually start the execution in background
    # This will integrate with the plan_cascade core package

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

    # Update status
    app_state.execution.status = ExecutionStatus.IDLE
    app_state.execution.error = "Cancelled by user"

    # Broadcast cancellation
    await app_state.emit_event("execution_cancelled", {
        "reason": "user_request"
    })

    return {"status": "cancelled", "message": "Task execution cancelled"}


@router.post("/prd/generate", response_model=PRDResponse)
async def generate_prd(request: Request, body: PRDRequest) -> PRDResponse:
    """
    Generate a PRD from task description.

    Creates a structured PRD with user stories based on the description.
    """
    # TODO: Integrate with plan_cascade PRD generator
    # For now, return a mock response

    mock_stories = [
        {
            "id": "story-001",
            "title": "Initial Setup",
            "description": "Set up project structure",
            "status": "pending",
            "priority": 1,
            "dependencies": [],
        },
        {
            "id": "story-002",
            "title": "Core Implementation",
            "description": "Implement core functionality",
            "status": "pending",
            "priority": 2,
            "dependencies": ["story-001"],
        },
    ]

    return PRDResponse(
        prd_path="/tmp/prd.json",
        stories=mock_stories,
    )


@router.post("/prd/approve")
async def approve_prd(request: Request) -> Dict[str, Any]:
    """
    Approve the generated PRD and start execution.
    """
    app_state = request.app.state.app_state

    if not app_state.execution.prd_path:
        raise HTTPException(
            status_code=400,
            detail="No PRD has been generated"
        )

    # Start execution
    app_state.execution.status = ExecutionStatus.RUNNING

    await app_state.emit_event("prd_approved", {
        "prd_path": app_state.execution.prd_path
    })

    return {"status": "approved", "message": "PRD approved, execution started"}
