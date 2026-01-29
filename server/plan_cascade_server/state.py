"""
Application State Management

Manages the global state for the sidecar server including
execution status, connected clients, and task management.
"""

import asyncio
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Dict, List, Optional, Set
from weakref import WeakSet

from fastapi import WebSocket


class ExecutionStatus(str, Enum):
    """Task execution status."""
    IDLE = "idle"
    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"


@dataclass
class StoryStatus:
    """Status of a single story in the PRD."""
    id: str
    title: str
    status: str = "pending"
    progress: float = 0.0
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    error: Optional[str] = None


@dataclass
class ExecutionState:
    """Current execution state."""
    status: ExecutionStatus = ExecutionStatus.IDLE
    task_description: str = ""
    prd_path: Optional[str] = None
    stories: List[StoryStatus] = field(default_factory=list)
    current_story_id: Optional[str] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "status": self.status.value,
            "task_description": self.task_description,
            "prd_path": self.prd_path,
            "stories": [
                {
                    "id": s.id,
                    "title": s.title,
                    "status": s.status,
                    "progress": s.progress,
                    "started_at": s.started_at.isoformat() if s.started_at else None,
                    "completed_at": s.completed_at.isoformat() if s.completed_at else None,
                    "error": s.error,
                }
                for s in self.stories
            ],
            "current_story_id": self.current_story_id,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
            "error": self.error,
        }


class AppState:
    """Global application state."""

    def __init__(self):
        self.execution = ExecutionState()
        self._websocket_clients: Set[WebSocket] = set()
        self._lock = asyncio.Lock()
        self._event_handlers: Dict[str, List[Callable]] = {}

    async def add_websocket(self, websocket: WebSocket):
        """Add a WebSocket client."""
        async with self._lock:
            self._websocket_clients.add(websocket)

    async def remove_websocket(self, websocket: WebSocket):
        """Remove a WebSocket client."""
        async with self._lock:
            self._websocket_clients.discard(websocket)

    async def broadcast(self, event_type: str, data: Dict[str, Any]):
        """Broadcast an event to all connected WebSocket clients."""
        message = {"type": event_type, "data": data}
        disconnected = set()

        for ws in self._websocket_clients:
            try:
                await ws.send_json(message)
            except Exception:
                disconnected.add(ws)

        # Clean up disconnected clients
        for ws in disconnected:
            await self.remove_websocket(ws)

    def on_event(self, event_type: str, handler: Callable):
        """Register an event handler."""
        if event_type not in self._event_handlers:
            self._event_handlers[event_type] = []
        self._event_handlers[event_type].append(handler)

    async def emit_event(self, event_type: str, data: Dict[str, Any]):
        """Emit an event to handlers and broadcast to WebSocket clients."""
        # Call local handlers
        for handler in self._event_handlers.get(event_type, []):
            if asyncio.iscoroutinefunction(handler):
                await handler(data)
            else:
                handler(data)

        # Broadcast to WebSocket clients
        await self.broadcast(event_type, data)

    async def update_execution_status(
        self,
        status: Optional[ExecutionStatus] = None,
        current_story_id: Optional[str] = None,
        error: Optional[str] = None,
    ):
        """Update execution status and broadcast to clients."""
        async with self._lock:
            if status is not None:
                self.execution.status = status
            if current_story_id is not None:
                self.execution.current_story_id = current_story_id
            if error is not None:
                self.execution.error = error

        await self.emit_event("execution_update", self.execution.to_dict())

    async def update_story_status(
        self,
        story_id: str,
        status: Optional[str] = None,
        progress: Optional[float] = None,
        error: Optional[str] = None,
    ):
        """Update a story's status and broadcast to clients."""
        async with self._lock:
            for story in self.execution.stories:
                if story.id == story_id:
                    if status is not None:
                        story.status = status
                    if progress is not None:
                        story.progress = progress
                    if error is not None:
                        story.error = error
                    break

        await self.emit_event("story_update", {
            "story_id": story_id,
            "status": status,
            "progress": progress,
            "error": error,
        })

    async def cleanup(self):
        """Cleanup resources on shutdown."""
        # Close all WebSocket connections
        for ws in list(self._websocket_clients):
            try:
                await ws.close()
            except Exception:
                pass
        self._websocket_clients.clear()
