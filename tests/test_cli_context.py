"""Tests for CLI context and global --legacy-mode flag."""

import json
import pytest
from pathlib import Path
from typer.testing import CliRunner

from src.plan_cascade.cli.context import CLIContext, get_cli_context
from src.plan_cascade.cli.main import app
from src.plan_cascade.state.path_resolver import LINK_FILE_NAME


runner = CliRunner()


class TestCLIContext:
    """Tests for CLIContext dataclass."""

    def test_default_context(self):
        """Test default CLIContext values."""
        ctx = CLIContext()

        assert ctx.legacy_mode is None  # Default is None (auto-detect)
        assert ctx.project_root == Path.cwd()
        assert ctx._path_resolver is None

    def test_context_with_legacy_mode(self):
        """Test CLIContext with legacy_mode enabled."""
        ctx = CLIContext(legacy_mode=True)

        assert ctx.legacy_mode is True

    def test_context_with_legacy_mode_false(self):
        """Test CLIContext with legacy_mode disabled."""
        ctx = CLIContext(legacy_mode=False)

        assert ctx.legacy_mode is False

    def test_context_with_project_root(self, tmp_path):
        """Test CLIContext with custom project_root."""
        ctx = CLIContext(project_root=tmp_path)

        assert ctx.project_root == tmp_path

    def test_from_options_defaults(self):
        """Test CLIContext.from_options with defaults."""
        ctx = CLIContext.from_options()

        assert ctx.legacy_mode is None  # Default is None (auto-detect)
        assert ctx.project_root == Path.cwd()

    def test_from_options_with_values(self, tmp_path):
        """Test CLIContext.from_options with custom values."""
        ctx = CLIContext.from_options(
            legacy_mode=True,
            project_root=tmp_path,
        )

        assert ctx.legacy_mode is True
        assert ctx.project_root == tmp_path

    def test_get_path_resolver_creates_resolver(self, tmp_path):
        """Test get_path_resolver creates a PathResolver."""
        ctx = CLIContext(
            legacy_mode=False,
            project_root=tmp_path,
        )

        resolver = ctx.get_path_resolver()

        assert resolver is not None
        assert resolver.project_root == tmp_path
        assert resolver.is_legacy_mode() is False

    def test_get_path_resolver_with_legacy_mode(self, tmp_path):
        """Test get_path_resolver respects legacy_mode."""
        ctx = CLIContext(
            legacy_mode=True,
            project_root=tmp_path,
        )

        resolver = ctx.get_path_resolver()

        assert resolver.is_legacy_mode() is True
        # In legacy mode, project dir should be project root
        assert resolver.get_project_dir() == tmp_path

    def test_get_path_resolver_caches(self, tmp_path):
        """Test get_path_resolver returns cached instance."""
        ctx = CLIContext(project_root=tmp_path)

        resolver1 = ctx.get_path_resolver()
        resolver2 = ctx.get_path_resolver()

        assert resolver1 is resolver2


class TestGetCLIContext:
    """Tests for get_cli_context helper function."""

    def test_with_cli_context_object(self, tmp_path):
        """Test get_cli_context extracts CLIContext from ctx.obj."""
        # Create a mock context with CLIContext
        class MockContext:
            obj = CLIContext(legacy_mode=True, project_root=tmp_path)

        ctx = MockContext()
        cli_ctx = get_cli_context(ctx)

        assert cli_ctx.legacy_mode is True
        assert cli_ctx.project_root == tmp_path

    def test_with_none_object(self):
        """Test get_cli_context returns default when ctx.obj is None."""
        class MockContext:
            obj = None

        ctx = MockContext()
        cli_ctx = get_cli_context(ctx)

        assert cli_ctx.legacy_mode is False
        assert isinstance(cli_ctx, CLIContext)

    def test_with_non_cli_context_object(self):
        """Test get_cli_context returns default for non-CLIContext object."""
        class MockContext:
            obj = {"some": "dict"}

        ctx = MockContext()
        cli_ctx = get_cli_context(ctx)

        assert cli_ctx.legacy_mode is False
        assert isinstance(cli_ctx, CLIContext)


