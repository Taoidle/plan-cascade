#!/usr/bin/env python3
"""
External Skill Loader for Plan Cascade

Loads framework-specific skills from three sources:
- builtin: Built-in skills bundled with Plan Cascade (priority 1-50)
- submodule: External skills from Git submodules (priority 51-100)
- user: User-defined skills from local paths or URLs (priority 101-200)

Skills are sorted by priority and same-name skills are deduplicated
with higher priority skills overriding lower priority ones.

User skills can be configured via:
- Project-level: .plan-cascade/skills.json
- User-level: ~/.plan-cascade/skills.json
"""

import hashlib
import json
import logging
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .skill_cache import SkillCache
    from .user_skill_config import UserSkillConfig

logger = logging.getLogger(__name__)


@dataclass
class LoadedSkill:
    """Represents a loaded external skill.

    Attributes:
        name: The skill identifier
        content: The skill content (from SKILL.md)
        priority: Skill priority (1-50 builtin, 51-100 submodule, 101-200 user)
        source: The source name from config
        source_type: The type of source ("builtin", "submodule", or "user")
        origin: Full path or URL where the skill was loaded from
    """
    name: str
    content: str
    priority: int
    source: str
    source_type: str = "submodule"  # "builtin" | "submodule" | "user"
    origin: str = ""  # Full path or URL where skill was loaded from


