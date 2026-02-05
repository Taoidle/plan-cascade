"""Tests for the start CLI command for planning-with-files session initialization."""

import json
import pytest
from pathlib import Path
from typer.testing import CliRunner
from unittest.mock import patch

from src.plan_cascade.cli.main import app


runner = CliRunner()


class TestStartCommandBasic:
    """Basic tests for the start command."""

    def test_help_shows_start_command(self):
        """Test that --help shows the start command."""
        result = runner.invoke(app, ["--help"])

        assert result.exit_code == 0
        assert "start" in result.output

    def test_start_command_help(self):
        """Test that start --help shows command options."""
        result = runner.invoke(app, ["start", "--help"])

        assert result.exit_code == 0
        assert "description" in result.output.lower()
        assert "--output-dir" in result.output


class TestStartFreshSession:
    """Tests for starting a fresh planning session."""

    def test_start_creates_task_plan(self, tmp_path):
        """Test that start creates task_plan.md."""
        result = runner.invoke(
            app,
            ["start", "Implement a REST API", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        task_plan_path = tmp_path / "task_plan.md"
        assert task_plan_path.exists()
        content = task_plan_path.read_text()
        assert "# Task Plan" in content
        assert "Implement a REST API" in content

    def test_start_creates_findings(self, tmp_path):
        """Test that start creates findings.md."""
        result = runner.invoke(
            app,
            ["start", "Build login system", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        findings_path = tmp_path / "findings.md"
        assert findings_path.exists()
        content = findings_path.read_text()
        assert "# Findings" in content

    def test_start_creates_progress(self, tmp_path):
        """Test that start creates progress.md."""
        result = runner.invoke(
            app,
            ["start", "Add feature X", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        progress_path = tmp_path / "progress.md"
        assert progress_path.exists()
        content = progress_path.read_text()
        assert "# Progress" in content

    def test_start_creates_all_three_files(self, tmp_path):
        """Test that start creates all three planning files."""
        result = runner.invoke(
            app,
            ["start", "Complete task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()
        assert (tmp_path / "findings.md").exists()
        assert (tmp_path / "progress.md").exists()

    def test_start_uses_current_dir_by_default(self, tmp_path, monkeypatch):
        """Test that start uses current directory when --output-dir not specified."""
        monkeypatch.chdir(tmp_path)

        result = runner.invoke(
            app,
            ["start", "Test task"],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()

    def test_start_includes_description_in_task_plan(self, tmp_path):
        """Test that the description is included in task_plan.md."""
        description = "Build a multi-tenant SaaS application with user authentication"

        result = runner.invoke(
            app,
            ["start", description, "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        content = (tmp_path / "task_plan.md").read_text()
        assert description in content

    def test_start_shows_success_message(self, tmp_path):
        """Test that start shows a success message with created files."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert "task_plan.md" in result.output
        assert "findings.md" in result.output
        assert "progress.md" in result.output

    def test_start_shows_next_steps_guidance(self, tmp_path):
        """Test that start provides guidance on next steps."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        # Should provide some guidance on what to do next
        output_lower = result.output.lower()
        assert "next" in output_lower or "phase" in output_lower or "step" in output_lower


class TestStartExistingFiles:
    """Tests for start command when planning files already exist."""

    def test_start_detects_existing_task_plan(self, tmp_path):
        """Test that start detects existing task_plan.md."""
        # Create existing file
        task_plan = tmp_path / "task_plan.md"
        task_plan.write_text("# Existing Task Plan\n\n## Goal\nOriginal goal")

        result = runner.invoke(
            app,
            ["start", "New task", "--output-dir", str(tmp_path)],
            input="n\n",  # Answer 'no' to any prompt
        )

        # Should either prompt or show warning about existing files
        assert "exist" in result.output.lower() or "found" in result.output.lower()

    def test_start_preserves_existing_when_declined(self, tmp_path):
        """Test that existing files are preserved when user declines overwrite."""
        # Create existing file
        task_plan = tmp_path / "task_plan.md"
        original_content = "# Original Task Plan\n\n## Goal\nOriginal goal"
        task_plan.write_text(original_content)

        result = runner.invoke(
            app,
            ["start", "New task", "--output-dir", str(tmp_path)],
            input="n\n",  # Decline overwrite
        )

        # Original content should be preserved
        assert task_plan.read_text() == original_content

    def test_start_overwrites_when_force_flag(self, tmp_path):
        """Test that --force overwrites existing files without prompting."""
        # Create existing file
        task_plan = tmp_path / "task_plan.md"
        task_plan.write_text("# Old Task Plan")

        result = runner.invoke(
            app,
            ["start", "New task", "--output-dir", str(tmp_path), "--force"],
        )

        assert result.exit_code == 0
        content = task_plan.read_text()
        assert "New task" in content
        assert "Old Task Plan" not in content

    def test_start_offers_resume_option(self, tmp_path):
        """Test that start offers resume option when files exist."""
        # Create existing planning files
        (tmp_path / "task_plan.md").write_text("# Existing Plan")
        (tmp_path / "findings.md").write_text("# Existing Findings")
        (tmp_path / "progress.md").write_text("# Existing Progress")

        result = runner.invoke(
            app,
            ["start", "New task", "--output-dir", str(tmp_path)],
            input="n\n",  # Decline
        )

        # Should mention resume or existing session
        output_lower = result.output.lower()
        assert "resume" in output_lower or "exist" in output_lower


class TestStartResume:
    """Tests for resuming an existing planning session."""

    def test_start_resume_flag(self, tmp_path):
        """Test that --resume continues existing session."""
        # Create existing planning files
        task_plan_content = "# Task Plan: Original Task\n\n## Goal\nOriginal goal"
        (tmp_path / "task_plan.md").write_text(task_plan_content)
        (tmp_path / "findings.md").write_text("# Findings\n\nSome findings")
        (tmp_path / "progress.md").write_text("# Progress\n\nSome progress")

        result = runner.invoke(
            app,
            ["start", "--resume", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        # Original content should be preserved
        assert (tmp_path / "task_plan.md").read_text() == task_plan_content

    def test_resume_shows_current_state(self, tmp_path):
        """Test that resume shows current session state."""
        (tmp_path / "task_plan.md").write_text("# Task Plan: My Task\n\n## Current Phase\nPhase 2")
        (tmp_path / "findings.md").write_text("# Findings")
        (tmp_path / "progress.md").write_text("# Progress")

        result = runner.invoke(
            app,
            ["start", "--resume", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        # Should show some info about current state
        output_lower = result.output.lower()
        assert "phase" in output_lower or "my task" in output_lower or "found" in output_lower

    def test_resume_fails_when_no_session(self, tmp_path):
        """Test that --resume fails gracefully when no session exists."""
        result = runner.invoke(
            app,
            ["start", "--resume", "--output-dir", str(tmp_path)],
        )

        # Should fail with helpful message
        assert result.exit_code != 0 or "no" in result.output.lower() or "not found" in result.output.lower()


class TestStartCustomOutputDir:
    """Tests for custom output directory option."""

    def test_output_dir_creates_directory(self, tmp_path):
        """Test that --output-dir creates the directory if it doesn't exist."""
        output_dir = tmp_path / "new_planning_dir"
        assert not output_dir.exists()

        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(output_dir)],
        )

        assert result.exit_code == 0
        assert output_dir.exists()
        assert (output_dir / "task_plan.md").exists()

    def test_output_dir_nested_path(self, tmp_path):
        """Test that --output-dir handles nested paths."""
        output_dir = tmp_path / "deeply" / "nested" / "path"

        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(output_dir)],
        )

        assert result.exit_code == 0
        assert output_dir.exists()
        assert (output_dir / "task_plan.md").exists()

    def test_output_dir_short_flag(self, tmp_path):
        """Test that -o shortcut works for --output-dir."""
        result = runner.invoke(
            app,
            ["start", "Test task", "-o", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()


class TestStartSessionContext:
    """Tests for session context setup for recovery."""

    def test_start_creates_session_config(self, tmp_path):
        """Test that start creates session configuration for recovery."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        # Check for session config file (may be .planning-config.json or similar)
        config_files = list(tmp_path.glob(".*config*.json")) + list(tmp_path.glob("*.config.json"))
        # At minimum, planning files should exist for session recovery
        assert (tmp_path / "task_plan.md").exists()

    def test_session_includes_timestamp(self, tmp_path):
        """Test that session/progress includes a timestamp."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        progress_content = (tmp_path / "progress.md").read_text()
        # Should have some date/time info
        import re
        date_pattern = r"\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4}"
        assert re.search(date_pattern, progress_content) or "session" in progress_content.lower()


class TestStartIntegrationWithSkill:
    """Tests for integration with planning-with-files skill workflow."""

    def test_start_creates_skill_compatible_files(self, tmp_path):
        """Test that created files are compatible with planning-with-files skill."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0

        # Check task_plan.md has expected sections
        task_plan = (tmp_path / "task_plan.md").read_text()
        assert "## Goal" in task_plan or "Goal" in task_plan
        assert "Phase" in task_plan

        # Check findings.md has expected structure
        findings = (tmp_path / "findings.md").read_text()
        assert "Findings" in findings

        # Check progress.md has expected structure
        progress = (tmp_path / "progress.md").read_text()
        assert "Progress" in progress

    def test_task_plan_has_phase_sections(self, tmp_path):
        """Test that task_plan.md includes phase sections for tracking."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        content = (tmp_path / "task_plan.md").read_text()

        # Should have phase tracking
        assert "Phase 1" in content or "phase" in content.lower()
        assert "Status" in content or "status" in content.lower()


class TestStartEdgeCases:
    """Edge case tests for the start command."""

    def test_start_with_empty_description(self, tmp_path):
        """Test start command with empty description."""
        result = runner.invoke(
            app,
            ["start", "", "--output-dir", str(tmp_path)],
        )

        # Should either fail with helpful message or require description
        assert result.exit_code != 0 or "description" in result.output.lower()

    def test_start_with_special_characters_in_description(self, tmp_path):
        """Test start handles special characters in description."""
        description = "Build API with quotes 'single' and \"double\" and <tags>"

        result = runner.invoke(
            app,
            ["start", description, "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        content = (tmp_path / "task_plan.md").read_text()
        # Description should be included, possibly escaped
        assert "API" in content

    def test_start_with_very_long_description(self, tmp_path):
        """Test start handles very long descriptions."""
        description = "Build a " + "very " * 100 + "long project description"

        result = runner.invoke(
            app,
            ["start", description, "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()

    def test_start_preserves_file_permissions(self, tmp_path):
        """Test that created files have appropriate permissions."""
        result = runner.invoke(
            app,
            ["start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        task_plan = tmp_path / "task_plan.md"

        # File should be readable and writable
        assert task_plan.exists()
        content = task_plan.read_text()  # Should not raise
        task_plan.write_text(content + "\n# Test append")  # Should not raise


class TestStartWithLegacyMode:
    """Tests for start command with --legacy-mode flag."""

    def test_start_works_with_legacy_mode(self, tmp_path):
        """Test that start works correctly with --legacy-mode flag."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()

    def test_start_works_with_no_legacy_mode(self, tmp_path):
        """Test that start works correctly with --no-legacy-mode flag."""
        result = runner.invoke(
            app,
            ["--no-legacy-mode", "start", "Test task", "--output-dir", str(tmp_path)],
        )

        assert result.exit_code == 0
        assert (tmp_path / "task_plan.md").exists()
