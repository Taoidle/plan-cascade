"""Tests for ReadinessGate (DoR) module."""

import pytest

from plan_cascade.core.readiness_gate import (
    GateMode,
    ReadinessCheckResult,
    ReadinessGate,
    check_acceptance_criteria,
    check_blast_radius,
    check_dependencies,
    check_dependency_dag,
    check_feature_boundaries,
    check_feature_count,
    check_risk_tags,
    check_rollback_hint,
    check_test_requirements,
    check_verification_hints,
)


class TestReadinessCheckResult:
    """Tests for ReadinessCheckResult dataclass."""

    def test_init_defaults(self):
        """Test default initialization."""
        result = ReadinessCheckResult()
        assert result.passed is True
        assert result.warnings == []
        assert result.errors == []

    def test_to_dict(self):
        """Test conversion to dictionary."""
        result = ReadinessCheckResult(passed=True, check_name="test")
        d = result.to_dict()
        assert d["passed"] is True
        assert d["check_name"] == "test"

    def test_from_dict(self):
        """Test creation from dictionary."""
        data = {"passed": False, "warnings": ["warn"], "check_name": "test"}
        result = ReadinessCheckResult.from_dict(data)
        assert result.passed is False
        assert result.warnings == ["warn"]

    def test_combine_empty(self):
        """Test combining empty list."""
        combined = ReadinessCheckResult.combine([])
        assert combined.passed is True

    def test_combine_multiple(self):
        """Test combining multiple results."""
        r1 = ReadinessCheckResult(passed=True, warnings=["w1"], check_name="c1")
        r2 = ReadinessCheckResult(passed=False, errors=["e1"], check_name="c2")
        combined = ReadinessCheckResult.combine([r1, r2])
        assert combined.passed is False
        assert combined.warnings == ["w1"]
        assert combined.errors == ["e1"]

    def test_add_warning(self):
        """Test adding a warning."""
        result = ReadinessCheckResult()
        result.add_warning("test warning")
        assert "test warning" in result.warnings
        assert result.passed is True

    def test_add_error(self):
        """Test adding an error."""
        result = ReadinessCheckResult()
        result.add_error("test error")
        assert "test error" in result.errors
        assert result.passed is False

    def test_has_issues(self):
        """Test has_issues method."""
        result = ReadinessCheckResult()
        assert result.has_issues() is False
        result.add_warning("w")
        assert result.has_issues() is True

    def test_get_summary(self):
        """Test get_summary method."""
        result = ReadinessCheckResult(check_name="test_check")
        result.add_error("error1")
        summary = result.get_summary()
        assert "[FAILED]" in summary
        assert "test_check" in summary


class TestPRDChecks:
    """Tests for PRD DoR checks."""

    def test_check_acceptance_criteria_valid(self):
        """Test with valid acceptance criteria."""
        stories = [
            {"id": "s1", "acceptance_criteria": ["Should return a value"]},
        ]
        result = check_acceptance_criteria(stories)
        assert result.passed is True

    def test_check_acceptance_criteria_missing(self):
        """Test with missing acceptance criteria."""
        stories = [{"id": "s1", "acceptance_criteria": []}]
        result = check_acceptance_criteria(stories)
        assert result.passed is False

    def test_check_dependencies_valid(self):
        """Test with valid dependencies (DAG)."""
        stories = [
            {"id": "s1", "dependencies": []},
            {"id": "s2", "dependencies": ["s1"]},
        ]
        result = check_dependencies(stories)
        assert result.passed is True

    def test_check_dependencies_unknown(self):
        """Test with unknown dependency."""
        stories = [{"id": "s1", "dependencies": ["unknown"]}]
        result = check_dependencies(stories)
        assert result.passed is False

    def test_check_dependencies_circular(self):
        """Test circular dependency detection."""
        stories = [
            {"id": "s1", "dependencies": ["s2"]},
            {"id": "s2", "dependencies": ["s1"]},
        ]
        result = check_dependencies(stories)
        assert result.passed is False
        assert any("Circular" in e for e in result.errors)

    def test_check_verification_hints_present(self):
        """Test with verification hints present."""
        stories = [{"id": "s1", "verification_hints": {"test_commands": ["pytest"]}}]
        result = check_verification_hints(stories)
        assert len(result.warnings) == 0

    def test_check_verification_hints_missing(self):
        """Test with missing verification hints."""
        stories = [{"id": "s1"}]
        result = check_verification_hints(stories)
        assert len(result.warnings) >= 1

    def test_check_risk_tags_no_risk(self):
        """Test story without risky keywords."""
        stories = [{"id": "s1", "title": "Add logging", "description": "Add logs"}]
        result = check_risk_tags(stories)
        assert len(result.warnings) == 0

    def test_check_risk_tags_risky_no_tag(self):
        """Test risky story without risk tags."""
        stories = [{
            "id": "s1",
            "title": "Database migration",
            "description": "Delete old tables",
            "risks": [],
        }]
        result = check_risk_tags(stories)
        assert len(result.warnings) >= 1


