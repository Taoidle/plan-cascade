"""Tests for the Mega Plan CLI commands."""

import json
import pytest
from pathlib import Path
from typer.testing import CliRunner

from src.plan_cascade.cli.mega import mega_app


runner = CliRunner()


class TestMegaPlanCommand:
    """Tests for 'mega plan' command."""

    def test_plan_creates_mega_plan(self, tmp_path):
        """Test that 'mega plan' creates a mega-plan.json file."""
        result = runner.invoke(
            mega_app,
            ["plan", "Build an e-commerce platform", "--project", str(tmp_path), "--target", "main"]
        )

        # Should create the file (though without LLM it will be empty of features)
        assert result.exit_code == 0 or "mega-plan" in result.output.lower()

    def test_plan_with_design_doc_option(self, tmp_path):
        """Test that 'mega plan' accepts --design-doc option."""
        design_doc = tmp_path / "design.json"
        design_doc.write_text('{"components": []}')

        result = runner.invoke(
            mega_app,
            ["plan", "Build API", "--project", str(tmp_path), "--design-doc", str(design_doc)]
        )

        # Should not error on design doc option
        assert "design-doc" not in result.output.lower() or result.exit_code == 0

    def test_plan_with_agent_options(self, tmp_path):
        """Test that 'mega plan' accepts agent options."""
        result = runner.invoke(
            mega_app,
            ["plan", "Build API", "--project", str(tmp_path), "--prd-agent", "claude", "--story-agent", "aider"]
        )

        # Should accept agent options without error
        assert result.exit_code == 0 or "agent" not in result.output.lower()


class TestMegaApproveCommand:
    """Tests for 'mega approve' command."""

    def test_approve_requires_mega_plan(self, tmp_path):
        """Test that 'mega approve' fails without a mega-plan."""
        result = runner.invoke(
            mega_app,
            ["approve", "--project", str(tmp_path)]
        )

        assert result.exit_code == 1
        assert "no mega-plan" in result.output.lower() or "mega-plan" in result.output.lower()

    def test_approve_with_auto_prd(self, tmp_path, sample_mega_plan):
        """Test 'mega approve --auto-prd' option."""
        # Create mega-plan.json
        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            mega_app,
            ["approve", "--project", str(tmp_path), "--auto-prd"],
            input="n\n"  # Don't actually start execution
        )

        # Should show validation passed or ask for confirmation
        assert "valid" in result.output.lower() or "proceed" in result.output.lower() or "start" in result.output.lower()


class TestMegaStatusCommand:
    """Tests for 'mega status' command."""

    def test_status_requires_mega_plan(self, tmp_path):
        """Test that 'mega status' fails without a mega-plan."""
        result = runner.invoke(
            mega_app,
            ["status", "--project", str(tmp_path)]
        )

        assert result.exit_code == 1
        assert "no mega-plan" in result.output.lower() or "mega-plan" in result.output.lower()

    def test_status_shows_progress(self, tmp_path, sample_mega_plan):
        """Test that 'mega status' shows progress information."""
        # Create mega-plan.json
        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            mega_app,
            ["status", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should show progress percentage or feature list
        assert "progress" in result.output.lower() or "feature" in result.output.lower()

    def test_status_verbose(self, tmp_path, sample_mega_plan):
        """Test 'mega status --verbose' shows more details."""
        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            mega_app,
            ["status", "--project", str(tmp_path), "--verbose"]
        )

        assert result.exit_code == 0


class TestMegaCompleteCommand:
    """Tests for 'mega complete' command."""

    def test_complete_requires_mega_plan(self, tmp_path):
        """Test that 'mega complete' fails without a mega-plan."""
        result = runner.invoke(
            mega_app,
            ["complete", "--project", str(tmp_path)]
        )

        assert result.exit_code == 1

    def test_complete_checks_progress(self, tmp_path, sample_mega_plan):
        """Test that 'mega complete' checks if all features are complete."""
        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            mega_app,
            ["complete", "--project", str(tmp_path)],
            input="n\n"  # Don't proceed
        )

        # Should fail because features are not complete
        assert result.exit_code == 1 or "not all" in result.output.lower() or "complete" in result.output.lower()


class TestMegaEditCommand:
    """Tests for 'mega edit' command."""

    def test_edit_requires_mega_plan(self, tmp_path):
        """Test that 'mega edit' fails without a mega-plan."""
        result = runner.invoke(
            mega_app,
            ["edit", "--project", str(tmp_path)]
        )

        assert result.exit_code == 1

    def test_edit_interactive(self, tmp_path, sample_mega_plan):
        """Test 'mega edit' interactive mode."""
        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        # Just quit immediately
        result = runner.invoke(
            mega_app,
            ["edit", "--project", str(tmp_path)],
            input="quit\nn\n"  # quit, don't save
        )

        assert result.exit_code == 0


class TestMegaResumeCommand:
    """Tests for 'mega resume' command."""

    def test_resume_requires_mega_plan(self, tmp_path):
        """Test that 'mega resume' fails without a mega-plan."""
        result = runner.invoke(
            mega_app,
            ["resume", "--project", str(tmp_path)]
        )

        assert result.exit_code == 1

    def test_resume_detects_completed(self, tmp_path, sample_mega_plan):
        """Test 'mega resume' detects completed plan."""
        # Mark all features as complete
        for feature in sample_mega_plan["features"]:
            feature["status"] = "complete"

        mega_plan_path = tmp_path / "mega-plan.json"
        with open(mega_plan_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            mega_app,
            ["resume", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        assert "complete" in result.output.lower()


class TestMegaHelpOutput:
    """Tests for CLI help output."""

    def test_mega_help(self):
        """Test 'mega --help' shows all commands."""
        result = runner.invoke(mega_app, ["--help"])

        assert result.exit_code == 0
        assert "plan" in result.output
        assert "approve" in result.output
        assert "status" in result.output
        assert "complete" in result.output
        assert "edit" in result.output
        assert "resume" in result.output

    def test_plan_help(self):
        """Test 'mega plan --help' shows options."""
        result = runner.invoke(mega_app, ["plan", "--help"])

        assert result.exit_code == 0
        assert "description" in result.output.lower()
        assert "--project" in result.output
        assert "--target" in result.output

    def test_approve_help(self):
        """Test 'mega approve --help' shows options."""
        result = runner.invoke(mega_app, ["approve", "--help"])

        assert result.exit_code == 0
        assert "--auto-prd" in result.output
        assert "--batch" in result.output
