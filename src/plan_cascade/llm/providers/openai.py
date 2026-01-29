"""
OpenAI LLM Provider

Implementation of LLMProvider for OpenAI's GPT models.
Uses the openai SDK for API communication.
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


class OpenAIProvider(LLMProvider):
    """
    LLM provider for OpenAI's GPT models.

    Supports:
    - GPT-4o, GPT-4 Turbo, GPT-4, GPT-3.5 Turbo
    - Function/tool calling
    - Automatic retry with exponential backoff

    Example:
        provider = OpenAIProvider(api_key="sk-...")
        response = await provider.complete([
            {"role": "user", "content": "Hello!"}
        ])
    """

    # Supported OpenAI models
    MODELS = [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4-turbo",
        "gpt-4",
        "gpt-3.5-turbo",
        "o1-preview",
        "o1-mini",
    ]

    DEFAULT_MODEL = "gpt-4o"

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        base_url: Optional[str] = None,
        organization: Optional[str] = None,
        max_retries: int = 3,
        timeout: float = 120.0,
        **kwargs: Any
    ):
        """
        Initialize the OpenAI provider.

        Args:
            api_key: OpenAI API key (uses OPENAI_API_KEY env var if not provided)
            model: Model identifier
            base_url: Custom API base URL (for Azure or other compatible APIs)
            organization: OpenAI organization ID
            max_retries: Maximum number of retries
            timeout: Request timeout in seconds
            **kwargs: Additional configuration
        """
        api_key = api_key or os.environ.get("OPENAI_API_KEY")
        super().__init__(api_key=api_key, model=model, base_url=base_url, **kwargs)

        self.organization = organization or os.environ.get("OPENAI_ORG_ID")
        self.max_retries = max_retries
        self.timeout = timeout
        self._client = None

    def _get_client(self):
        """Get or create the OpenAI client."""
        if self._client is None:
            try:
                from openai import AsyncOpenAI
            except ImportError:
                raise LLMError(
                    "openai package not installed. Install with: pip install openai",
                    provider="openai"
                )

            client_kwargs = {}
            if self.api_key:
                client_kwargs["api_key"] = self.api_key
            if self.base_url:
                client_kwargs["base_url"] = self.base_url
            if self.organization:
                client_kwargs["organization"] = self.organization

            self._client = AsyncOpenAI(**client_kwargs)

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
        Send a completion request to OpenAI.

        Args:
            messages: List of message dictionaries
            tools: Optional list of tool definitions
            tool_choice: Tool choice ("auto", "none", "required", or specific tool)
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
        }

        if max_tokens:
            params["max_tokens"] = max_tokens

        # Add tools if provided
        if tools:
            params["tools"] = self._format_tools(tools)
            if tool_choice:
                params["tool_choice"] = self._format_tool_choice(tool_choice)

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
                        f"OpenAI authentication failed: {e}",
                        provider="openai",
                        status_code=401
                    )

                # Check for model not found
                if "model" in error_msg and ("not found" in error_msg or "does not exist" in error_msg):
                    raise ModelNotFoundError(
                        f"Model not found: {self.model}",
                        provider="openai"
                    )

                # Other errors - retry if we have attempts left
                if attempt < self.max_retries - 1:
                    await asyncio.sleep(1.0)
                    continue

                raise LLMError(f"OpenAI API error: {e}", provider="openai")

        raise LLMError(f"Max retries exceeded: {last_error}", provider="openai")

    def _format_messages(self, messages: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Format messages for the OpenAI API."""
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

    def _dict_to_json(self, d: Dict[str, Any]) -> str:
        """Convert dictionary to JSON string."""
        import json
        return json.dumps(d)

    def _format_tools(self, tools: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Format tool definitions for the OpenAI API."""
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
        tool_choice: Union[str, Dict[str, Any]]
    ) -> Union[str, Dict[str, Any]]:
        """Format tool choice for the OpenAI API."""
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
        """Parse the OpenAI API response."""
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
        return "openai"

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
                "OpenAI API key is required. "
                "Set OPENAI_API_KEY environment variable or pass api_key parameter."
            )
        return True
