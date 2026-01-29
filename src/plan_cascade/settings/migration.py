"""
Configuration migration and version management for Plan Cascade.

This module provides version-aware configuration file handling:
- Version detection for existing configuration files
- Chain migration from older versions to current
- Automatic migration during load operations
- Version tagging during save operations
"""

import logging
from typing import Any, Callable, Dict, List, Optional

logger = logging.getLogger(__name__)


class ConfigMigration:
    """
    Configuration migration manager.

    Handles version detection and migration of configuration files
    to ensure compatibility across Plan Cascade versions.

    The migration system supports chain migrations, allowing a config
    file to be migrated through multiple versions (e.g., v0 -> v1 -> v2).

    Attributes:
        CURRENT_VERSION: The current configuration file version.
        MIGRATIONS: Dictionary mapping source versions to migration functions.
    """

    CURRENT_VERSION: str = "1.0.0"

    # Migration registry: maps source version to (target_version, migration_function)
    # Each migration function takes a dict and returns a modified dict
    MIGRATIONS: Dict[str, tuple[str, Callable[[Dict[str, Any]], Dict[str, Any]]]] = {}

    @classmethod
    def register_migration(
        cls,
        from_version: str,
        to_version: str,
        migration_func: Callable[[Dict[str, Any]], Dict[str, Any]],
    ) -> None:
        """
        Register a migration function.

        Args:
            from_version: Source version string.
            to_version: Target version string.
            migration_func: Function that transforms config dict from source to target.
        """
        cls.MIGRATIONS[from_version] = (to_version, migration_func)

    @classmethod
    def get_version(cls, data: Dict[str, Any]) -> str:
        """
        Get the version of a configuration dictionary.

        Args:
            data: Configuration dictionary.

        Returns:
            Version string, or "0" if no version field exists (legacy config).
        """
        return data.get("config_version", "0")

    @classmethod
    def set_version(cls, data: Dict[str, Any], version: Optional[str] = None) -> Dict[str, Any]:
        """
        Set the version field in a configuration dictionary.

        Args:
            data: Configuration dictionary.
            version: Version string to set. Defaults to CURRENT_VERSION.

        Returns:
            Modified configuration dictionary with version field.
        """
        data["config_version"] = version or cls.CURRENT_VERSION
        return data

    @classmethod
    def migrate(cls, data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Migrate configuration data to the current version.

        Applies all necessary migrations in sequence to bring
        a configuration file from its current version to CURRENT_VERSION.

        Args:
            data: Configuration dictionary to migrate.

        Returns:
            Migrated configuration dictionary at CURRENT_VERSION.
        """
        current_version = cls.get_version(data)

        if current_version == cls.CURRENT_VERSION:
            logger.debug(f"Config already at current version {cls.CURRENT_VERSION}")
            return data

        logger.info(f"Migrating config from version {current_version}")

        # Apply migrations in chain
        migrated_data = data.copy()
        migration_path: List[str] = []

        while current_version != cls.CURRENT_VERSION:
            if current_version not in cls.MIGRATIONS:
                # No migration available, try to migrate from v0
                if current_version == "0":
                    migrated_data = cls._migrate_from_v0(migrated_data)
                    current_version = "1.0.0"
                    migration_path.append("0 -> 1.0.0")
                else:
                    # No migration path available
                    logger.warning(
                        f"No migration path from version {current_version} "
                        f"to {cls.CURRENT_VERSION}. Using data as-is."
                    )
                    break
            else:
                target_version, migration_func = cls.MIGRATIONS[current_version]
                migrated_data = migration_func(migrated_data)
                migration_path.append(f"{current_version} -> {target_version}")
                current_version = target_version

        # Set final version
        migrated_data["config_version"] = cls.CURRENT_VERSION

        if migration_path:
            logger.info(f"Migration complete: {' -> '.join(migration_path)}")

        return migrated_data

    @classmethod
    def needs_migration(cls, data: Dict[str, Any]) -> bool:
        """
        Check if configuration data needs migration.

        Args:
            data: Configuration dictionary.

        Returns:
            True if the configuration version differs from CURRENT_VERSION.
        """
        return cls.get_version(data) != cls.CURRENT_VERSION

    @classmethod
    def _migrate_from_v0(cls, data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Migrate from v0 (legacy config without version) to v1.0.0.

        This handles configuration files created before version tracking
        was implemented.

        Args:
            data: Legacy configuration dictionary.

        Returns:
            Configuration dictionary compatible with v1.0.0.
        """
        migrated = data.copy()

        # Ensure all required fields exist with defaults
        if "backend" not in migrated:
            migrated["backend"] = "claude-code"

        if "provider" not in migrated:
            migrated["provider"] = "claude"

        if "model" not in migrated:
            migrated["model"] = ""

        if "agents" not in migrated:
            migrated["agents"] = [
                {
                    "name": "claude-code",
                    "enabled": True,
                    "command": "claude",
                    "is_default": True,
                },
                {"name": "aider", "enabled": False, "command": "aider"},
                {"name": "codex", "enabled": False, "command": "codex"},
            ]

        if "agent_selection" not in migrated:
            migrated["agent_selection"] = "prefer_default"

        if "default_agent" not in migrated:
            migrated["default_agent"] = "claude-code"

        if "quality_gates" not in migrated:
            migrated["quality_gates"] = {
                "typecheck": True,
                "test": True,
                "lint": True,
                "custom": False,
                "custom_script": "",
                "max_retries": 3,
            }

        if "max_parallel_stories" not in migrated:
            migrated["max_parallel_stories"] = 3

        if "max_iterations" not in migrated:
            migrated["max_iterations"] = 50

        if "timeout_seconds" not in migrated:
            migrated["timeout_seconds"] = 300

        if "default_mode" not in migrated:
            migrated["default_mode"] = "simple"

        if "theme" not in migrated:
            migrated["theme"] = "system"

        migrated["config_version"] = "1.0.0"

        logger.debug("Migrated legacy config to v1.0.0")
        return migrated


# Register future migrations here
# Example:
# def migrate_v1_to_v2(data: Dict[str, Any]) -> Dict[str, Any]:
#     """Migrate from v1.0.0 to v2.0.0"""
#     migrated = data.copy()
#     # Add new fields, transform existing fields, etc.
#     migrated["config_version"] = "2.0.0"
#     return migrated
#
# ConfigMigration.register_migration("1.0.0", "2.0.0", migrate_v1_to_v2)
