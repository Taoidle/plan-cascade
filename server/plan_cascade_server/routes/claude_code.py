"""
Claude Code Integration Routes

Provides endpoints for spawning and managing Claude Code CLI process,
capturing streaming output, and managing conversation lifecycle.
"""

import asyncio
import json
import subprocess
import sys
from typing import Optional, Dict, Any, List
from pathlib import Path
import uuid
import logging

from fastapi import APIRouter, HTTPException, WebSocket, WebSocketDisconnect
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/claude-code")


# ============================================================================
# Models
# ============================================================================


class ClaudeCodeMessage(BaseModel):
    """Message to send to Claude Code."""

    content: str
    conversation_id: Optional[str] = None


class ClaudeCodeResponse(BaseModel):
    """Response from starting a Claude Code session."""

    session_id: str
    status: str
    message: str


class ToolCallEvent(BaseModel):
    """Tool call event from Claude Code."""

    id: str
    name: str
    parameters: Dict[str, Any]
    status: str = "pending"


class ToolResultEvent(BaseModel):
    """Tool result event from Claude Code."""

    id: str
    success: bool
    output: Optional[str] = None
    error: Optional[str] = None
    files: Optional[List[str]] = None
    content: Optional[str] = None
    matches: Optional[List[Dict[str, Any]]] = None


# ============================================================================
# Claude Code Process Manager
# ============================================================================


class ClaudeCodeProcessManager:
    """Manages Claude Code CLI process lifecycle."""

    def __init__(self):
        self.processes: Dict[str, asyncio.subprocess.Process] = {}
        self.sessions: Dict[str, Dict[str, Any]] = {}

    async def start_session(
        self,
        working_dir: Optional[str] = None,
        model: Optional[str] = None,
    ) -> str:
        """Start a new Claude Code session."""
        session_id = str(uuid.uuid4())

        # Build command
        cmd = ["claude"]  # Assumes claude is in PATH

        if model:
            cmd.extend(["--model", model])

        # Use JSON output mode for parsing
        cmd.append("--output-format")
        cmd.append("stream-json")

        # Set working directory
        cwd = working_dir or str(Path.cwd())

        # Store session info
        self.sessions[session_id] = {
            "id": session_id,
            "working_dir": cwd,
            "model": model,
            "status": "ready",
            "messages": [],
            "tool_calls": [],
        }

        logger.info(f"Created Claude Code session: {session_id}")
        return session_id

    async def send_message(
        self,
        session_id: str,
        content: str,
        websocket: Optional[WebSocket] = None,
    ) -> Dict[str, Any]:
        """Send a message to Claude Code and stream the response."""
        if session_id not in self.sessions:
            raise ValueError(f"Session {session_id} not found")

        session = self.sessions[session_id]
        session["status"] = "running"

        # Build command for this message
        cmd = ["claude", "--output-format", "stream-json", "-p", content]

        try:
            # Start process
            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=session["working_dir"],
            )

            self.processes[session_id] = process
            full_response = ""
            tool_calls = []

            # Read stdout line by line
            while True:
                line = await process.stdout.readline()
                if not line:
                    break

                try:
                    # Parse JSON line
                    data = json.loads(line.decode("utf-8").strip())
                    event_type = data.get("type", "")

                    if event_type == "assistant":
                        # Text content from assistant
                        text = data.get("message", {}).get("content", [])
                        for block in text:
                            if block.get("type") == "text":
                                chunk = block.get("text", "")
                                full_response += chunk
                                if websocket:
                                    await websocket.send_json(
                                        {
                                            "type": "claude_code_response",
                                            "data": {"content": chunk, "streaming": True},
                                        }
                                    )

                    elif event_type == "content_block_delta":
                        # Streaming text delta
                        delta = data.get("delta", {})
                        if delta.get("type") == "text_delta":
                            chunk = delta.get("text", "")
                            full_response += chunk
                            if websocket:
                                await websocket.send_json(
                                    {
                                        "type": "claude_code_response",
                                        "data": {"content": chunk, "streaming": True},
                                    }
                                )

                    elif event_type == "tool_use":
                        # Tool call started
                        tool_call = {
                            "id": data.get("id", str(uuid.uuid4())),
                            "name": data.get("name", "Unknown"),
                            "parameters": data.get("input", {}),
                            "status": "executing",
                        }
                        tool_calls.append(tool_call)
                        if websocket:
                            await websocket.send_json(
                                {"type": "claude_code_tool_call", "data": tool_call}
                            )

                    elif event_type == "tool_result":
                        # Tool call completed
                        tool_id = data.get("tool_use_id", "")
                        result = {
                            "id": tool_id,
                            "success": not data.get("is_error", False),
                            "output": data.get("content", ""),
                        }

                        # Parse output for specific tool types
                        content = data.get("content", "")
                        if isinstance(content, str):
                            # Try to extract file list for Glob
                            if "files" in content.lower():
                                try:
                                    files = content.strip().split("\n")
                                    result["files"] = [f for f in files if f.strip()]
                                except Exception:
                                    pass

                        if websocket:
                            await websocket.send_json(
                                {"type": "claude_code_tool_result", "data": result}
                            )

                    elif event_type == "error":
                        error_msg = data.get("error", {}).get("message", "Unknown error")
                        if websocket:
                            await websocket.send_json(
                                {
                                    "type": "claude_code_error",
                                    "data": {"message": error_msg},
                                }
                            )

                except json.JSONDecodeError:
                    # Not JSON, treat as plain text
                    text = line.decode("utf-8").strip()
                    if text:
                        full_response += text + "\n"
                        if websocket:
                            await websocket.send_json(
                                {
                                    "type": "claude_code_response",
                                    "data": {"content": text + "\n", "streaming": True},
                                }
                            )

            # Wait for process to complete
            await process.wait()

            # Clean up
            if session_id in self.processes:
                del self.processes[session_id]

            # Store message
            session["messages"].append({"role": "user", "content": content})
            session["messages"].append(
                {"role": "assistant", "content": full_response, "tool_calls": tool_calls}
            )
            session["tool_calls"].extend(tool_calls)
            session["status"] = "ready"

            # Send completion event
            if websocket:
                await websocket.send_json(
                    {
                        "type": "claude_code_complete",
                        "data": {"content": full_response, "tool_calls": tool_calls},
                    }
                )

            return {
                "content": full_response,
                "tool_calls": tool_calls,
            }

        except Exception as e:
            logger.error(f"Error in Claude Code session: {e}")
            session["status"] = "error"

            if websocket:
                await websocket.send_json(
                    {"type": "claude_code_error", "data": {"message": str(e)}}
                )

            raise

    async def cancel_session(self, session_id: str) -> bool:
        """Cancel an active Claude Code session."""
        if session_id in self.processes:
            process = self.processes[session_id]
            process.terminate()
            await process.wait()
            del self.processes[session_id]

            if session_id in self.sessions:
                self.sessions[session_id]["status"] = "cancelled"

            logger.info(f"Cancelled Claude Code session: {session_id}")
            return True

        return False

    def get_session(self, session_id: str) -> Optional[Dict[str, Any]]:
        """Get session info."""
        return self.sessions.get(session_id)

    def list_sessions(self) -> List[Dict[str, Any]]:
        """List all sessions."""
        return list(self.sessions.values())

    async def cleanup(self):
        """Clean up all processes."""
        for session_id in list(self.processes.keys()):
            await self.cancel_session(session_id)


