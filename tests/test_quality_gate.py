"""Tests for QualityGate module."""

import asyncio
import time
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.core.quality_gate import (
    Gate,
    GateConfig,
    GateOutput,
    GateType,
    LintGate,
    ProjectType,
    QualityGate,
    TestGate,
    TypeCheckGate,
)


class TestGateConfig:
    """Tests for GateConfig class."""

    def test_init(self):
        """Test GateConfig initialization."""
        config = GateConfig(
            name="test-gate",
            type=GateType.TEST,
            enabled=True,
            required=True
        )

        assert config.name == "test-gate"
        assert config.type == GateType.TEST
        assert config.enabled is True

    def test_to_dict(self):
        """Test converting to dictionary."""
        config = GateConfig(name="test", type=GateType.LINT)
        result = config.to_dict()

        assert result["name"] == "test"
        assert result["type"] == "lint"

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "name": "typecheck",
            "type": "typecheck",
            "enabled": True,
            "required": False
        }
        config = GateConfig.from_dict(data)

        assert config.name == "typecheck"
        assert config.type == GateType.TYPECHECK
        assert config.required is False


class TestQualityGate:
    """Tests for QualityGate class."""

    def test_init(self, tmp_path: Path):
        """Test QualityGate initialization."""
        qg = QualityGate(tmp_path)
        assert qg.project_root == tmp_path

    def test_detect_project_type_python(self, tmp_path: Path):
        """Test Python project type detection."""
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'test'")

        qg = QualityGate(tmp_path)
        project_type = qg.detect_project_type()

        assert project_type == ProjectType.PYTHON

    def test_detect_project_type_nodejs(self, tmp_path: Path):
        """Test Node.js project type detection."""
        (tmp_path / "package.json").write_text('{"name": "test"}')

        qg = QualityGate(tmp_path)
        project_type = qg.detect_project_type()

        assert project_type == ProjectType.NODEJS

    def test_detect_project_type_unknown(self, tmp_path: Path):
        """Test unknown project type detection."""
        qg = QualityGate(tmp_path)
        project_type = qg.detect_project_type()

        assert project_type == ProjectType.UNKNOWN

    def test_should_allow_progression_no_gates(self, tmp_path: Path):
        """Test progression allowed with no gates."""
        qg = QualityGate(tmp_path, gates=[])
        result = qg.should_allow_progression({})

        assert result is True

    def test_should_allow_progression_with_passing_required(self, tmp_path: Path):
        """Test progression allowed when required gates pass."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, required=True)
        ]
        qg = QualityGate(tmp_path, gates=gates)

        outputs = {
            "test": type("GateOutput", (), {"passed": True})()
        }
        result = qg.should_allow_progression(outputs)

        assert result is True

    def test_should_block_progression_with_failing_required(self, tmp_path: Path):
        """Test progression blocked when required gates fail."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, required=True)
        ]
        qg = QualityGate(tmp_path, gates=gates)

        outputs = {
            "test": type("GateOutput", (), {"passed": False})()
        }
        result = qg.should_allow_progression(outputs)

        assert result is False

    def test_create_default(self, tmp_path: Path):
        """Test creating default quality gate."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path)

        assert len(qg.gates) > 0
        # Should have typecheck for Python
        gate_names = [g.name for g in qg.gates]
        assert "typecheck" in gate_names

    def test_to_dict(self, tmp_path: Path):
        """Test converting to dictionary."""
        gates = [
            GateConfig(name="test", type=GateType.TEST)
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = qg.to_dict()

        assert result["enabled"] is True
        assert len(result["gates"]) == 1

    def test_get_failure_summary_no_failures(self, tmp_path: Path):
        """Test failure summary with no failures."""
        qg = QualityGate(tmp_path, gates=[])
        result = qg.get_failure_summary({})

        assert result is None


class TestGateAsyncExecution:
    """Tests for async gate execution."""

    @pytest.mark.asyncio
    async def test_gate_execute_async_default_implementation(self, tmp_path: Path):
        """Test that Gate.execute_async() wraps execute() correctly."""
        config = GateConfig(name="test", type=GateType.TEST)
        gate = TestGate(config, tmp_path)

        # Mock the synchronous execute method
        expected_output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="success",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        with patch.object(gate, 'execute', return_value=expected_output) as mock_execute:
            result = await gate.execute_async("story-001", {"project_type": ProjectType.PYTHON})

            mock_execute.assert_called_once_with("story-001", {"project_type": ProjectType.PYTHON})
            assert result == expected_output

    @pytest.mark.asyncio
    async def test_typecheck_gate_execute_async(self, tmp_path: Path):
        """Test TypeCheckGate async execution."""
        config = GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["typecheck"])
        gate = TypeCheckGate(config, tmp_path)

        result = await gate.execute_async("story-001", {"project_type": ProjectType.PYTHON})

        assert result.gate_name == "typecheck"
        assert result.gate_type == GateType.TYPECHECK
        # echo command should pass
        assert result.passed is True

    @pytest.mark.asyncio
    async def test_test_gate_execute_async(self, tmp_path: Path):
        """Test TestGate async execution."""
        config = GateConfig(name="tests", type=GateType.TEST, command="echo", args=["tests passed"])
        gate = TestGate(config, tmp_path)

        result = await gate.execute_async("story-001", {"project_type": ProjectType.PYTHON})

        assert result.gate_name == "tests"
        assert result.gate_type == GateType.TEST
        assert result.passed is True

    @pytest.mark.asyncio
    async def test_lint_gate_execute_async(self, tmp_path: Path):
        """Test LintGate async execution."""
        config = GateConfig(name="lint", type=GateType.LINT, command="echo", args=["lint passed"])
        gate = LintGate(config, tmp_path)

        result = await gate.execute_async("story-001", {"project_type": ProjectType.PYTHON})

        assert result.gate_name == "lint"
        assert result.gate_type == GateType.LINT
        assert result.passed is True


class TestQualityGateAsyncExecution:
    """Tests for QualityGate.execute_all_async()."""

    @pytest.mark.asyncio
    async def test_execute_all_async_empty_gates(self, tmp_path: Path):
        """Test execute_all_async with no gates configured."""
        qg = QualityGate(tmp_path, gates=[])
        result = await qg.execute_all_async("story-001")

        assert result == {}

    @pytest.mark.asyncio
    async def test_execute_all_async_single_gate(self, tmp_path: Path):
        """Test execute_all_async with a single gate."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["test passed"])
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = await qg.execute_all_async("story-001")

        assert "test" in result
        assert result["test"].passed is True

    @pytest.mark.asyncio
    async def test_execute_all_async_multiple_gates(self, tmp_path: Path):
        """Test execute_all_async with multiple gates."""
        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["typecheck"]),
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["test"]),
            GateConfig(name="lint", type=GateType.LINT, command="echo", args=["lint"]),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = await qg.execute_all_async("story-001")

        assert len(result) == 3
        assert "typecheck" in result
        assert "test" in result
        assert "lint" in result
        assert all(output.passed for output in result.values())

    @pytest.mark.asyncio
    async def test_execute_all_async_skips_disabled_gates(self, tmp_path: Path):
        """Test that disabled gates are skipped in async execution."""
        gates = [
            GateConfig(name="enabled", type=GateType.TEST, command="echo", args=["enabled"], enabled=True),
            GateConfig(name="disabled", type=GateType.TEST, command="echo", args=["disabled"], enabled=False),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = await qg.execute_all_async("story-001")

        assert "enabled" in result
        assert "disabled" not in result

    @pytest.mark.asyncio
    async def test_execute_all_async_parallel_execution_time(self, tmp_path: Path):
        """Test that parallel execution is faster than sequential.

        This test verifies that gates run in parallel by checking that
        the total execution time is closer to the slowest gate's time
        rather than the sum of all gate times.
        """
        # Use sleep commands to simulate gates with different execution times
        # On Windows, we use ping with timeout; on Unix, we use sleep
        import sys

        if sys.platform == "win32":
            # ping -n 2 localhost takes about 1 second (2 pings with 1 second interval)
            sleep_cmd = "ping"
            sleep_args_short = ["-n", "2", "127.0.0.1"]
        else:
            # sleep command on Unix
            sleep_cmd = "sleep"
            sleep_args_short = ["0.5"]

        gates = [
            GateConfig(name="gate1", type=GateType.TEST, command=sleep_cmd, args=sleep_args_short),
            GateConfig(name="gate2", type=GateType.LINT, command=sleep_cmd, args=sleep_args_short),
            GateConfig(name="gate3", type=GateType.TYPECHECK, command=sleep_cmd, args=sleep_args_short),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        start_time = time.time()
        result = await qg.execute_all_async("story-001")
        total_time = time.time() - start_time

        assert len(result) == 3

        # If executed sequentially, total time would be ~3x the individual gate time
        # With parallel execution, total time should be closer to 1x (plus overhead)
        # We use a generous threshold to account for system variability
        # The key assertion is that it's significantly less than sequential execution
        sum_of_durations = sum(output.duration_seconds for output in result.values())

        # Parallel execution should be faster than sequential
        # Allow some overhead, but it should be at most ~2x the slowest gate (not 3x)
        max_single_duration = max(output.duration_seconds for output in result.values())
        assert total_time < sum_of_durations * 0.8 or total_time < max_single_duration * 2.5

    @pytest.mark.asyncio
    async def test_execute_all_async_handles_failures(self, tmp_path: Path):
        """Test that execute_all_async handles gate failures correctly."""
        import sys

        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="passing", type=GateType.TEST, command="echo", args=["pass"]),
            GateConfig(name="failing", type=GateType.LINT, command=fail_cmd, args=fail_args),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = await qg.execute_all_async("story-001")

        assert len(result) == 2
        assert result["passing"].passed is True
        assert result["failing"].passed is False

    @pytest.mark.asyncio
    async def test_execute_all_async_compatible_with_should_allow_progression(self, tmp_path: Path):
        """Test that async results work with should_allow_progression."""
        gates = [
            GateConfig(name="required", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="optional", type=GateType.LINT, command="echo", args=["pass"], required=False),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        result = await qg.execute_all_async("story-001")

        # Results should be compatible with synchronous helper methods
        assert qg.should_allow_progression(result) is True
        assert qg.get_failure_summary(result) is None

    def test_execute_all_sync_backward_compatibility(self, tmp_path: Path):
        """Test that synchronous execute_all still works correctly."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["test passed"])
        ]
        qg = QualityGate(tmp_path, gates=gates)

        # Synchronous method should still work
        result = qg.execute_all("story-001")

        assert "test" in result
        assert result["test"].passed is True


class TestMixedProjectTypeDetection:
    """Tests for mixed project type detection."""

    def test_detect_project_types_single_python(self, tmp_path: Path):
        """Test detection of single Python project."""
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'test'")

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert types == [ProjectType.PYTHON]

    def test_detect_project_types_single_nodejs(self, tmp_path: Path):
        """Test detection of single Node.js project."""
        (tmp_path / "package.json").write_text('{"name": "test"}')

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert types == [ProjectType.NODEJS]

    def test_detect_project_types_mixed_root(self, tmp_path: Path):
        """Test detection of mixed project at root level."""
        (tmp_path / "pyproject.toml").write_text("[project]\nname = 'backend'")
        (tmp_path / "package.json").write_text('{"name": "frontend"}')

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        # Should detect both, ordered by type_order (NODEJS first, then PYTHON)
        assert ProjectType.NODEJS in types
        assert ProjectType.PYTHON in types
        assert len(types) == 2

    def test_detect_project_types_subdirectory_frontend_backend(self, tmp_path: Path):
        """Test detection of project types in frontend/backend subdirectories."""
        # Create frontend with Node.js
        frontend = tmp_path / "frontend"
        frontend.mkdir()
        (frontend / "package.json").write_text('{"name": "frontend"}')

        # Create backend with Python
        backend = tmp_path / "backend"
        backend.mkdir()
        (backend / "pyproject.toml").write_text("[project]\nname = 'backend'")

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert ProjectType.NODEJS in types
        assert ProjectType.PYTHON in types
        assert len(types) == 2

    def test_detect_project_types_subdirectory_api_web(self, tmp_path: Path):
        """Test detection of project types in api/web subdirectories."""
        # Create web with Node.js
        web = tmp_path / "web"
        web.mkdir()
        (web / "package.json").write_text('{"name": "web"}')

        # Create api with Python
        api = tmp_path / "api"
        api.mkdir()
        (api / "requirements.txt").write_text("flask\n")

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert ProjectType.NODEJS in types
        assert ProjectType.PYTHON in types

    def test_detect_project_types_mixed_with_rust(self, tmp_path: Path):
        """Test detection of mixed project including Rust."""
        (tmp_path / "Cargo.toml").write_text('[package]\nname = "core"')

        frontend = tmp_path / "frontend"
        frontend.mkdir()
        (frontend / "package.json").write_text('{"name": "ui"}')

        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert ProjectType.NODEJS in types
        assert ProjectType.RUST in types

    def test_detect_project_types_unknown(self, tmp_path: Path):
        """Test detection returns UNKNOWN when no project files found."""
        qg = QualityGate(tmp_path)
        types = qg.detect_project_types()

        assert types == [ProjectType.UNKNOWN]

    def test_detect_project_type_backward_compatible(self, tmp_path: Path):
        """Test that detect_project_type() returns first type for mixed projects."""
        (tmp_path / "package.json").write_text('{"name": "frontend"}')
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate(tmp_path)
        primary_type = qg.detect_project_type()

        # Should return first type (NODEJS is first in ordering)
        assert primary_type == ProjectType.NODEJS

    def test_detect_project_types_caching(self, tmp_path: Path):
        """Test that project type detection is cached."""
        (tmp_path / "package.json").write_text('{"name": "test"}')

        qg = QualityGate(tmp_path)
        types1 = qg.detect_project_types()
        types2 = qg.detect_project_types()

        # Should return same list object (cached)
        assert types1 is types2


class TestGateConfigProjectType:
    """Tests for GateConfig project_type field."""

    def test_gate_config_with_project_type(self):
        """Test GateConfig with project_type field."""
        config = GateConfig(
            name="typecheck-python",
            type=GateType.TYPECHECK,
            project_type=ProjectType.PYTHON,
        )

        assert config.project_type == ProjectType.PYTHON

    def test_gate_config_to_dict_with_project_type(self):
        """Test GateConfig.to_dict() includes project_type."""
        config = GateConfig(
            name="tests-nodejs",
            type=GateType.TEST,
            project_type=ProjectType.NODEJS,
        )

        result = config.to_dict()

        assert result["project_type"] == "nodejs"

    def test_gate_config_to_dict_without_project_type(self):
        """Test GateConfig.to_dict() omits project_type when None."""
        config = GateConfig(
            name="tests",
            type=GateType.TEST,
        )

        result = config.to_dict()

        assert "project_type" not in result

    def test_gate_config_from_dict_with_project_type(self):
        """Test GateConfig.from_dict() with project_type."""
        data = {
            "name": "lint-python",
            "type": "lint",
            "project_type": "python",
        }

        config = GateConfig.from_dict(data)

        assert config.project_type == ProjectType.PYTHON

    def test_gate_config_from_dict_without_project_type(self):
        """Test GateConfig.from_dict() without project_type."""
        data = {
            "name": "lint",
            "type": "lint",
        }

        config = GateConfig.from_dict(data)

        assert config.project_type is None


class TestCreateDefaultMixedProject:
    """Tests for create_default() with mixed projects."""

    def test_create_default_single_project(self, tmp_path: Path):
        """Test create_default for single project type."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path)

        # Should have gates without suffixes
        gate_names = [g.name for g in qg.gates]
        assert "typecheck" in gate_names
        assert "tests" in gate_names
        assert "lint" in gate_names

        # All gates should have project_type set
        for gate in qg.gates:
            if gate.name != "tests" or ProjectType.PYTHON in [g.project_type for g in qg.gates]:
                assert gate.project_type == ProjectType.PYTHON

    def test_create_default_mixed_python_nodejs(self, tmp_path: Path):
        """Test create_default for mixed Python + Node.js project."""
        (tmp_path / "pyproject.toml").write_text("[project]")
        (tmp_path / "package.json").write_text('{"name": "frontend"}')

        qg = QualityGate.create_default(tmp_path)

        gate_names = [g.name for g in qg.gates]

        # Should have suffixed gates for each type
        assert "typecheck-nodejs" in gate_names
        assert "typecheck-python" in gate_names
        assert "tests-nodejs" in gate_names
        assert "tests-python" in gate_names
        assert "lint-nodejs" in gate_names
        assert "lint-python" in gate_names

        # Verify project_type is set correctly
        for gate in qg.gates:
            if "-nodejs" in gate.name:
                assert gate.project_type == ProjectType.NODEJS
            elif "-python" in gate.name:
                assert gate.project_type == ProjectType.PYTHON

    def test_create_default_mixed_subdirectory(self, tmp_path: Path):
        """Test create_default for mixed project with subdirectories."""
        frontend = tmp_path / "frontend"
        frontend.mkdir()
        (frontend / "package.json").write_text('{"name": "frontend"}')

        backend = tmp_path / "backend"
        backend.mkdir()
        (backend / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path)

        gate_names = [g.name for g in qg.gates]

        # Should detect both and create suffixed gates
        assert "typecheck-nodejs" in gate_names
        assert "typecheck-python" in gate_names

    def test_create_default_unknown_project(self, tmp_path: Path):
        """Test create_default for unknown project type."""
        qg = QualityGate.create_default(tmp_path)

        gate_names = [g.name for g in qg.gates]

        # Should have generic gates without suffixes
        assert "tests" in gate_names
        assert "lint" in gate_names
        # No typecheck for unknown projects
        assert "typecheck" not in gate_names

    def test_create_default_rust_no_typecheck(self, tmp_path: Path):
        """Test create_default for Rust project (no typecheck gate)."""
        (tmp_path / "Cargo.toml").write_text('[package]\nname = "test"')

        qg = QualityGate.create_default(tmp_path)

        gate_names = [g.name for g in qg.gates]

        # Rust doesn't have typecheck gate
        assert "typecheck" not in gate_names
        assert "tests" in gate_names
        assert "lint" in gate_names

    def test_create_default_go_project(self, tmp_path: Path):
        """Test create_default for Go project."""
        (tmp_path / "go.mod").write_text("module test\n\ngo 1.21")

        qg = QualityGate.create_default(tmp_path)

        gate_names = [g.name for g in qg.gates]

        # Go doesn't have typecheck gate
        assert "typecheck" not in gate_names
        assert "tests" in gate_names
        assert "lint" in gate_names

        # Verify project_type
        for gate in qg.gates:
            assert gate.project_type == ProjectType.GO


class TestFailFast:
    """Tests for fail_fast feature."""

    def test_fail_fast_default_false(self, tmp_path: Path):
        """Test that fail_fast defaults to False."""
        qg = QualityGate(tmp_path)
        assert qg.fail_fast is False

    def test_fail_fast_can_be_enabled(self, tmp_path: Path):
        """Test that fail_fast can be set to True."""
        qg = QualityGate(tmp_path, fail_fast=True)
        assert qg.fail_fast is True

    def test_gate_output_skipped_default_false(self):
        """Test that GateOutput.skipped defaults to False."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="echo test",
        )
        assert output.skipped is False

    def test_gate_output_skipped_can_be_set(self):
        """Test that GateOutput.skipped can be set to True."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=False,
            exit_code=-1,
            stdout="",
            stderr="Skipped",
            duration_seconds=0.0,
            command="",
            skipped=True,
        )
        assert output.skipped is True


class TestFailFastSequential:
    """Tests for fail_fast in sequential execution."""

    def test_execute_all_without_fail_fast_runs_all(self, tmp_path: Path):
        """Test that without fail_fast, all gates run even after failure."""
        import sys
        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="gate1", type=GateType.TYPECHECK, command=fail_cmd, args=fail_args, required=True),
            GateConfig(name="gate2", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="gate3", type=GateType.LINT, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=False)

        result = qg.execute_all("story-001")

        # All gates should have run
        assert len(result) == 3
        assert "gate1" in result
        assert "gate2" in result
        assert "gate3" in result
        assert result["gate1"].passed is False
        assert result["gate2"].passed is True
        assert result["gate3"].passed is True
        # None should be skipped
        assert result["gate1"].skipped is False
        assert result["gate2"].skipped is False
        assert result["gate3"].skipped is False

    def test_execute_all_with_fail_fast_stops_on_required_failure(self, tmp_path: Path):
        """Test that with fail_fast, gates after required failure are skipped."""
        import sys
        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="gate1", type=GateType.TYPECHECK, command=fail_cmd, args=fail_args, required=True),
            GateConfig(name="gate2", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="gate3", type=GateType.LINT, command="echo", args=["pass"], required=False),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        result = qg.execute_all("story-001")

        # All gates should be in results
        assert len(result) == 3

        # First gate failed normally
        assert result["gate1"].passed is False
        assert result["gate1"].skipped is False

        # Remaining gates should be skipped
        assert result["gate2"].passed is False
        assert result["gate2"].skipped is True
        assert result["gate3"].passed is False
        assert result["gate3"].skipped is True

    def test_execute_all_with_fail_fast_continues_on_optional_failure(self, tmp_path: Path):
        """Test that fail_fast continues when optional gate fails."""
        import sys
        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="optional", type=GateType.LINT, command=fail_cmd, args=fail_args, required=False),
            GateConfig(name="required1", type=GateType.TYPECHECK, command="echo", args=["pass"], required=True),
            GateConfig(name="required2", type=GateType.TEST, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        result = qg.execute_all("story-001")

        # All gates should run since optional failure doesn't trigger fail_fast
        assert len(result) == 3
        assert result["optional"].passed is False
        assert result["optional"].skipped is False
        assert result["required1"].passed is True
        assert result["required1"].skipped is False
        assert result["required2"].passed is True
        assert result["required2"].skipped is False

    def test_execute_all_with_fail_fast_all_pass(self, tmp_path: Path):
        """Test that fail_fast doesn't affect execution when all gates pass."""
        gates = [
            GateConfig(name="gate1", type=GateType.TYPECHECK, command="echo", args=["pass"], required=True),
            GateConfig(name="gate2", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="gate3", type=GateType.LINT, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        result = qg.execute_all("story-001")

        assert len(result) == 3
        assert all(output.passed for output in result.values())
        assert all(not output.skipped for output in result.values())


class TestFailFastParallel:
    """Tests for fail_fast in parallel (async) execution."""

    @pytest.mark.asyncio
    async def test_execute_all_async_without_fail_fast_runs_all(self, tmp_path: Path):
        """Test that without fail_fast, all gates run even after failure."""
        import sys
        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="gate1", type=GateType.TYPECHECK, command=fail_cmd, args=fail_args, required=True),
            GateConfig(name="gate2", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="gate3", type=GateType.LINT, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=False)

        result = await qg.execute_all_async("story-001")

        # All gates should have run
        assert len(result) == 3
        assert result["gate1"].passed is False
        assert result["gate2"].passed is True
        assert result["gate3"].passed is True

    @pytest.mark.asyncio
    async def test_execute_all_async_with_fail_fast_cancels_running_tasks(self, tmp_path: Path):
        """Test that with fail_fast, running tasks are cancelled when a required gate fails."""
        import sys

        # Use a command that takes some time to simulate parallel execution
        if sys.platform == "win32":
            # Instant failure
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
            # Slow command (ping takes time)
            slow_cmd = "ping"
            slow_args = ["-n", "5", "127.0.0.1"]  # Takes about 4 seconds
        else:
            fail_cmd = "false"
            fail_args = []
            slow_cmd = "sleep"
            slow_args = ["3"]

        gates = [
            # This gate fails quickly
            GateConfig(name="failing", type=GateType.TYPECHECK, command=fail_cmd, args=fail_args, required=True),
            # These gates are slow and should be cancelled
            GateConfig(name="slow1", type=GateType.TEST, command=slow_cmd, args=slow_args, required=True),
            GateConfig(name="slow2", type=GateType.LINT, command=slow_cmd, args=slow_args, required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        start_time = time.time()
        result = await qg.execute_all_async("story-001")
        elapsed = time.time() - start_time

        # All gates should be in results
        assert len(result) == 3

        # The failing gate should have failed normally
        assert result["failing"].passed is False
        assert result["failing"].skipped is False

        # Slow gates should be either skipped (cancelled) or completed
        # Due to timing, we can't guarantee they were cancelled, but if they were
        # cancelled, they should be marked as skipped
        for name in ["slow1", "slow2"]:
            if result[name].skipped:
                assert result[name].passed is False
                assert "fail_fast" in result[name].stderr.lower() or "fail_fast" in result[name].error_summary.lower()

        # Execution should be faster than running all slow gates sequentially
        # If cancellation worked, total time should be much less than 6+ seconds
        # Allow generous margin for system variability
        assert elapsed < 5.0, f"Execution took {elapsed}s, expected faster due to cancellation"

    @pytest.mark.asyncio
    async def test_execute_all_async_with_fail_fast_continues_on_optional_failure(self, tmp_path: Path):
        """Test that fail_fast in async continues when optional gate fails."""
        import sys
        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(name="optional", type=GateType.LINT, command=fail_cmd, args=fail_args, required=False),
            GateConfig(name="required1", type=GateType.TYPECHECK, command="echo", args=["pass"], required=True),
            GateConfig(name="required2", type=GateType.TEST, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        result = await qg.execute_all_async("story-001")

        # All gates should run since optional failure doesn't trigger fail_fast
        assert len(result) == 3
        assert result["optional"].passed is False
        assert result["required1"].passed is True
        assert result["required2"].passed is True
        # None should be skipped
        for output in result.values():
            assert output.skipped is False

    @pytest.mark.asyncio
    async def test_execute_all_async_with_fail_fast_all_pass(self, tmp_path: Path):
        """Test that fail_fast doesn't affect async execution when all gates pass."""
        gates = [
            GateConfig(name="gate1", type=GateType.TYPECHECK, command="echo", args=["pass"], required=True),
            GateConfig(name="gate2", type=GateType.TEST, command="echo", args=["pass"], required=True),
            GateConfig(name="gate3", type=GateType.LINT, command="echo", args=["pass"], required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        result = await qg.execute_all_async("story-001")

        assert len(result) == 3
        assert all(output.passed for output in result.values())
        assert all(not output.skipped for output in result.values())


class TestFailFastSummary:
    """Tests for failure summary with fail_fast."""

    def test_get_failure_summary_with_skipped_gates(self, tmp_path: Path):
        """Test that failure summary correctly shows skipped gates."""
        gates = [
            GateConfig(name="failed", type=GateType.TYPECHECK, required=True),
            GateConfig(name="skipped1", type=GateType.TEST, required=True),
            GateConfig(name="skipped2", type=GateType.LINT, required=False),
        ]
        qg = QualityGate(tmp_path, gates=gates, fail_fast=True)

        outputs = {
            "failed": GateOutput(
                gate_name="failed",
                gate_type=GateType.TYPECHECK,
                passed=False,
                exit_code=1,
                stdout="",
                stderr="Type error",
                duration_seconds=0.5,
                command="mypy .",
                error_summary="Type error in file.py",
                skipped=False,
            ),
            "skipped1": GateOutput(
                gate_name="skipped1",
                gate_type=GateType.TEST,
                passed=False,
                exit_code=-1,
                stdout="",
                stderr="Skipped due to fail_fast",
                duration_seconds=0.0,
                command="",
                error_summary="Skipped due to fail_fast",
                skipped=True,
            ),
            "skipped2": GateOutput(
                gate_name="skipped2",
                gate_type=GateType.LINT,
                passed=False,
                exit_code=-1,
                stdout="",
                stderr="Skipped due to fail_fast",
                duration_seconds=0.0,
                command="",
                error_summary="Skipped due to fail_fast",
                skipped=True,
            ),
        }

        summary = qg.get_failure_summary(outputs)

        assert summary is not None
        assert "Quality gate failures:" in summary
        assert "failed (required)" in summary
        assert "Type error in file.py" in summary
        assert "Skipped gates (due to fail_fast):" in summary
        assert "skipped1 (required): Skipped" in summary
        assert "skipped2 (optional): Skipped" in summary

    def test_get_failure_summary_no_skipped_gates(self, tmp_path: Path):
        """Test that failure summary works without skipped gates."""
        gates = [
            GateConfig(name="failed", type=GateType.TYPECHECK, required=True),
        ]
        qg = QualityGate(tmp_path, gates=gates)

        outputs = {
            "failed": GateOutput(
                gate_name="failed",
                gate_type=GateType.TYPECHECK,
                passed=False,
                exit_code=1,
                stdout="",
                stderr="Type error",
                duration_seconds=0.5,
                command="mypy .",
                error_summary="Type error",
                skipped=False,
            ),
        }

        summary = qg.get_failure_summary(outputs)

        assert summary is not None
        assert "Quality gate failures:" in summary
        assert "Skipped gates" not in summary


class TestFailFastPRDSerialization:
    """Tests for fail_fast PRD serialization."""

    def test_to_dict_includes_fail_fast(self, tmp_path: Path):
        """Test that to_dict includes fail_fast setting."""
        qg = QualityGate(tmp_path, gates=[], fail_fast=True)
        result = qg.to_dict()

        assert "fail_fast" in result
        assert result["fail_fast"] is True

    def test_to_dict_fail_fast_false_by_default(self, tmp_path: Path):
        """Test that to_dict shows fail_fast as False when not set."""
        qg = QualityGate(tmp_path, gates=[])
        result = qg.to_dict()

        assert result["fail_fast"] is False

    def test_from_prd_reads_fail_fast(self, tmp_path: Path):
        """Test that from_prd reads fail_fast setting."""
        prd = {
            "quality_gates": {
                "enabled": True,
                "fail_fast": True,
                "gates": [
                    {"name": "test", "type": "test", "required": True}
                ]
            }
        }

        qg = QualityGate.from_prd(tmp_path, prd)

        assert qg.fail_fast is True

    def test_from_prd_fail_fast_defaults_to_false(self, tmp_path: Path):
        """Test that from_prd defaults fail_fast to False when not specified."""
        prd = {
            "quality_gates": {
                "enabled": True,
                "gates": [
                    {"name": "test", "type": "test", "required": True}
                ]
            }
        }

        qg = QualityGate.from_prd(tmp_path, prd)

        assert qg.fail_fast is False

    def test_create_default_with_fail_fast(self, tmp_path: Path):
        """Test that create_default accepts fail_fast parameter."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path, fail_fast=True)

        assert qg.fail_fast is True
        assert len(qg.gates) > 0

    def test_create_default_fail_fast_defaults_to_false(self, tmp_path: Path):
        """Test that create_default has fail_fast False by default."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path)

        assert qg.fail_fast is False
