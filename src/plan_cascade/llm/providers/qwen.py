"""
Qwen (Alibaba Cloud DashScope) LLM Provider

Implementation of LLMProvider for Alibaba Cloud's Qwen models.
Uses the OpenAI-compatible API format via DashScope.
"""

import asyncio
import os
from typing import Any

from ..base import (
    AuthenticationError,
    LLMError,
    LLMProvider,
    LLMResponse,
    ModelNotFoundError,
    TokenUsage,
    ToolCall,
)


class QwenProvider(LLMProvider):
    """
    LLM provider for Alibaba Cloud's Qwen models via DashScope.

    DashScope provides an OpenAI-compatible API. This provider supports:
    - Qwen commercial series (qwen-plus, qwen-max, qwen-turbo, qwen-long)
    - Qwen3 series with optional deep thinking (enable_thinking)
    - Tool/function calling
    - Automatic retry with exponential backoff

    Example:
        provider = QwenProvider(api_key="sk-...")
        response = await provider.complete([
            {"role": "user", "content": "Hello!"}
        ])

        # With thinking enabled (Qwen3 models)
        response = await provider.complete(
            messages=[{"role": "user", "content": "Solve this step by step"}],
            enable_thinking=True,
        )
    """

    # DashScope OpenAI-compatible API base URL
    BASE_URL = "https://dashscope.aliyuncs.com/compatible-mode/v1"

    # Supported Qwen models
    MODELS = [
        "qwen-max",
        "qwen-plus",
        "qwen-turbo",
        "qwen-long",
        "qwen3-max",
        "qwen3-plus",
        "qwen3-turbo",
        "qwen3-flash",
        "qwen3-235b-a22b",
        "qwen3-32b",
        "qwen3-14b",
        "qwen3-8b",
        "qwq-plus",
    ]

    DEFAULT_MODEL = "qwen-plus"

    def __init__(
        self,
        api_key: str | None = None,
        model: str | None = None,
        base_url: str | None = None,
        max_retries: int = 3,
        timeout: float = 120.0,
        **kwargs: Any
    ):
        api_key = api_key or os.environ.get("DASHSCOPE_API_KEY")
        base_url = base_url or self.BASE_URL
        super().__init__(api_key=api_key, model=model, base_url=base_url, **kwargs)

        self.max_retries = max_retries
        self.timeout = timeout
        self._client = None

    def _get_client(self):
        """Get or create the OpenAI-compatible client for DashScope."""
        if self._client is None:
            try:
                from openai import AsyncOpenAI
            except ImportError:
                raise LLMError(
                    "openai package not installed. Install with: pip install openai",
                    provider="qwen"
                )

            self._client = AsyncOpenAI(
                api_key=self.api_key,
                base_url=self.base_url,
            )

        return self._client

    async def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        tool_choice: str | dict[str, Any] | None = None,
        temperature: float = 0.7,
        max_tokens: int | None = None,
        enable_thinking: bool = False,
        **kwargs: Any
    ) -> LLMResponse:
        client = self._get_client()

        # Build request parameters
        params: dict[str, Any] = {
            "model": self.model,
            "messages": self._format_messages(messages),
            "temperature": temperature,
        }

        if max_tokens:
            params["max_tokens"] = max_tokens

        # Add tools if provided
        if tools:
            params["tools"] = self._format_tools(tools)
            if tool_choice:
                params["tool_choice"] = self._format_tool_choice(tool_choice)

        # Enable thinking for Qwen3 mixed-mode models via extra_body
        if enable_thinking:
            params["extra_body"] = {"enable_thinking": True}

        # Execute with retry
        last_error = None
        for attempt in range(self.max_retries):
            try:
                response = await client.chat.completions.create(**params)
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
                        f"Qwen/DashScope authentication failed: {e}",
                        provider="qwen",
                        status_code=401
                    )

                # Check for model not found
                if "model" in error_msg and ("not found" in error_msg or "does not exist" in error_msg):
                    raise ModelNotFoundError(
                        f"Model not found: {self.model}",
                        provider="qwen"
                    )

                # Check for insufficient balance
                if "balance" in error_msg or "quota" in error_msg:
                    raise LLMError(
                        f"DashScope API quota exceeded: {e}",
                        provider="qwen"
                    )

                # Other errors - retry if we have attempts left
                if attempt < self.max_retries - 1:
                    await asyncio.sleep(1.0)
                    continue

                raise LLMError(f"Qwen/DashScope API error: {e}", provider="qwen")

        raise LLMError(f"Max retries exceeded: {last_error}", provider="qwen")

    def _format_messages(self, messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """Format messages for the DashScope API (OpenAI-compatible)."""
        formatted = []
        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            # Handle tool results
            if role == "tool":
                formatted.append({
                    "role": "tool",
                    "tool_call_id": msg.get("tool_call_id", ""),
                    "content": content,
                })
            elif role == "assistant" and msg.get("tool_calls"):
                # Assistant message with tool calls
                tool_calls = []
                for tc in msg.get("tool_calls", []):
                    tool_calls.append({
                        "id": tc.get("id", ""),
                        "type": "function",
                        "function": {
                            "name": tc.get("name", ""),
                            "arguments": (
                                tc.get("arguments", {})
                                if isinstance(tc.get("arguments"), str)
                                else self._dict_to_json(tc.get("arguments", {}))
                            ),
                        }
                    })
                formatted.append({
                    "role": "assistant",
                    "content": content or None,
                    "tool_calls": tool_calls,
                })
            else:
                formatted.append({"role": role, "content": content})

        return formatted

    def _dict_to_json(self, d: dict[str, Any]) -> str:
        """Convert dictionary to JSON string."""
        import json
        return json.dumps(d)

    def _format_tools(self, tools: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """Format tool definitions for the DashScope API."""
        formatted = []
        for tool in tools:
            formatted.append({
                "type": "function",
                "function": {
                    "name": tool.get("name", ""),
                    "description": tool.get("description", ""),
                    "parameters": tool.get("parameters", {}),
                }
            })
        return formatted

    def _format_tool_choice(
        self,
        tool_choice: str | dict[str, Any]
    ) -> str | dict[str, Any]:
        """Format tool choice for the DashScope API."""
        if isinstance(tool_choice, str):
            if tool_choice in ("auto", "none", "required"):
                return tool_choice
            else:
                # Specific function
                return {
                    "type": "function",
                    "function": {"name": tool_choice}
                }
        return tool_choice

    def _parse_response(self, response: Any) -> LLMResponse:
        """Parse the DashScope API response."""
        choice = response.choices[0]
        message = choice.message

        content = message.content or ""
        tool_calls = []

        if message.tool_calls:
            import json
            for tc in message.tool_calls:
                try:
                    args = json.loads(tc.function.arguments)
                except json.JSONDecodeError:
                    args = {"raw": tc.function.arguments}

                tool_calls.append(ToolCall(
                    id=tc.id,
                    name=tc.function.name,
                    arguments=args,
                ))

        # Map finish reason
        stop_reason_map = {
            "stop": "end_turn",
            "tool_calls": "tool_use",
            "length": "max_tokens",
            "content_filter": "end_turn",
        }
        stop_reason = stop_reason_map.get(choice.finish_reason, "end_turn")

        # Parse usage
        usage = None
        if response.usage:
            usage = TokenUsage(
                input_tokens=response.usage.prompt_tokens,
                output_tokens=response.usage.completion_tokens,
                total_tokens=response.usage.total_tokens,
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
        return "qwen"

    def get_default_model(self) -> str:
        """Get the default model."""
        return self.DEFAULT_MODEL

    def get_supported_models(self) -> list[str]:
        """Get supported models."""
        return self.MODELS.copy()

    def validate_config(self) -> bool:
        """Validate the provider configuration."""
        if not self.api_key:
            raise ValueError(
                "DashScope API key is required. "
                "Set DASHSCOPE_API_KEY environment variable or pass api_key parameter."
            )
        return True