# Global process manager
process_manager = ClaudeCodeProcessManager()


# ============================================================================
# REST Endpoints
# ============================================================================


@router.post("/session", response_model=ClaudeCodeResponse)
async def create_session(
    working_dir: Optional[str] = None, model: Optional[str] = None
):
    """Create a new Claude Code session."""
    try:
        session_id = await process_manager.start_session(
            working_dir=working_dir, model=model
        )
        return ClaudeCodeResponse(
            session_id=session_id,
            status="ready",
            message="Session created successfully",
        )
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/session/{session_id}")
async def get_session(session_id: str):
    """Get session information."""
    session = process_manager.get_session(session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")
    return session


@router.delete("/session/{session_id}")
async def cancel_session(session_id: str):
    """Cancel and clean up a session."""
    success = await process_manager.cancel_session(session_id)
    if not success:
        raise HTTPException(status_code=404, detail="Session not found or not running")
    return {"status": "cancelled", "session_id": session_id}


@router.get("/sessions")
async def list_sessions():
    """List all Claude Code sessions."""
    return process_manager.list_sessions()


@router.post("/session/{session_id}/message")
async def send_message(session_id: str, message: ClaudeCodeMessage):
    """Send a message to Claude Code (non-streaming)."""
    try:
        result = await process_manager.send_message(session_id, message.content)
        return result
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


# ============================================================================
# WebSocket Endpoint for Streaming
# ============================================================================


@router.websocket("/ws/{session_id}")
async def claude_code_websocket(websocket: WebSocket, session_id: str):
    """WebSocket endpoint for streaming Claude Code responses."""
    await websocket.accept()

    session = process_manager.get_session(session_id)
    if not session:
        await websocket.send_json(
            {"type": "error", "data": {"message": "Session not found"}}
        )
        await websocket.close()
        return

    try:
        while True:
            # Wait for messages from client
            data = await websocket.receive_json()
            msg_type = data.get("type", "")

            if msg_type == "message":
                content = data.get("content", "")
                if content:
                    await process_manager.send_message(
                        session_id, content, websocket=websocket
                    )

            elif msg_type == "cancel":
                await process_manager.cancel_session(session_id)
                await websocket.send_json(
                    {
                        "type": "cancelled",
                        "data": {"message": "Session cancelled"},
                    }
                )

            elif msg_type == "ping":
                await websocket.send_json({"type": "pong"})

    except WebSocketDisconnect:
        logger.info(f"WebSocket disconnected for session: {session_id}")
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
        await websocket.send_json({"type": "error", "data": {"message": str(e)}})
    finally:
        # Don't automatically cancel session on disconnect
        pass
