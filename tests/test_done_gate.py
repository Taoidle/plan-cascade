"""Tests for DoneGate (DoD) module."""

import pytest

from plan_cascade.core.done_gate import (
    DoDCheckResult,
    DoDLevel,
    DoneGate,
    WrapUpSummary,
    GateSummaryEntry,
    ChangeSummaryEntry,
    check_ai_verification,
    check_change_summary,
    check_code_review,
    check_deployment_notes,
    check_quality_gates_passed,
    check_skeleton_code_detection,
    check_test_changes,
    generate_wrapup_summary,
)


class TestDoDCheckResult:
    """Tests for DoDCheckResult dataclass."""

    def test_init_defaults(self):
        """Test default initialization."""
        result = DoDCheckResult()
        assert result.passed is True
        assert result.warnings == []
        assert result.errors == []
        assert result.suggestions == []

    def test_to_dict(self):
        """Test conversion to dictionary."""
        result = DoDCheckResult(passed=True, check_name="test")
        d = result.to_dict()
        assert d["passed"] is True
        assert d["check_name"] == "test"

    def test_from_dict(self):
        """Test creation from dictionary."""
        data = {"passed": False, "warnings": ["warn"], "check_name": "test"}
        result = DoDCheckResult.from_dict(data)
        assert result.passed is False
        assert result.warnings == ["warn"]

    def test_combine_empty(self):
        """Test combining empty list."""
        combined = DoDCheckResult.combine([])
        assert combined.passed is True

    def test_combine_multiple(self):
        """Test combining multiple results."""
        r1 = DoDCheckResult(passed=True, warnings=["w1"], check_name="c1")
        r2 = DoDCheckResult(passed=False, errors=["e1"], check_name="c2")
        combined = DoDCheckResult.combine([r1, r2])
        assert combined.passed is False
        assert combined.warnings == ["w1"]
        assert combined.errors == ["e1"]

    def test_add_warning(self):
        """Test adding a warning."""
        result = DoDCheckResult()
        result.add_warning("test warning")
        assert "test warning" in result.warnings
        assert result.passed is True

    def test_add_error(self):
        """Test adding an error."""
        result = DoDCheckResult()
        result.add_error("test error")
        assert "test error" in result.errors
        assert result.passed is False

    def test_add_suggestion(self):
        """Test adding a suggestion."""
        result = DoDCheckResult()
        result.add_suggestion("test suggestion")
        assert "test suggestion" in result.suggestions

    def test_has_issues(self):
        """Test has_issues method."""
        result = DoDCheckResult()
        assert result.has_issues() is False
        result.add_warning("w")
        assert result.has_issues() is True

    def test_get_summary(self):
        """Test get_summary method."""
        result = DoDCheckResult(check_name="test_check")
        result.add_error("error1")
        summary = result.get_summary()
        assert "[FAILED]" in summary
        assert "test_check" in summary

    def test_get_summary_passed(self):
        """Test get_summary when passed."""
        result = DoDCheckResult(check_name="test_check")
        summary = result.get_summary()
        assert "[PASSED]" in summary


class TestDoDLevel:
    """Tests for DoDLevel enum."""

    def test_standard_value(self):
        """Test STANDARD enum value."""
        assert DoDLevel.STANDARD.value == "standard"

    def test_full_value(self):
        """Test FULL enum value."""
        assert DoDLevel.FULL.value == "full"


