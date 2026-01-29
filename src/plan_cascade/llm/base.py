"""
LLM Provider Base Classes

Defines the abstract interface for LLM providers and standardized response types.
All LLM provider implementations must inherit from LLMProvider.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ToolCall:
    """
    Represents a tool call requested by the LLM.

    Attributes:
        id: Unique identifier for this tool call
        name: Name of the tool to execute
        arguments: Dictionary of arguments to pass to the tool
    """
    id: str
    name: str
    arguments: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "id": self.id,
            "name": self.name,
            "arguments": self.arguments,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ToolCall":
        """Create from dictionary."""
        return cls(
            id=data.get("id", ""),
            name=data.get("name", ""),
            arguments=data.get("arguments", {}),
        )


@dataclass
class TokenUsage:
    """
    Token usage statistics from an LLM response.

    Attributes:
        input_tokens: Number of tokens in the input/prompt
        output_tokens: Number of tokens in the output/response
        total_tokens: Total tokens used (input + output)
    """
    input_tokens: int = 0
    output_tokens: int = 0
    total_tokens: int = 0

    def to_dict(self) -> dict[str, int]:
        """Convert to dictionary."""
        return {
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "total_tokens": self.total_tokens,
        }


@dataclass
class LLMResponse:
    """
    Standardized response from an LLM provider.

    Attributes:
        content: Text content of the response
        tool_calls: List of tool calls requested by the LLM
        stop_reason: Reason for stopping generation ("end_turn", "tool_use", "max_tokens")
        usage: Token usage statistics
        model: Model identifier that generated this response
        metadata: Additional provider-specific metadata
    """
    content: str
    tool_calls: list[ToolCall] = field(default_factory=list)
    stop_reason: str = "end_turn"
    usage: TokenUsage | None = None
    model: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)

    def has_tool_calls(self) -> bool:
        """Check if the response contains tool calls."""
        return len(self.tool_calls) > 0

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "content": self.content,
            "tool_calls": [tc.to_dict() for tc in self.tool_calls],
            "stop_reason": self.stop_reason,
            "usage": self.usage.to_dict() if self.usage else None,
            "model": self.model,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "LLMResponse":
        """Create from dictionary."""
        usage_data = data.get("usage")
        usage = TokenUsage(**usage_data) if usage_data else None

        tool_calls = [
            ToolCall.from_dict(tc) for tc in data.get("tool_calls", [])
        ]

        return cls(
            content=data.get("content", ""),
            tool_calls=tool_calls,
            stop_reason=data.get("stop_reason", "end_turn"),
            usage=usage,
            model=data.get("model", ""),
            metadata=data.get("metadata", {}),
        )


class LLMProvider(ABC):
    """
    Abstract base class for LLM providers.

    LLM providers handle communication with specific LLM APIs (Claude, OpenAI, etc.)
    and translate between the standardized interface and provider-specific formats.

    Subclasses must implement:
    - complete(): Send a completion request and return the response
    - get_name(): Return the provider name
    - get_default_model(): Return the default model for this provider
    """

    def __init__(
        self,
        api_key: str | None = None,
        model: str | None = None,
        base_url: str | None = None,
        **kwargs: Any
    ):
        """
        Initialize the LLM provider.

        Args:
            api_key: API key for authentication (optional for some providers)
            model: Model identifier to use (uses default if not specified)
            base_url: Base URL for API requests (optional)
            **kwargs: Additional provider-specific configuration
        """
        self.api_key = api_key
        self._model = model
        self.base_url = base_url
        self.config = kwargs

    @property
    def model(self) -> str:
        """Get the model identifier to use."""
        return self._model or self.get_default_model()

    @abstractmethod
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
        Send a completion request to the LLM.

        Args:
            messages: List of message dictionaries with 'role' and 'content' keys
            tools: Optional list of tool definitions for function calling
            tool_choice: Optional tool choice specification ("auto", "none", or specific tool)
            temperature: Sampling temperature (0.0 to 1.0)
            max_tokens: Maximum tokens to generate (uses model default if not specified)
            **kwargs: Additional provider-specific parameters

        Returns:
            LLMResponse containing the model's response

        Raises:
            LLMError: If the request fails
        """
        pass

    @abstractmethod
    def get_name(self) -> str:
        """
        Get the name of this provider.

        Returns:
            Provider name (e.g., "claude", "openai", "ollama")
        """
        pass

    @abstractmethod
    def get_default_model(self) -> str:
        """
        Get the default model for this provider.

        Returns:
            Default model identifier
        """
        pass

    def get_supported_models(self) -> list[str]:
        """
        Get list of supported models for this provider.

        Returns:
            List of model identifiers
        """
        return [self.get_default_model()]

    def validate_config(self) -> bool:
        """
        Validate the provider configuration.

        Returns:
            True if configuration is valid

        Raises:
            ValueError: If configuration is invalid
        """
        return True


class LLMError(Exception):
    """Base exception for LLM-related errors."""

    def __init__(
        self,
        message: str,
        provider: str | None = None,
        status_code: int | None = None,
        response: dict[str, Any] | None = None
    ):
        """
        Initialize LLM error.

        Args:
            message: Error message
            provider: Name of the provider that raised the error
            status_code: HTTP status code (if applicable)
            response: Raw response data (if available)
        """
        super().__init__(message)
        self.provider = provider
        self.status_code = status_code
        self.response = response


class RateLimitError(LLMError):
    """Raised when rate limit is exceeded."""
    pass


class AuthenticationError(LLMError):
    """Raised when authentication fails."""
    pass


class ModelNotFoundError(LLMError):
    """Raised when the specified model is not available."""
    pass