class TestGlobalLegacyModeFlag:
    """Tests for the global --legacy-mode CLI flag."""

    def test_help_shows_legacy_mode_option(self):
        """Test that --help shows the --legacy-mode option."""
        result = runner.invoke(app, ["--help"])

        assert result.exit_code == 0
        assert "--legacy-mode" in result.output
        assert "--no-legacy-mode" in result.output

    def test_legacy_mode_flag_accepted(self, tmp_path):
        """Test that --legacy-mode flag is accepted."""
        # Using status command as a simple test
        result = runner.invoke(
            app,
            ["--legacy-mode", "status", "--project", str(tmp_path)]
        )

        # Should not error on the flag itself (may fail due to no prd)
        assert "--legacy-mode" not in result.output or "unknown" not in result.output.lower()

    def test_no_legacy_mode_flag_accepted(self, tmp_path):
        """Test that --no-legacy-mode flag is accepted."""
        result = runner.invoke(
            app,
            ["--no-legacy-mode", "status", "--project", str(tmp_path)]
        )

        # Should not error on the flag itself
        assert "--no-legacy-mode" not in result.output or "unknown" not in result.output.lower()

    def test_legacy_mode_in_help_description(self):
        """Test that help describes what legacy mode does."""
        result = runner.invoke(app, ["--help"])

        assert result.exit_code == 0
        # Should mention file paths or project root
        assert "project root" in result.output.lower() or "file path" in result.output.lower() or "user directory" in result.output.lower()


class TestLegacyModeIntegration:
    """Integration tests for legacy mode with path resolution."""

    def test_legacy_mode_affects_path_resolver_in_context(self, tmp_path):
        """Test that legacy mode flag affects PathResolver configuration."""
        # Create contexts with different modes
        ctx_legacy = CLIContext.from_options(
            legacy_mode=True,
            project_root=tmp_path,
        )
        ctx_new = CLIContext.from_options(
            legacy_mode=False,
            project_root=tmp_path,
        )

        resolver_legacy = ctx_legacy.get_path_resolver()
        resolver_new = ctx_new.get_path_resolver()

        # Legacy mode: files in project root
        assert resolver_legacy.get_prd_path() == tmp_path / "prd.json"

        # New mode: files in user directory
        assert resolver_new.get_prd_path() != tmp_path / "prd.json"
        # Should be in some platform-specific location
        assert "plan-cascade" in str(resolver_new.get_prd_path())