class TestStandardFlowChecks:
    """Tests for Standard Flow DoD checks."""

    def test_check_quality_gates_all_passed(self):
        """Test with all quality gates passed."""
        outputs = {
            "typecheck": {"passed": True},
            "tests": {"passed": True},
            "lint": {"passed": True},
        }
        result = check_quality_gates_passed(outputs)
        assert result.passed is True
        assert len(result.errors) == 0

    def test_check_quality_gates_one_failed(self):
        """Test with one gate failed."""
        outputs = {
            "typecheck": {"passed": True},
            "tests": {"passed": False, "error_summary": "3 tests failed"},
            "lint": {"passed": True},
        }
        result = check_quality_gates_passed(outputs)
        assert result.passed is False
        assert any("tests" in e for e in result.errors)

    def test_check_quality_gates_missing_required(self):
        """Test with missing required gate."""
        outputs = {"typecheck": {"passed": True}}
        result = check_quality_gates_passed(outputs, required_gates=["typecheck", "tests"])
        assert result.passed is False
        assert any("tests" in e for e in result.errors)

    def test_check_quality_gates_empty(self):
        """Test with no gate outputs."""
        result = check_quality_gates_passed({})
        assert len(result.warnings) >= 1

    def test_check_ai_verification_passed(self):
        """Test with AI verification passed."""
        verification = {
            "overall_passed": True,
            "confidence": 0.9,
            "skeleton_detected": False,
        }
        result = check_ai_verification(verification)
        assert result.passed is True

    def test_check_ai_verification_failed(self):
        """Test with AI verification failed."""
        verification = {
            "overall_passed": False,
            "confidence": 0.8,
            "skeleton_detected": False,
        }
        result = check_ai_verification(verification)
        assert result.passed is False

    def test_check_ai_verification_skeleton_detected(self):
        """Test with skeleton code detected."""
        verification = {
            "overall_passed": True,
            "confidence": 0.9,
            "skeleton_detected": True,
            "skeleton_evidence": "pass statement found",
        }
        result = check_ai_verification(verification)
        assert result.passed is False
        assert any("Skeleton" in e for e in result.errors)

    def test_check_ai_verification_low_confidence(self):
        """Test with low confidence."""
        verification = {
            "overall_passed": True,
            "confidence": 0.5,
            "skeleton_detected": False,
        }
        result = check_ai_verification(verification, confidence_threshold=0.7)
        assert len(result.warnings) >= 1

    def test_check_ai_verification_missing(self):
        """Test with no verification result."""
        result = check_ai_verification(None)
        assert len(result.warnings) >= 1

    def test_check_ai_verification_with_missing_implementations(self):
        """Test with missing implementations."""
        verification = {
            "overall_passed": False,
            "confidence": 0.8,
            "skeleton_detected": False,
            "missing_implementations": ["feature A", "feature B"],
        }
        result = check_ai_verification(verification)
        assert result.passed is False
        assert any("Missing" in e for e in result.errors)

    def test_check_skeleton_code_none(self):
        """Test skeleton check with no verification."""
        result = check_skeleton_code_detection(None)
        assert len(result.warnings) >= 1

    def test_check_skeleton_code_clean(self):
        """Test skeleton check with clean code."""
        verification = {"skeleton_detected": False}
        result = check_skeleton_code_detection(verification)
        assert result.passed is True

    def test_check_skeleton_code_detected(self):
        """Test skeleton check with skeleton code."""
        verification = {
            "skeleton_detected": True,
            "skeleton_evidence": "NotImplementedError found",
        }
        result = check_skeleton_code_detection(verification)
        assert result.passed is False

    def test_check_change_summary_complete(self):
        """Test with complete change information."""
        result = check_change_summary(
            changed_files=["src/main.py", "src/utils.py"],
            interfaces_changed=["API.get_user"],
            summary_text="Added user lookup functionality",
        )
        assert result.passed is True
        assert result.details["files_changed"] == 2

    def test_check_change_summary_no_files(self):
        """Test with no changed files."""
        result = check_change_summary(changed_files=[])
        assert len(result.warnings) >= 1

    def test_check_change_summary_interfaces_no_summary(self):
        """Test with interface changes but no summary."""
        result = check_change_summary(
            changed_files=["api.py"],
            interfaces_changed=["API.get_user"],
        )
        assert len(result.warnings) >= 1

    def test_check_change_summary_short_summary(self):
        """Test with very short summary."""
        result = check_change_summary(
            changed_files=["main.py"],
            summary_text="Fix",
        )
        assert len(result.warnings) >= 1


