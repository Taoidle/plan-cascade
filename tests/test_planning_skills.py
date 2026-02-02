"""Tests for planning phase skill injection in ExternalSkillLoader."""

import json
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.core.external_skill_loader import ExternalSkillLoader, LoadedSkill


class TestPlanningPhaseSkills:
    """Tests for planning phase skill detection and injection."""

    @pytest.fixture
    def skills_config(self):
        """Sample skills configuration with planning phase."""
        return {
            "version": "1.1.0",
            "priority_ranges": {
                "builtin": {"min": 1, "max": 50},
                "submodule": {"min": 51, "max": 100},
                "user": {"min": 101, "max": 200},
            },
            "sources": {
                "builtin": {
                    "type": "builtin",
                    "path": "builtin-skills",
                },
            },
            "skills": {
                "python-best-practices": {
                    "source": "builtin",
                    "skill_path": "python",
                    "detect": {
                        "files": ["pyproject.toml"],
                        # Patterns must match content for skill detection to work
                        "patterns": ["[project]", "name"],
                    },
                    "inject_into": ["planning", "implementation", "retry"],
                    "priority": 30,
                },
                "typescript-best-practices": {
                    "source": "builtin",
                    "skill_path": "typescript",
                    "detect": {
                        "files": ["tsconfig.json", "package.json"],
                        "patterns": ["\"typescript\""],
                    },
                    "inject_into": ["planning", "implementation", "retry"],
                    "priority": 36,
                },
                "web-design-guidelines": {
                    "source": "builtin",
                    "skill_path": "web",
                    "detect": {
                        "files": ["package.json"],
                        "patterns": ["\"react\""],
                    },
                    # Note: NO planning phase - implementation only
                    "inject_into": ["implementation"],
                    "priority": 55,
                },
            },
            "settings": {
                "max_skills_per_story": 3,
                "max_content_lines": 200,
            },
        }

    @pytest.fixture
    def mock_skill_content(self):
        """Sample skill content with planning-relevant sections."""
        return """# Python Best Practices

## Project Structure and Organization

A well-organized Python project should follow these conventions:

- Use `src/` layout for package code
- Keep tests in `tests/` directory
- Use `pyproject.toml` for project configuration

### Directory Layout

```
project/
├── src/
│   └── package/
│       ├── __init__.py
│       └── module.py
├── tests/
│   └── test_module.py
└── pyproject.toml
```

## Code Patterns

### Repository Pattern

Use repository pattern for data access to separate concerns.

### Service Layer

Implement business logic in service classes.

## Implementation Details

This section has implementation-specific details that are not
relevant for planning phase.

### Error Handling

Use custom exceptions for domain errors.
"""

    def test_get_skills_for_planning_phase(
        self, tmp_path: Path, skills_config: dict, mock_skill_content: str
    ):
        """Test that only skills with planning phase are returned."""
        # Create project with Python marker - the detect pattern requires this
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text("[project]\nname = 'test'")

        # Create mock LoadedSkill to return
        mock_skill = LoadedSkill(
            name="python-best-practices",
            content=mock_skill_content,
            source="builtin",
            source_type="builtin",
            priority=30,
        )

        # Mock the plugin root and user skills
        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                # Set config after init to ensure our test config is used
                loader.config = skills_config
                # Clear cache to force reload
                loader._cache = {}

                # Manually test skill matching since project_root is tmp_path
                matched = loader._skill_matches_project(skills_config["skills"]["python-best-practices"])
                assert matched, "python-best-practices should match pyproject.toml"

                # Mock load_skill_content to return our test skill
                with patch.object(loader, "load_skill_content", return_value=mock_skill):
                    skills = loader.get_skills_for_phase("planning")

                    # Should get python-best-practices (matches pyproject.toml and has planning)
                    assert len(skills) >= 1, f"Expected at least 1 skill, got {len(skills)}"
                    skill_names = [s.name for s in skills]
                    assert "python-best-practices" in skill_names
                    # web-design-guidelines should NOT be included (no planning phase)
                    assert "web-design-guidelines" not in skill_names

    def test_get_planning_skills_summary_returns_content(
        self, tmp_path: Path, skills_config: dict, mock_skill_content: str
    ):
        """Test that planning skills summary contains planning-relevant content."""
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'test'")

        # Create mock LoadedSkill to return
        mock_skill = LoadedSkill(
            name="python-best-practices",
            content=mock_skill_content,
            source="builtin",
            source_type="builtin",
            priority=30,
        )

        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                loader.config = skills_config
                loader._cache = {}

                # Mock load_skill_content to return our test skill
                with patch.object(loader, "load_skill_content", return_value=mock_skill):
                    summary = loader.get_planning_skills_summary()

                    # Should contain planning guidance header
                    assert "Planning Guidance" in summary or "planning" in summary.lower(), \
                        f"Expected planning content, got: '{summary[:200]}...'"

    def test_get_planning_skills_summary_empty_when_no_skills(
        self, tmp_path: Path, skills_config: dict
    ):
        """Test that empty string returned when no planning skills match."""
        # No project markers - no skills should match
        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                loader.config = skills_config

                summary = loader.get_planning_skills_summary()

                assert summary == ""

    def test_implementation_only_skills_excluded_from_planning(
        self, tmp_path: Path, skills_config: dict
    ):
        """Test that implementation-only skills are excluded from planning phase."""
        # Create markers for both Python and React
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'test'")
        (tmp_path / "package.json").write_text('{"dependencies": {"react": "18.0.0"}}')

        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                loader.config = skills_config

                # Mock skill files
                for skill_name in ["python", "web"]:
                    skill_path = tmp_path / "builtin-skills" / skill_name / "SKILL.md"
                    skill_path.parent.mkdir(parents=True, exist_ok=True)
                    skill_path.write_text(f"# {skill_name} Guidelines\n\nContent here.")

                skills = loader.get_skills_for_phase("planning")
                skill_names = [s.name for s in skills]

                # web-design-guidelines has inject_into: ["implementation"] only
                assert "web-design-guidelines" not in skill_names

    def test_extract_planning_content_finds_structure_sections(
        self, tmp_path: Path, mock_skill_content: str
    ):
        """Test that planning-relevant sections are extracted."""
        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                loader.config = {"skills": {}, "sources": {}, "settings": {}}

                result = loader._extract_planning_content(mock_skill_content)

                # Should include structure/organization sections
                assert "Project Structure" in result or "structure" in result.lower()
                # Should include pattern sections
                assert "Pattern" in result or "pattern" in result.lower()

    def test_extract_planning_content_fallback(self, tmp_path: Path):
        """Test fallback when no planning sections found."""
        content_without_planning = """# Implementation Guide

## Error Handling

Use try/except blocks.

## Logging

Use logging module.
"""
        with patch.object(ExternalSkillLoader, "_find_plugin_root", return_value=tmp_path):
            with patch.object(ExternalSkillLoader, "_integrate_user_skills"):
                loader = ExternalSkillLoader(tmp_path, plugin_root=tmp_path)
                loader.config = {"skills": {}, "sources": {}, "settings": {}}

                result = loader._extract_planning_content(content_without_planning)

                # Should return first 50 lines as fallback
                assert len(result) > 0
                assert "Implementation Guide" in result


