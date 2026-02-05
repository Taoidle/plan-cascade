"""Tests for dashboard CLI command."""

import json
import pytest
from pathlib import Path
from typer.testing import CliRunner

from src.plan_cascade.cli.main import app
from src.plan_cascade.core.dashboard import (
    DashboardAggregator,
    DashboardFormatter,
    DashboardState,
    ExecutionStatus,
    StoryStatus,
)


runner = CliRunner()


class TestDashboardCommand:
    """Tests for the dashboard CLI command."""

    def test_help_shows_dashboard_command(self):
        """Test that dashboard command appears in help."""
        result = runner.invoke(app, ["--help"])

        assert result.exit_code == 0
        assert "dashboard" in result.output

    def test_dashboard_help(self):
        """Test dashboard command help text."""
        result = runner.invoke(app, ["dashboard", "--help"])

        assert result.exit_code == 0
        assert "--verbose" in result.output or "-v" in result.output
        assert "--json" in result.output

    def test_dashboard_no_execution_state(self, tmp_path):
        """Test dashboard when no PRD or execution state exists."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--project", str(tmp_path)]
        )

        # Should handle gracefully when no state exists
        assert result.exit_code == 0
        # Should indicate no active execution or show minimal info
        assert "N/A" in result.output or "not_started" in result.output.lower() or "0%" in result.output

    def test_dashboard_with_prd(self, tmp_path):
        """Test dashboard with a PRD file (HYBRID mode)."""
        # Create a PRD in the project
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "complete", "dependencies": []},
                {"id": "story-002", "title": "Second story", "status": "in_progress", "dependencies": ["story-001"]},
                {"id": "story-003", "title": "Third story", "status": "pending", "dependencies": ["story-002"]},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should show HYBRID strategy
        assert "HYBRID" in result.output
        # Should show progress
        assert "1/3" in result.output or "33%" in result.output or "stories" in result.output.lower()

    def test_dashboard_with_mega_plan(self, tmp_path):
        """Test dashboard with a mega-plan file (MEGA mode)."""
        # Create a mega-plan in the project
        mega_plan_data = {
            "goal": "Multi-feature project",
            "description": "A complex project",
            "features": [
                {
                    "id": "feature-001",
                    "name": "auth",
                    "title": "Authentication",
                    "status": "complete",
                    "dependencies": [],
                },
                {
                    "id": "feature-002",
                    "name": "api",
                    "title": "REST API",
                    "status": "in_progress",
                    "dependencies": ["feature-001"],
                },
            ],
            "target_branch": "main",
        }
        mega_path = tmp_path / "mega-plan.json"
        mega_path.write_text(json.dumps(mega_plan_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should detect MEGA strategy
        assert "MEGA" in result.output

    def test_dashboard_verbose_mode(self, tmp_path):
        """Test dashboard verbose mode shows detailed output."""
        # Create a PRD with stories
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "complete", "dependencies": []},
                {"id": "story-002", "title": "Second story", "status": "pending", "dependencies": []},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--verbose", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Verbose mode should show more details
        # Check for section headers or story IDs
        assert "DASHBOARD" in result.output.upper() or "story-001" in result.output.lower()

    def test_dashboard_json_output(self, tmp_path):
        """Test dashboard JSON output mode."""
        # Create a PRD
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "complete", "dependencies": []},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        # Should output valid JSON
        try:
            output_data = json.loads(result.output)
            assert "status" in output_data
            assert "strategy" in output_data
            assert "total_stories" in output_data
        except json.JSONDecodeError:
            pytest.fail("Dashboard --json output is not valid JSON")

    def test_dashboard_json_includes_recommendations(self, tmp_path):
        """Test that JSON output includes recommended actions."""
        # Create a PRD with incomplete stories
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "pending", "dependencies": []},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        output_data = json.loads(result.output)
        assert "recommended_actions" in output_data

    def test_dashboard_verbose_short_flag(self, tmp_path):
        """Test dashboard -v short flag for verbose mode."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "-v", "--project", str(tmp_path)]
        )

        # Should work the same as --verbose
        assert result.exit_code == 0


class TestDashboardWithProgressFile:
    """Tests for dashboard reading progress.txt."""

    def test_dashboard_reads_progress_txt(self, tmp_path):
        """Test that dashboard reads story status from progress.txt."""
        # Create a PRD
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "pending", "dependencies": []},
                {"id": "story-002", "title": "Second story", "status": "pending", "dependencies": []},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        # Create progress.txt marking story-001 as complete
        progress_path = tmp_path / "progress.txt"
        progress_path.write_text("[COMPLETE] story-001 - First story completed\n")

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        output_data = json.loads(result.output)
        # Should detect 1 completed story
        assert output_data.get("completed_stories", 0) >= 1


class TestDashboardAggregatorIntegration:
    """Integration tests for DashboardAggregator via CLI."""

    def test_aggregator_detects_hybrid_strategy(self, tmp_path):
        """Test that CLI correctly uses aggregator to detect HYBRID strategy."""
        # Create only a PRD (indicates HYBRID)
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test",
            "stories": [],
        }
        (tmp_path / "prd.json").write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        output_data = json.loads(result.output)
        assert output_data.get("strategy") == "HYBRID"

    def test_aggregator_detects_mega_strategy(self, tmp_path):
        """Test that CLI correctly uses aggregator to detect MEGA strategy."""
        # Create a mega-plan file (indicates MEGA)
        mega_plan = {"goal": "Test", "features": []}
        (tmp_path / "mega-plan.json").write_text(json.dumps(mega_plan))

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        output_data = json.loads(result.output)
        assert output_data.get("strategy") == "MEGA"


class TestDashboardEdgeCases:
    """Edge case tests for dashboard command."""

    def test_dashboard_with_invalid_prd(self, tmp_path):
        """Test dashboard handles corrupted PRD gracefully."""
        # Create an invalid PRD
        prd_path = tmp_path / "prd.json"
        prd_path.write_text("{ invalid json")

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--project", str(tmp_path)]
        )

        # Should not crash
        assert result.exit_code == 0

    def test_dashboard_empty_project(self, tmp_path):
        """Test dashboard in empty project directory."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--project", str(tmp_path)]
        )

        # Should complete without error
        assert result.exit_code == 0
        # Should show NOT_STARTED or similar
        assert "not_started" in result.output.lower() or "N/A" in result.output or "0%" in result.output

    def test_dashboard_completed_project(self, tmp_path):
        """Test dashboard shows complete status."""
        # Create a PRD with all stories
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test project",
            "stories": [
                {"id": "story-001", "title": "First story", "status": "complete", "dependencies": []},
                {"id": "story-002", "title": "Second story", "status": "complete", "dependencies": []},
            ],
        }
        prd_path = tmp_path / "prd.json"
        prd_path.write_text(json.dumps(prd_data))

        # Create progress.txt marking all stories as complete
        # (Dashboard reads completion status from progress.txt, not PRD status field)
        progress_path = tmp_path / "progress.txt"
        progress_path.write_text(
            "[COMPLETE] story-001 - First story completed\n"
            "[COMPLETE] story-002 - Second story completed\n"
        )

        result = runner.invoke(
            app,
            ["--legacy-mode", "dashboard", "--json", "--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        output_data = json.loads(result.output)
        assert output_data.get("status") == "completed"
        assert output_data.get("completed_stories") == 2
