"""
LLM Factory

Factory pattern for creating LLM provider instances based on configuration.
Supports registration of custom providers and configuration validation.
"""

from typing import Any

from .base import LLMError, LLMProvider

# Type alias for provider classes
ProviderClass = type[LLMProvider]


class LLMFactory:
    """
    Factory for creating LLM provider instances.

    Supports built-in providers (claude, openai, ollama) and custom
    provider registration for extensibility.

    Example:
        # Create a Claude provider
        llm = LLMFactory.create("claude", api_key="sk-...")

        # Create with full configuration
        llm = LLMFactory.create(
            provider="openai",
            model="gpt-4o",
            api_key="sk-...",
            temperature=0.7
        )

        # Register a custom provider
        LLMFactory.register("custom", CustomProvider)
    """

    # Registry of provider classes
    _providers: dict[str, ProviderClass] = {}

    # Default configuration for providers
    _default_configs: dict[str, dict[str, Any]] = {
        "claude": {
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 8192,
        },
        "claude-max": {
            # Claude Max uses Claude Code CLI, no model override needed
        },
        "openai": {
            "model": "gpt-4o",
            "max_tokens": 4096,
        },
        "deepseek": {
            "model": "deepseek-chat",
            "max_tokens": 8192,
        },
        "ollama": {
            "model": "llama3.2",
            "base_url": "http://localhost:11434",
        },
    }

    @classmethod
    def register(cls, name: str, provider_class: ProviderClass) -> None:
        """
        Register a provider class.

        Args:
            name: Provider name (e.g., "claude", "openai")
            provider_class: Provider class to register
        """
        cls._providers[name.lower()] = provider_class

    @classmethod
    def unregister(cls, name: str) -> None:
        """
        Unregister a provider.

        Args:
            name: Provider name to unregister
        """
        cls._providers.pop(name.lower(), None)

    @classmethod
    def create(
        cls,
        provider: str,
        api_key: str | None = None,
        model: str | None = None,
        **kwargs: Any
    ) -> LLMProvider:
        """
        Create an LLM provider instance.

        Args:
            provider: Provider name ("claude", "openai", "ollama", etc.)
            api_key: API key for authentication
            model: Model identifier (uses provider default if not specified)
            **kwargs: Additional provider-specific configuration

        Returns:
            LLMProvider instance

        Raises:
            ValueError: If provider is not supported
            LLMError: If provider creation fails
        """
        provider_name = provider.lower()

        # Apply default configuration
        config = cls._default_configs.get(provider_name, {}).copy()
        config.update(kwargs)

        if model:
            config["model"] = model

        # Get provider class
        provider_class = cls._get_provider_class(provider_name)

        # Create instance
        try:
            instance = provider_class(api_key=api_key, **config)
            instance.validate_config()
            return instance
        except Exception as e:
            raise LLMError(
                f"Failed to create {provider_name} provider: {e}",
                provider=provider_name
            )

    @classmethod
    def _get_provider_class(cls, provider_name: str) -> ProviderClass:
        """
        Get the provider class for a provider name.

        Args:
            provider_name: Provider name (lowercase)

        Returns:
            Provider class

        Raises:
            ValueError: If provider is not supported
        """
        # Check registry first
        if provider_name in cls._providers:
            return cls._providers[provider_name]

        # Lazy import built-in providers
        if provider_name == "claude":
            from .providers.claude import ClaudeProvider
            cls._providers["claude"] = ClaudeProvider
            return ClaudeProvider
        elif provider_name == "claude-max":
            from .providers.claude_max import ClaudeMaxProvider
            cls._providers["claude-max"] = ClaudeMaxProvider
            return ClaudeMaxProvider
        elif provider_name == "openai":
            from .providers.openai import OpenAIProvider
            cls._providers["openai"] = OpenAIProvider
            return OpenAIProvider
        elif provider_name == "deepseek":
            from .providers.deepseek import DeepSeekProvider
            cls._providers["deepseek"] = DeepSeekProvider
            return DeepSeekProvider
        elif provider_name == "ollama":
            from .providers.ollama import OllamaProvider
            cls._providers["ollama"] = OllamaProvider
            return OllamaProvider
        else:
            raise ValueError(
                f"Unknown provider: {provider_name}. "
                f"Supported: {cls.get_supported_providers()}"
            )

    @classmethod
    def get_supported_providers(cls) -> list[str]:
        """
        Get list of supported provider names.

        Returns:
            List of provider names
        """
        # Built-in providers plus any registered
        built_in = ["claude", "claude-max", "openai", "deepseek", "ollama"]
        registered = list(cls._providers.keys())
        return list(set(built_in + registered))

    @classmethod
    def get_default_config(cls, provider: str) -> dict[str, Any]:
        """
        Get the default configuration for a provider.

        Args:
            provider: Provider name

        Returns:
            Default configuration dictionary
        """
        return cls._default_configs.get(provider.lower(), {}).copy()

    @classmethod
    def set_default_config(cls, provider: str, config: dict[str, Any]) -> None:
        """
        Set the default configuration for a provider.

        Args:
            provider: Provider name
            config: Configuration dictionary
        """
        cls._default_configs[provider.lower()] = config

    @classmethod
    def create_from_settings(cls, settings: Any) -> LLMProvider:
        """
        Create a provider from a Settings object.

        Args:
            settings: Settings object with provider, model, api_key attributes

        Returns:
            LLMProvider instance
        """
        return cls.create(
            provider=getattr(settings, "provider", "claude"),
            model=getattr(settings, "model", None),
            api_key=getattr(settings, "api_key", None),
        )