class TestMegaChecks:
    """Tests for Mega DoR checks."""

    def test_check_feature_count_optimal(self):
        """Test optimal feature count (2-6)."""
        features = [{"id": f"f{i}"} for i in range(3)]
        result = check_feature_count(features)
        assert len(result.warnings) == 0
        assert result.details["feature_count"] == 3

    def test_check_feature_count_too_few(self):
        """Test with too few features."""
        features = [{"id": "f1"}]
        result = check_feature_count(features)
        assert len(result.warnings) >= 1

    def test_check_feature_count_too_many(self):
        """Test with too many features."""
        features = [{"id": f"f{i}"} for i in range(8)]
        result = check_feature_count(features)
        assert len(result.warnings) >= 1

    def test_check_dependency_dag_valid(self):
        """Test valid feature DAG."""
        features = [
            {"id": "f1", "dependencies": []},
            {"id": "f2", "dependencies": ["f1"]},
        ]
        result = check_dependency_dag(features)
        assert result.passed is True
        assert result.details.get("batch_1_count") == 1

    def test_check_dependency_dag_cycle(self):
        """Test cycle detection in feature DAG."""
        features = [
            {"id": "f1", "dependencies": ["f2"]},
            {"id": "f2", "dependencies": ["f1"]},
        ]
        result = check_dependency_dag(features)
        assert result.passed is False
        assert any("Circular" in e for e in result.errors)

    def test_check_feature_boundaries_valid(self):
        """Test features with clear boundaries."""
        features = [{"id": "f1", "description": "User authentication with OAuth2 support"}]
        result = check_feature_boundaries(features)
        assert result.passed is True

    def test_check_feature_boundaries_no_description(self):
        """Test feature without description."""
        features = [{"id": "f1"}]
        result = check_feature_boundaries(features)
        assert result.passed is False

    def test_check_feature_boundaries_short(self):
        """Test feature with very short description."""
        features = [{"id": "f1", "description": "Short"}]
        result = check_feature_boundaries(features)
        assert len(result.warnings) >= 1


class TestDirectChecks:
    """Tests for Direct DoR checks."""

    def test_check_blast_radius_contained(self):
        """Test contained blast radius."""
        result = check_blast_radius("Fix bug in user.py file")
        assert len(result.warnings) == 0

    def test_check_blast_radius_high(self):
        """Test high blast radius detection."""
        result = check_blast_radius("Refactor all files in the entire codebase")
        assert len(result.warnings) >= 1
        assert "blast_indicators" in result.details

    def test_check_rollback_hint_safe(self):
        """Test non-destructive operation."""
        result = check_rollback_hint("Add new feature")
        assert len(result.suggestions) == 0

    def test_check_rollback_hint_destructive(self):
        """Test destructive operation detection."""
        result = check_rollback_hint("Delete old user records and clear cache")
        assert len(result.suggestions) >= 1

    def test_check_test_requirements_not_needed(self):
        """Test task not requiring tests."""
        result = check_test_requirements("Fix typo in README", has_tests=False)
        assert len(result.suggestions) == 0

    def test_check_test_requirements_suggested(self):
        """Test task suggesting tests."""
        result = check_test_requirements("Implement new API endpoint", has_tests=False)
        assert len(result.suggestions) >= 1


class TestReadinessGate:
    """Tests for ReadinessGate class."""

    def test_init_default_mode(self):
        """Test default initialization."""
        gate = ReadinessGate()
        assert gate.mode == GateMode.SOFT

    def test_init_hard_mode(self):
        """Test hard mode initialization."""
        gate = ReadinessGate(GateMode.HARD)
        assert gate.mode == GateMode.HARD

    def test_from_flow_quick(self):
        """Test gate from quick flow."""
        gate = ReadinessGate.from_flow("quick")
        assert gate.mode == GateMode.SOFT

    def test_from_flow_standard(self):
        """Test gate from standard flow."""
        gate = ReadinessGate.from_flow("standard")
        assert gate.mode == GateMode.SOFT

    def test_from_flow_full(self):
        """Test gate from full flow."""
        gate = ReadinessGate.from_flow("full")
        assert gate.mode == GateMode.HARD

    def test_check_prd_soft_mode(self):
        """Test PRD check in soft mode converts errors to warnings."""
        prd = {"stories": [{"id": "s1", "acceptance_criteria": []}]}
        gate = ReadinessGate(GateMode.SOFT)
        result = gate.check_prd(prd)
        assert result.passed is True
        assert any("[Soft]" in w for w in result.warnings)

    def test_check_prd_hard_mode(self):
        """Test PRD check in hard mode blocks on errors."""
        prd = {"stories": [{"id": "s1", "acceptance_criteria": []}]}
        gate = ReadinessGate(GateMode.HARD)
        result = gate.check_prd(prd)
        assert result.passed is False
        assert len(result.errors) >= 1

    def test_check_mega_soft_mode(self):
        """Test Mega check in soft mode."""
        mega_plan = {"features": [{"id": "f1", "description": ""}]}
        gate = ReadinessGate(GateMode.SOFT)
        result = gate.check_mega(mega_plan)
        assert result.passed is True

    def test_check_mega_hard_mode(self):
        """Test Mega check in hard mode."""
        mega_plan = {"features": [{"id": "f1", "description": ""}]}
        gate = ReadinessGate(GateMode.HARD)
        result = gate.check_mega(mega_plan)
        assert result.passed is False

    def test_check_direct(self):
        """Test Direct check."""
        gate = ReadinessGate(GateMode.SOFT)
        result = gate.check_direct("Fix small bug")
        assert result.check_name == "direct_readiness"


class TestGateMode:
    """Tests for GateMode enum."""

    def test_soft_value(self):
        """Test SOFT enum value."""
        assert GateMode.SOFT.value == "soft"

    def test_hard_value(self):
        """Test HARD enum value."""
        assert GateMode.HARD.value == "hard"