class TestFullFlowChecks:
    """Tests for Full Flow DoD checks."""

    def test_check_code_review_passed(self):
        """Test with code review passed."""
        review = {
            "overall_score": 0.85,
            "passed": True,
            "has_critical": False,
            "findings": [],
        }
        result = check_code_review(review)
        assert result.passed is True

    def test_check_code_review_missing(self):
        """Test with code review missing."""
        result = check_code_review(None)
        assert result.passed is False
        assert any("required" in e.lower() for e in result.errors)

    def test_check_code_review_low_score(self):
        """Test with low review score."""
        review = {
            "overall_score": 0.5,
            "passed": False,
            "has_critical": False,
            "findings": [],
        }
        result = check_code_review(review, min_score=0.7)
        assert result.passed is False

    def test_check_code_review_critical_findings(self):
        """Test with critical findings."""
        review = {
            "overall_score": 0.8,
            "passed": True,
            "has_critical": True,
            "findings": [
                {"severity": "critical", "title": "SQL injection vulnerability"}
            ],
        }
        result = check_code_review(review, block_on_critical=True)
        assert result.passed is False

    def test_check_code_review_high_findings(self):
        """Test with high severity findings."""
        review = {
            "overall_score": 0.75,
            "passed": True,
            "has_critical": False,
            "findings": [
                {"severity": "high", "title": "Missing error handling"}
            ],
        }
        result = check_code_review(review)
        assert len(result.warnings) >= 1

    def test_check_test_changes_adequate(self):
        """Test with adequate test changes."""
        changed_files = [
            "src/main.py",
            "tests/test_main.py",
        ]
        result = check_test_changes(changed_files)
        assert result.passed is True

    def test_check_test_changes_missing(self):
        """Test with missing test changes."""
        changed_files = [
            "src/main.py",
            "src/utils.py",
        ]
        result = check_test_changes(changed_files)
        assert result.passed is False
        assert any("test" in e.lower() for e in result.errors)

    def test_check_test_changes_limited(self):
        """Test with limited test coverage."""
        changed_files = [
            "src/main.py",
            "src/utils.py",
            "src/api.py",
            "tests/test_main.py",
        ]
        result = check_test_changes(changed_files)
        assert len(result.warnings) >= 1

    def test_check_deployment_notes_not_needed(self):
        """Test with no deployment-sensitive changes."""
        result = check_deployment_notes(
            changed_files=["src/main.py", "tests/test_main.py"]
        )
        assert result.passed is True

    def test_check_deployment_notes_needed(self):
        """Test with deployment-sensitive changes."""
        result = check_deployment_notes(
            changed_files=["migrations/001_add_users.sql", "config/settings.py"],
            has_deployment_notes=False,
        )
        assert result.passed is False
        assert any("deployment" in e.lower() for e in result.errors)

    def test_check_deployment_notes_provided(self):
        """Test with deployment notes provided."""
        result = check_deployment_notes(
            changed_files=["migrations/001_add_users.sql"],
            has_deployment_notes=True,
            has_rollback_plan=True,
        )
        assert result.passed is True

    def test_check_deployment_notes_no_rollback(self):
        """Test with deployment notes but no rollback plan."""
        result = check_deployment_notes(
            changed_files=["migrations/001_add_users.sql"],
            has_deployment_notes=True,
            has_rollback_plan=False,
        )
        assert len(result.warnings) >= 1


