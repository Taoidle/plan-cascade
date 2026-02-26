"""Tests for dependency resolution: command --project flags and CLI auto-env.

Verify that:
1. commands/*.md files use the correct --project "${CLAUDE_PLUGIN_ROOT}" pattern
2. The CLI entry point always ensures it runs in plan-cascade's own venv
3. The _SETTINGS_AVAILABLE flag exists for graceful degradation
"""

import os
import re
import sys
from pathlib import Path
from unittest.mock import MagicMock, call, patch

import pytest

# Root of the project
PROJECT_ROOT = Path(__file__).resolve().parent.parent
COMMANDS_DIR = PROJECT_ROOT / "commands"


# ---------------------------------------------------------------------------
# Test 1: Specific spec command files have --project flag
# ---------------------------------------------------------------------------

class TestSpecCommandsHaveProjectFlag:
    """Verify spec-plan.md, spec-resume.md, spec-cleanup.md use --project."""

    @pytest.fixture(params=["spec-plan.md", "spec-resume.md", "spec-cleanup.md"])
    def spec_file(self, request):
        path = COMMANDS_DIR / request.param
        assert path.exists(), f"{request.param} not found in commands/"
        return path

    def test_uv_run_plan_cascade_includes_project_flag(self, spec_file):
        """Every 'uv run ... plan-cascade' invocation must include --project."""
        content = spec_file.read_text()

        # Find all lines that contain 'uv run' and 'plan-cascade'
        uv_run_lines = [
            line.strip()
            for line in content.splitlines()
            if "uv run" in line and "plan-cascade" in line
        ]

        assert len(uv_run_lines) > 0, (
            f"{spec_file.name} should contain at least one 'uv run ... plan-cascade' call"
        )

        for line in uv_run_lines:
            assert '--project' in line, (
                f"{spec_file.name} has a bare 'uv run plan-cascade' without --project:\n  {line}"
            )
            assert '${CLAUDE_PLUGIN_ROOT}' in line, (
                f"{spec_file.name} uses --project but not with ${{CLAUDE_PLUGIN_ROOT}}:\n  {line}"
            )

    def test_no_bare_uv_run_plan_cascade(self, spec_file):
        """No 'uv run plan-cascade' without a preceding --project flag."""
        content = spec_file.read_text()

        bare_pattern = re.compile(r'uv\s+run\s+plan-cascade')
        matches = bare_pattern.findall(content)

        assert len(matches) == 0, (
            f"{spec_file.name} contains bare 'uv run plan-cascade' (without --project):\n"
            + "\n".join(matches)
        )


# ---------------------------------------------------------------------------
# Test 2: No bare uv run plan-cascade in ANY command file
# ---------------------------------------------------------------------------

class TestAllCommandFilesProjectFlag:
    """Scan ALL command files to ensure no bare 'uv run plan-cascade'."""

    def test_no_command_file_has_bare_uv_run_plan_cascade(self):
        """No command file should have 'uv run plan-cascade' without --project."""
        bare_pattern = re.compile(r'uv\s+run\s+plan-cascade')

        violations = []
        for md_file in sorted(COMMANDS_DIR.glob("*.md")):
            content = md_file.read_text()
            for i, line in enumerate(content.splitlines(), start=1):
                if bare_pattern.search(line):
                    violations.append(f"  {md_file.name}:{i}: {line.strip()}")

        assert len(violations) == 0, (
            "Found bare 'uv run plan-cascade' (without --project) in command files:\n"
            + "\n".join(violations)
        )


# ---------------------------------------------------------------------------
# Test 3: __init__.py has _SETTINGS_AVAILABLE flag
# ---------------------------------------------------------------------------

class TestSettingsAvailableFlag:
    """Verify the _SETTINGS_AVAILABLE attribute exists in the plan_cascade package."""

    def test_settings_available_attribute_exists(self):
        """plan_cascade should expose _SETTINGS_AVAILABLE."""
        import plan_cascade

        assert hasattr(plan_cascade, "_SETTINGS_AVAILABLE"), (
            "plan_cascade module must expose a _SETTINGS_AVAILABLE attribute"
        )

    def test_settings_available_is_true_in_correct_environment(self):
        """In the project environment, _SETTINGS_AVAILABLE should be True."""
        import plan_cascade

        assert plan_cascade._SETTINGS_AVAILABLE is True, (
            "_SETTINGS_AVAILABLE should be True when all dependencies are installed"
        )


# ---------------------------------------------------------------------------
# Test 4: _ensure_correct_env() always re-execs when outside own venv
# ---------------------------------------------------------------------------

