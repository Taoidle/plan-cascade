"""
Claude LLM Provider

Implementation of LLMProvider for Anthropic's Claude models.
Uses the anthropic SDK for API communication.
"""

import asyncio
import os
from typing import Any, Dict, List, Optional, Union

from ..base import (
    LLMProvider,
    LLMResponse,
    ToolCall,
    TokenUsage,
    LLMError,
    RateLimitError,
    AuthenticationError,
    ModelNotFoundError,
)


class ClaudeProvider(LLMProvider):
    """
    LLM provider for Anthropic's Claude models.

    Supports:
    - Claude Opus 4.5, Claude Sonnet 4, Claude Haiku 3.5
    - Tool/function calling
    - Streaming responses (future)
    - Automatic retry with exponential backoff

    Example:
        provider = ClaudeProvider(api_key="sk-ant-...")
        response = await provider.complete([
            {"role": "user", "content": "Hello!"}
        ])
    """

    # Supported Claude models
    MODELS = [
        "claude-opus-4-5-20251101",
        "claude-sonnet-4-20250514",
        "claude-3-5-haiku-20241022",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-3-sonnet-20240229",
        "claude-3-haiku-20240307",
    ]

    DEFAULT_MODEL = "claude-sonnet-4-20250514"

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        base_url: Optional[str] = None,
        max_retries: int = 3,
        timeout: float = 120.0,
        **kwargs: Any
    ):
        """
        Initialize the Claude provider.

        Args:
            api_key: Anthropic API key (uses ANTHROPIC_API_KEY env var if not provided)
            model: Model identifier (uses DEFAULT_MODEL if not provided)
            base_url: Custom API base URL (optional)
            max_retries: Maximum number of retries for transient errors
            timeout: Request timeout in seconds
            **kwargs: Additional configuration
        """
        api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        super().__init__(api_key=api_key, model=model, base_url=base_url, **kwargs)

        self.max_retries = max_retries
        self.timeout = timeout
        self._client = None

    def _get_client(self):
        """Get or create the Anthropic client."""
        if self._client is None:
            try:
                import anthropic
            except ImportError:
                raise LLMError(
                    "anthropic package not installed. Install with: pip install anthropic",
                    provider="claude"
                )

            client_kwargs = {}
            if self.api_key:
                client_kwargs["api_key"] = self.api_key
            if self.base_url:
                client_kwargs["base_url"] = self.base_url

            self._client = anthropic.AsyncAnthropic(**client_kwargs)

        return self._client

    async def complete(
        self,
        messages: List[Dict[str, Any]],
        tools: Optional[List[Dict[str, Any]]] = None,
        tool_choice: Optional[Union[str, Dict[str, Any]]] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        **kwargs: Any
    ) -> LLMResponse:
        """
        Send a completion request to Claude.

        Args:
            messages: List of message dictionaries
            tools: Optional list of tool definitions
            tool_choice: Tool choice ("auto", "any", "none", or specific tool)
            temperature: Sampling temperature
            max_tokens: Maximum tokens to generate
            **kwargs: Additional parameters

        Returns:
            LLMResponse with the model's response
        """
        client = self._get_client()

        # Build request parameters
        params = {
            "model": self.model,
            "messages": self._format_messages(messages),
            "temperature": temperature,
            "max_tokens": max_tokens or 8192,
        }

        # Handle system message
        system_messages = [m for m in messages if m.get("role") == "system"]
        if system_messages:
            params["system"] = system_messages[0].get("content", "")

        # Add tools if provided
        if tools:
            params["tools"] = self._format_tools(tools)
            if tool_choice:
                params["tool_choice"] = self._format_tool_choice(tool_choice)

        # Execute with retry
        last_error = None
        for attempt in range(self.max_retries):
            try:
                response = await client.messages.create(**params)
                return self._parse_response(response)
            except Exception as e:
                last_error = e
                error_msg = str(e).lower()

                # Check for rate limit
                if "rate" in error_msg and "limit" in error_msg:
                    wait_time = (2 ** attempt) * 1.0
                    await asyncio.sleep(wait_time)
                    continue

                # Check for authentication error
                if "auth" in error_msg or "key" in error_msg or "401" in str(e):
                    raise AuthenticationError(
                        f"Claude authentication failed: {e}",
                        provider="claude",
                        status_code=401
                    )

                # Check for model not found
                if "model" in error_msg and "not found" in error_msg:
                    raise ModelNotFoundError(
                        f"Model not found: {self.model}",
                        provider="claude"
                    )

                # Other errors - retry if we have attempts left
                if attempt < self.max_retries - 1:
                    await asyncio.sleep(1.0)
                    continue

                raise LLMError(f"Claude API error: {e}", provider="claude")

        raise LLMError(f"Max retries exceeded: {last_error}", provider="claude")

    def _format_messages(self, messages: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Format messages for the Claude API."""
        formatted = []
        for msg in messages:
            role = msg.get("role", "user")
            # Skip system messages - handled separately
            if role == "system":
                continue

            content = msg.get("content", "")

            # Handle tool results
            if role == "tool":
                formatted.append({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": msg.get("tool_call_id", ""),
                        "content": content,
                    }]
                })
            elif role == "assistant" and msg.get("tool_calls"):
                # Assistant message with tool calls
                content_blocks = []
                if content:
                    content_blocks.append({"type": "text", "text": content})
                for tc in msg.get("tool_calls", []):
                    content_blocks.append({
                        "type": "tool_use",
                        "id": tc.get("id", ""),
                        "name": tc.get("name", ""),
                        "input": tc.get("arguments", {}),
                    })
                formatted.append({"role": "assistant", "content": content_blocks})
            else:
                formatted.append({"role": role, "content": content})

        return formatted

    def _format_tools(self, tools: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Format tool definitions for the Claude API."""
        formatted = []
        for tool in tools:
            formatted.append({
                "name": tool.get("name", ""),
                "description": tool.get("description", ""),
                "input_schema": tool.get("parameters", tool.get("input_schema", {})),
            })
        return formatted

    def _format_tool_choice(
        self,
        tool_choice: Union[str, Dict[str, Any]]
    ) -> Dict[str, Any]:
        """Format tool choice for the Claude API."""
        if isinstance(tool_choice, str):
            if tool_choice == "auto":
                return {"type": "auto"}
            elif tool_choice == "any":
                return {"type": "any"}
            elif tool_choice == "none":
                return {"type": "none"}
            else:
                return {"type": "tool", "name": tool_choice}
        return tool_choice

    def _parse_response(self, response: Any) -> LLMResponse:
        """Parse the Claude API response."""
        content = ""
        tool_calls = []

        for block in response.content:
            if block.type == "text":
                content = block.text
            elif block.type == "tool_use":
                tool_calls.append(ToolCall(
                    id=block.id,
                    name=block.name,
                    arguments=block.input,
                ))

        # Map stop reason
        stop_reason_map = {
            "end_turn": "end_turn",
            "tool_use": "tool_use",
            "max_tokens": "max_tokens",
            "stop_sequence": "end_turn",
        }
        stop_reason = stop_reason_map.get(response.stop_reason, "end_turn")

        # Parse usage
        usage = None
        if hasattr(response, "usage"):
            usage = TokenUsage(
                input_tokens=response.usage.input_tokens,
                output_tokens=response.usage.output_tokens,
                total_tokens=response.usage.input_tokens + response.usage.output_tokens,
            )

        return LLMResponse(
            content=content,
            tool_calls=tool_calls,
            stop_reason=stop_reason,
            usage=usage,
            model=response.model,
        )

    def get_name(self) -> str:
        """Get the provider name."""
        return "claude"

    def get_default_model(self) -> str:
        """Get the default model."""
        return self.DEFAULT_MODEL

    def get_supported_models(self) -> List[str]:
        """Get supported models."""
        return self.MODELS.copy()

    def validate_config(self) -> bool:
        """Validate the provider configuration."""
        if not self.api_key:
            raise ValueError(
                "Claude API key is required. "
                "Set ANTHROPIC_API_KEY environment variable or pass api_key parameter."
            )
        return True