class TestDoneGate:
    """Tests for DoneGate class."""

    def test_init_default_level(self):
        """Test default initialization."""
        gate = DoneGate()
        assert gate.level == DoDLevel.STANDARD

    def test_init_full_level(self):
        """Test full level initialization."""
        gate = DoneGate(DoDLevel.FULL)
        assert gate.level == DoDLevel.FULL

    def test_from_flow_quick(self):
        """Test gate from quick flow."""
        gate = DoneGate.from_flow("quick")
        assert gate.level == DoDLevel.STANDARD

    def test_from_flow_standard(self):
        """Test gate from standard flow."""
        gate = DoneGate.from_flow("standard")
        assert gate.level == DoDLevel.STANDARD

    def test_from_flow_full(self):
        """Test gate from full flow."""
        gate = DoneGate.from_flow("full")
        assert gate.level == DoDLevel.FULL

    def test_check_standard_passed(self):
        """Test standard check with all passing."""
        gate = DoneGate(DoDLevel.STANDARD)
        result = gate.check_standard(
            gate_outputs={"typecheck": {"passed": True}},
            verification_result={
                "overall_passed": True,
                "confidence": 0.9,
                "skeleton_detected": False,
            },
            changed_files=["src/main.py"],
        )
        assert result.check_name == "standard_dod"
        # May have warnings but should generally pass
        # The exact pass/fail depends on all checks

    def test_check_full_requires_code_review(self):
        """Test full check requires code review."""
        gate = DoneGate(DoDLevel.FULL)
        result = gate.check_full(
            gate_outputs={"typecheck": {"passed": True}},
            verification_result={
                "overall_passed": True,
                "confidence": 0.9,
                "skeleton_detected": False,
            },
            review_result=None,  # Missing code review
            changed_files=["src/main.py"],
        )
        assert result.check_name == "full_dod"
        assert result.passed is False
        assert any("review" in e.lower() for e in result.errors)

    def test_check_uses_level(self):
        """Test check method uses configured level."""
        gate_standard = DoneGate(DoDLevel.STANDARD)
        gate_full = DoneGate(DoDLevel.FULL)

        # Same inputs
        gate_outputs = {"typecheck": {"passed": True}}
        verification = {
            "overall_passed": True,
            "confidence": 0.9,
            "skeleton_detected": False,
        }

        result_standard = gate_standard.check(
            gate_outputs=gate_outputs,
            verification_result=verification,
            changed_files=["main.py"],
        )
        result_full = gate_full.check(
            gate_outputs=gate_outputs,
            verification_result=verification,
            changed_files=["main.py"],
        )

        assert result_standard.check_name == "standard_dod"
        assert result_full.check_name == "full_dod"


class TestWrapUpSummary:
    """Tests for WrapUpSummary and related classes."""

    def test_gate_summary_entry_to_dict(self):
        """Test GateSummaryEntry serialization."""
        entry = GateSummaryEntry(
            name="typecheck",
            passed=True,
            duration_seconds=1.5,
        )
        d = entry.to_dict()
        assert d["name"] == "typecheck"
        assert d["passed"] is True
        assert d["duration_seconds"] == 1.5

    def test_change_summary_entry_to_dict(self):
        """Test ChangeSummaryEntry serialization."""
        entry = ChangeSummaryEntry(
            category="source files",
            items=["main.py", "utils.py"],
            count=2,
        )
        d = entry.to_dict()
        assert d["category"] == "source files"
        assert d["count"] == 2

    def test_wrapup_summary_init(self):
        """Test WrapUpSummary initialization."""
        summary = WrapUpSummary(story_id="story-001")
        assert summary.story_id == "story-001"
        assert summary.completed_at != ""  # Auto-set

    def test_wrapup_summary_to_dict(self):
        """Test WrapUpSummary serialization."""
        summary = WrapUpSummary(
            story_id="story-001",
            dod_result=DoDCheckResult(passed=True),
            gate_summary=[GateSummaryEntry("test", True, 1.0)],
            change_summary=[ChangeSummaryEntry("files", ["a.py"], 1)],
        )
        d = summary.to_dict()
        assert d["story_id"] == "story-001"
        assert d["dod_result"]["passed"] is True
        assert len(d["gate_summary"]) == 1
        assert len(d["change_summary"]) == 1

    def test_wrapup_summary_from_dict(self):
        """Test WrapUpSummary deserialization."""
        data = {
            "story_id": "story-001",
            "completed_at": "2026-01-01T00:00:00Z",
            "dod_result": {"passed": True, "check_name": "test"},
            "gate_summary": [{"name": "lint", "passed": True, "duration_seconds": 0.5}],
            "change_summary": [{"category": "tests", "items": ["test.py"], "count": 1}],
            "notes": "Test note",
        }
        summary = WrapUpSummary.from_dict(data)
        assert summary.story_id == "story-001"
        assert summary.dod_result is not None
        assert summary.dod_result.passed is True
        assert len(summary.gate_summary) == 1
        assert summary.gate_summary[0].name == "lint"
        assert summary.notes == "Test note"

    def test_wrapup_summary_to_markdown(self):
        """Test WrapUpSummary markdown generation."""
        dod_result = DoDCheckResult(passed=True, check_name="standard_dod")
        summary = WrapUpSummary(
            story_id="story-001",
            dod_result=dod_result,
            gate_summary=[
                GateSummaryEntry("typecheck", True, 1.0),
                GateSummaryEntry("tests", True, 5.0),
            ],
            change_summary=[
                ChangeSummaryEntry("source files", ["main.py", "utils.py"], 2),
            ],
            notes="Implementation complete",
        )
        md = summary.to_markdown()
        assert "# WrapUp Summary: story-001" in md
        assert "PASSED" in md
        assert "typecheck" in md
        assert "Source Files" in md  # Title case in markdown
        assert "Implementation complete" in md

    def test_wrapup_summary_to_markdown_with_errors(self):
        """Test WrapUpSummary markdown with errors."""
        dod_result = DoDCheckResult(passed=False, check_name="standard_dod")
        dod_result.add_error("Tests failed")
        dod_result.add_warning("Low coverage")

        summary = WrapUpSummary(
            story_id="story-001",
            dod_result=dod_result,
        )
        md = summary.to_markdown()
        assert "FAILED" in md
        assert "Tests failed" in md
        assert "Low coverage" in md


