"""Tests for QualityGate module."""

import pytest
from pathlib import Path

from plan_cascade.core.quality_gate import (
    QualityGate,
    GateConfig,
    GateType,
    ProjectType,
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
