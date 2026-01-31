#!/usr/bin/env python3
"""
External Skill Loader for Plan Cascade

Loads framework-specific skills (React, Vue, etc.) from Git submodules
and injects them into story execution context.
"""

import json
import re
from pathlib import Path
from dataclasses import dataclass
from typing import Optional


@dataclass
class LoadedSkill:
    """Represents a loaded external skill."""
    name: str
    content: str
    priority: int
    source: str


class ExternalSkillLoader:
    """Loads and manages external framework skills."""

    def __init__(self, project_root: Path, plugin_root: Path = None):
        self.project_root = Path(project_root)
        self.plugin_root = plugin_root or self._find_plugin_root()
        self.config = self._load_config()
        self._cache: dict[str, str] = {}

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
            return {"skills": {}, "sources": {}, "settings": {}}

        try:
            return json.loads(config_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            print(f"[ExternalSkillLoader] Warning: Could not load config: {e}")
            return {"skills": {}, "sources": {}, "settings": {}}

    def detect_applicable_skills(self) -> list[str]:
        """Detect which skills apply to the current project."""
        applicable = []

        for skill_name, skill_config in self.config.get("skills", {}).items():
            if self._skill_matches_project(skill_config):
                applicable.append(skill_name)

        # Sort by priority (highest first)
        applicable.sort(
            key=lambda s: self.config["skills"][s].get("priority", 0),
            reverse=True
        )

        # Limit to max skills
        max_skills = self.config.get("settings", {}).get("max_skills_per_story", 3)
        return applicable[:max_skills]

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

    def load_skill_content(self, skill_name: str) -> Optional[LoadedSkill]:
        """Load a skill's SKILL.md content."""
        # Check cache
        if skill_name in self._cache:
            skill_config = self.config["skills"].get(skill_name, {})
            return LoadedSkill(
                name=skill_name,
                content=self._cache[skill_name],
                priority=skill_config.get("priority", 0),
                source=skill_config.get("source", "unknown")
            )

        skill_config = self.config.get("skills", {}).get(skill_name)
        if not skill_config:
            return None

        source_name = skill_config.get("source")
        source_config = self.config.get("sources", {}).get(source_name)
        if not source_config:
            return None

        # Build path to SKILL.md
        skill_path = (
            self.plugin_root
            / source_config["path"]
            / skill_config["skill_path"]
            / "SKILL.md"
        )

        if not skill_path.exists():
            print(f"[ExternalSkillLoader] Skill not found: {skill_path}")
            print(f"[ExternalSkillLoader] Run: git submodule update --init --recursive")
            return None

        try:
            content = skill_path.read_text(encoding="utf-8")
            # Extract key content (skip YAML frontmatter, limit lines)
            content = self._extract_key_content(content)
            self._cache[skill_name] = content

            return LoadedSkill(
                name=skill_name,
                content=content,
                priority=skill_config.get("priority", 0),
                source=source_name
            )
        except OSError as e:
            print(f"[ExternalSkillLoader] Error reading {skill_path}: {e}")
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
        """Format loaded skills for injection into agent prompt."""
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
            sections.append(f"*Source: {skill.source}*")
            sections.append("")
            sections.append(skill.content)
            sections.append("")

        return "\n".join(sections)

    def get_skill_context(self, phase: str = "implementation") -> str:
        """Get formatted skill context for the current project and phase."""
        skills = self.get_skills_for_phase(phase)
        return self.format_skills_for_prompt(skills)