class TestExternalSkillsConfigPlanning:
    """Tests for external-skills.json planning configuration."""

    def test_main_skills_have_planning_phase(self, tmp_path: Path):
        """Test that main framework skills include planning phase."""
        # Load actual external-skills.json
        project_root = Path(__file__).parent.parent
        config_path = project_root / "external-skills.json"

        if not config_path.exists():
            pytest.skip("external-skills.json not found")

        config = json.loads(config_path.read_text())
        skills = config.get("skills", {})

        # These skills should have "planning" in inject_into
        planning_skills = [
            "python-best-practices",
            "typescript-best-practices",
            "react-best-practices",
            "vue-best-practices",
            "rust-coding-guidelines",
        ]

        for skill_name in planning_skills:
            if skill_name in skills:
                inject_into = skills[skill_name].get("inject_into", [])
                assert "planning" in inject_into, (
                    f"Skill '{skill_name}' should have 'planning' in inject_into, "
                    f"got: {inject_into}"
                )

    def test_implementation_only_skills_no_planning(self, tmp_path: Path):
        """Test that certain skills don't have planning phase."""
        project_root = Path(__file__).parent.parent
        config_path = project_root / "external-skills.json"

        if not config_path.exists():
            pytest.skip("external-skills.json not found")

        config = json.loads(config_path.read_text())
        skills = config.get("skills", {})

        # These skills should NOT have "planning" in inject_into
        # (they are implementation-specific details)
        implementation_only_skills = [
            "web-design-guidelines",
            "vue-router-best-practices",
            "vue-pinia-best-practices",
            "rust-concurrency",
        ]

        for skill_name in implementation_only_skills:
            if skill_name in skills:
                inject_into = skills[skill_name].get("inject_into", [])
                assert "planning" not in inject_into, (
                    f"Skill '{skill_name}' should NOT have 'planning' in inject_into, "
                    f"got: {inject_into}"
                )
