"""
Ollama LLM Provider

Implementation of LLMProvider for local Ollama models.
Uses HTTP requests to communicate with the Ollama API.
"""

import asyncio
from typing import Any

from ..base import (
    LLMError,
    LLMProvider,
    LLMResponse,
    ModelNotFoundError,
    TokenUsage,
    ToolCall,
)


class OllamaProvider(LLMProvider):
    """
    LLM provider for local Ollama models.

    Supports:
    - Any model available in Ollama (llama3.2, codellama, mistral, etc.)
    - Tool/function calling (for supported models)
    - No API key required - runs locally

    Requires Ollama to be running locally (default: http://localhost:11434)

    Example:
        provider = OllamaProvider(model="llama3.2")
        response = await provider.complete([
            {"role": "user", "content": "Hello!"}
        ])
    """

    # Commonly used Ollama models
    MODELS = [
        "llama3.2",
        "llama3.2:3b",
        "llama3.1",
        "codellama",
        "mistral",
        "mixtral",
        "phi3",
        "qwen2.5",
        "deepseek-coder-v2",
    ]

    DEFAULT_MODEL = "llama3.2"
    DEFAULT_BASE_URL = "http://localhost:11434"

    def __init__(
        self,
        model: str | None = None,
        base_url: str | None = None,
        timeout: float = 300.0,
        **kwargs: Any
    ):
        """
        Initialize the Ollama provider.

        Args:
            model: Model identifier (uses DEFAULT_MODEL if not provided)
            base_url: Ollama API URL (uses DEFAULT_BASE_URL if not provided)
            timeout: Request timeout in seconds (longer for local models)
            **kwargs: Additional configuration
        """
        base_url = base_url or self.DEFAULT_BASE_URL
        super().__init__(api_key=None, model=model, base_url=base_url, **kwargs)

        self.timeout = timeout
        self._session = None

    async def _get_session(self):
        """Get or create an aiohttp session."""
        if self._session is None:
            try:
                import aiohttp
            except ImportError:
                raise LLMError(
                    "aiohttp package not installed. Install with: pip install aiohttp",
                    provider="ollama"
                )
            self._session = aiohttp.ClientSession()
        return self._session

    async def close(self):
        """Close the session."""
        if self._session:
            await self._session.close()
            self._session = None

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
        Send a completion request to Ollama.

        Args:
            messages: List of message dictionaries
            tools: Optional list of tool definitions
            tool_choice: Tool choice (limited support in Ollama)
            temperature: Sampling temperature
            max_tokens: Maximum tokens to generate
            **kwargs: Additional parameters

        Returns:
            LLMResponse with the model's response
        """
        import aiohttp

        session = await self._get_session()

        # Build request payload
        payload = {
            "model": self.model,
            "messages": self._format_messages(messages),
            "stream": False,
            "options": {
                "temperature": temperature,
            }
        }

        if max_tokens:
            payload["options"]["num_predict"] = max_tokens

        # Add tools if provided (Ollama 0.4+ supports tools)
        if tools:
            payload["tools"] = self._format_tools(tools)

        url = f"{self.base_url}/api/chat"

        try:
            async with session.post(
                url,
                json=payload,
                timeout=aiohttp.ClientTimeout(total=self.timeout)
            ) as response:
                if response.status == 404:
                    raise ModelNotFoundError(
                        f"Model '{self.model}' not found. "
                        f"Run: ollama pull {self.model}",
                        provider="ollama"
                    )

                if response.status != 200:
                    error_text = await response.text()
                    raise LLMError(
                        f"Ollama API error ({response.status}): {error_text}",
                        provider="ollama",
                        status_code=response.status
                    )

                data = await response.json()
                return self._parse_response(data)

        except aiohttp.ClientConnectorError:
            raise LLMError(
                f"Cannot connect to Ollama at {self.base_url}. "
                "Make sure Ollama is running: ollama serve",
                provider="ollama"
            )
        except asyncio.TimeoutError:
            raise LLMError(
                f"Request timed out after {self.timeout}s. "
                "The model may be loading or the request is too large.",
                provider="ollama"
            )

    def _format_messages(self, messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """Format messages for the Ollama API."""
        formatted = []
        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            # Handle tool results
            if role == "tool":
                formatted.append({
                    "role": "tool",
                    "content": content,
                })
            elif role == "assistant" and msg.get("tool_calls"):
                # Assistant message with tool calls
                formatted.append({
                    "role": "assistant",
                    "content": content,
                    "tool_calls": [
                        {
                            "function": {
                                "name": tc.get("name", ""),
                                "arguments": tc.get("arguments", {}),
                            }
                        }
                        for tc in msg.get("tool_calls", [])
                    ]
                })
            else:
                formatted.append({"role": role, "content": content})

        return formatted

    def _format_tools(self, tools: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """Format tool definitions for the Ollama API."""
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

    def _parse_response(self, data: dict[str, Any]) -> LLMResponse:
        """Parse the Ollama API response."""
        message = data.get("message", {})
        content = message.get("content", "")
        tool_calls = []

        # Parse tool calls if present
        if message.get("tool_calls"):
            for i, tc in enumerate(message["tool_calls"]):
                func = tc.get("function", {})
                tool_calls.append(ToolCall(
                    id=f"call_{i}",
                    name=func.get("name", ""),
                    arguments=func.get("arguments", {}),
                ))

        # Determine stop reason
        stop_reason = "tool_use" if tool_calls else "end_turn"
        if data.get("done_reason") == "length":
            stop_reason = "max_tokens"

        # Parse usage
        usage = None
        if "prompt_eval_count" in data or "eval_count" in data:
            input_tokens = data.get("prompt_eval_count", 0)
            output_tokens = data.get("eval_count", 0)
            usage = TokenUsage(
                input_tokens=input_tokens,
                output_tokens=output_tokens,
                total_tokens=input_tokens + output_tokens,
            )

        return LLMResponse(
            content=content,
            tool_calls=tool_calls,
            stop_reason=stop_reason,
            usage=usage,
            model=data.get("model", self.model),
            metadata={
                "total_duration": data.get("total_duration"),
                "load_duration": data.get("load_duration"),
                "eval_duration": data.get("eval_duration"),
            }
        )

    def get_name(self) -> str:
        """Get the provider name."""
        return "ollama"

    def get_default_model(self) -> str:
        """Get the default model."""
        return self.DEFAULT_MODEL

    def get_supported_models(self) -> list[str]:
        """Get commonly used models."""
        return self.MODELS.copy()

    async def list_local_models(self) -> list[str]:
        """
        List models available locally in Ollama.

        Returns:
            List of model names
        """
        session = await self._get_session()
        url = f"{self.base_url}/api/tags"

        try:
            async with session.get(url) as response:
                if response.status == 200:
                    data = await response.json()
                    return [m["name"] for m in data.get("models", [])]
                return []
        except Exception:
            return []

    def validate_config(self) -> bool:
        """Validate the provider configuration."""
        # Ollama doesn't require API key
        return True
