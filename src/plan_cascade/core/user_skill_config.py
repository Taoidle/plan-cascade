#!/usr/bin/env python3
"""
User Skill Configuration for Plan Cascade

Manages user-defined skill configuration from two sources:
- Project-level: .plan-cascade/skills.json (in project root)
- User-level: ~/.plan-cascade/skills.json (in user's home directory)

Configuration cascade: project-level > user-level > builtin defaults
Higher priority skills override lower priority ones when conflicts arise.
"""

import json
import logging
from dataclasses import dataclass, field
from pathlib import Path

logger = logging.getLogger(__name__)


@dataclass
class UserSkillEntry:
    """Represents a user-defined skill entry.

    Attributes:
        name: The skill identifier (must be unique)
        path: Local path to the skill directory (relative or absolute)
        url: Remote URL for the skill
        detect: Detection configuration with 'files' and 'patterns' lists
        priority: Skill priority (should be in range 101-200 for user skills)
        inject_into: List of phases to inject skill into
    """

    name: str
    path: str | None = None
    url: str | None = None
    detect: dict | None = None
    priority: int = 150
    inject_into: list | None = field(default_factory=lambda: ["implementation"])

    def __post_init__(self):
        if self.detect is None:
            self.detect = {"files": [], "patterns": []}
        if self.inject_into is None:
            self.inject_into = ["implementation"]