class ExternalSkillLoader:
    """Loads and manages external framework skills from multiple sources.

    Supports three source types with fixed priority ranges:
    - builtin (1-50): Built-in skills bundled with Plan Cascade
    - submodule (51-100): External skills from Git submodules
    - user (101-200): User-defined skills from local paths or URLs

    Higher priority skills override lower priority skills with the same name.
    """

    # Source type priority ranges
    PRIORITY_RANGES = {
        "builtin": (1, 50),
        "submodule": (51, 100),
        "user": (101, 200),
    }

    def __init__(self, project_root: Path, plugin_root: Path = None, verbose: bool = False):
        """Initialize the skill loader.

        Args:
            project_root: Path to the current project
            plugin_root: Path to the Plan Cascade plugin (auto-detected if None)
            verbose: Enable verbose logging for debugging
        """
        self.project_root = Path(project_root)
        self.plugin_root = plugin_root or self._find_plugin_root()
        self._verbose = verbose
        self._cache: dict[str, LoadedSkill] = {}  # Changed to cache full LoadedSkill
        self._skill_cache: SkillCache | None = None  # For caching downloaded skills

        # Load base config first
        self.config = self._load_config()

        # Load and merge user skill config
        self._user_skill_config: UserSkillConfig | None = None
        self._integrate_user_skills()

    def _find_plugin_root(self) -> Path:
        """Find the Plan Cascade plugin root directory."""
        # Check common locations
        candidates = [
            Path(__file__).parent.parent.parent.parent,  # From core module
            Path.home() / ".claude" / "plugins" / "plan-cascade",
        ]
        for candidate in candidates:
            if (candidate / "external-skills.json").exists():
                return candidate
        return self.project_root

    def _load_config(self) -> dict:
        """Load external skills configuration."""
        config_path = self.plugin_root / "external-skills.json"
        if not config_path.exists():
            return {"skills": {}, "sources": {}, "settings": {}, "priority_ranges": self.PRIORITY_RANGES}

        try:
            config = json.loads(config_path.read_text(encoding="utf-8"))
            # Ensure priority_ranges exists
            if "priority_ranges" not in config:
                config["priority_ranges"] = {
                    "builtin": {"min": 1, "max": 50},
                    "submodule": {"min": 51, "max": 100},
                    "user": {"min": 101, "max": 200},
                }
            return config
        except (OSError, json.JSONDecodeError) as e:
            self._log(f"Warning: Could not load config: {e}")
            return {"skills": {}, "sources": {}, "settings": {}, "priority_ranges": self.PRIORITY_RANGES}

    def _integrate_user_skills(self) -> None:
        """Integrate user-defined skills from .plan-cascade/skills.json.

        Loads skills from both project-level and user-level config files
        and merges them into the main config with source_type="user".
        """
        try:
            from .user_skill_config import UserSkillConfig

            self._user_skill_config = UserSkillConfig(
                self.project_root, verbose=self._verbose
            )

            # Check for validation errors
            if not self._user_skill_config.is_valid():
                errors = self._user_skill_config.get_validation_errors()
                for error in errors:
                    self._log(f"User skill config error: {error}", "warning")
                # Continue with valid skills

            # Get skills and sources from user config
            user_skills, user_sources = self._user_skill_config.get_skills_for_loader()

            if not user_skills:
                self._log("No user skills found in config files", "debug")
                return

            # Merge user sources into config
            if "sources" not in self.config:
                self.config["sources"] = {}
            self.config["sources"].update(user_sources)

            # Merge user skills into config
            if "skills" not in self.config:
                self.config["skills"] = {}

            for skill_name, skill_config in user_skills.items():
                if skill_name in self.config["skills"]:
                    existing_priority = self.config["skills"][skill_name].get("priority", 0)
                    new_priority = skill_config.get("priority", 150)
                    if new_priority > existing_priority:
                        self._log(
                            f"User skill '{skill_name}' (priority {new_priority}) "
                            f"overrides existing skill (priority {existing_priority})",
                            "debug"
                        )
                        self.config["skills"][skill_name] = skill_config
                    else:
                        self._log(
                            f"User skill '{skill_name}' (priority {new_priority}) "
                            f"skipped - existing has higher priority ({existing_priority})",
                            "debug"
                        )
                else:
                    self.config["skills"][skill_name] = skill_config
                    self._log(f"Added user skill: {skill_name}", "debug")

            self._log(f"Integrated {len(user_skills)} user skill(s)", "debug")

        except ImportError as e:
            self._log(f"Could not import UserSkillConfig: {e}", "warning")
        except Exception as e:
            self._log(f"Error integrating user skills: {e}", "warning")

    def get_user_skill_config(self) -> "UserSkillConfig | None":
        """Get the UserSkillConfig instance if available.

        Returns:
            UserSkillConfig instance or None
        """
        return self._user_skill_config

    def _log(self, message: str, level: str = "info") -> None:
        """Log a message with appropriate formatting.

        Args:
            message: Message to log
            level: Log level ("info", "warning", "debug")
        """
        prefix = "[ExternalSkillLoader]"
        if level == "debug" and not self._verbose:
            return
        if level == "warning":
            logger.warning(f"{prefix} {message}")
        elif level == "debug":
            logger.debug(f"{prefix} {message}")
        else:
            logger.info(f"{prefix} {message}")
        # Also print for CLI visibility
        if self._verbose or level != "debug":
            print(f"{prefix} {message}")

    def _get_source_type(self, source_name: str) -> str:
        """Get the type of a source from configuration.

        Args:
            source_name: Name of the source in config

        Returns:
            Source type string: "builtin", "submodule", or "user"
        """
        source_config = self.config.get("sources", {}).get(source_name, {})
        return source_config.get("type", "submodule")

    def _resolve_source_path(self, source_config: dict, skill_path: str) -> tuple[Path | None, str]:
        """Resolve the full path to a skill based on source type.

        Args:
            source_config: Source configuration dict
            skill_path: Relative path to skill within source

        Returns:
            Tuple of (resolved_path, origin_string) or (None, error_message) if resolution fails
        """
        source_type = source_config.get("type", "submodule")
        source_path = source_config.get("path", "")

        if source_type == "builtin":
            # Resolve relative to plugin root
            full_path = self.plugin_root / source_path / skill_path / "SKILL.md"
            return full_path, str(full_path)

        elif source_type == "submodule":
            # Resolve relative to plugin root (existing behavior)
            full_path = self.plugin_root / source_path / skill_path / "SKILL.md"
            return full_path, str(full_path)

        elif source_type == "user":
            # Handle both local paths and URLs
            location = source_config.get("path") or source_config.get("url", "")

            if location.startswith(("http://", "https://")):
                # URL - download to temp cache
                return self._download_skill_from_url(location, skill_path)
            else:
                # Local path - could be absolute or relative
                local_path = Path(location)
                if not local_path.is_absolute():
                    # Relative to project root for user-defined paths
                    local_path = self.project_root / local_path
                full_path = local_path / skill_path / "SKILL.md"
                return full_path, str(full_path)

        else:
            self._log(f"Unknown source type: {source_type}", "warning")
            return None, f"unknown source type: {source_type}"

    def _get_skill_cache(self) -> "SkillCache":
        """Get or create the SkillCache instance.

        Returns:
            SkillCache instance for caching remote skills
        """
        if self._skill_cache is None:
            from .skill_cache import SkillCache

            self._skill_cache = SkillCache(verbose=self._verbose)
        return self._skill_cache

    def _download_skill_from_url(self, base_url: str, skill_path: str) -> tuple[Path | None, str]:
        """Download a skill from a remote URL using the persistent cache.

        Uses SkillCache for persistent caching with:
        - 7-day default TTL
        - Graceful degradation using expired cache on network errors
        - Stored in ~/.plan-cascade/cache/skills/

        Args:
            base_url: Base URL of the skill repository
            skill_path: Relative path to skill within repository

        Returns:
            Tuple of (cached_file_path, origin_url) or (None, error_message)
        """
        # Construct full URL
        if skill_path:
            skill_url = f"{base_url.rstrip('/')}/{skill_path}/SKILL.md"
        else:
            # Direct URL to SKILL.md (user skills may point directly)
            skill_url = base_url if base_url.endswith("SKILL.md") else f"{base_url.rstrip('/')}/SKILL.md"
        origin = skill_url

        # Use SkillCache for persistent caching with graceful degradation
        cache = self._get_skill_cache()
        content = cache.get_or_download(skill_url)

        if content is None:
            return None, f"failed to load: {skill_url}"

        # Return the cached file path
        cache_path = cache.get_skill_path(skill_url)
        return cache_path, origin

    def refresh_remote_skills(self, url: str | None = None) -> dict:
        """Force refresh of cached remote skills.

        Args:
            url: Specific URL to refresh, or None to refresh all

        Returns:
            Dict with refresh results: {"refreshed": [], "failed": [], "skipped": []}
        """
        cache = self._get_skill_cache()
        return cache.refresh(url)

    def clear_skill_cache(self, url: str | None = None) -> dict:
        """Clear cached remote skills.

        Args:
            url: Specific URL to clear, or None to clear all

        Returns:
            Dict with clear results: {"cleared": [], "failed": []}
        """
        cache = self._get_skill_cache()
        return cache.clear(url)

    def get_skill_cache_stats(self) -> dict:
        """Get skill cache statistics.

        Returns:
            Dict with cache statistics
        """
        cache = self._get_skill_cache()
        return cache.get_cache_stats()

    def list_cached_skills(self) -> list:
        """List all cached remote skills.

        Returns:
            List of SkillCacheEntry objects
        """
        cache = self._get_skill_cache()
        return cache.list_cached()

    def detect_applicable_skills(self, verbose: bool = False) -> list[str]:
        """Detect which skills apply to the current project.

        Skills are collected from all three source types (builtin, submodule, user),
        sorted by priority (highest first), and deduplicated so that same-name
        skills are resolved with the highest priority version winning.

        Args:
            verbose: If True, print detection results to stdout
        """
        # Use instance verbose setting if not overridden
        show_verbose = verbose or self._verbose
        applicable = []
        detection_log = []

        for skill_name, skill_config in self.config.get("skills", {}).items():
            if self._skill_matches_project(skill_config):
                source = skill_config.get("source", "unknown")
                source_type = self._get_source_type(source)
                priority = skill_config.get("priority", 0)

                applicable.append(skill_name)
                detection_log.append({
                    "name": skill_name,
                    "source": source,
                    "source_type": source_type,
                    "priority": priority,
                })

        # Sort by priority (highest first)
        applicable.sort(
            key=lambda s: self.config["skills"][s].get("priority", 0),
            reverse=True
        )

        # Deduplicate same-base-name skills, keeping highest priority
        deduplicated, overrides = self._deduplicate_by_priority(applicable, detection_log)

        # Log overrides for debugging
        if show_verbose and overrides:
            self._log("Skill overrides (higher priority wins):")
            for override in overrides:
                self._log(f"  {override['winner']} (priority {override['winner_priority']}) "
                         f"overrides {override['loser']} (priority {override['loser_priority']})")

        # Limit to max skills
        max_skills = self.config.get("settings", {}).get("max_skills_per_story", 3)
        result = deduplicated[:max_skills]

        if show_verbose and result:
            self._log("Detected applicable skills:")
            for log_entry in sorted(detection_log, key=lambda x: -x["priority"]):
                if log_entry["name"] in result:
                    self._log(f"  ✓ {log_entry['name']} "
                             f"(source: {log_entry['source']}, "
                             f"type: {log_entry['source_type']}, "
                             f"priority: {log_entry['priority']})")

        return result

    def _deduplicate_by_priority(
        self, skills: list[str], detection_log: list[dict]
    ) -> tuple[list[str], list[dict]]:
        """Deduplicate skills with same base name, keeping highest priority.

        When multiple skills have the same base name (e.g., "react-best-practices"
        from both builtin and user sources), only the highest priority version
        is kept.

        Args:
            skills: List of skill names sorted by priority (highest first)
            detection_log: List of detection info dicts for logging

        Returns:
            Tuple of (deduplicated_skills, override_records)
        """
        # Build lookup from name to log entry
        log_lookup = {entry["name"]: entry for entry in detection_log}

        # Track seen base names and their winning skill
        seen_base_names: dict[str, str] = {}  # base_name -> winning skill name
        deduplicated: list[str] = []
        overrides: list[dict] = []

        for skill_name in skills:
            # Extract base name (e.g., "react-best-practices" from both sources)
            # For now, use the full skill name as the base name
            # Skills with same name from different sources will conflict
            base_name = self._get_skill_base_name(skill_name)

            if base_name in seen_base_names:
                # This is a lower priority duplicate
                winner = seen_base_names[base_name]
                winner_entry = log_lookup.get(winner, {})
                loser_entry = log_lookup.get(skill_name, {})

                overrides.append({
                    "base_name": base_name,
                    "winner": winner,
                    "winner_priority": winner_entry.get("priority", 0),
                    "winner_source_type": winner_entry.get("source_type", "unknown"),
                    "loser": skill_name,
                    "loser_priority": loser_entry.get("priority", 0),
                    "loser_source_type": loser_entry.get("source_type", "unknown"),
                })
            else:
                # First occurrence (highest priority) wins
                seen_base_names[base_name] = skill_name
                deduplicated.append(skill_name)

        return deduplicated, overrides

    def _get_skill_base_name(self, skill_name: str) -> str:
        """Extract the base name of a skill for deduplication.

        This allows skills from different sources to be compared.
        For example, "python-best-practices" from both builtin and user
        sources would have the same base name.

        Args:
            skill_name: Full skill name

        Returns:
            Base name for deduplication comparison
        """
        # Currently, skill names are already unique identifiers
        # If we want source-agnostic deduplication, we could strip prefixes
        # For now, return the full name as skills are defined uniquely in config
        return skill_name

    def _skill_matches_project(self, skill_config: dict) -> bool:
        """Check if a skill matches the current project."""
        detect = skill_config.get("detect", {})
        files_to_check = detect.get("files", [])
        patterns = detect.get("patterns", [])

        for filename in files_to_check:
            file_path = self.project_root / filename
            if file_path.exists():
                try:
                    content = file_path.read_text(encoding="utf-8")
                    for pattern in patterns:
                        if pattern in content:
                            return True
                except OSError:
                    continue
        return False

    def load_skill_content(self, skill_name: str) -> LoadedSkill | None:
        """Load a skill's SKILL.md content.

        Supports loading from three source types:
        - builtin: From builtin-skills/ directory
        - submodule: From external-skills/ Git submodules
        - user: From user-specified local paths or URLs

        Args:
            skill_name: Name of the skill to load

        Returns:
            LoadedSkill instance or None if loading fails
        """
        # Check cache - now caches full LoadedSkill
        if skill_name in self._cache:
            return self._cache[skill_name]

        skill_config = self.config.get("skills", {}).get(skill_name)
        if not skill_config:
            self._log(f"Skill not found in config: {skill_name}", "warning")
            return None

        source_name = skill_config.get("source")
        source_config = self.config.get("sources", {}).get(source_name)
        if not source_config:
            self._log(f"Source not found in config: {source_name}", "warning")
            return None

        # Get source type for tracking
        source_type = self._get_source_type(source_name)

        # Resolve path based on source type
        skill_path_str = skill_config.get("skill_path", "")
        resolved_path, origin = self._resolve_source_path(source_config, skill_path_str)

        if resolved_path is None:
            self._log(f"Could not resolve path for skill {skill_name}: {origin}", "warning")
            return None

        if not resolved_path.exists():
            self._log(f"Skill file not found: {resolved_path}", "warning")
            if source_type == "submodule":
                self._log("Run: git submodule update --init --recursive", "info")
            return None

        try:
            content = resolved_path.read_text(encoding="utf-8")
            # Extract key content (skip YAML frontmatter, limit lines)
            content = self._extract_key_content(content)

            # Create LoadedSkill with full tracking info
            loaded_skill = LoadedSkill(
                name=skill_name,
                content=content,
                priority=skill_config.get("priority", 0),
                source=source_name,
                source_type=source_type,
                origin=origin,
            )

            # Cache the full LoadedSkill
            self._cache[skill_name] = loaded_skill

            # Log successful load with source type info
            content_lines = len(content.split("\n"))
            self._log(f"Loaded: {skill_name} ({content_lines} lines, "
                     f"type: {source_type}, source: {source_name})")
            if self._verbose:
                self._log(f"  Origin: {origin}", "debug")

            return loaded_skill

        except OSError as e:
            self._log(f"Error reading {resolved_path}: {e}", "warning")
            return None

    def _extract_key_content(self, content: str) -> str:
        """Extract key content from SKILL.md, removing YAML frontmatter."""
        lines = content.split("\n")
        max_lines = self.config.get("settings", {}).get("max_content_lines", 200)

        # Skip YAML frontmatter
        start = 0
        if lines and lines[0].strip() == "---":
            for i, line in enumerate(lines[1:], 1):
                if line.strip() == "---":
                    start = i + 1
                    break

        # Extract content with limit
        extracted = lines[start:start + max_lines]
        if len(lines) > start + max_lines:
            extracted.append(f"\n... ({len(lines) - start - max_lines} more lines)")

        return "\n".join(extracted)

    def get_skills_for_phase(self, phase: str = "implementation") -> list[LoadedSkill]:
        """Get all applicable skills for a specific phase."""
        applicable = self.detect_applicable_skills()
        loaded = []

        for skill_name in applicable:
            skill_config = self.config["skills"].get(skill_name, {})
            inject_into = skill_config.get("inject_into", ["implementation"])

            if phase in inject_into:
                skill = self.load_skill_content(skill_name)
                if skill:
                    loaded.append(skill)

        return loaded

    def format_skills_for_prompt(self, skills: list[LoadedSkill]) -> str:
        """Format loaded skills for injection into agent prompt.

        Args:
            skills: List of LoadedSkill instances to format

        Returns:
            Formatted string for prompt injection
        """
        if not skills:
            return ""

        sections = [
            "## Framework-Specific Best Practices",
            "",
            "The following guidelines apply to this project based on detected frameworks:",
            ""
        ]

        for skill in skills:
            sections.append(f"### {skill.name.replace('-', ' ').title()}")
            sections.append(f"*Source: {skill.source} ({skill.source_type}) | Priority: {skill.priority}*")
            sections.append("")
            sections.append(skill.content)
            sections.append("")

        return "\n".join(sections)

    def get_skill_context(self, phase: str = "implementation") -> str:
        """Get formatted skill context for the current project and phase."""
        skills = self.get_skills_for_phase(phase)
        return self.format_skills_for_prompt(skills)

    def get_skills_summary(self, phase: str = "implementation") -> str:
        """Get a brief summary of loaded skills for display.

        Args:
            phase: The execution phase to get skills for

        Returns:
            Formatted summary string for display
        """
        skills = self.get_skills_for_phase(phase)

        if not skills:
            return ""

        lines = [
            "┌" + "─" * 68 + "┐",
            "│  EXTERNAL FRAMEWORK SKILLS LOADED" + " " * 33 + "│",
            "├" + "─" * 68 + "┤",
        ]

        for skill in skills:
            name_display = skill.name.replace("-", " ").title()
            type_badge = f"[{skill.source_type}]"
            source_display = f"{type_badge} {skill.source} (priority: {skill.priority})"
            line = f"│  + {name_display}"
            lines.append(line + " " * max(0, 69 - len(line)) + "│")
            detail_line = f"│      {source_display}"
            lines.append(detail_line + " " * max(0, 69 - len(detail_line)) + "│")

        lines.append("├" + "─" * 68 + "┤")

        # Count by source type
        type_counts = {}
        for skill in skills:
            type_counts[skill.source_type] = type_counts.get(skill.source_type, 0) + 1
        type_summary = ", ".join(f"{k}: {v}" for k, v in sorted(type_counts.items()))

        summary_line = f"│  Phase: {phase} | Total: {len(skills)} | {type_summary}"
        lines.append(summary_line + " " * max(0, 69 - len(summary_line)) + "│")
        lines.append("└" + "─" * 68 + "┘")

        return "\n".join(lines)

    def display_skills_summary(self, phase: str = "implementation") -> None:
        """Print the skills summary to stdout.

        Args:
            phase: The execution phase to get skills for
        """
        summary = self.get_skills_summary(phase)
        if summary:
            print()
            print(summary)
            print()

    def list_all_skills(self) -> list[dict]:
        """List all configured skills with their source information.

        Useful for debugging and understanding skill configuration.

        Returns:
            List of dicts with skill info (name, source, source_type, priority, skill_path)
        """
        skills_info = []

        for skill_name, skill_config in self.config.get("skills", {}).items():
            source_name = skill_config.get("source", "unknown")
            source_type = self._get_source_type(source_name)
            priority = skill_config.get("priority", 0)

            skills_info.append({
                "name": skill_name,
                "source": source_name,
                "source_type": source_type,
                "priority": priority,
                "skill_path": skill_config.get("skill_path", ""),
                "inject_into": skill_config.get("inject_into", ["implementation"]),
            })

        # Sort by priority (highest first)
        skills_info.sort(key=lambda x: x["priority"], reverse=True)
        return skills_info

    def get_skills_by_type(self) -> dict[str, list[str]]:
        """Get skills grouped by source type.

        Returns:
            Dict mapping source_type to list of skill names
        """
        by_type: dict[str, list[str]] = {
            "builtin": [],
            "submodule": [],
            "user": [],
        }

        for skill_name, skill_config in self.config.get("skills", {}).items():
            source_name = skill_config.get("source", "unknown")
            source_type = self._get_source_type(source_name)

            if source_type in by_type:
                by_type[source_type].append(skill_name)

        return by_type

    def validate_priorities(self) -> list[str]:
        """Validate that all skills have priorities within their source type's range.

        Returns:
            List of warning messages for any out-of-range priorities
        """
        warnings = []
        priority_ranges = self.config.get("priority_ranges", {})

        for skill_name, skill_config in self.config.get("skills", {}).items():
            source_name = skill_config.get("source", "unknown")
            source_type = self._get_source_type(source_name)
            priority = skill_config.get("priority", 0)

            # Get range for this source type
            range_config = priority_ranges.get(source_type, {})
            min_priority = range_config.get("min", 0)
            max_priority = range_config.get("max", 200)

            if not (min_priority <= priority <= max_priority):
                warnings.append(
                    f"Skill '{skill_name}' has priority {priority} but source type "
                    f"'{source_type}' expects range {min_priority}-{max_priority}"
                )

        return warnings
