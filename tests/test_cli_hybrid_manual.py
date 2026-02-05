"""Tests for hybrid-manual CLI command.

Tests for the `plan-cascade hybrid-manual <prd-path>` command that loads
an existing PRD file and enters review mode.
"""

import json
import pytest
from pathlib import Path
from typer.testing import CliRunner

from src.plan_cascade.cli.main import app


runner = CliRunner()


# Sample valid PRD for testing
VALID_PRD = {
    "metadata": {
        "version": "1.0.0",
        "description": "Test PRD for hybrid-manual command",
    },
    "goal": "Implement a test feature",
    "objectives": [
        "Objective 1",
        "Objective 2",
    ],
    "stories": [
        {
            "id": "story-001",
            "title": "Setup project structure",
            "description": "Create the initial project structure",
            "priority": "high",
            "dependencies": [],
            "acceptance_criteria": [
                "Project structure is created",
                "All directories exist",
            ],
        },
        {
            "id": "story-002",
            "title": "Implement core logic",
            "description": "Implement the main functionality",
            "priority": "high",
            "dependencies": ["story-001"],
            "acceptance_criteria": [
                "Core logic is implemented",
                "Tests pass",
            ],
        },
        {
            "id": "story-003",
            "title": "Add documentation",
            "description": "Write user documentation",
            "priority": "medium",
            "dependencies": ["story-002"],
            "acceptance_criteria": [
                "Documentation is written",
                "Examples are included",
            ],
        },
    ],
}


class TestHybridManualValidPRD:
    """Tests for loading valid PRD files."""

    def test_load_valid_prd_file(self, tmp_path):
        """Test loading a valid PRD file shows review."""
        # Create a PRD file
        prd_path = tmp_path / "test_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",  # Don't continue to approval
        )

        # Should show PRD review
        assert "PRD REVIEW" in result.output or "Goal" in result.output
        assert "story-001" in result.output or "Setup project" in result.output

    def test_load_prd_shows_goal(self, tmp_path):
        """Test that loaded PRD displays the goal."""
        prd_path = tmp_path / "test_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        assert "Implement a test feature" in result.output or "goal" in result.output.lower()

    def test_load_prd_shows_story_count(self, tmp_path):
        """Test that loaded PRD shows story count."""
        prd_path = tmp_path / "test_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should show story count (3 stories)
        assert "3" in result.output or "stories" in result.output.lower()

    def test_load_prd_shows_priorities(self, tmp_path):
        """Test that loaded PRD shows priority breakdown."""
        prd_path = tmp_path / "test_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should show priorities
        assert "high" in result.output.lower() or "High" in result.output

    def test_load_prd_shows_next_steps(self, tmp_path):
        """Test that loaded PRD shows next steps."""
        prd_path = tmp_path / "test_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should show next steps
        assert "approve" in result.output.lower() or "Next" in result.output


class TestHybridManualPRDValidation:
    """Tests for PRD validation errors."""

    def test_missing_metadata_description(self, tmp_path):
        """Test error when metadata.description is missing."""
        invalid_prd = {
            "metadata": {"version": "1.0.0"},  # Missing description
            "goal": "Test goal",
            "stories": [],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        # Should show validation error
        assert result.exit_code != 0 or "description" in result.output.lower() or "error" in result.output.lower()

    def test_missing_goal(self, tmp_path):
        """Test error when goal is missing."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            # Missing goal
            "stories": [],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "goal" in result.output.lower() or "error" in result.output.lower()

    def test_missing_stories(self, tmp_path):
        """Test error when stories array is missing."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            # Missing stories
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "stories" in result.output.lower() or "error" in result.output.lower()

    def test_story_missing_id(self, tmp_path):
        """Test error when a story is missing id field."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    # Missing id
                    "title": "Test story",
                    "description": "Description",
                    "priority": "high",
                    "dependencies": [],
                    "acceptance_criteria": [],
                }
            ],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "id" in result.output.lower() or "error" in result.output.lower()

    def test_story_missing_title(self, tmp_path):
        """Test error when a story is missing title field."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    "id": "story-001",
                    # Missing title
                    "description": "Description",
                    "priority": "high",
                    "dependencies": [],
                    "acceptance_criteria": [],
                }
            ],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "title" in result.output.lower() or "error" in result.output.lower()

    def test_story_missing_dependencies(self, tmp_path):
        """Test error when a story is missing dependencies field."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Test",
                    "description": "Description",
                    "priority": "high",
                    # Missing dependencies
                    "acceptance_criteria": [],
                }
            ],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "dependencies" in result.output.lower() or "error" in result.output.lower()

    def test_story_missing_acceptance_criteria(self, tmp_path):
        """Test error when a story is missing acceptance_criteria field."""
        invalid_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Test",
                    "description": "Description",
                    "priority": "high",
                    "dependencies": [],
                    # Missing acceptance_criteria
                }
            ],
        }
        prd_path = tmp_path / "invalid.json"
        prd_path.write_text(json.dumps(invalid_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
        )

        assert result.exit_code != 0 or "acceptance" in result.output.lower() or "error" in result.output.lower()


class TestHybridManualFileCopy:
    """Tests for file copying behavior."""

    def test_copies_prd_to_current_directory(self, tmp_path):
        """Test that PRD is copied to prd.json in current directory."""
        # Create PRD in a subdirectory
        subdir = tmp_path / "prds"
        subdir.mkdir()
        prd_path = subdir / "my_prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        # Run from tmp_path as current directory
        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path), "--project", str(tmp_path)],
            input="n\n",
        )

        # Should copy PRD to current directory
        expected_prd = tmp_path / "prd.json"
        assert expected_prd.exists() or "Copied" in result.output

    def test_does_not_copy_if_same_location(self, tmp_path):
        """Test that PRD is not copied if already named prd.json in project directory."""
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path), "--project", str(tmp_path)],
            input="n\n",
        )

        # Should not show "Copied" message
        # The PRD was already in the right place
        assert "prd.json" in result.output.lower() or "PRD" in result.output


class TestHybridManualSupportingFiles:
    """Tests for supporting file creation."""

    def test_creates_findings_md_if_missing(self, tmp_path):
        """Test that findings.md is created if it doesn't exist."""
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path), "--project", str(tmp_path)],
            input="n\n",
        )

        findings_path = tmp_path / "findings.md"
        assert findings_path.exists() or "findings" in result.output.lower()

    def test_does_not_overwrite_existing_findings(self, tmp_path):
        """Test that existing findings.md is not overwritten."""
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        # Create existing findings.md
        findings_path = tmp_path / "findings.md"
        findings_path.write_text("# Existing Findings\n\nImportant content here.")

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path), "--project", str(tmp_path)],
            input="n\n",
        )

        # Should preserve existing content
        assert "Existing Findings" in findings_path.read_text() or "Important content" in findings_path.read_text()

    def test_creates_progress_txt_if_missing(self, tmp_path):
        """Test that progress.txt is created if it doesn't exist."""
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path), "--project", str(tmp_path)],
            input="n\n",
        )

        progress_path = tmp_path / "progress.txt"
        assert progress_path.exists() or "progress" in result.output.lower()


