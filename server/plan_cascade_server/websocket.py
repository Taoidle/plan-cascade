"""
WebSocket Handler

Provides real-time communication between the server and desktop frontend.
"""

import asyncio
import json
from typing import Any, Dict

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

router = APIRouter()


@router.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    """
    WebSocket endpoint for real-time updates.

    Handles bidirectional communication between the server and desktop app.
    Events sent to client:
    - execution_started: Task execution has started
    - execution_update: Execution status has changed
    - story_update: A story's status has changed
    - execution_completed: Task execution has completed
    - execution_failed: Task execution has failed
    - execution_cancelled: Task execution was cancelled

    Events received from client:
    - subscribe: Subscribe to specific event types
    - unsubscribe: Unsubscribe from event types
    - ping: Keep-alive ping
    """
    await websocket.accept()

    # Get app state
    app_state = websocket.app.state.app_state
    await app_state.add_websocket(websocket)

    # Send initial state
    await websocket.send_json({
        "type": "connected",
        "data": {
            "message": "Connected to Plan Cascade server",
            "current_status": app_state.execution.to_dict(),
        }
    })

    try:
        while True:
            try:
                # Wait for messages from client
                data = await asyncio.wait_for(
                    websocket.receive_text(),
                    timeout=30.0  # Send ping if no message in 30 seconds
                )

                # Parse and handle message
                try:
                    message = json.loads(data)
                    await handle_client_message(websocket, app_state, message)
                except json.JSONDecodeError:
                    await websocket.send_json({
                        "type": "error",
                        "data": {"message": "Invalid JSON"}
                    })

            except asyncio.TimeoutError:
                # Send ping to keep connection alive
                await websocket.send_json({"type": "ping"})

    except WebSocketDisconnect:
        pass
    except Exception as e:
        # Log the error
        print(f"WebSocket error: {e}")
    finally:
        await app_state.remove_websocket(websocket)


async def handle_client_message(
    websocket: WebSocket,
    app_state: Any,
    message: Dict[str, Any]
):
    """Handle messages received from client."""
    msg_type = message.get("type", "")

    if msg_type == "ping":
        await websocket.send_json({"type": "pong"})

    elif msg_type == "get_status":
        await websocket.send_json({
            "type": "status",
            "data": app_state.execution.to_dict()
        })

    elif msg_type == "subscribe":
        # Client wants to subscribe to specific events
        # For now, all clients receive all events
        await websocket.send_json({
            "type": "subscribed",
            "data": {"events": message.get("events", ["all"])}
        })

    elif msg_type == "unsubscribe":
        await websocket.send_json({
            "type": "unsubscribed",
            "data": {"events": message.get("events", [])}
        })

    else:
        await websocket.send_json({
            "type": "error",
            "data": {"message": f"Unknown message type: {msg_type}"}
        })


# Export router with name expected by main.py
websocket_router = router
