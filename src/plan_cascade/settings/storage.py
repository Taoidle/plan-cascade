"""
Settings storage management for Plan Cascade.

This module provides YAML-based configuration file persistence with:
- Automatic directory creation
- Dataclass to dict conversion for serialization
- Proper handling of enums and nested dataclasses
- Default Settings when no configuration exists
- Secure API key storage using system keyring
"""

import logging
from pathlib import Path
from typing import Any

import keyring
import keyring.errors
import yaml

from .migration import ConfigMigration
from .models import AgentConfig, BackendType, QualityGateConfig, Settings

logger = logging.getLogger(__name__)


class SettingsStorage:
    """
    Settings storage manager.

    Handles loading and saving Settings objects to YAML configuration files.
    Configuration is stored at ~/.plan-cascade/config.yaml by default.

    Attributes:
        config_dir: Directory path for configuration files.
        config_file: Path to the main configuration file.
    """

    KEYRING_SERVICE = "plan-cascade"

    def __init__(self, config_dir: Path | None = None) -> None:
        """
        Initialize the settings storage.

        Args:
            config_dir: Optional path to configuration directory.
                       Defaults to ~/.plan-cascade/
        """
        self.config_dir = config_dir or Path.home() / ".plan-cascade"
        self.config_file = self.config_dir / "config.yaml"

    def load(self) -> Settings:
        """
        Load settings from the configuration file.

        Automatically performs configuration migration if the file
        contains an older version format.

        Returns:
            Settings object loaded from config file, or default Settings
            if the configuration file does not exist.
        """
        if not self.config_file.exists():
            return Settings()

        with open(self.config_file, encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}

        # Check and perform migration if needed
        if ConfigMigration.needs_migration(data):
            logger.info("Configuration file needs migration")
            data = ConfigMigration.migrate(data)
            # Save migrated config to disk
            self._save_raw_data(data)

        return self._dict_to_settings(data)

    def save(self, settings: Settings) -> None:
        """
        Save settings to the configuration file.

        Creates the configuration directory if it does not exist.
        Automatically adds the current configuration version.

        Args:
            settings: Settings object to save.
        """
        self.config_dir.mkdir(parents=True, exist_ok=True)

        data = self._settings_to_dict(settings)
        # Add current version to the config
        data = ConfigMigration.set_version(data)

        self._save_raw_data(data)

    def _save_raw_data(self, data: dict[str, Any]) -> None:
        """
        Save raw dictionary data to the configuration file.

        Args:
            data: Dictionary to save as YAML.
        """
        self.config_dir.mkdir(parents=True, exist_ok=True)

        with open(self.config_file, "w", encoding="utf-8") as f:
            yaml.dump(data, f, default_flow_style=False, allow_unicode=True, sort_keys=False)

    def _settings_to_dict(self, settings: Settings) -> dict[str, Any]:
        """
        Convert Settings object to a dictionary suitable for YAML serialization.

        Handles:
        - BackendType enum serialization to string value
        - AgentConfig list serialization
        - QualityGateConfig nested dataclass serialization

        Args:
            settings: Settings object to convert.

        Returns:
            Dictionary representation of the settings.
        """
        result = {}

        # Backend configuration
        result["backend"] = settings.backend.value
        result["provider"] = settings.provider
        result["model"] = settings.model

        # Agents list
        result["agents"] = [
            {
                "name": agent.name,
                "enabled": agent.enabled,
                "command": agent.command,
                "is_default": agent.is_default,
            }
            for agent in settings.agents
        ]

        # Agent selection
        result["agent_selection"] = settings.agent_selection
        result["default_agent"] = settings.default_agent

        # Quality gates
        result["quality_gates"] = {
            "typecheck": settings.quality_gates.typecheck,
            "test": settings.quality_gates.test,
            "lint": settings.quality_gates.lint,
            "custom": settings.quality_gates.custom,
            "custom_script": settings.quality_gates.custom_script,
            "max_retries": settings.quality_gates.max_retries,
        }

        # Execution configuration
        result["max_parallel_stories"] = settings.max_parallel_stories
        result["max_iterations"] = settings.max_iterations
        result["timeout_seconds"] = settings.timeout_seconds

        # UI configuration
        result["default_mode"] = settings.default_mode
        result["theme"] = settings.theme

        return result

    def _dict_to_settings(self, data: dict[str, Any]) -> Settings:
        """
        Convert a dictionary to a Settings object.

        Handles:
        - BackendType enum deserialization from string
        - AgentConfig list reconstruction
        - QualityGateConfig nested dataclass reconstruction

        Args:
            data: Dictionary loaded from YAML file.

        Returns:
            Settings object with values from the dictionary.
        """
        # Parse backend type
        backend_value = data.get("backend", "claude-code")
        try:
            backend = BackendType(backend_value)
        except ValueError:
            backend = BackendType.CLAUDE_CODE

        # Parse agents list
        agents_data = data.get("agents", [])
        agents = []
        for agent_dict in agents_data:
            agents.append(
                AgentConfig(
                    name=agent_dict.get("name", ""),
                    enabled=agent_dict.get("enabled", True),
                    command=agent_dict.get("command", ""),
                    is_default=agent_dict.get("is_default", False),
                )
            )

        # Use defaults if no agents provided
        if not agents:
            agents = Settings().agents

        # Parse quality gates
        qg_data = data.get("quality_gates", {})
        quality_gates = QualityGateConfig(
            typecheck=qg_data.get("typecheck", True),
            test=qg_data.get("test", True),
            lint=qg_data.get("lint", True),
            custom=qg_data.get("custom", False),
            custom_script=qg_data.get("custom_script", ""),
            max_retries=qg_data.get("max_retries", 3),
        )

        return Settings(
            backend=backend,
            provider=data.get("provider", "claude"),
            model=data.get("model", ""),
            agents=agents,
            agent_selection=data.get("agent_selection", "prefer_default"),
            default_agent=data.get("default_agent", "claude-code"),
            quality_gates=quality_gates,
            max_parallel_stories=data.get("max_parallel_stories", 3),
            max_iterations=data.get("max_iterations", 50),
            timeout_seconds=data.get("timeout_seconds", 300),
            default_mode=data.get("default_mode", "simple"),
            theme=data.get("theme", "system"),
        )

    # ========================================================================
    # API Key Management (using keyring for secure storage)
    # ========================================================================

    def get_api_key(self, provider: str) -> str | None:
        """
        Get the API key for a provider from the system keyring.

        Uses the system's secure credential storage (Keychain on macOS,
        Credential Manager on Windows, Secret Service on Linux).

        Args:
            provider: The provider name (e.g., "claude", "openai", "deepseek").

        Returns:
            The API key if found, or None if not stored or keyring unavailable.
        """
        try:
            password = keyring.get_password(self.KEYRING_SERVICE, provider)
            return password
        except keyring.errors.KeyringError as e:
            logger.warning(f"Keyring unavailable, cannot retrieve API key: {e}")
            return None
        except Exception as e:
            logger.error(f"Unexpected error retrieving API key: {e}")
            return None

    def set_api_key(self, provider: str, api_key: str) -> None:
        """
        Store an API key for a provider in the system keyring.

        Args:
            provider: The provider name (e.g., "claude", "openai", "deepseek").
            api_key: The API key to store.

        Raises:
            keyring.errors.KeyringError: If keyring is not available.
        """
        try:
            keyring.set_password(self.KEYRING_SERVICE, provider, api_key)
            logger.debug(f"API key for {provider} stored successfully")
        except keyring.errors.KeyringError as e:
            logger.error(f"Failed to store API key in keyring: {e}")
            raise

    def delete_api_key(self, provider: str) -> None:
        """
        Delete the API key for a provider from the system keyring.

        This method handles the case where the password doesn't exist
        gracefully (no error is raised).

        Args:
            provider: The provider name (e.g., "claude", "openai", "deepseek").
        """
        try:
            keyring.delete_password(self.KEYRING_SERVICE, provider)
            logger.debug(f"API key for {provider} deleted successfully")
        except keyring.errors.PasswordDeleteError:
            # Password doesn't exist - this is fine
            logger.debug(f"No API key found for {provider} to delete")
        except keyring.errors.KeyringError as e:
            logger.warning(f"Keyring error while deleting API key: {e}")

    def has_api_key(self, provider: str) -> bool:
        """
        Check if an API key exists for a provider.

        Args:
            provider: The provider name (e.g., "claude", "openai", "deepseek").

        Returns:
            True if an API key is stored for this provider, False otherwise.
        """
        api_key = self.get_api_key(provider)
        return api_key is not None and len(api_key) > 0
