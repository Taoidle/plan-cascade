"""
Claude Max LLM Provider

Implementation of LLMProvider that uses Claude Code CLI for LLM capabilities.
This enables Claude Max users to use Plan Cascade without requiring an API key.

The provider uses Claude Code CLI in print mode to get LLM responses,
allowing Plan Cascade to leverage Claude Code's authentication and LLM access.
"""

import asyncio
import json
import shutil
from pathlib import Path
from typing import Any

from ..base import (
    LLMError,
    LLMProvider,
    LLMResponse,
    TokenUsage,
    ToolCall,
)


class ClaudeMaxProvider(LLMProvider):
    """
    LLM provider that uses Claude Code CLI for completion.

    This provider enables Claude Max subscribers to use Plan Cascade
    without needing a separate API key. It communicates with Claude Code
    via subprocess and parses its stream-json output.

    Key features:
    - No API key required (uses Claude Code's authentication)
    - Real-time streaming support via callbacks
    - Compatible with Plan Cascade's LLM interface

    Example:
        provider = ClaudeMaxProvider()
        response = await provider.complete([
            {"role": "user", "content": "Hello!"}
        ])

        # With streaming
        def on_text(text):
            print(text, end="", flush=True)

        response = await provider.complete(
            [{"role": "user", "content": "Tell me a story"}],
            on_text=on_text
        )
    """

    def __init__(
        self,
        api_key: str | None = None,  # Ignored, kept for interface compatibility
        model: str | None = None,
        base_url: str | None = None,  # Ignored
        claude_path: str = "claude",
        project_root: Path | None = None,
        **kwargs: Any
    ):
        """
        Initialize the Claude Max provider.

        Args:
            api_key: Ignored (Claude Code handles authentication)
            model: Model identifier (passed to Claude Code, optional)
            base_url: Ignored
            claude_path: Path to claude CLI executable (default: "claude")
            project_root: Project root directory for execution context
            **kwargs: Additional configuration
        """
        super().__init__(api_key=api_key, model=model, base_url=base_url, **kwargs)

        self.claude_path = claude_path
        self.project_root = Path(project_root) if project_root else Path.cwd()
        self._session_id: str | None = None

    def _check_claude_available(self) -> bool:
        """Check if claude CLI is available."""
        return shutil.which(self.claude_path) is not None

    async def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        tool_choice: str | dict[str, Any] | None = None,
        temperature: float = 0.7,
        max_tokens: int | None = None,
        **kwargs: Any
    ) -> LLMResponse:
        """
        Send a completion request via Claude Code CLI.

        The provider formats messages into a prompt and calls Claude Code
        in print mode. Tool definitions are ignored as Plan Cascade handles
        tool execution separately when using this provider.

        Args:
            messages: List of message dictionaries
            tools: Tool definitions (ignored - Plan Cascade handles tools)
            tool_choice: Tool choice (ignored)
            temperature: Sampling temperature (not directly supported)
            max_tokens: Max tokens (not directly supported)
            **kwargs: Additional parameters (on_text callback for streaming)

        Returns:
            LLMResponse with the model's response
        """
        if not self._check_claude_available():
            raise LLMError(
                f"Claude Code CLI not found at '{self.claude_path}'. "
                "Please install Claude Code or specify the correct path.",
                provider="claude-max"
            )

        # Get streaming callback from kwargs
        on_text = kwargs.get("on_text")

        # Build prompt from messages
        prompt = self._format_messages_to_prompt(messages)

        # Build command - use print mode for LLM-only responses
        cmd = [
            self.claude_path,
            "--print",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
        ]

        # Add model if specified
        if self._model:
            cmd.extend(["--model", self._model])

        # Resume session if we have one (for multi-turn conversations)
        if self._session_id:
            cmd.extend(["--resume", self._session_id])

        cmd.append(prompt)

        # Execute and collect output
        output_text = ""
        tool_calls: list[ToolCall] = []

        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(self.project_root),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                limit=10 * 1024 * 1024,  # 10 MB limit for large outputs
            )

            if process.stdout:
                async for line in process.stdout:
                    try:
                        data = json.loads(line.decode().strip())
                        text, tcs, session_id = self._handle_stream_event(data, on_text)
                        if text:
                            output_text += text
                        if tcs:
                            tool_calls.extend(tcs)
                        if session_id:
                            self._session_id = session_id
                    except json.JSONDecodeError:
                        # Non-JSON output - treat as text
                        text = line.decode().strip()
                        if text:
                            output_text += text
                            if on_text:
                                try:
                                    on_text(text)
                                except Exception:
                                    pass

            await process.wait()

            # Determine stop reason
            stop_reason = "tool_use" if tool_calls else "end_turn"

            return LLMResponse(
                content=output_text,
                tool_calls=tool_calls,
                stop_reason=stop_reason,
                model="claude-max",
            )

        except asyncio.CancelledError:
            raise
        except Exception as e:
            raise LLMError(f"Claude Code execution failed: {e}", provider="claude-max")

    def _format_messages_to_prompt(self, messages: list[dict[str, Any]]) -> str:
        """
        Format messages into a prompt string for Claude Code.

        Args:
            messages: List of message dictionaries

        Returns:
            Formatted prompt string
        """
        prompt_parts = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            if role == "system":
                # System messages go at the beginning
                prompt_parts.insert(0, f"[System Instructions]\n{content}\n")
            elif role == "assistant":
                prompt_parts.append(f"[Previous Response]\n{content}\n")
            elif role == "tool":
                # Tool results
                tool_call_id = msg.get("tool_call_id", "unknown")
                prompt_parts.append(f"[Tool Result: {tool_call_id}]\n{content}\n")
            else:
                # User messages
                prompt_parts.append(content)

        return "\n\n".join(prompt_parts)

    def _handle_stream_event(
        self,
        data: dict[str, Any],
        on_text: Any | None
    ) -> tuple[str, list[ToolCall], str | None]:
        """
        Handle a stream event from Claude Code.

        Args:
            data: Event data
            on_text: Text callback for streaming

        Returns:
            Tuple of (extracted_text, tool_calls, session_id)
        """
        event_type = data.get("type", "")
        text = ""
        tool_calls: list[ToolCall] = []
        session_id = None

        if event_type == "stream_event":
            # Handle real-time streaming from --include-partial-messages
            inner_event = data.get("event", {})
            inner_type = inner_event.get("type", "")

            if inner_type == "content_block_delta":
                delta = inner_event.get("delta", {})
                delta_type = delta.get("type", "")

                if delta_type == "text_delta":
                    text = delta.get("text", "")
                    if text and on_text:
                        try:
                            on_text(text)
                        except Exception:
                            pass

        elif event_type == "assistant":
            # AI response - extract text from message.content
            message = data.get("message", {})
            content_blocks = message.get("content", [])
            for block in content_blocks:
                block_type = block.get("type", "")
                if block_type == "text":
                    # Text already streamed, just collect for final output
                    pass
                elif block_type == "tool_use":
                    # Tool use within message (for future use)
                    tool_calls.append(ToolCall(
                        id=block.get("id", ""),
                        name=block.get("name", ""),
                        arguments=block.get("input", {}),
                    ))

        elif event_type == "text":
            # Direct text output
            text = data.get("content", "")
            if text and on_text:
                try:
                    on_text(text)
                except Exception:
                    pass

        elif event_type == "content_block_delta":
            # Legacy streaming format
            delta = data.get("delta", {})
            if delta.get("type") == "text_delta":
                text = delta.get("text", "")
                if text and on_text:
                    try:
                        on_text(text)
                    except Exception:
                        pass

        elif event_type == "result":
            # Final result - fallback text and session_id
            if not text:
                text = data.get("result", "")
                if text and on_text:
                    try:
                        on_text(text)
                    except Exception:
                        pass
            session_id = data.get("session_id")

        elif event_type == "system":
            # System messages - may contain session_id
            session_id = data.get("session_id")

        return text, tool_calls, session_id

    def get_name(self) -> str:
        """Get the provider name."""
        return "claude-max"

    def get_default_model(self) -> str:
        """Get the default model."""
        return "claude-max"

    def get_supported_models(self) -> list[str]:
        """Get supported models."""
        return ["claude-max"]

    def validate_config(self) -> bool:
        """
        Validate the provider configuration.

        Claude Max doesn't require an API key - it uses Claude Code's
        authentication.
        """
        if not self._check_claude_available():
            raise ValueError(
                f"Claude Code CLI not found at '{self.claude_path}'. "
                "Please install Claude Code: https://claude.ai/code"
            )
        return True

    def clear_session(self) -> None:
        """Clear the session ID to start a fresh conversation."""
        self._session_id = None

    def get_session_id(self) -> str | None:
        """Get the current session ID."""
        return self._session_id