class TestEnsureCorrectEnv:
    """Verify _ensure_correct_env() re-execs with --project when not in own venv."""

    def test_reexecs_when_interpreter_outside_own_venv(self, tmp_path):
        """Should re-exec with uv run --project when interpreter is outside own venv."""
        from plan_cascade.cli.main import _ensure_correct_env

        # The real project root (where pyproject.toml lives)
        project_root = Path(__file__).resolve().parent.parent

        # Fake interpreter outside the project venv
        fake_python = tmp_path / "fake_venv" / "bin" / "python"
        fake_python.parent.mkdir(parents=True)
        fake_python.touch()

        with patch.dict(os.environ, {}, clear=False), \
             patch("shutil.which", return_value="/usr/bin/uv"), \
             patch("subprocess.call", return_value=0) as mock_call, \
             patch("sys.executable", str(fake_python)), \
             pytest.raises(SystemExit) as exc_info:
            os.environ.pop("_PLAN_CASCADE_REEXEC", None)
            _ensure_correct_env()

        assert exc_info.value.code == 0
        mock_call.assert_called_once()

        # Verify the command includes --project with our project root
        cmd = mock_call.call_args[0][0]
        assert cmd[0] == "uv"
        assert cmd[1] == "run"
        assert "--project" in cmd
        project_idx = cmd.index("--project")
        assert cmd[project_idx + 1] == str(project_root)

        # Verify _PLAN_CASCADE_REEXEC is set in the env
        env = mock_call.call_args[1]["env"]
        assert env["_PLAN_CASCADE_REEXEC"] == "1"

    def test_no_reexec_when_env_var_set(self):
        """Should not re-exec when _PLAN_CASCADE_REEXEC is already set."""
        from plan_cascade.cli.main import _ensure_correct_env

        with patch.dict(os.environ, {"_PLAN_CASCADE_REEXEC": "1"}), \
             patch("subprocess.call") as mock_call:
            _ensure_correct_env()
            mock_call.assert_not_called()

    def test_no_reexec_when_uv_not_available(self):
        """Should not re-exec when uv is not installed."""
        from plan_cascade.cli.main import _ensure_correct_env

        with patch.dict(os.environ, {}, clear=False), \
             patch("shutil.which", return_value=None), \
             patch("subprocess.call") as mock_call:
            os.environ.pop("_PLAN_CASCADE_REEXEC", None)
            _ensure_correct_env()
            mock_call.assert_not_called()

    def test_no_reexec_when_already_in_correct_venv(self):
        """Should not re-exec when interpreter is already in plan-cascade's venv."""
        from plan_cascade.cli.main import _ensure_correct_env

        # sys.executable is already inside .venv when running tests with uv
        # so _ensure_correct_env should return without re-exec
        with patch.dict(os.environ, {}, clear=False), \
             patch("subprocess.call") as mock_call:
            os.environ.pop("_PLAN_CASCADE_REEXEC", None)
            _ensure_correct_env()
            mock_call.assert_not_called()

    def test_filters_user_project_flag_from_argv(self, tmp_path):
        """User --project in sys.argv must be stripped to prevent override."""
        from plan_cascade.cli.main import _ensure_correct_env

        project_root = Path(__file__).resolve().parent.parent
        fake_python = tmp_path / "other_venv" / "bin" / "python"
        fake_python.parent.mkdir(parents=True)
        fake_python.touch()

        with patch.dict(os.environ, {}, clear=False), \
             patch("shutil.which", return_value="/usr/bin/uv"), \
             patch("subprocess.call", return_value=0) as mock_call, \
             patch("sys.executable", str(fake_python)), \
             patch("sys.argv", ["plan-cascade", "--project", "/evil/path", "spec", "plan"]), \
             pytest.raises(SystemExit):
            os.environ.pop("_PLAN_CASCADE_REEXEC", None)
            _ensure_correct_env()

        cmd = mock_call.call_args[0][0]
        # Our --project must point to the real project root
        project_idx = cmd.index("--project")
        assert cmd[project_idx + 1] == str(project_root)
        # The user's --project and its value must be stripped
        assert "/evil/path" not in cmd
        # The remaining args (spec, plan) should still be forwarded
        assert "spec" in cmd
        assert "plan" in cmd

    def test_handles_subprocess_oserror_gracefully(self, tmp_path):
        """OSError from subprocess.call should not crash the process."""
        from plan_cascade.cli.main import _ensure_correct_env

        fake_python = tmp_path / "other_venv" / "bin" / "python"
        fake_python.parent.mkdir(parents=True)
        fake_python.touch()

        with patch.dict(os.environ, {}, clear=False), \
             patch("shutil.which", return_value="/usr/bin/uv"), \
             patch("subprocess.call", side_effect=OSError("Permission denied")), \
             patch("sys.executable", str(fake_python)):
            os.environ.pop("_PLAN_CASCADE_REEXEC", None)
            # Should NOT raise — falls through gracefully
            _ensure_correct_env()


# ---------------------------------------------------------------------------
# Test 5: CLI main() fallback when _SETTINGS_AVAILABLE is False
# ---------------------------------------------------------------------------

class TestCLIMainFallback:
    """Verify main() shows error when _ensure_correct_env can't fix things."""

    def test_main_exits_when_settings_unavailable_and_reexec_done(self, capsys):
        """After re-exec (env var set), main() should show error if deps still missing."""
        from plan_cascade.cli.main import main

        with patch("plan_cascade.cli.main._SETTINGS_AVAILABLE", False), \
             patch("plan_cascade.cli.main._ensure_correct_env"), \
             pytest.raises(SystemExit) as exc_info:
            main()

        assert exc_info.value.code == 1

        captured = capsys.readouterr()
        assert "missing required dependencies" in captured.err.lower()
        assert "pip install" in captured.err

    def test_main_cleans_up_reexec_env_var(self):
        """main() should remove _PLAN_CASCADE_REEXEC from environment."""
        from plan_cascade.cli.main import main

        with patch("plan_cascade.cli.main._ensure_correct_env"), \
             patch("plan_cascade.cli.main._SETTINGS_AVAILABLE", True), \
             patch("plan_cascade.cli.main.HAS_TYPER", True), \
             patch("plan_cascade.cli.main.app") as mock_app, \
             patch.dict(os.environ, {"_PLAN_CASCADE_REEXEC": "1"}):
            main()

        assert "_PLAN_CASCADE_REEXEC" not in os.environ
