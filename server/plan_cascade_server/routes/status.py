"""
Status Routes

Provides endpoints for checking execution status.
"""

from typing import Any, Dict, List

from fastapi import APIRouter, Request
from pydantic import BaseModel

from ..state import ExecutionStatus

router = APIRouter()


class StatusResponse(BaseModel):
    """Response model for status endpoint."""
    status: str
    task_description: str
    current_story_id: str | None
    stories: List[Dict[str, Any]]
    progress: float


@router.get("/status", response_model=StatusResponse)
async def get_status(request: Request) -> StatusResponse:
    """
    Get current execution status.

    Returns detailed status of the current or last execution.
    """
    app_state = request.app.state.app_state
    execution = app_state.execution

    # Calculate overall progress
    total_stories = len(execution.stories)
    completed_stories = sum(
        1 for s in execution.stories
        if s.status == "completed"
    )
    progress = (completed_stories / total_stories * 100) if total_stories > 0 else 0

    return StatusResponse(
        status=execution.status.value,
        task_description=execution.task_description,
        current_story_id=execution.current_story_id,
        stories=[
            {
                "id": s.id,
                "title": s.title,
                "status": s.status,
                "progress": s.progress,
                "error": s.error,
            }
            for s in execution.stories
        ],
        progress=progress,
    )


@router.get("/status/stories")
async def get_stories_status(request: Request) -> List[Dict[str, Any]]:
    """
    Get status of all stories.

    Returns a list of all stories with their current status.
    """
    app_state = request.app.state.app_state

    return [
        {
            "id": s.id,
            "title": s.title,
            "status": s.status,
            "progress": s.progress,
            "started_at": s.started_at.isoformat() if s.started_at else None,
            "completed_at": s.completed_at.isoformat() if s.completed_at else None,
            "error": s.error,
        }
        for s in app_state.execution.stories
    ]


@router.get("/status/story/{story_id}")
async def get_story_status(request: Request, story_id: str) -> Dict[str, Any]:
    """
    Get status of a specific story.
    """
    app_state = request.app.state.app_state

    for story in app_state.execution.stories:
        if story.id == story_id:
            return {
                "id": story.id,
                "title": story.title,
                "status": story.status,
                "progress": story.progress,
                "started_at": story.started_at.isoformat() if story.started_at else None,
                "completed_at": story.completed_at.isoformat() if story.completed_at else None,
                "error": story.error,
            }

    return {"error": "Story not found"}
