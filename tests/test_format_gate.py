"""Tests for FormatGate module."""

import asyncio
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.core.quality_gate import (
    FormatGate,
    GateConfig,
    GateGroup,
    GateType,
    ProjectType,
    QualityGate,
)


class TestFormatGate:
    """Tests for FormatGate class."""

    @pytest.fixture
    def gate_config(self):
        """Create a test gate configuration."""
        return GateConfig(
            name="format",
            type=GateType.FORMAT,
            enabled=True,
            required=False,
        )

    @pytest.fixture
    def gate_config_check_only(self):
        """Create a test gate configuration with check_only mode."""
        return GateConfig(
            name="format-check",
            type=GateType.FORMAT,
            enabled=True,
            required=True,
            check_only=True,
        )

    def test_init(self, tmp_path: Path, gate_config: GateConfig):
        """Test FormatGate initialization."""
        gate = FormatGate(gate_config, tmp_path)

        assert gate.config == gate_config
        assert gate.project_root == tmp_path

    def test_gate_group(self):
        """Test that FormatGate is in PRE_VALIDATION group."""
        assert FormatGate.GATE_GROUP == GateGroup.PRE_VALIDATION

    def test_commands_defined_for_project_types(self):
        """Test that format commands are defined for major project types."""
        assert ProjectType.PYTHON in FormatGate.COMMANDS
        assert ProjectType.NODEJS in FormatGate.COMMANDS
        assert ProjectType.RUST in FormatGate.COMMANDS
        assert ProjectType.GO in FormatGate.COMMANDS

    def test_check_commands_defined(self):
        """Test that check-only commands are defined."""
        assert ProjectType.PYTHON in FormatGate.CHECK_COMMANDS
        assert ProjectType.NODEJS in FormatGate.CHECK_COMMANDS
        assert ProjectType.RUST in FormatGate.CHECK_COMMANDS
        assert ProjectType.GO in FormatGate.CHECK_COMMANDS

    def test_fallback_commands_defined(self):
        """Test that fallback commands are defined for Python and Node."""
        assert ProjectType.PYTHON in FormatGate.FALLBACK_COMMANDS
        assert ProjectType.NODEJS in FormatGate.FALLBACK_COMMANDS

    @patch.object(FormatGate, '_run_command')
    def test_execute_python_format(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test executing Python format command."""
        mock_run.return_value = (0, "Formatted 3 files", "", 1.5)

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.PYTHON}

        output = gate.execute("story-001", context)

        assert output.passed is True
        assert output.gate_type == GateType.FORMAT
        assert "ruff" in output.command or "format" in output.command.lower()

    @patch.object(FormatGate, '_run_command')
    def test_execute_python_format_check_only(
        self, mock_run: MagicMock, tmp_path: Path, gate_config_check_only: GateConfig
    ):
        """Test executing Python format in check-only mode."""
        mock_run.return_value = (0, "", "", 1.0)

        gate = FormatGate(gate_config_check_only, tmp_path)
        context = {"project_type": ProjectType.PYTHON}

        output = gate.execute("story-001", context)

        assert output.passed is True
        # Should use check command
        mock_run.assert_called_once()
        call_args = mock_run.call_args[0][0]
        assert "--check" in call_args

    @patch.object(FormatGate, '_run_command')
    def test_execute_nodejs_format(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test executing Node.js format command."""
        mock_run.return_value = (0, "", "", 2.0)

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.NODEJS}

        output = gate.execute("story-001", context)

        assert output.passed is True
        assert "prettier" in output.command.lower()

    @patch.object(FormatGate, '_run_command')
    def test_execute_rust_format(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test executing Rust format command."""
        mock_run.return_value = (0, "", "", 0.8)

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.RUST}

        output = gate.execute("story-001", context)

        assert output.passed is True
        assert "cargo" in output.command and "fmt" in output.command

    @patch.object(FormatGate, '_run_command')
    def test_execute_go_format(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test executing Go format command."""
        mock_run.return_value = (0, "", "", 0.5)

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.GO}

        output = gate.execute("story-001", context)

        assert output.passed is True
        assert "gofmt" in output.command

    @patch.object(FormatGate, '_run_command')
    def test_execute_go_check_files_need_formatting(
        self, mock_run: MagicMock, tmp_path: Path, gate_config_check_only: GateConfig
    ):
        """Test Go format check mode detects files needing formatting."""
        # gofmt -l returns files that need formatting (exit 0 but non-empty stdout)
        mock_run.return_value = (0, "main.go\nutils.go\n", "", 0.5)

        gate = FormatGate(gate_config_check_only, tmp_path)
        context = {"project_type": ProjectType.GO}

        output = gate.execute("story-001", context)

        # Should fail because files need formatting
        assert output.passed is False
        assert "Files need formatting" in output.stderr

    @patch.object(FormatGate, '_run_command')
    def test_execute_format_failed(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test handling format command failure."""
        mock_run.return_value = (1, "", "Error: Syntax error in file.py", 0.5)

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.PYTHON}

        output = gate.execute("story-001", context)

        assert output.passed is False
        assert output.error_summary is not None
        assert "Syntax error" in output.error_summary

    @patch.object(FormatGate, '_run_command')
    def test_execute_with_fallback(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test fallback when primary formatter not found."""
        # First call fails (ruff not found), second succeeds (black)
        mock_run.side_effect = [
            (-1, "", "Command not found: ruff", 0.1),
            (0, "Reformatted 2 files", "", 1.5),
        ]

        gate = FormatGate(gate_config, tmp_path)
        context = {"project_type": ProjectType.PYTHON}

        output = gate.execute("story-001", context)

        assert output.passed is True
        assert mock_run.call_count == 2

    @patch.object(FormatGate, '_run_command')
    def test_execute_skips_when_no_changed_files(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test gate skips when incremental mode has no changed files."""
        gate_config.incremental = True

        # Initialize git repo
        (tmp_path / ".git").mkdir()

        gate = FormatGate(gate_config, tmp_path)

        with patch.object(gate, '_get_changed_files_for_gate', return_value=[]):
            context = {"project_type": ProjectType.PYTHON}
            output = gate.execute("story-001", context)

        assert output.passed is True
        assert output.skipped is True
        assert "No formattable files" in output.stdout
        mock_run.assert_not_called()

    @patch.object(FormatGate, '_run_command')
    def test_execute_incremental_with_changed_files(
        self, mock_run: MagicMock, tmp_path: Path, gate_config: GateConfig
    ):
        """Test incremental mode includes changed files in command."""
        gate_config.incremental = True
        mock_run.return_value = (0, "", "", 0.5)

        # Initialize git repo
        (tmp_path / ".git").mkdir()

        gate = FormatGate(gate_config, tmp_path)

        changed_files = ["src/main.py", "src/utils.py"]
        with patch.object(gate, '_get_changed_files_for_gate', return_value=changed_files):
            context = {"project_type": ProjectType.PYTHON}
            output = gate.execute("story-001", context)

        assert output.passed is True
        assert output.checked_files == changed_files
        # Verify the command includes the changed files
        call_args = mock_run.call_args[0][0]
        assert "src/main.py" in call_args
        assert "src/utils.py" in call_args


class TestGateOrdering:
    """Tests for gate execution ordering."""

    def test_format_gate_in_pre_validation_group(self, tmp_path: Path):
        """Test that FORMAT gate is assigned to PRE_VALIDATION group."""
        qg = QualityGate(tmp_path)
        group = qg._get_gate_group(GateType.FORMAT)
        assert group == GateGroup.PRE_VALIDATION

    def test_validation_gates_in_validation_group(self, tmp_path: Path):
        """Test that TYPECHECK, TEST, LINT are in VALIDATION group."""
        qg = QualityGate(tmp_path)

        assert qg._get_gate_group(GateType.TYPECHECK) == GateGroup.VALIDATION
        assert qg._get_gate_group(GateType.TEST) == GateGroup.VALIDATION
        assert qg._get_gate_group(GateType.LINT) == GateGroup.VALIDATION

    def test_post_validation_gates_in_post_validation_group(self, tmp_path: Path):
        """Test that IMPLEMENTATION_VERIFY and CODE_REVIEW are in POST_VALIDATION group."""
        qg = QualityGate(tmp_path)

        assert qg._get_gate_group(GateType.IMPLEMENTATION_VERIFY) == GateGroup.POST_VALIDATION
        assert qg._get_gate_group(GateType.CODE_REVIEW) == GateGroup.POST_VALIDATION

    def test_group_gates_by_execution_order(self, tmp_path: Path):
        """Test grouping gates by execution order."""
        gates = [
            GateConfig(name="lint", type=GateType.LINT, enabled=True),
            GateConfig(name="format", type=GateType.FORMAT, enabled=True),
            GateConfig(name="test", type=GateType.TEST, enabled=True),
            GateConfig(name="code-review", type=GateType.CODE_REVIEW, enabled=True),
        ]

        qg = QualityGate(tmp_path, gates=gates)
        groups = qg._group_gates_by_execution_order(gates)

        assert len(groups[GateGroup.PRE_VALIDATION]) == 1  # format
        assert len(groups[GateGroup.VALIDATION]) == 2  # lint, test
        assert len(groups[GateGroup.POST_VALIDATION]) == 1  # code-review

    def test_disabled_gates_excluded_from_groups(self, tmp_path: Path):
        """Test that disabled gates are excluded from groups."""
        gates = [
            GateConfig(name="format", type=GateType.FORMAT, enabled=False),
            GateConfig(name="lint", type=GateType.LINT, enabled=True),
        ]

        qg = QualityGate(tmp_path, gates=gates)
        groups = qg._group_gates_by_execution_order(gates)

        assert len(groups[GateGroup.PRE_VALIDATION]) == 0  # format disabled
        assert len(groups[GateGroup.VALIDATION]) == 1  # lint only


class TestQualityGateWithFormat:
    """Tests for QualityGate with FORMAT gate integration."""

    def test_format_gate_registered(self, tmp_path: Path):
        """Test that FORMAT gate is registered in GATE_CLASSES."""
        assert GateType.FORMAT in QualityGate.GATE_CLASSES
        assert QualityGate.GATE_CLASSES[GateType.FORMAT] == FormatGate

    def test_get_gate_class_returns_format_gate(self, tmp_path: Path):
        """Test _get_gate_class returns FormatGate for FORMAT type."""
        gate_class = QualityGate._get_gate_class(GateType.FORMAT)
        assert gate_class == FormatGate

    @pytest.mark.asyncio
    async def test_execute_all_async_with_format_gate(self, tmp_path: Path):
        """Test execute_all_async runs FORMAT gate in correct order."""
        # Create a Python project
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'test'")

        gates = [
            GateConfig(name="format", type=GateType.FORMAT, enabled=True, required=False),
            GateConfig(name="lint", type=GateType.LINT, enabled=True, required=False),
        ]

        qg = QualityGate(tmp_path, gates=gates)

        # Mock the execution to track order
        execution_order = []

        async def mock_execute(gate, story_id, context):
            execution_order.append(gate.config.name)
            from plan_cascade.core.quality_gate import GateOutput
            return GateOutput(
                gate_name=gate.config.name,
                gate_type=gate.config.type,
                passed=True,
                exit_code=0,
                stdout="",
                stderr="",
                duration_seconds=0.1,
                command="mock",
            )

        with patch.object(FormatGate, 'execute_async', lambda self, s, c: mock_execute(self, s, c)):
            with patch.object(
                qg._get_gate_class(GateType.LINT), 'execute_async',
                lambda self, s, c: mock_execute(self, s, c)
            ):
                results = await qg.execute_all_async("story-001", {})

        # Format should run before lint (PRE_VALIDATION before VALIDATION)
        assert execution_order.index("format") < execution_order.index("lint")
