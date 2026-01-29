"""
Log Streaming Routes

Provides endpoints for accessing execution logs via Server-Sent Events (SSE)
and REST API for historical log retrieval.
"""

import asyncio
from typing import Any, Dict, Optional

from fastapi import APIRouter, Query, Request
from fastapi.responses import StreamingResponse

from ..state import LogLevel

router = APIRouter()


@router.get("/logs")
async def get_logs(
    request: Request,
    limit: int = Query(default=100, ge=1, le=1000, description="Maximum entries to return"),
    level: Optional[str] = Query(default=None, description="Minimum log level: debug, info, warn, error"),
    stream: bool = Query(default=False, description="Stream logs via SSE"),
) -> Any:
    """
    Get execution logs.

    Returns recent log entries or streams new entries via Server-Sent Events.

    **Query Parameters:**
    - `limit`: Maximum number of log entries to return (1-1000, default: 100)
    - `level`: Minimum log level filter (debug, info, warn, error)
    - `stream`: Set to true to receive logs via SSE stream

    **SSE Stream:**
    When `stream=true`, returns a Server-Sent Events stream that sends new
    log entries as they occur. The connection stays open until the client
    disconnects.

    **SSE Event Format:**
    ```
    event: log
    data: {"timestamp": "...", "level": "info", "source": "...", "message": "..."}
    ```
    """
    app_state = request.app.state.app_state

    # Parse log level
    log_level = None
    if level:
        try:
            log_level = LogLevel(level.lower())
        except ValueError:
            pass

    if stream:
        # Return SSE stream
        return StreamingResponse(
            _stream_logs(app_state, log_level),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
                "X-Accel-Buffering": "no",
            },
        )
    else:
        # Return historical logs
        logs = app_state.get_recent_logs(limit=limit, level=log_level)
        return {
            "logs": logs,
            "count": len(logs),
            "limit": limit,
            "level_filter": level,
        }


async def _stream_logs(app_state: Any, min_level: Optional[LogLevel] = None):
    """
    Generator that yields SSE events for log entries.
    """
    import json

    # Send initial keepalive
    yield ": connected\n\n"

    # Subscribe to log stream
    queue = app_state.subscribe_to_logs()

    try:
        # Send recent logs first (buffer)
        recent_logs = app_state.get_recent_logs(limit=50, level=min_level)
        for log in recent_logs:
            yield f"event: log\ndata: {json.dumps(log)}\n\n"

        # Stream new logs
        while True:
            try:
                # Wait for new log entry with timeout for keepalive
                entry = await asyncio.wait_for(queue.get(), timeout=30.0)

                # Apply level filter
                if min_level:
                    level_order = [LogLevel.DEBUG, LogLevel.INFO, LogLevel.WARN, LogLevel.ERROR]
                    if level_order.index(entry.level) < level_order.index(min_level):
                        continue

                # Send log entry
                yield f"event: log\ndata: {json.dumps(entry.to_dict())}\n\n"

            except asyncio.TimeoutError:
                # Send keepalive ping
                yield ": ping\n\n"

    except asyncio.CancelledError:
        pass
    finally:
        app_state.unsubscribe_from_logs(queue)


@router.get("/logs/levels")
async def get_log_levels() -> Dict[str, Any]:
    """
    Get available log levels and their descriptions.
    """
    return {
        "levels": [
            {
                "id": "debug",
                "name": "Debug",
                "description": "Detailed debugging information",
            },
            {
                "id": "info",
                "name": "Info",
                "description": "General information about execution progress",
            },
            {
                "id": "warn",
                "name": "Warning",
                "description": "Warning messages that don't stop execution",
            },
            {
                "id": "error",
                "name": "Error",
                "description": "Error messages indicating failures",
            },
        ],
        "default": "info",
    }


@router.delete("/logs")
async def clear_logs(request: Request) -> Dict[str, Any]:
    """
    Clear the log buffer.

    Removes all stored log entries. New logs will continue to be captured.
    """
    app_state = request.app.state.app_state

    async with app_state._lock:
        app_state._log_buffer.clear()

    return {"status": "cleared", "message": "Log buffer cleared"}


@router.get("/logs/events")
async def get_event_types() -> Dict[str, Any]:
    """
    Get all available WebSocket event types.

    Returns documentation for all event types that can be sent via WebSocket.
    """
    from ..state import EVENT_TYPES

    return {
        "event_types": [
            {"type": event_type, "description": description}
            for event_type, description in EVENT_TYPES.items()
        ],
        "categories": {
            "execution": [
                "execution_started", "execution_completed", "execution_failed",
                "execution_cancelled", "execution_paused", "execution_resumed",
                "execution_update"
            ],
            "strategy": ["strategy_decided"],
            "batch": ["batch_started", "batch_completed", "batch_failed"],
            "story": [
                "story_started", "story_progress", "story_completed",
                "story_failed", "story_update"
            ],
            "quality_gate": [
                "quality_gate_started", "quality_gate_passed", "quality_gate_failed"
            ],
            "retry": ["retry_started"],
            "prd": ["prd_generated", "prd_approved", "prd_updated"],
            "log": ["log_entry"],
            "connection": ["connected", "ping", "pong"],
        },
    }