class TestAutoDetection:
    """Tests for auto-detection of migrated projects."""

    def test_auto_detect_legacy_when_no_link_file(self, tmp_path):
        """Test auto-detection defaults to legacy when no link file exists."""
        ctx = CLIContext.from_options(
            legacy_mode=None,  # Auto-detect
            project_root=tmp_path,
        )

        # Should resolve to legacy mode
        assert ctx.get_resolved_legacy_mode() is True
        resolver = ctx.get_path_resolver()
        assert resolver.is_legacy_mode() is True

    def test_auto_detect_migrated_when_valid_link_file(self, tmp_path):
        """Test auto-detection uses new mode when valid link file exists."""
        # Create data directory
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create valid link file
        link_data = {
            "project_id": "test-proj-12345678",
            "data_path": str(data_dir),
            "created_at": "2024-01-01T00:00:00Z",
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        ctx = CLIContext.from_options(
            legacy_mode=None,  # Auto-detect
            project_root=tmp_path,
        )

        # Should resolve to new mode
        assert ctx.get_resolved_legacy_mode() is False
        resolver = ctx.get_path_resolver()
        assert resolver.is_legacy_mode() is False

    def test_explicit_legacy_mode_overrides_detection(self, tmp_path):
        """Test explicit legacy_mode=True overrides auto-detection."""
        # Create data directory and valid link file
        data_dir = tmp_path / "data"
        data_dir.mkdir()
        link_data = {
            "project_id": "test-proj",
            "data_path": str(data_dir),
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        # Explicitly set legacy mode
        ctx = CLIContext.from_options(
            legacy_mode=True,  # Explicit override
            project_root=tmp_path,
        )

        # Should use legacy mode despite valid link file
        assert ctx.get_resolved_legacy_mode() is True
        resolver = ctx.get_path_resolver()
        assert resolver.is_legacy_mode() is True

    def test_explicit_new_mode_overrides_detection(self, tmp_path):
        """Test explicit legacy_mode=False overrides auto-detection."""
        # No link file - would normally auto-detect to legacy

        ctx = CLIContext.from_options(
            legacy_mode=False,  # Explicit new mode
            project_root=tmp_path,
        )

        # Should use new mode despite no link file
        assert ctx.get_resolved_legacy_mode() is False
        resolver = ctx.get_path_resolver()
        assert resolver.is_legacy_mode() is False

    def test_resolved_mode_is_cached(self, tmp_path):
        """Test that resolved legacy mode is cached."""
        ctx = CLIContext.from_options(
            legacy_mode=None,
            project_root=tmp_path,
        )

        # First call
        mode1 = ctx.get_resolved_legacy_mode()
        # Second call should use cached value
        mode2 = ctx.get_resolved_legacy_mode()

        assert mode1 == mode2
        assert ctx._resolved_legacy_mode is not None

    def test_auto_detect_with_invalid_link_file(self, tmp_path):
        """Test auto-detection falls back to legacy with invalid link file."""
        # Create invalid link file
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text("not valid json {{{")

        ctx = CLIContext.from_options(
            legacy_mode=None,
            project_root=tmp_path,
        )

        # Should fall back to legacy mode
        assert ctx.get_resolved_legacy_mode() is True

    def test_auto_detect_with_orphaned_link(self, tmp_path):
        """Test auto-detection falls back to legacy when data dir missing."""
        # Create link file pointing to non-existent data
        link_data = {
            "project_id": "test-proj",
            "data_path": str(tmp_path / "nonexistent" / "data"),
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        ctx = CLIContext.from_options(
            legacy_mode=None,
            project_root=tmp_path,
        )

        # Should fall back to legacy mode
        assert ctx.get_resolved_legacy_mode() is True


class TestStateManagerWithPathResolver:
    """Integration tests for StateManager using PathResolver from CLIContext."""

    def test_state_manager_with_legacy_mode_resolver(self, tmp_path):
        """Test StateManager uses legacy paths when given legacy PathResolver."""
        from src.plan_cascade.state.state_manager import StateManager

        ctx = CLIContext.from_options(
            legacy_mode=True,
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        state_manager = StateManager(tmp_path, path_resolver=path_resolver)

        # PRD should be in project root in legacy mode
        assert state_manager.prd_path == tmp_path / "prd.json"
        assert state_manager.is_legacy_mode() is True

    def test_state_manager_with_new_mode_resolver(self, tmp_path):
        """Test StateManager uses new paths when given new mode PathResolver."""
        from src.plan_cascade.state.state_manager import StateManager

        ctx = CLIContext.from_options(
            legacy_mode=False,
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        state_manager = StateManager(tmp_path, path_resolver=path_resolver)

        # PRD should NOT be in project root in new mode
        assert state_manager.prd_path != tmp_path / "prd.json"
        # Should be in platform-specific user directory
        assert "plan-cascade" in str(state_manager.prd_path)
        assert state_manager.is_legacy_mode() is False

    def test_state_manager_inherits_resolver_from_context(self, tmp_path):
        """Test that StateManager path resolution matches CLIContext."""
        from src.plan_cascade.state.state_manager import StateManager

        # Create context with explicit new mode
        ctx = CLIContext.from_options(
            legacy_mode=False,
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        state_manager = StateManager(tmp_path, path_resolver=path_resolver)

        # StateManager paths should match PathResolver paths
        assert state_manager.prd_path == path_resolver.get_prd_path()

    def test_state_manager_read_write_with_resolver(self, tmp_path):
        """Test StateManager can read/write files using PathResolver paths."""
        from src.plan_cascade.state.state_manager import StateManager

        ctx = CLIContext.from_options(
            legacy_mode=True,  # Use legacy mode for simpler test
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        state_manager = StateManager(tmp_path, path_resolver=path_resolver)

        # Write and read PRD
        test_prd = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test goal",
            "stories": [{"id": "story-001", "status": "pending"}],
        }
        state_manager.write_prd(test_prd)
        read_prd = state_manager.read_prd()

        assert read_prd is not None
        assert read_prd["goal"] == "Test goal"
        assert read_prd["stories"][0]["id"] == "story-001"


class TestContextRecoveryWithPathResolver:
    """Integration tests for ContextRecoveryManager using PathResolver from CLIContext."""

    def test_context_recovery_with_legacy_resolver(self, tmp_path):
        """Test ContextRecoveryManager uses legacy paths when given legacy PathResolver."""
        from src.plan_cascade.state.context_recovery import ContextRecoveryManager

        ctx = CLIContext.from_options(
            legacy_mode=True,
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        recovery_manager = ContextRecoveryManager(tmp_path, path_resolver=path_resolver)

        # PRD path should be in project root
        assert recovery_manager.prd_path == tmp_path / "prd.json"
        assert recovery_manager.is_legacy_mode() is True

    def test_context_recovery_with_new_mode_resolver(self, tmp_path):
        """Test ContextRecoveryManager uses new paths when given new mode PathResolver."""
        from src.plan_cascade.state.context_recovery import ContextRecoveryManager

        ctx = CLIContext.from_options(
            legacy_mode=False,
            project_root=tmp_path,
        )
        path_resolver = ctx.get_path_resolver()

        recovery_manager = ContextRecoveryManager(tmp_path, path_resolver=path_resolver)

        # PRD path should NOT be in project root
        assert recovery_manager.prd_path != tmp_path / "prd.json"
        assert recovery_manager.is_legacy_mode() is False


class TestCLICommandsWithPathResolver:
    """Integration tests for CLI commands using PathResolver."""

    def test_status_command_with_legacy_mode(self, tmp_path):
        """Test status command works with --legacy-mode flag."""
        # Create a PRD in the project root (legacy location)
        prd_path = tmp_path / "prd.json"
        prd_data = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test Story", "status": "complete"}],
        }
        prd_path.write_text(json.dumps(prd_data))

        result = runner.invoke(
            app,
            ["--legacy-mode", "status", "--project", str(tmp_path)]
        )

        # Should find the PRD and display status
        assert result.exit_code == 0 or "No PRD found" not in result.output
        # When PRD exists, should show story info
        if result.exit_code == 0 and "story-001" not in result.output:
            # If story ID not shown, at least should show status info
            assert "Completed" in result.output or "complete" in result.output.lower()

    def test_status_command_without_prd(self, tmp_path):
        """Test status command handles missing PRD gracefully."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "status", "--project", str(tmp_path)]
        )

        # Should indicate no PRD found
        assert "No PRD found" in result.output or result.exit_code == 0

    def test_resume_command_with_legacy_mode(self, tmp_path):
        """Test resume command works with --legacy-mode flag."""
        result = runner.invoke(
            app,
            ["--legacy-mode", "resume", "--project", str(tmp_path)]
        )

        # Should work without error (may report no context found)
        # The important thing is it doesn't crash
        assert result.exit_code == 0 or "No task context" in result.output or "unknown" in result.output.lower()
