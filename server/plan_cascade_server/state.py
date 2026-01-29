"""
Application State Management

Manages the global state for the sidecar server including
execution status, connected clients, task management, and logging.
"""

import asyncio
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Dict, List, Optional, Set

from fastapi import WebSocket


class ExecutionStatus(str, Enum):
    """Task execution status."""
    IDLE = "idle"
    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"


class LogLevel(str, Enum):
    """Log entry severity levels."""
    DEBUG = "debug"
    INFO = "info"
    WARN = "warn"
    ERROR = "error"


@dataclass
class LogEntry:
    """A single log entry."""
    timestamp: datetime
    level: LogLevel
    source: str
    message: str
    details: Optional[Dict[str, Any]] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "timestamp": self.timestamp.isoformat(),
            "level": self.level.value,
            "source": self.source,
            "message": self.message,
            "details": self.details,
        }


@dataclass
class BatchStatus:
    """Status of an execution batch."""
    batch_num: int
    total_batches: int
    story_ids: List[str]
    status: str = "pending"  # pending, in_progress, completed, failed
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "batch_num": self.batch_num,
            "total_batches": self.total_batches,
            "story_ids": self.story_ids,
            "status": self.status,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
        }


@dataclass
class QualityGateResult:
    """Result of a quality gate check."""
    gate_type: str  # typecheck, test, lint, custom
    passed: bool
    output: str = ""
    duration_seconds: float = 0.0

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "gate_type": self.gate_type,
            "passed": self.passed,
            "output": self.output,
            "duration_seconds": self.duration_seconds,
        }


@dataclass
class StoryStatus:
    """Status of a single story in the PRD."""
    id: str
    title: str
    status: str = "pending"  # pending, in_progress, completed, failed
    progress: float = 0.0
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    error: Optional[str] = None
    retry_count: int = 0
    quality_gate_results: List[QualityGateResult] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "id": self.id,
            "title": self.title,
            "status": self.status,
            "progress": self.progress,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
            "error": self.error,
            "retry_count": self.retry_count,
            "quality_gate_results": [qg.to_dict() for qg in self.quality_gate_results],
        }


@dataclass
class ExecutionState:
    """Current execution state."""
    status: ExecutionStatus = ExecutionStatus.IDLE
    task_id: Optional[str] = None
    task_description: str = ""
    strategy: Optional[str] = None  # direct, hybrid_auto, mega_plan
    prd_path: Optional[str] = None
    stories: List[StoryStatus] = field(default_factory=list)
    batches: List[BatchStatus] = field(default_factory=list)
    current_batch: int = 0
    current_story_id: Optional[str] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        # Calculate overall progress
        total = len(self.stories)
        completed = sum(1 for s in self.stories if s.status == "completed")
        overall_progress = (completed / total * 100) if total > 0 else 0

        return {
            "status": self.status.value,
            "task_id": self.task_id,
            "task_description": self.task_description,
            "strategy": self.strategy,
            "prd_path": self.prd_path,
            "stories": [s.to_dict() for s in self.stories],
            "batches": [b.to_dict() for b in self.batches],
            "current_batch": self.current_batch,
            "current_story_id": self.current_story_id,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
            "error": self.error,
            "overall_progress": overall_progress,
            "stories_completed": completed,
            "stories_total": total,
        }


# Event types documentation
EVENT_TYPES = {
    # Execution lifecycle
    "execution_started": "Task execution has started",
    "execution_completed": "Task execution completed successfully",
    "execution_failed": "Task execution failed with an error",
    "execution_cancelled": "Task execution was cancelled by user",
    "execution_paused": "Task execution was paused",
    "execution_resumed": "Task execution was resumed",
    "execution_update": "General execution state update",

    # Strategy events
    "strategy_decided": "AI determined the execution strategy",

    # Batch events
    "batch_started": "A batch of stories has started execution",
    "batch_completed": "A batch of stories has completed",
    "batch_failed": "A batch of stories has failed",

    # Story events
    "story_started": "A story has started execution",
    "story_progress": "Progress update for a story (includes percentage)",
    "story_completed": "A story has completed successfully",
    "story_failed": "A story has failed",
    "story_update": "General story status update",

    # Quality gate events
    "quality_gate_started": "A quality gate check has started",
    "quality_gate_passed": "A quality gate check passed",
    "quality_gate_failed": "A quality gate check failed",

    # Retry events
    "retry_started": "A retry attempt has started for a failed story",

    # PRD events
    "prd_generated": "PRD has been generated",
    "prd_approved": "PRD has been approved for execution",
    "prd_updated": "PRD has been updated",

    # Log events
    "log_entry": "A new log entry has been recorded",

    # Connection events
    "connected": "WebSocket client connected",
    "ping": "Keep-alive ping",
    "pong": "Keep-alive pong response",
}


