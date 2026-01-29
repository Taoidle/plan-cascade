"""
Configuration validation for Plan Cascade.

This module provides comprehensive validation of Settings objects:
- Backend configuration validation
- Credentials verification
- Agent configuration validation
- Quality gate configuration checks
"""

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, List

from .models import BackendType, Settings

if TYPE_CHECKING:
    from .storage import SettingsStorage


@dataclass
class ValidationResult:
    """
    Result of a configuration validation.

    Attributes:
        valid: Whether the configuration passed all validation checks.
        errors: List of error messages (validation failures).
        warnings: List of warning messages (non-critical issues).
    """

    valid: bool = True
    errors: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)

    def add_error(self, message: str) -> None:
        """Add an error message and mark validation as failed."""
        self.errors.append(message)
        self.valid = False

    def add_warning(self, message: str) -> None:
        """Add a warning message (does not affect validity)."""
        self.warnings.append(message)

    def merge(self, other: "ValidationResult") -> None:
        """Merge another ValidationResult into this one."""
        if not other.valid:
            self.valid = False
        self.errors.extend(other.errors)
        self.warnings.extend(other.warnings)


class ConfigValidator:
    """
    Configuration validator for Plan Cascade settings.

    Provides validation methods for different aspects of the configuration:
    - Full configuration validation
    - Backend-specific validation
    - Credentials validation
    - Agent configuration validation
    """

    # Backends that require an API key
    BACKENDS_REQUIRING_API_KEY = {
        BackendType.CLAUDE_API,
        BackendType.OPENAI,
        BackendType.DEEPSEEK,
    }

    # Backends that optionally use an API key
    BACKENDS_OPTIONAL_API_KEY = {
        BackendType.OLLAMA,
    }

    # Backends that don't need an API key
    BACKENDS_NO_API_KEY = {
        BackendType.CLAUDE_CODE,
    }

    # Provider mapping for API key lookup
    BACKEND_TO_PROVIDER = {
        BackendType.CLAUDE_API: "claude",
        BackendType.OPENAI: "openai",
        BackendType.DEEPSEEK: "deepseek",
        BackendType.OLLAMA: "ollama",
    }

    def validate(self, settings: Settings) -> ValidationResult:
        """
        Validate the complete settings configuration.

        Checks:
        - Backend configuration
        - Agent configuration
        - Quality gate configuration

        Note: Does NOT validate credentials (use validate_credentials for that).

        Args:
            settings: Settings object to validate.

        Returns:
            ValidationResult with any errors or warnings.
        """
        result = ValidationResult()

        # Validate backend configuration
        backend_result = self.validate_backend(settings)
        result.merge(backend_result)

        # Validate agent configuration
        agent_result = self._validate_agents(settings)
        result.merge(agent_result)

        # Validate quality gates
        qg_result = self._validate_quality_gates(settings)
        result.merge(qg_result)

        # Validate execution configuration
        exec_result = self._validate_execution_config(settings)
        result.merge(exec_result)

        return result

    def validate_backend(self, settings: Settings) -> ValidationResult:
        """
        Validate backend configuration.

        Checks that the backend type is valid and properly configured.

        Args:
            settings: Settings object to validate.

        Returns:
            ValidationResult for backend configuration.
        """
        result = ValidationResult()

        # Check backend type is valid
        if not isinstance(settings.backend, BackendType):
            result.add_error(f"Invalid backend type: {settings.backend}")
            return result

        # Provider validation for non-Claude-Code backends
        if settings.backend != BackendType.CLAUDE_CODE:
            if not settings.provider:
                result.add_error("Provider must be specified for non-Claude-Code backends")

        return result

    def validate_credentials(
        self, settings: Settings, storage: "SettingsStorage"
    ) -> ValidationResult:
        """
        Validate that required credentials are available.

        Checks:
        - Claude Code backend: No API key required
        - Claude API, OpenAI, DeepSeek: API key required
        - Ollama: API key optional

        Args:
            settings: Settings object to validate.
            storage: SettingsStorage instance for API key lookup.

        Returns:
            ValidationResult for credentials validation.
        """
        result = ValidationResult()

        # Claude Code doesn't need API key
        if settings.backend == BackendType.CLAUDE_CODE:
            return result

        # Get the provider for API key lookup
        provider = self.BACKEND_TO_PROVIDER.get(settings.backend, settings.provider)

        # Check if API key is required
        if settings.backend in self.BACKENDS_REQUIRING_API_KEY:
            if not storage.has_api_key(provider):
                result.add_error(
                    f"API key required for {settings.backend.value} backend. "
                    f"Provider '{provider}' has no stored API key."
                )
        elif settings.backend in self.BACKENDS_OPTIONAL_API_KEY:
            if not storage.has_api_key(provider):
                result.add_warning(
                    f"No API key configured for {settings.backend.value}. "
                    "This may be fine for local instances."
                )

        return result

    def _validate_agents(self, settings: Settings) -> ValidationResult:
        """
        Validate agent configuration.

        Checks:
        - At least one agent is enabled
        - default_agent exists in agents list
        - Agent names are unique

        Args:
            settings: Settings object to validate.

        Returns:
            ValidationResult for agent configuration.
        """
        result = ValidationResult()

        # Check for at least one enabled agent
        enabled_agents = settings.get_enabled_agents()
        if not enabled_agents:
            result.add_error("At least one agent must be enabled")

        # Check that default_agent exists in agents list
        agent_names = [agent.name for agent in settings.agents]
        if settings.default_agent not in agent_names:
            result.add_error(
                f"default_agent '{settings.default_agent}' not found in agents list. "
                f"Available agents: {agent_names}"
            )

        # Check that default_agent is enabled
        default_agent = settings.get_default_agent()
        if default_agent is None and settings.default_agent in agent_names:
            result.add_warning(
                f"default_agent '{settings.default_agent}' exists but is not enabled"
            )

        # Check for duplicate agent names
        if len(agent_names) != len(set(agent_names)):
            result.add_error("Duplicate agent names found in agents list")

        # Validate agent_selection value
        valid_selections = {"smart", "prefer_default", "manual"}
        if settings.agent_selection not in valid_selections:
            result.add_warning(
                f"Unknown agent_selection value: '{settings.agent_selection}'. "
                f"Valid values: {valid_selections}"
            )

        return result

    def _validate_quality_gates(self, settings: Settings) -> ValidationResult:
        """
        Validate quality gate configuration.

        Checks:
        - custom_script path validity when custom is enabled
        - max_retries is positive

        Args:
            settings: Settings object to validate.

        Returns:
            ValidationResult for quality gate configuration.
        """
        result = ValidationResult()
        qg = settings.quality_gates

        # Check custom script
        if qg.custom and not qg.custom_script:
            result.add_error(
                "custom_script path must be specified when custom quality gate is enabled"
            )

        # Check max_retries
        if qg.max_retries < 0:
            result.add_error("max_retries must be non-negative")
        elif qg.max_retries == 0:
            result.add_warning(
                "max_retries is 0, quality gate failures will not be retried"
            )

        return result

    def _validate_execution_config(self, settings: Settings) -> ValidationResult:
        """
        Validate execution configuration values.

        Args:
            settings: Settings object to validate.

        Returns:
            ValidationResult for execution configuration.
        """
        result = ValidationResult()

        # Validate max_parallel_stories
        if settings.max_parallel_stories < 1:
            result.add_error("max_parallel_stories must be at least 1")
        elif settings.max_parallel_stories > 10:
            result.add_warning(
                f"max_parallel_stories is {settings.max_parallel_stories}, "
                "high parallelism may cause resource issues"
            )

        # Validate max_iterations
        if settings.max_iterations < 1:
            result.add_error("max_iterations must be at least 1")

        # Validate timeout_seconds
        if settings.timeout_seconds < 30:
            result.add_warning(
                f"timeout_seconds is {settings.timeout_seconds}, "
                "very short timeouts may cause premature failures"
            )

        # Validate default_mode
        valid_modes = {"simple", "expert"}
        if settings.default_mode not in valid_modes:
            result.add_warning(
                f"Unknown default_mode: '{settings.default_mode}'. "
                f"Valid values: {valid_modes}"
            )

        # Validate theme
        valid_themes = {"light", "dark", "system"}
        if settings.theme not in valid_themes:
            result.add_warning(
                f"Unknown theme: '{settings.theme}'. "
                f"Valid values: {valid_themes}"
            )

        return result