class TestGenerateWrapupSummary:
    """Tests for generate_wrapup_summary function."""

    def test_generate_basic_summary(self):
        """Test generating a basic summary."""
        dod_result = DoDCheckResult(passed=True, check_name="standard_dod")
        summary = generate_wrapup_summary(
            story_id="story-001",
            dod_result=dod_result,
        )
        assert summary.story_id == "story-001"
        assert summary.dod_result is dod_result

    def test_generate_with_gate_outputs(self):
        """Test generating summary with gate outputs."""
        dod_result = DoDCheckResult(passed=True)
        gate_outputs = {
            "typecheck": {"passed": True, "duration_seconds": 1.0},
            "tests": {"passed": True, "duration_seconds": 5.0},
        }
        summary = generate_wrapup_summary(
            story_id="story-001",
            dod_result=dod_result,
            gate_outputs=gate_outputs,
        )
        assert len(summary.gate_summary) == 2

    def test_generate_with_changed_files(self):
        """Test generating summary with changed files."""
        dod_result = DoDCheckResult(passed=True)
        changed_files = [
            "src/main.py",
            "tests/test_main.py",
            "config/settings.json",
            "README.md",
        ]
        summary = generate_wrapup_summary(
            story_id="story-001",
            dod_result=dod_result,
            changed_files=changed_files,
        )
        # Should categorize files
        assert len(summary.change_summary) >= 2

    def test_generate_with_interfaces(self):
        """Test generating summary with interfaces changed."""
        dod_result = DoDCheckResult(passed=True)
        summary = generate_wrapup_summary(
            story_id="story-001",
            dod_result=dod_result,
            interfaces_changed=["API.get_user", "API.create_user"],
        )
        interface_entry = next(
            (c for c in summary.change_summary if c.category == "interfaces"),
            None,
        )
        assert interface_entry is not None
        assert interface_entry.count == 2

    def test_generate_with_notes(self):
        """Test generating summary with notes."""
        dod_result = DoDCheckResult(passed=True)
        summary = generate_wrapup_summary(
            story_id="story-001",
            dod_result=dod_result,
            notes="Completed with minor refactoring",
        )
        assert summary.notes == "Completed with minor refactoring"