class AppState:
    """Global application state."""

    # Maximum log entries to keep in memory
    MAX_LOG_ENTRIES = 1000

    def __init__(self):
        self.execution = ExecutionState()
        self._websocket_clients: Set[WebSocket] = set()
        self._lock = asyncio.Lock()
        self._event_handlers: Dict[str, List[Callable]] = {}
        self._log_buffer: deque[LogEntry] = deque(maxlen=self.MAX_LOG_ENTRIES)
        self._log_subscribers: Set[asyncio.Queue] = set()

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
        message = {"type": event_type, "data": data, "timestamp": datetime.utcnow().isoformat()}
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
            try:
                if asyncio.iscoroutinefunction(handler):
                    await handler(data)
                else:
                    handler(data)
            except Exception:
                pass  # Don't let handler errors break event emission

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
                        if status == "in_progress" and story.started_at is None:
                            story.started_at = datetime.utcnow()
                        elif status in ("completed", "failed"):
                            story.completed_at = datetime.utcnow()
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

    async def start_batch(self, batch_num: int, total_batches: int, story_ids: List[str]):
        """Start a new batch of stories."""
        async with self._lock:
            batch = BatchStatus(
                batch_num=batch_num,
                total_batches=total_batches,
                story_ids=story_ids,
                status="in_progress",
                started_at=datetime.utcnow(),
            )
            self.execution.batches.append(batch)
            self.execution.current_batch = batch_num

        await self.emit_event("batch_started", {
            "batch_num": batch_num,
            "total_batches": total_batches,
            "story_ids": story_ids,
        })

    async def complete_batch(self, batch_num: int, success: bool = True):
        """Complete a batch of stories."""
        async with self._lock:
            for batch in self.execution.batches:
                if batch.batch_num == batch_num:
                    batch.status = "completed" if success else "failed"
                    batch.completed_at = datetime.utcnow()
                    break

        event_type = "batch_completed" if success else "batch_failed"
        await self.emit_event(event_type, {"batch_num": batch_num, "success": success})

    async def record_quality_gate(
        self,
        story_id: str,
        gate_type: str,
        passed: bool,
        output: str = "",
        duration_seconds: float = 0.0,
    ):
        """Record a quality gate result for a story."""
        result = QualityGateResult(
            gate_type=gate_type,
            passed=passed,
            output=output,
            duration_seconds=duration_seconds,
        )

        async with self._lock:
            for story in self.execution.stories:
                if story.id == story_id:
                    story.quality_gate_results.append(result)
                    break

        event_type = "quality_gate_passed" if passed else "quality_gate_failed"
        await self.emit_event(event_type, {
            "story_id": story_id,
            "gate_type": gate_type,
            "passed": passed,
            "output": output[:500],  # Truncate output for event
        })

    async def start_retry(self, story_id: str, attempt: int, max_attempts: int):
        """Record a retry attempt for a story."""
        async with self._lock:
            for story in self.execution.stories:
                if story.id == story_id:
                    story.retry_count = attempt
                    story.status = "in_progress"
                    story.error = None
                    break

        await self.emit_event("retry_started", {
            "story_id": story_id,
            "attempt": attempt,
            "max_attempts": max_attempts,
        })

    # Logging methods

    async def log(
        self,
        level: LogLevel,
        source: str,
        message: str,
        details: Optional[Dict[str, Any]] = None,
    ):
        """Add a log entry and notify subscribers."""
        entry = LogEntry(
            timestamp=datetime.utcnow(),
            level=level,
            source=source,
            message=message,
            details=details,
        )

        async with self._lock:
            self._log_buffer.append(entry)

        # Notify log subscribers
        for queue in list(self._log_subscribers):
            try:
                queue.put_nowait(entry)
            except asyncio.QueueFull:
                pass  # Drop if queue is full

        # Also emit as event
        await self.emit_event("log_entry", entry.to_dict())

    def get_recent_logs(
        self,
        limit: int = 100,
        level: Optional[LogLevel] = None,
    ) -> List[Dict[str, Any]]:
        """Get recent log entries."""
        logs = list(self._log_buffer)

        if level:
            # Filter by level (and higher severity)
            level_order = [LogLevel.DEBUG, LogLevel.INFO, LogLevel.WARN, LogLevel.ERROR]
            min_index = level_order.index(level)
            logs = [log for log in logs if level_order.index(log.level) >= min_index]

        # Return most recent entries
        return [log.to_dict() for log in logs[-limit:]]

    def subscribe_to_logs(self) -> asyncio.Queue:
        """Subscribe to log stream. Returns a queue that receives new log entries."""
        queue = asyncio.Queue(maxsize=100)
        self._log_subscribers.add(queue)
        return queue

    def unsubscribe_from_logs(self, queue: asyncio.Queue):
        """Unsubscribe from log stream."""
        self._log_subscribers.discard(queue)

    async def cleanup(self):
        """Cleanup resources on shutdown."""
        # Close all WebSocket connections
        for ws in list(self._websocket_clients):
            try:
                await ws.close()
            except Exception:
                pass
        self._websocket_clients.clear()

        # Clear log subscribers
        self._log_subscribers.clear()
