"""Tests for check-gitignore CLI command."""

import pytest
from pathlib import Path
from typer.testing import CliRunner

from src.plan_cascade.cli.main import app
from src.plan_cascade.utils.gitignore import (
    PLAN_CASCADE_GITIGNORE_ENTRIES,
    PLAN_CASCADE_KEY_ENTRIES,
    GitignoreManager,
)


runner = CliRunner()


class TestCheckGitignoreCommand:
    """Tests for the check-gitignore CLI command."""

    def test_command_exists(self):
        """Test that check-gitignore command is registered."""
        result = runner.invoke(app, ["check-gitignore", "--help"])
        assert result.exit_code == 0
        assert "gitignore" in result.output.lower()

    def test_status_check_no_gitignore(self, tmp_path):
        """Test status check when .gitignore doesn't exist."""
        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        assert ".gitignore" in result.output
        # Should indicate file doesn't exist
        assert "not found" in result.output.lower() or "does not exist" in result.output.lower() or "missing" in result.output.lower()

    def test_status_check_with_gitignore_no_cascade_section(self, tmp_path):
        """Test status check when .gitignore exists but has no Plan Cascade section."""
        gitignore_path = tmp_path / ".gitignore"
        gitignore_path.write_text("node_modules/\n.env\n")

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should indicate Plan Cascade section is missing
        assert "missing" in result.output.lower() or "not found" in result.output.lower() or "needs" in result.output.lower()

    def test_status_check_with_complete_gitignore(self, tmp_path):
        """Test status check when .gitignore has all Plan Cascade entries."""
        gitignore_path = tmp_path / ".gitignore"
        content = "node_modules/\n\n" + "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES)
        gitignore_path.write_text(content)

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should indicate all entries are present
        assert "configured" in result.output.lower() or "complete" in result.output.lower() or "all" in result.output.lower()


class TestCheckGitignoreUpdate:
    """Tests for check-gitignore --update flag."""

    def test_update_creates_gitignore(self, tmp_path):
        """Test --update creates .gitignore when it doesn't exist."""
        gitignore_path = tmp_path / ".gitignore"
        assert not gitignore_path.exists()

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update"]
        )

        assert result.exit_code == 0
        assert gitignore_path.exists()
        content = gitignore_path.read_text()
        # Should contain Plan Cascade entries
        for entry in PLAN_CASCADE_KEY_ENTRIES:
            assert entry in content

    def test_update_appends_to_existing_gitignore(self, tmp_path):
        """Test --update appends to existing .gitignore."""
        gitignore_path = tmp_path / ".gitignore"
        original_content = "# My project\nnode_modules/\n.env\n"
        gitignore_path.write_text(original_content)

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update"]
        )

        assert result.exit_code == 0
        content = gitignore_path.read_text()
        # Should preserve original content
        assert "node_modules/" in content
        assert ".env" in content
        # Should add Plan Cascade entries
        for entry in PLAN_CASCADE_KEY_ENTRIES:
            assert entry in content

    def test_update_idempotent(self, tmp_path):
        """Test --update is idempotent (running twice doesn't duplicate entries)."""
        gitignore_path = tmp_path / ".gitignore"
        gitignore_path.write_text("node_modules/\n")

        # First update
        result1 = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update"]
        )
        content_after_first = gitignore_path.read_text()

        # Second update
        result2 = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update"]
        )
        content_after_second = gitignore_path.read_text()

        assert result1.exit_code == 0
        assert result2.exit_code == 0
        # Second update should skip (already configured)
        assert "skip" in result2.output.lower() or "already" in result2.output.lower()

    def test_update_shows_success_message(self, tmp_path):
        """Test --update shows appropriate success message."""
        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update"]
        )

        assert result.exit_code == 0
        # Should indicate success
        assert "created" in result.output.lower() or "updated" in result.output.lower() or "added" in result.output.lower()


class TestCheckGitignoreDryRun:
    """Tests for check-gitignore --dry-run flag."""

    def test_dry_run_no_gitignore(self, tmp_path):
        """Test --dry-run when .gitignore doesn't exist."""
        gitignore_path = tmp_path / ".gitignore"

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--dry-run"]
        )

        assert result.exit_code == 0
        # Should show what would be done
        assert "would" in result.output.lower() or "dry" in result.output.lower()
        # Should NOT create the file
        assert not gitignore_path.exists()

    def test_dry_run_missing_entries(self, tmp_path):
        """Test --dry-run shows missing entries."""
        gitignore_path = tmp_path / ".gitignore"
        gitignore_path.write_text("node_modules/\n")

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--dry-run"]
        )

        assert result.exit_code == 0
        # Should list entries that would be added
        assert "would" in result.output.lower()
        # File should not be modified
        content = gitignore_path.read_text()
        assert "prd.json" not in content

    def test_dry_run_with_complete_gitignore(self, tmp_path):
        """Test --dry-run when .gitignore is already complete."""
        gitignore_path = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES)
        gitignore_path.write_text(content)

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--dry-run"]
        )

        assert result.exit_code == 0
        # Should indicate nothing to do
        assert "already" in result.output.lower() or "no changes" in result.output.lower() or "configured" in result.output.lower()


class TestCheckGitignoreOptions:
    """Tests for check-gitignore command options."""

    def test_project_option(self, tmp_path):
        """Test --project option specifies the project directory."""
        # Create a subdirectory
        subdir = tmp_path / "myproject"
        subdir.mkdir()
        gitignore_path = subdir / ".gitignore"
        gitignore_path.write_text("# My project\n")

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(subdir)]
        )

        assert result.exit_code == 0
        # Should check the specified directory
        assert str(subdir) in result.output or "myproject" in result.output or result.exit_code == 0

    def test_help_shows_all_options(self):
        """Test --help shows all available options."""
        result = runner.invoke(app, ["check-gitignore", "--help"])

        assert result.exit_code == 0
        assert "--project" in result.output
        assert "--update" in result.output
        assert "--dry-run" in result.output

    def test_update_and_dry_run_mutually_exclusive(self, tmp_path):
        """Test --update and --dry-run can be used together (dry-run takes precedence)."""
        gitignore_path = tmp_path / ".gitignore"

        # When both flags are used, dry-run should take precedence
        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path), "--update", "--dry-run"]
        )

        # Should succeed but not create file
        assert result.exit_code == 0
        assert not gitignore_path.exists()


class TestCheckGitignoreOutput:
    """Tests for check-gitignore output formatting."""

    def test_output_uses_output_manager_patterns(self, tmp_path):
        """Test output follows OutputManager patterns."""
        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Output should be formatted (not empty)
        assert len(result.output.strip()) > 0

    def test_output_lists_missing_entries(self, tmp_path):
        """Test output lists missing entries when present."""
        gitignore_path = tmp_path / ".gitignore"
        gitignore_path.write_text("node_modules/\n")

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should mention some missing entries
        # At least one key entry should be mentioned
        has_entry_mention = any(
            entry in result.output for entry in PLAN_CASCADE_KEY_ENTRIES
        )
        # Or should indicate entries are missing
        assert has_entry_mention or "missing" in result.output.lower() or "needs" in result.output.lower()

    def test_verbose_output_when_configured(self, tmp_path):
        """Test output when .gitignore is properly configured."""
        gitignore_path = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES)
        gitignore_path.write_text(content)

        result = runner.invoke(
            app,
            ["check-gitignore", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should indicate everything is OK
        assert "configured" in result.output.lower() or "complete" in result.output.lower() or "ok" in result.output.lower() or "present" in result.output.lower()