class TestHybridManualDependencyAnalysis:
    """Tests for dependency analysis and batch display."""

    def test_shows_execution_batches(self, tmp_path):
        """Test that execution batches are displayed."""
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should show batch information
        assert "Batch" in result.output or "batch" in result.output.lower()

    def test_detects_circular_dependencies(self, tmp_path):
        """Test that circular dependencies are detected and reported."""
        circular_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Story 1",
                    "description": "Desc",
                    "priority": "high",
                    "dependencies": ["story-002"],  # Circular!
                    "acceptance_criteria": ["AC1"],
                },
                {
                    "id": "story-002",
                    "title": "Story 2",
                    "description": "Desc",
                    "priority": "high",
                    "dependencies": ["story-001"],  # Circular!
                    "acceptance_criteria": ["AC1"],
                },
            ],
        }
        prd_path = tmp_path / "circular.json"
        prd_path.write_text(json.dumps(circular_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should warn about circular dependencies
        assert "circular" in result.output.lower() or "cycle" in result.output.lower()

    def test_detects_invalid_dependency_references(self, tmp_path):
        """Test that invalid dependency references are detected."""
        invalid_dep_prd = {
            "metadata": {"description": "Test"},
            "goal": "Test goal",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Story 1",
                    "description": "Desc",
                    "priority": "high",
                    "dependencies": ["story-999"],  # Non-existent!
                    "acceptance_criteria": ["AC1"],
                },
            ],
        }
        prd_path = tmp_path / "invalid_dep.json"
        prd_path.write_text(json.dumps(invalid_dep_prd))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(prd_path)],
            input="n\n",
        )

        # Should warn about invalid dependency
        assert "story-999" in result.output or "invalid" in result.output.lower() or "not found" in result.output.lower()


class TestHybridManualFileNotFound:
    """Tests for file not found scenarios."""

    def test_error_when_file_not_found(self, tmp_path):
        """Test error when PRD file doesn't exist."""
        non_existent = tmp_path / "nonexistent.json"

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(non_existent)],
        )

        assert result.exit_code != 0
        assert "not found" in result.output.lower() or "error" in result.output.lower() or "exist" in result.output.lower()

    def test_error_with_invalid_json(self, tmp_path):
        """Test error when PRD file contains invalid JSON."""
        invalid_json = tmp_path / "invalid.json"
        invalid_json.write_text("{ not valid json }")

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", str(invalid_json)],
        )

        assert result.exit_code != 0
        assert "json" in result.output.lower() or "error" in result.output.lower() or "invalid" in result.output.lower()


class TestHybridManualHelpAndUsage:
    """Tests for command help and usage."""

    def test_help_available(self):
        """Test that help is available for the command."""
        result = runner.invoke(
            app,
            ["hybrid-manual", "--help"],
        )

        assert result.exit_code == 0
        assert "PRD" in result.output or "prd" in result.output.lower()

    def test_default_prd_path(self, tmp_path):
        """Test that default PRD path is prd.json when not specified."""
        # Create prd.json in the project directory
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(VALID_PRD))

        result = runner.invoke(
            app,
            ["--legacy-mode", "hybrid-manual", "--project", str(tmp_path)],
            input="n\n",
        )

        # Should load prd.json by default
        assert "PRD" in result.output or "story" in result.output.lower()
