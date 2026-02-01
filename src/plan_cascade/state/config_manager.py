#!/usr/bin/env python3
"""
Configuration Manager for Plan Cascade

Provides unified configuration management with a hierarchical system:
1. Environment variables (highest priority)
2. Project-specific config (.plan-cascade.json in project root)
3. Global config (~/.plan-cascade/config.json)
4. Default values (lowest priority)

Follows ADR-F003 (legacy mode) and ADR-F007 (configuration precedence).
"""

import json
import os
import sys
from pathlib import Path
from typing import Any


class ConfigManager:
    """
    Manages configuration for Plan Cascade with hierarchical precedence.

    Configuration sources (in order of priority):
    1. Environment variables (PLAN_CASCADE_*)
    2. Project config (.plan-cascade.json in project root)
    3. Global config (~/.plan-cascade/config.json)
    4. Default values

    Attributes:
        project_root: Root directory of the project
        _global_config: Cached global configuration
        _project_config: Cached project configuration
    """

    # Environment variable names
    ENV_DATA_DIR = "PLAN_CASCADE_DATA_DIR"
    ENV_LEGACY_MODE = "PLAN_CASCADE_LEGACY_MODE"

    # Config file names
    GLOBAL_CONFIG_FILE = "config.json"
    PROJECT_CONFIG_FILE = ".plan-cascade.json"

    # Default values
    DEFAULTS = {
        "legacy_mode": False,
        "data_dir": None,  # None means use platform default
        "max_parallel_stories": 3,
        "max_retries": 5,
        "timeout_seconds": 300,
        "quality_gates": {
            "typecheck": True,
            "test": True,
            "lint": True,
        },
    }

    def __init__(self, project_root: Path | None = None):
        """
        Initialize the configuration manager.

        Args:
            project_root: Root directory of the project. Defaults to current directory.
        """
        self.project_root = Path(project_root).resolve() if project_root else Path.cwd().resolve()
        self._global_config: dict[str, Any] | None = None
        self._project_config: dict[str, Any] | None = None

    def _get_global_config_path(self) -> Path:
        """
        Get the path to the global configuration file.

        Returns:
            Path to ~/.plan-cascade/config.json
        """
        if sys.platform == "win32":
            appdata = os.environ.get("APPDATA")
            if appdata:
                return Path(appdata) / "plan-cascade" / self.GLOBAL_CONFIG_FILE
            return Path.home() / "AppData" / "Roaming" / "plan-cascade" / self.GLOBAL_CONFIG_FILE
        return Path.home() / ".plan-cascade" / self.GLOBAL_CONFIG_FILE

    def _get_project_config_path(self) -> Path:
        """
        Get the path to the project configuration file.

        Returns:
            Path to .plan-cascade.json in project root
        """
        return self.project_root / self.PROJECT_CONFIG_FILE

    def _load_global_config(self) -> dict[str, Any]:
        """
        Load the global configuration file.

        Returns:
            Configuration dictionary, or empty dict if file doesn't exist
        """
        if self._global_config is not None:
            return self._global_config

        config_path = self._get_global_config_path()
        if config_path.exists():
            try:
                with open(config_path, encoding="utf-8") as f:
                    self._global_config = json.load(f)
            except (OSError, json.JSONDecodeError):
                self._global_config = {}
        else:
            self._global_config = {}

        return self._global_config

    def _load_project_config(self) -> dict[str, Any]:
        """
        Load the project configuration file.

        Returns:
            Configuration dictionary, or empty dict if file doesn't exist
        """
        if self._project_config is not None:
            return self._project_config

        config_path = self._get_project_config_path()
        if config_path.exists():
            try:
                with open(config_path, encoding="utf-8") as f:
                    self._project_config = json.load(f)
            except (OSError, json.JSONDecodeError):
                self._project_config = {}
        else:
            self._project_config = {}

        return self._project_config

    def _get_env_value(self, key: str) -> Any | None:
        """
        Get a configuration value from environment variables.

        Args:
            key: Configuration key (without PLAN_CASCADE_ prefix)

        Returns:
            Value from environment variable, or None if not set
        """
        # Map config keys to environment variable names
        env_map = {
            "data_dir": self.ENV_DATA_DIR,
            "legacy_mode": self.ENV_LEGACY_MODE,
        }

        env_name = env_map.get(key)
        if not env_name:
            # Check for generic PLAN_CASCADE_<KEY> pattern
            env_name = f"PLAN_CASCADE_{key.upper()}"

        value = os.environ.get(env_name)
        if value is None:
            return None

        # Parse boolean values
        if key == "legacy_mode":
            return value.lower() in ("1", "true", "yes", "on")

        # Parse integer values
        if key in ("max_parallel_stories", "max_retries", "timeout_seconds"):
            try:
                return int(value)
            except ValueError:
                return None

        return value

    def get(self, key: str, default: Any = None) -> Any:
        """
        Get a configuration value with hierarchical precedence.

        Priority order:
        1. Environment variables
        2. Project config (.plan-cascade.json)
        3. Global config (~/.plan-cascade/config.json)
        4. Provided default, or built-in default

        Args:
            key: Configuration key (supports dot notation for nested keys, e.g., "quality_gates.test")
            default: Default value if key not found

        Returns:
            Configuration value
        """
        # Check environment variable first
        env_value = self._get_env_value(key)
        if env_value is not None:
            return env_value

        # Check project config
        project_config = self._load_project_config()
        project_value = self._get_nested(project_config, key)
        if project_value is not None:
            return project_value

        # Check global config
        global_config = self._load_global_config()
        global_value = self._get_nested(global_config, key)
        if global_value is not None:
            return global_value

        # Return provided default or built-in default
        if default is not None:
            return default

        return self._get_nested(self.DEFAULTS, key)

    def _get_nested(self, data: dict[str, Any], key: str) -> Any | None:
        """
        Get a nested value from a dictionary using dot notation.

        Args:
            data: Dictionary to search
            key: Key with optional dot notation (e.g., "quality_gates.test")

        Returns:
            Value if found, None otherwise
        """
        parts = key.split(".")
        current = data

        for part in parts:
            if not isinstance(current, dict):
                return None
            if part not in current:
                return None
            current = current[part]

        return current

    def _set_nested(self, data: dict[str, Any], key: str, value: Any) -> None:
        """
        Set a nested value in a dictionary using dot notation.

        Args:
            data: Dictionary to modify
            key: Key with optional dot notation (e.g., "quality_gates.test")
            value: Value to set
        """
        parts = key.split(".")
        current = data

        for part in parts[:-1]:
            if part not in current:
                current[part] = {}
            current = current[part]

        current[parts[-1]] = value

    def set(self, key: str, value: Any, scope: str = "global") -> None:
        """
        Set a configuration value.

        Args:
            key: Configuration key (supports dot notation)
            value: Value to set
            scope: Configuration scope - "global" or "project"

        Raises:
            ValueError: If scope is invalid
        """
        if scope == "global":
            config = self._load_global_config()
            self._set_nested(config, key, value)
            self._save_global_config(config)
            self._global_config = config
        elif scope == "project":
            config = self._load_project_config()
            self._set_nested(config, key, value)
            self._save_project_config(config)
            self._project_config = config
        else:
            raise ValueError(f"Invalid scope: {scope}. Must be 'global' or 'project'")

    def _save_global_config(self, config: dict[str, Any]) -> None:
        """
        Save the global configuration file.

        Args:
            config: Configuration dictionary to save
        """
        config_path = self._get_global_config_path()
        config_path.parent.mkdir(parents=True, exist_ok=True)

        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)

    def _save_project_config(self, config: dict[str, Any]) -> None:
        """
        Save the project configuration file.

        Args:
            config: Configuration dictionary to save
        """
        config_path = self._get_project_config_path()

        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)

    def reset(self, key: str | None = None, scope: str = "global") -> None:
        """
        Reset configuration to defaults.

        Args:
            key: Specific key to reset, or None to reset all
            scope: Configuration scope - "global" or "project"

        Raises:
            ValueError: If scope is invalid
        """
        if scope == "global":
            if key is None:
                # Reset entire global config
                config_path = self._get_global_config_path()
                if config_path.exists():
                    config_path.unlink()
                self._global_config = None
            else:
                config = self._load_global_config()
                self._delete_nested(config, key)
                self._save_global_config(config)
                self._global_config = config
        elif scope == "project":
            if key is None:
                # Reset entire project config
                config_path = self._get_project_config_path()
                if config_path.exists():
                    config_path.unlink()
                self._project_config = None
            else:
                config = self._load_project_config()
                self._delete_nested(config, key)
                self._save_project_config(config)
                self._project_config = config
        else:
            raise ValueError(f"Invalid scope: {scope}. Must be 'global' or 'project'")

    def _delete_nested(self, data: dict[str, Any], key: str) -> None:
        """
        Delete a nested key from a dictionary.

        Args:
            data: Dictionary to modify
            key: Key with optional dot notation
        """
        parts = key.split(".")
        current = data

        for part in parts[:-1]:
            if part not in current:
                return
            current = current[part]

        if parts[-1] in current:
            del current[parts[-1]]

    def get_data_dir(self) -> Path:
        """
        Get the data directory for Plan Cascade runtime files.

        Checks configuration sources in priority order:
        1. PLAN_CASCADE_DATA_DIR environment variable
        2. data_dir in project config
        3. data_dir in global config
        4. Platform-specific default

        Returns:
            Path to the data directory
        """
        # Check environment variable
        env_dir = os.environ.get(self.ENV_DATA_DIR)
        if env_dir:
            return Path(env_dir)

        # Check project config
        project_config = self._load_project_config()
        if "data_dir" in project_config and project_config["data_dir"]:
            return Path(project_config["data_dir"])

        # Check global config
        global_config = self._load_global_config()
        if "data_dir" in global_config and global_config["data_dir"]:
            return Path(global_config["data_dir"])

        # Return platform-specific default
        return self._get_default_data_dir()

    def _get_default_data_dir(self) -> Path:
        """
        Get the platform-specific default data directory.

        Returns:
            Path to the default data directory:
            - Windows: %APPDATA%/plan-cascade
            - Unix/macOS: ~/.plan-cascade
        """
        if sys.platform == "win32":
            appdata = os.environ.get("APPDATA")
            if appdata:
                return Path(appdata) / "plan-cascade"
            return Path.home() / "AppData" / "Roaming" / "plan-cascade"
        return Path.home() / ".plan-cascade"

    def is_legacy_mode(self) -> bool:
        """
        Check if legacy mode is enabled.

        Legacy mode stores all runtime files in the project root
        instead of the user data directory.

        Checks in priority order:
        1. PLAN_CASCADE_LEGACY_MODE environment variable (1, true, yes, on)
        2. legacy_mode in project config
        3. legacy_mode in global config
        4. Auto-detection: checks if prd.json exists in project root

        Returns:
            True if legacy mode is enabled
        """
        # Check environment variable
        env_value = os.environ.get(self.ENV_LEGACY_MODE)
        if env_value is not None:
            return env_value.lower() in ("1", "true", "yes", "on")

        # Check project config
        project_config = self._load_project_config()
        if "legacy_mode" in project_config:
            return bool(project_config["legacy_mode"])

        # Check global config
        global_config = self._load_global_config()
        if "legacy_mode" in global_config:
            return bool(global_config["legacy_mode"])

        # Auto-detection: check for existing prd.json in project root
        # This enables backward compatibility for unmigrated projects
        legacy_prd = self.project_root / "prd.json"
        if legacy_prd.exists():
            return True

        return False

    def get_all(self) -> dict[str, Any]:
        """
        Get all configuration values (merged from all sources).

        Returns:
            Dictionary with all configuration values after merging
        """
        # Start with defaults
        result = dict(self.DEFAULTS)

        # Merge global config
        global_config = self._load_global_config()
        self._deep_merge(result, global_config)

        # Merge project config
        project_config = self._load_project_config()
        self._deep_merge(result, project_config)

        # Override with environment variables
        for key in ["data_dir", "legacy_mode", "max_parallel_stories", "max_retries", "timeout_seconds"]:
            env_value = self._get_env_value(key)
            if env_value is not None:
                result[key] = env_value

        return result

    def _deep_merge(self, base: dict[str, Any], override: dict[str, Any]) -> None:
        """
        Deep merge override into base dictionary.

        Args:
            base: Base dictionary to merge into
            override: Dictionary with override values
        """
        for key, value in override.items():
            if key in base and isinstance(base[key], dict) and isinstance(value, dict):
                self._deep_merge(base[key], value)
            else:
                base[key] = value

    def reload(self) -> None:
        """
        Reload configuration from files.

        Clears cached configuration to force re-reading from disk.
        """
        self._global_config = None
        self._project_config = None


