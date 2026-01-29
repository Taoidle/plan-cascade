"""
Claude Code Backend

Implementation of AgentBackend that uses Claude Code CLI as the execution engine.
Plan Cascade acts as a GUI/orchestrator on top of Claude Code.

Key features:
- No API key required (uses Claude Code's authentication)
- Subprocess communication with stream-json output format
- Tool call visualization via callbacks
- Seamless integration with Claude Code's capabilities
"""

import asyncio
import json
import shutil
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .base import AgentBackend, ExecutionResult

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


class ClaudeCodeBackend(AgentBackend):
    """
    Claude Code Backend - Plan Cascade as GUI for Claude Code.

    This backend communicates with Claude Code CLI via subprocess,
    parsing its JSON stream output for visualization and monitoring.

    Example:
        backend = ClaudeCodeBackend()
        await backend.start_session("/path/to/project")

        # Set up visualization callback
        backend.on_tool_call = lambda data: print(f"Tool: {data['name']}")

        result = await backend.execute({
            "id": "story-001",
            "title": "Add login feature",
            "description": "Implement user login",
            "acceptance_criteria": ["Users can log in", "Errors shown for invalid creds"]
        })
    """

    def __init__(
        self,
        claude_path: str = "claude",
        project_root: Path | None = None,
        output_format: str = "stream-json",
        print_mode: str = "tools"
    ):
        """
        Initialize the Claude Code backend.

        Args:
            claude_path: Path to claude CLI (default: "claude")
            project_root: Project root directory
            output_format: Output format ("stream-json" recommended)
            print_mode: What to print ("tools", "all", "none")
        """
        super().__init__(project_root)

        self.claude_path = claude_path
        self.output_format = output_format
        self.print_mode = print_mode

        self._process: asyncio.subprocess.Process | None = None
        self._llm: LLMProvider | None = None
        self._session_active = False

    def _check_claude_available(self) -> bool:
        """Check if claude CLI is available."""
        return shutil.which(self.claude_path) is not None

    async def start_session(self, project_path: str | None = None) -> None:
        """
        Start a Claude Code session.

        Args:
            project_path: Project path for the session
        """
        if project_path:
            self.project_root = Path(project_path)

        if not self._check_claude_available():
            raise RuntimeError(
                f"Claude Code CLI not found at '{self.claude_path}'. "
                "Please install Claude Code or specify the correct path."
            )

        self._session_active = True

    async def execute(
        self,
        story: dict[str, Any],
        context: str = ""
    ) -> ExecutionResult:
        """
        Execute a story using Claude Code.

        Args:
            story: Story dictionary
            context: Additional context

        Returns:
            ExecutionResult with outcome
        """
        if not self._session_active:
            await self.start_session()

        story_id = story.get("id", "unknown")
        prompt = self._build_prompt(story, context)

        # Build command
        cmd = [
            self.claude_path,
            "--output-format", self.output_format,
            "--print", self.print_mode,
            "--verbose",
            "-p", prompt,
        ]

        # Collect output
        output_lines: list[str] = []
        tool_calls: list[dict[str, Any]] = []

        try:
            # Start subprocess
            process = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(self.project_root),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )

            self._process = process

            # Read stderr in background
            stderr_lines: list[str] = []

            async def read_stderr():
                if process.stderr:
                    async for line in process.stderr:
                        text = line.decode().strip()
                        if text:
                            stderr_lines.append(text)

            stderr_task = asyncio.create_task(read_stderr())

            # Read output stream
            if process.stdout:
                async for line in process.stdout:
                    try:
                        data = json.loads(line.decode().strip())
                        await self._handle_stream_event(data, output_lines, tool_calls)
                    except json.JSONDecodeError:
                        # Non-JSON output - treat as text
                        text = line.decode().strip()
                        if text:
                            output_lines.append(text)
                            await self._emit_text(text)

            # Wait for stderr task and process completion
            await stderr_task
            await process.wait()
            stderr_text = "\n".join(stderr_lines)

            # Check result
            success = process.returncode == 0

            # Build error message with stderr if available
            error_msg = None
            if not success:
                error_msg = f"Claude Code exited with code {process.returncode}"
                if stderr_text:
                    error_msg += f"\nStderr: {stderr_text}"

            return ExecutionResult(
                success=success,
                output="\n".join(output_lines),
                iterations=len(tool_calls),
                error=error_msg,
                story_id=story_id,
                agent="claude-code",
                tool_calls=tool_calls,
                metadata={
                    "exit_code": process.returncode,
                    "output_format": self.output_format,
                    "stderr": stderr_text,
                }
            )

        except asyncio.CancelledError:
            if self._process:
                self._process.terminate()
            raise
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=str(e),
                story_id=story_id,
                agent="claude-code",
            )
        finally:
            self._process = None

    async def _handle_stream_event(
        self,
        data: dict[str, Any],
        output_lines: list[str],
        tool_calls: list[dict[str, Any]]
    ) -> None:
        """
        Handle a stream event from Claude Code.

        Args:
            data: Event data
            output_lines: List to append text output
            tool_calls: List to append tool calls
        """
        event_type = data.get("type", "")

        if event_type == "tool_use":
            # Tool use event
            tool_data = {
                "name": data.get("name", ""),
                "arguments": data.get("input", {}),
                "id": data.get("id", ""),
            }
            tool_calls.append(tool_data)
            await self._emit_tool_call(tool_data)

        elif event_type == "tool_result":
            # Tool result
            tool_data = {
                "type": "tool_result",
                "tool_use_id": data.get("tool_use_id", ""),
                "content": data.get("content", ""),
                "is_error": data.get("is_error", False),
            }
            await self._emit_tool_call(tool_data)

        elif event_type == "text":
            # Text output
            content = data.get("content", "")
            if content:
                output_lines.append(content)
                await self._emit_text(content)

        elif event_type == "content_block_delta":
            # Streaming text delta
            delta = data.get("delta", {})
            if delta.get("type") == "text_delta":
                text = delta.get("text", "")
                if text:
                    await self._emit_text(text)

        elif event_type == "message_stop" or event_type == "end":
            # Message complete
            pass

    async def stop(self) -> None:
        """Stop the current execution."""
        if self._process:
            self._process.terminate()
            try:
                await asyncio.wait_for(self._process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                self._process.kill()
            self._process = None

        self._session_active = False

    def get_llm(self) -> "LLMProvider":
        """
        Get the LLM provider for this backend.

        Returns a ClaudeCodeLLM wrapper that uses Claude Code
        for LLM operations.
        """
        if self._llm is None:
            self._llm = ClaudeCodeLLM(self)
        return self._llm

    def get_name(self) -> str:
        """Get the backend name."""
        return "claude-code"

    def get_status(self) -> dict[str, Any]:
        """Get current status."""
        return {
            "backend": self.get_name(),
            "project_root": str(self.project_root),
            "session_active": self._session_active,
            "process_running": self._process is not None,
            "claude_available": self._check_claude_available(),
        }


class ClaudeCodeLLM:
    """
    LLM wrapper that uses Claude Code for completion.

    This allows using Claude Code's LLM capabilities for
    PRD generation and other tasks without requiring an API key.
    """

    def __init__(self, backend: ClaudeCodeBackend):
        """
        Initialize the ClaudeCodeLLM.

        Args:
            backend: ClaudeCodeBackend instance
        """
        self.backend = backend

    async def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        **kwargs: Any
    ):
        """
        Send a completion request via Claude Code.

        Args:
            messages: List of message dictionaries
            tools: Optional tool definitions (ignored - uses Claude Code's tools)
            **kwargs: Additional parameters (ignored)

        Returns:
            LLMResponse-like object
        """
        # Extract the prompt from messages
        prompt_parts = []
        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")
            if role == "system":
                prompt_parts.insert(0, f"[System]\n{content}\n")
            elif role == "user":
                prompt_parts.append(content)

        prompt = "\n\n".join(prompt_parts)

        # Use Claude Code to complete
        cmd = [
            self.backend.claude_path,
            "--output-format", "stream-json",
            "--print", "none",
            "--verbose",
            "-p", prompt,
        ]

        output_text = ""

        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(self.backend.project_root),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )

            if process.stdout:
                async for line in process.stdout:
                    try:
                        data = json.loads(line.decode().strip())
                        if data.get("type") == "text":
                            output_text += data.get("content", "")
                        elif data.get("type") == "content_block_delta":
                            delta = data.get("delta", {})
                            if delta.get("type") == "text_delta":
                                output_text += delta.get("text", "")
                    except json.JSONDecodeError:
                        pass

            await process.wait()

            # Return a simple response object
            from ..llm.base import LLMResponse
            return LLMResponse(
                content=output_text,
                tool_calls=[],
                stop_reason="end_turn",
                model="claude-code",
            )

        except Exception as e:
            from ..llm.base import LLMResponse
            return LLMResponse(
                content=f"Error: {e}",
                tool_calls=[],
                stop_reason="error",
                model="claude-code",
            )

    def get_name(self) -> str:
        """Get provider name."""
        return "claude-code"

    def get_default_model(self) -> str:
        """Get default model."""
        return "claude-code"