class UserSkillConfig:
    """Manages user-defined skill configuration.

    Loads and merges skill configurations from:
    1. Project-level: .plan-cascade/skills.json
    2. User-level: ~/.plan-cascade/skills.json

    Configuration precedence: project > user > defaults
    """

    CONFIG_FILENAME = "skills.json"
    CONFIG_DIR = ".plan-cascade"

    # Priority range for user-defined skills
    USER_PRIORITY_MIN = 101
    USER_PRIORITY_MAX = 200
    USER_PRIORITY_DEFAULT = 150

    def __init__(self, project_root: Path, verbose: bool = False):
        """Initialize the user skill config.

        Args:
            project_root: Path to the current project
            verbose: Enable verbose logging
        """
        self.project_root = Path(project_root)
        self._verbose = verbose
        self._validation_errors: list[str] = []

        # Load configurations
        self._user_config = self._load_user_config()
        self._project_config = self._load_project_config()
        self._merged_config = self._merge_configs(self._user_config, self._project_config)

        # Validate merged config
        self._validation_errors = self._validate_config(self._merged_config)
        if self._validation_errors and self._verbose:
            for error in self._validation_errors:
                self._log(f"Config validation warning: {error}", "warning")

    def _log(self, message: str, level: str = "info") -> None:
        """Log a message with appropriate formatting.

        Args:
            message: Message to log
            level: Log level ("info", "warning", "debug")
        """
        prefix = "[UserSkillConfig]"
        if level == "debug" and not self._verbose:
            return
        if level == "warning":
            logger.warning(f"{prefix} {message}")
        elif level == "debug":
            logger.debug(f"{prefix} {message}")
        else:
            logger.info(f"{prefix} {message}")
        if self._verbose or level != "debug":
            print(f"{prefix} {message}")

    def _get_user_config_path(self) -> Path:
        """Get the path to user-level config file.

        Returns:
            Path to ~/.plan-cascade/skills.json
        """
        return Path.home() / self.CONFIG_DIR / self.CONFIG_FILENAME

    def _get_project_config_path(self) -> Path:
        """Get the path to project-level config file.

        Returns:
            Path to .plan-cascade/skills.json in project root
        """
        return self.project_root / self.CONFIG_DIR / self.CONFIG_FILENAME

    def _load_config_file(self, config_path: Path) -> dict:
        """Load a configuration file.

        Args:
            config_path: Path to the config file

        Returns:
            Parsed config dict, or empty dict if file doesn't exist
        """
        if not config_path.exists():
            self._log(f"Config file not found: {config_path}", "debug")
            return {}

        try:
            content = config_path.read_text(encoding="utf-8")
            config = json.loads(content)
            self._log(f"Loaded config from: {config_path}", "debug")
            return config
        except json.JSONDecodeError as e:
            self._log(f"Invalid JSON in {config_path}: {e}", "warning")
            return {}
        except OSError as e:
            self._log(f"Error reading {config_path}: {e}", "warning")
            return {}

    def _load_user_config(self) -> dict:
        """Load user-level configuration from ~/.plan-cascade/skills.json.

        Returns:
            User-level config dict
        """
        return self._load_config_file(self._get_user_config_path())

    def _load_project_config(self) -> dict:
        """Load project-level configuration from .plan-cascade/skills.json.

        Returns:
            Project-level config dict
        """
        return self._load_config_file(self._get_project_config_path())

    def _merge_configs(self, user_config: dict, project_config: dict) -> dict:
        """Merge user and project configurations.

        Project-level config takes precedence over user-level config.
        Skills with the same name in project config override user config.

        Args:
            user_config: User-level config dict
            project_config: Project-level config dict

        Returns:
            Merged config dict
        """
        merged = {
            "version": project_config.get("version") or user_config.get("version") or "1.0.0",
            "skills": [],
        }

        # Build skill map: user skills first, then project skills override
        skill_map: dict[str, dict] = {}

        # Add user-level skills
        for skill in user_config.get("skills", []):
            name = skill.get("name")
            if name:
                skill_map[name] = {**skill, "_source_level": "user"}

        # Add project-level skills (override user-level)
        for skill in project_config.get("skills", []):
            name = skill.get("name")
            if name:
                if name in skill_map:
                    self._log(f"Project skill '{name}' overrides user skill", "debug")
                skill_map[name] = {**skill, "_source_level": "project"}

        # Convert back to list
        merged["skills"] = list(skill_map.values())

        return merged

    def _validate_config(self, config: dict) -> list[str]:
        """Validate the merged configuration.

        Checks:
        - Either path or url must be provided (not both, not neither)
        - Priority must be in range 101-200
        - Name must be unique
        - detect.files should be a list

        Args:
            config: Config dict to validate

        Returns:
            List of validation error messages
        """
        errors = []
        seen_names: set[str] = set()

        for idx, skill in enumerate(config.get("skills", [])):
            skill_ref = skill.get("name", f"skill[{idx}]")

            # Check name uniqueness
            name = skill.get("name")
            if not name:
                errors.append(f"{skill_ref}: 'name' is required")
            elif name in seen_names:
                errors.append(f"{skill_ref}: duplicate skill name '{name}'")
            else:
                seen_names.add(name)

            # Check path/url mutual exclusivity
            has_path = bool(skill.get("path"))
            has_url = bool(skill.get("url"))

            if not has_path and not has_url:
                errors.append(f"{skill_ref}: either 'path' or 'url' must be provided")
            elif has_path and has_url:
                errors.append(f"{skill_ref}: only one of 'path' or 'url' should be provided, not both")

            # Check priority range
            priority = skill.get("priority", self.USER_PRIORITY_DEFAULT)
            if not isinstance(priority, int):
                errors.append(f"{skill_ref}: 'priority' must be an integer")
            elif not (self.USER_PRIORITY_MIN <= priority <= self.USER_PRIORITY_MAX):
                errors.append(
                    f"{skill_ref}: priority {priority} is outside valid range "
                    f"({self.USER_PRIORITY_MIN}-{self.USER_PRIORITY_MAX})"
                )

            # Check detect.files is a list
            detect = skill.get("detect", {})
            if detect:
                files = detect.get("files")
                if files is not None and not isinstance(files, list):
                    errors.append(f"{skill_ref}: 'detect.files' must be a list")
                patterns = detect.get("patterns")
                if patterns is not None and not isinstance(patterns, list):
                    errors.append(f"{skill_ref}: 'detect.patterns' must be a list")

            # Check inject_into is a list
            inject_into = skill.get("inject_into")
            if inject_into is not None and not isinstance(inject_into, list):
                errors.append(f"{skill_ref}: 'inject_into' must be a list")

        return errors

    def get_validation_errors(self) -> list[str]:
        """Get validation errors from the current configuration.

        Returns:
            List of validation error messages
        """
        return self._validation_errors.copy()

    def is_valid(self) -> bool:
        """Check if the configuration is valid.

        Returns:
            True if no validation errors
        """
        return len(self._validation_errors) == 0

    def get_user_skills(self) -> list[dict]:
        """Get the merged list of user-defined skills.

        Returns:
            List of skill configuration dicts
        """
        return self._merged_config.get("skills", []).copy()

    def get_user_sources(self) -> dict:
        """Get user sources configuration for ExternalSkillLoader.

        Converts user skill entries into the sources format expected
        by ExternalSkillLoader.

        Returns:
            Dict of source configurations keyed by source name
        """
        sources = {}

        for skill in self._merged_config.get("skills", []):
            name = skill.get("name")
            if not name:
                continue

            # Create a unique source name for this user skill
            source_name = f"user-{name}"

            path = skill.get("path")
            url = skill.get("url")

            if path:
                # Resolve relative paths
                resolved_path = self._resolve_path(path, skill.get("_source_level", "project"))
                sources[source_name] = {
                    "type": "user",
                    "path": str(resolved_path),
                }
            elif url:
                sources[source_name] = {
                    "type": "user",
                    "url": url,
                }

        return sources

    def _resolve_path(self, path: str, source_level: str = "project") -> Path:
        """Resolve a path relative to the appropriate config location.

        Args:
            path: The path string (relative or absolute)
            source_level: "user" or "project" indicating config source

        Returns:
            Resolved absolute path
        """
        path_obj = Path(path)

        if path_obj.is_absolute():
            return path_obj

        # Resolve relative to config file location
        if source_level == "user":
            base = Path.home() / self.CONFIG_DIR
        else:  # project
            base = self.project_root / self.CONFIG_DIR

        return (base / path_obj).resolve()

    def get_skills_for_loader(self) -> tuple[dict, dict]:
        """Get skills and sources formatted for ExternalSkillLoader integration.

        Returns:
            Tuple of (skills_config, sources_config) for ExternalSkillLoader
        """
        skills_config = {}
        sources_config = self.get_user_sources()

        for skill in self._merged_config.get("skills", []):
            name = skill.get("name")
            if not name:
                continue

            source_name = f"user-{name}"

            # Build skill config matching ExternalSkillLoader format
            skills_config[name] = {
                "source": source_name,
                "skill_path": "",  # User skills point directly to SKILL.md location
                "detect": skill.get("detect", {"files": [], "patterns": []}),
                "inject_into": skill.get("inject_into", ["implementation"]),
                "priority": skill.get("priority", self.USER_PRIORITY_DEFAULT),
            }

        return skills_config, sources_config

    def _save_config_file(self, config_path: Path, config: dict) -> bool:
        """Save a configuration to file.

        Args:
            config_path: Path to save to
            config: Config dict to save

        Returns:
            True if save succeeded
        """
        try:
            # Ensure directory exists
            config_path.parent.mkdir(parents=True, exist_ok=True)

            # Remove internal fields before saving
            skills_to_save = []
            for skill in config.get("skills", []):
                skill_copy = {k: v for k, v in skill.items() if not k.startswith("_")}
                skills_to_save.append(skill_copy)

            save_config = {
                "version": config.get("version", "1.0.0"),
                "skills": skills_to_save,
            }

            content = json.dumps(save_config, indent=2, ensure_ascii=False)
            config_path.write_text(content, encoding="utf-8")
            self._log(f"Saved config to: {config_path}")
            return True
        except OSError as e:
            self._log(f"Error saving {config_path}: {e}", "warning")
            return False

    def add_skill(self, skill_entry: dict, level: str = "project") -> bool:
        """Add a skill to the configuration.

        Args:
            skill_entry: Skill configuration dict
            level: "project" or "user" indicating which config to modify

        Returns:
            True if skill was added successfully
        """
        name = skill_entry.get("name")
        if not name:
            self._log("Cannot add skill without a name", "warning")
            return False

        # Validate the new skill
        test_config = {"skills": [skill_entry]}
        errors = self._validate_config(test_config)
        if errors:
            for error in errors:
                self._log(f"Validation error: {error}", "warning")
            return False

        # Load the appropriate config
        if level == "user":
            config = self._user_config.copy()
            config_path = self._get_user_config_path()
        else:
            config = self._project_config.copy()
            config_path = self._get_project_config_path()

        # Ensure skills list exists
        if "skills" not in config:
            config["skills"] = []
        if "version" not in config:
            config["version"] = "1.0.0"

        # Check for existing skill with same name
        existing_idx = None
        for idx, skill in enumerate(config["skills"]):
            if skill.get("name") == name:
                existing_idx = idx
                break

        if existing_idx is not None:
            # Update existing
            config["skills"][existing_idx] = skill_entry
            self._log(f"Updated existing skill: {name}")
        else:
            # Add new
            config["skills"].append(skill_entry)
            self._log(f"Added new skill: {name}")

        # Save and reload
        if self._save_config_file(config_path, config):
            # Reload configurations
            self._user_config = self._load_user_config()
            self._project_config = self._load_project_config()
            self._merged_config = self._merge_configs(self._user_config, self._project_config)
            self._validation_errors = self._validate_config(self._merged_config)
            return True

        return False

    def remove_skill(self, skill_name: str, level: str = "project") -> bool:
        """Remove a skill from the configuration.

        Args:
            skill_name: Name of the skill to remove
            level: "project" or "user" indicating which config to modify

        Returns:
            True if skill was removed successfully
        """
        # Load the appropriate config
        if level == "user":
            config = self._user_config.copy()
            config_path = self._get_user_config_path()
        else:
            config = self._project_config.copy()
            config_path = self._get_project_config_path()

        # Find and remove the skill
        skills = config.get("skills", [])
        original_count = len(skills)
        config["skills"] = [s for s in skills if s.get("name") != skill_name]

        if len(config["skills"]) == original_count:
            self._log(f"Skill not found in {level} config: {skill_name}", "warning")
            return False

        self._log(f"Removed skill: {skill_name}")

        # Save and reload
        if self._save_config_file(config_path, config):
            # Reload configurations
            self._user_config = self._load_user_config()
            self._project_config = self._load_project_config()
            self._merged_config = self._merge_configs(self._user_config, self._project_config)
            self._validation_errors = self._validate_config(self._merged_config)
            return True

        return False

    def list_skills(self) -> list[dict]:
        """List all configured skills with their source level.

        Returns:
            List of skill info dicts including _source_level
        """
        return self._merged_config.get("skills", []).copy()

    def get_skill(self, skill_name: str) -> dict | None:
        """Get a specific skill by name.

        Args:
            skill_name: Name of the skill

        Returns:
            Skill config dict or None if not found
        """
        for skill in self._merged_config.get("skills", []):
            if skill.get("name") == skill_name:
                return skill.copy()
        return None