def main():
    """CLI interface for configuration management."""
    import argparse

    parser = argparse.ArgumentParser(description="Plan Cascade Configuration Manager")
    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # show command
    show_parser = subparsers.add_parser("show", help="Show current configuration")
    show_parser.add_argument("--key", help="Specific key to show")

    # set command
    set_parser = subparsers.add_parser("set", help="Set a configuration value")
    set_parser.add_argument("key", help="Configuration key")
    set_parser.add_argument("value", help="Configuration value")
    set_parser.add_argument("--scope", choices=["global", "project"], default="global",
                           help="Configuration scope")

    # reset command
    reset_parser = subparsers.add_parser("reset", help="Reset configuration")
    reset_parser.add_argument("--key", help="Specific key to reset")
    reset_parser.add_argument("--scope", choices=["global", "project"], default="global",
                             help="Configuration scope")

    # data-dir command
    subparsers.add_parser("data-dir", help="Show data directory")

    # legacy-mode command
    subparsers.add_parser("legacy-mode", help="Check legacy mode status")

    args = parser.parse_args()

    config = ConfigManager()

    if args.command == "show":
        if args.key:
            value = config.get(args.key)
            print(f"{args.key}: {value}")
        else:
            all_config = config.get_all()
            print(json.dumps(all_config, indent=2))

    elif args.command == "set":
        # Try to parse value as JSON, otherwise use as string
        try:
            value = json.loads(args.value)
        except json.JSONDecodeError:
            # Check for boolean-like strings
            if args.value.lower() in ("true", "yes", "on"):
                value = True
            elif args.value.lower() in ("false", "no", "off"):
                value = False
            else:
                # Try to parse as int
                try:
                    value = int(args.value)
                except ValueError:
                    value = args.value

        config.set(args.key, value, scope=args.scope)
        print(f"Set {args.key} = {value} (scope: {args.scope})")

    elif args.command == "reset":
        config.reset(key=args.key, scope=args.scope)
        if args.key:
            print(f"Reset {args.key} (scope: {args.scope})")
        else:
            print(f"Reset all configuration (scope: {args.scope})")

    elif args.command == "data-dir":
        print(config.get_data_dir())

    elif args.command == "legacy-mode":
        is_legacy = config.is_legacy_mode()
        print(f"Legacy mode: {'enabled' if is_legacy else 'disabled'}")

    else:
        parser.print_help()


if __name__ == "__main__":
    main()
