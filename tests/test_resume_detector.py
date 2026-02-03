"""Tests for Resume Detector module."""

import json
import pytest
from pathlib import Path

from plan_cascade.state.resume_detector import (
    ResumeReason,
    IncompleteStateInfo,
    ResumeSuggestion,
    detect_incomplete_state,
    get_resume_suggestion,
    format_resume_display,
    check_and_suggest_resume,
)
from plan_cascade.state.path_resolver import PathResolver
from plan_cascade.core.stage_state import (
    StageStateMachine,
    ExecutionStage,
    StageStatus,
)


class TestResumeReason:
    """Tests for ResumeReason enum."""

    def test_all_reasons_defined(self):
        """Test that all resume reasons are defined."""
        assert ResumeReason.STAGE_IN_PROGRESS.value == "stage_in_progress"
        assert ResumeReason.STAGE_FAILED.value == "stage_failed"
        assert ResumeReason.EXECUTION_INCOMPLETE.value == "execution_incomplete"
        assert ResumeReason.PRD_NEEDS_APPROVAL.value == "prd_needs_approval"
        assert ResumeReason.MEGA_PLAN_INCOMPLETE.value == "mega_plan_incomplete"


class TestIncompleteStateInfo:
    """Tests for IncompleteStateInfo dataclass."""

    def test_default_initialization(self):
        """Test IncompleteStateInfo with default values."""
        info = IncompleteStateInfo()

        assert info.execution_id == ""
        assert info.strategy is None
        assert info.flow is None
        assert info.last_stage is None
        assert info.last_stage_status is None
        assert info.timestamp == ""
        assert info.completed_work == []
        assert info.suggested_resume_point is None
        assert info.resume_reason is None
        assert info.progress_percent == 0
        assert info.completed_stages == []
        assert info.failed_stages == []
        assert info.error_messages == []

    def test_full_initialization(self):
        """Test IncompleteStateInfo with all values."""
        info = IncompleteStateInfo(
            execution_id="test-001",
            strategy="HYBRID_AUTO",
            flow="standard",
            last_stage="execute",
            last_stage_status="in_progress",
            timestamp="2026-02-03T10:00:00Z",
            completed_work=["intake: context", "analyze: strategy"],
            suggested_resume_point="execute",
            resume_reason=ResumeReason.STAGE_IN_PROGRESS,
            progress_percent=50,
            completed_stages=["intake", "analyze", "plan"],
            failed_stages=[],
            error_messages=[],
        )

        assert info.execution_id == "test-001"
        assert info.strategy == "HYBRID_AUTO"
        assert info.flow == "standard"
        assert info.last_stage == "execute"
        assert info.resume_reason == ResumeReason.STAGE_IN_PROGRESS
        assert info.progress_percent == 50

    def test_to_dict(self):
        """Test JSON serialization."""
        info = IncompleteStateInfo(
            execution_id="test-001",
            strategy="HYBRID_AUTO",
            resume_reason=ResumeReason.STAGE_FAILED,
            progress_percent=25,
        )

        data = info.to_dict()

        assert data["execution_id"] == "test-001"
        assert data["strategy"] == "HYBRID_AUTO"
        assert data["resume_reason"] == "stage_failed"
        assert data["progress_percent"] == 25

    def test_from_dict(self):
        """Test JSON deserialization."""
        data = {
            "execution_id": "test-002",
            "strategy": "MEGA_PLAN",
            "flow": "full",
            "last_stage": "design",
            "last_stage_status": "failed",
            "timestamp": "2026-02-03T11:00:00Z",
            "completed_work": ["intake: done"],
            "suggested_resume_point": "design",
            "resume_reason": "stage_failed",
            "progress_percent": 37,
            "completed_stages": ["intake", "analyze", "plan"],
            "failed_stages": ["design"],
            "error_messages": ["Design validation failed"],
        }

        info = IncompleteStateInfo.from_dict(data)

        assert info.execution_id == "test-002"
        assert info.strategy == "MEGA_PLAN"
        assert info.resume_reason == ResumeReason.STAGE_FAILED
        assert info.progress_percent == 37
        assert "Design validation failed" in info.error_messages

    def test_from_dict_with_invalid_reason(self):
        """Test from_dict handles invalid resume reason gracefully."""
        data = {
            "execution_id": "test-003",
            "resume_reason": "invalid_reason",
        }

        info = IncompleteStateInfo.from_dict(data)

        assert info.resume_reason is None

    def test_has_incomplete_state(self):
        """Test has_incomplete_state check."""
        empty = IncompleteStateInfo()
        assert not empty.has_incomplete_state()

        with_stage = IncompleteStateInfo(last_stage="execute")
        assert with_stage.has_incomplete_state()

        with_progress = IncompleteStateInfo(progress_percent=10)
        assert with_progress.has_incomplete_state()

    def test_is_failed(self):
        """Test is_failed check."""
        not_failed = IncompleteStateInfo()
        assert not not_failed.is_failed()

        failed = IncompleteStateInfo(failed_stages=["execute"])
        assert failed.is_failed()

    def test_time_since_last_activity(self):
        """Test time_since_last_activity formatting."""
        # No timestamp
        no_time = IncompleteStateInfo()
        assert no_time.time_since_last_activity() is None

        # Invalid timestamp
        invalid = IncompleteStateInfo(timestamp="invalid")
        assert invalid.time_since_last_activity() is None


class TestResumeSuggestion:
    """Tests for ResumeSuggestion dataclass."""

    def test_basic_suggestion(self):
        """Test basic suggestion creation."""
        suggestion = ResumeSuggestion(
            title="Resume Execution",
            message="Execution was interrupted.",
            command="/plan-cascade:resume",
        )

        assert suggestion.title == "Resume Execution"
        assert suggestion.command == "/plan-cascade:resume"
        assert suggestion.priority == 1
        assert not suggestion.can_auto_resume

    def test_suggestion_with_details(self):
        """Test suggestion with all fields."""
        suggestion = ResumeSuggestion(
            title="Stage In Progress",
            message="Execution was interrupted during execute stage.",
            command="/plan-cascade:resume",
            details=["Completed: intake", "Completed: analyze", "Completed: plan"],
            priority=1,
            can_auto_resume=True,
        )

        assert len(suggestion.details) == 3
        assert suggestion.can_auto_resume

    def test_to_dict_from_dict(self):
        """Test serialization roundtrip."""
        original = ResumeSuggestion(
            title="Test",
            message="Test message",
            command="test-command",
            details=["detail1", "detail2"],
            priority=2,
            can_auto_resume=True,
        )

        data = original.to_dict()
        restored = ResumeSuggestion.from_dict(data)

        assert restored.title == original.title
        assert restored.command == original.command
        assert restored.details == original.details
        assert restored.can_auto_resume == original.can_auto_resume

    def test_format_display(self):
        """Test format_display output."""
        suggestion = ResumeSuggestion(
            title="Resume Test",
            message="Test message",
            command="/plan-cascade:resume",
            details=["Completed: stage1", "Completed: stage2"],
            can_auto_resume=True,
        )

        output = suggestion.format_display()

        assert "## Resume Test" in output
        assert "Test message" in output
        assert "/plan-cascade:resume" in output
        assert "Completed: stage1" in output
        assert "Auto-resume is safe" in output


class TestDetectIncompleteState:
    """Tests for detect_incomplete_state function."""

    def test_no_state_file(self, tmp_path: Path):
        """Test detection when no state file exists."""
        resolver = PathResolver(
            project_root=tmp_path,
            legacy_mode=True,
        )

        result = detect_incomplete_state(resolver, include_prd_check=False)

        assert result is None

    def test_detect_in_progress_stage(self, tmp_path: Path):
        """Test detecting an in-progress stage."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create a state machine with in-progress stage
        machine = StageStateMachine(
            execution_id="test-001",
            strategy="hybrid_auto",
            flow="standard",
        )

        # Complete some stages
        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {}, "task_normalized": "test"
        })

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.complete_stage(ExecutionStage.ANALYZE, outputs={
            "strategy": "hybrid", "flow": "standard", "confidence": 0.9, "reasoning": "test"
        })

        # Start but don't complete PLAN
        machine.start_stage(ExecutionStage.PLAN)

        # Save the state
        machine.save_state(resolver)

        # Detect incomplete state
        result = detect_incomplete_state(resolver)

        assert result is not None
        assert result.execution_id == "test-001"
        assert result.strategy == "hybrid_auto"
        assert result.last_stage == "plan"
        assert result.last_stage_status == "in_progress"
        assert result.resume_reason == ResumeReason.STAGE_IN_PROGRESS
        assert "intake" in result.completed_stages
        assert "analyze" in result.completed_stages

    def test_detect_failed_stage(self, tmp_path: Path):
        """Test detecting a failed stage."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create a state machine with failed stage
        machine = StageStateMachine(
            execution_id="test-002",
            strategy="mega_plan",
            flow="full",
        )

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {}, "task_normalized": "test"
        })

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.fail_stage(ExecutionStage.ANALYZE, errors=["Analysis failed: timeout"])

        machine.save_state(resolver)

        # Detect incomplete state
        result = detect_incomplete_state(resolver)

        assert result is not None
        assert result.last_stage == "analyze"
        assert result.last_stage_status == "failed"
        assert result.resume_reason == ResumeReason.STAGE_FAILED
        assert "analyze" in result.failed_stages
        assert "Analysis failed: timeout" in result.error_messages

    def test_detect_complete_execution_returns_none(self, tmp_path: Path):
        """Test that complete execution returns None."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create a state machine and complete all stages
        machine = StageStateMachine(execution_id="test-003")

        for stage in ExecutionStage.get_order():
            machine.start_stage(stage)
            machine.complete_stage(stage, outputs={})

        machine.save_state(resolver)

        # Detect incomplete state
        result = detect_incomplete_state(resolver)

        assert result is None

    def test_detect_prd_needs_approval(self, tmp_path: Path):
        """Test detecting PRD that needs approval."""
        resolver = PathResolver(
            project_root=tmp_path,
            legacy_mode=True,
        )

        # Create a PRD file with pending stories (no execution started)
        prd = {
            "metadata": {"created_at": "2026-02-03T10:00:00Z"},
            "goal": "Test goal",
            "stories": [
                {"id": "story-001", "title": "Story 1", "status": "pending"},
                {"id": "story-002", "title": "Story 2", "status": "pending"},
            ],
        }

        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        # Detect - with PRD check enabled
        result = detect_incomplete_state(resolver, include_prd_check=True)

        assert result is not None
        assert result.strategy == "HYBRID_AUTO"
        assert result.resume_reason == ResumeReason.PRD_NEEDS_APPROVAL
        assert result.suggested_resume_point == "approve"

    def test_detect_mega_plan_incomplete(self, tmp_path: Path):
        """Test detecting incomplete mega-plan."""
        resolver = PathResolver(
            project_root=tmp_path,
            legacy_mode=True,
        )

        # Create a mega-plan with some completed features
        mega_plan = {
            "metadata": {"created_at": "2026-02-03T10:00:00Z"},
            "goal": "Test mega plan",
            "features": [
                {"id": "feature-001", "title": "Feature 1", "status": "complete"},
                {"id": "feature-002", "title": "Feature 2", "status": "pending"},
                {"id": "feature-003", "title": "Feature 3", "status": "pending"},
            ],
        }

        mega_path = tmp_path / "mega-plan.json"
        with open(mega_path, "w") as f:
            json.dump(mega_plan, f)

        # Detect - with PRD check enabled
        result = detect_incomplete_state(resolver, include_prd_check=True)

        assert result is not None
        assert result.strategy == "MEGA_PLAN"
        assert result.resume_reason == ResumeReason.MEGA_PLAN_INCOMPLETE
        assert result.progress_percent == 33  # 1/3 complete
        assert "Feature: Feature 1" in result.completed_work


class TestGetResumeSuggestion:
    """Tests for get_resume_suggestion function."""

    def test_suggestion_for_prd_needs_approval(self):
        """Test suggestion for PRD that needs approval."""
        info = IncompleteStateInfo(
            strategy="HYBRID_AUTO",
            resume_reason=ResumeReason.PRD_NEEDS_APPROVAL,
        )

        suggestion = get_resume_suggestion(info)

        assert "PRD Ready" in suggestion.title
        assert "/plan-cascade:approve" in suggestion.command
        assert suggestion.can_auto_resume

    def test_suggestion_for_mega_plan_incomplete(self):
        """Test suggestion for incomplete mega-plan."""
        info = IncompleteStateInfo(
            strategy="MEGA_PLAN",
            resume_reason=ResumeReason.MEGA_PLAN_INCOMPLETE,
            progress_percent=50,
            completed_work=["Feature: Auth", "Feature: Products"],
        )

        suggestion = get_resume_suggestion(info)

        assert "Mega Plan" in suggestion.title
        assert "mega-resume" in suggestion.command
        assert "50%" in suggestion.message
        assert suggestion.can_auto_resume

    def test_suggestion_for_stage_failed(self):
        """Test suggestion for failed stage."""
        info = IncompleteStateInfo(
            strategy="HYBRID_AUTO",
            last_stage="execute",
            resume_reason=ResumeReason.STAGE_FAILED,
            error_messages=["Test execution failed: assertion error"],
            completed_work=["intake: done", "analyze: done"],
        )

        suggestion = get_resume_suggestion(info)

        assert "Failed" in suggestion.title
        assert "execute" in suggestion.title
        assert "/plan-cascade:resume" in suggestion.command
        assert not suggestion.can_auto_resume  # Failed state needs review

    def test_suggestion_for_stage_in_progress(self):
        """Test suggestion for in-progress stage."""
        info = IncompleteStateInfo(
            strategy="HYBRID_AUTO",
            last_stage="plan",
            resume_reason=ResumeReason.STAGE_IN_PROGRESS,
            progress_percent=25,
            timestamp="2026-02-03T10:00:00Z",
        )

        suggestion = get_resume_suggestion(info)

        assert "In Progress" in suggestion.title
        assert "plan" in suggestion.title
        assert "/plan-cascade:resume" in suggestion.command
        assert "25%" in suggestion.message
        assert suggestion.can_auto_resume

    def test_suggestion_for_execution_incomplete(self):
        """Test suggestion for generic execution incomplete."""
        info = IncompleteStateInfo(
            strategy="HYBRID_AUTO",
            resume_reason=ResumeReason.EXECUTION_INCOMPLETE,
            progress_percent=62,
            suggested_resume_point="verify_review",
        )

        suggestion = get_resume_suggestion(info)

        assert "Incomplete" in suggestion.title
        assert "hybrid-resume" in suggestion.command
        assert "62%" in suggestion.message
        assert suggestion.can_auto_resume


class TestFormatResumeDisplay:
    """Tests for format_resume_display function."""

    def test_display_no_state(self):
        """Test display when no incomplete state."""
        output = format_resume_display(None)

        assert "No incomplete execution detected" in output
        assert "/plan-cascade:auto" in output

    def test_display_with_state(self):
        """Test display with incomplete state."""
        info = IncompleteStateInfo(
            strategy="HYBRID_AUTO",
            flow="standard",
            progress_percent=50,
            completed_stages=["intake", "analyze", "plan"],
            failed_stages=["execute"],
            error_messages=["Test failed"],
            resume_reason=ResumeReason.STAGE_FAILED,
            last_stage="execute",
        )

        output = format_resume_display(info)

        assert "PLAN CASCADE - RESUME DETECTION" in output
        assert "Strategy:  HYBRID_AUTO" in output
        assert "Progress:  50%" in output
        assert "[OK] intake" in output
        assert "[X] execute" in output
        assert "Test failed" in output

    def test_display_with_pregenerated_suggestion(self):
        """Test display with pre-generated suggestion."""
        info = IncompleteStateInfo(
            strategy="MEGA_PLAN",
            progress_percent=75,
        )

        suggestion = ResumeSuggestion(
            title="Custom Title",
            message="Custom message",
            command="/custom-command",
            can_auto_resume=True,
        )

        output = format_resume_display(info, suggestion)

        assert "Custom Title" in output
        assert "Custom message" in output
        assert "/custom-command" in output


class TestCheckAndSuggestResume:
    """Tests for check_and_suggest_resume convenience function."""

    def test_no_state_returns_none_tuple(self, tmp_path: Path):
        """Test that no state returns (None, None)."""
        incomplete, suggestion = check_and_suggest_resume(
            project_root=tmp_path,
            legacy_mode=True,
        )

        assert incomplete is None
        assert suggestion is None

    def test_with_state_returns_both(self, tmp_path: Path):
        """Test that incomplete state returns both info and suggestion."""
        # Create a PRD that needs approval
        prd = {
            "metadata": {"created_at": "2026-02-03T10:00:00Z"},
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Story 1", "status": "pending"},
            ],
        }

        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        incomplete, suggestion = check_and_suggest_resume(
            project_root=tmp_path,
            legacy_mode=True,
        )

        assert incomplete is not None
        assert suggestion is not None
        assert incomplete.resume_reason == ResumeReason.PRD_NEEDS_APPROVAL
        assert "/plan-cascade:approve" in suggestion.command


class TestIntegrationWithContextRecovery:
    """Integration tests with ContextRecoveryManager."""

    def test_context_recovery_includes_stage_info(self, tmp_path: Path):
        """Test that ContextRecoveryManager includes stage state info."""
        from plan_cascade.state.context_recovery import ContextRecoveryManager

        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        # Create state machine with incomplete execution
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        machine = StageStateMachine(
            execution_id="integration-test",
            strategy="hybrid_auto",
            flow="standard",
        )

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {}, "task_normalized": "test"
        })

        machine.start_stage(ExecutionStage.ANALYZE)
        # Leave ANALYZE in progress

        machine.save_state(resolver)

        # Also create a PRD file so context is detected
        prd = {
            "metadata": {"created_at": "2026-02-03T10:00:00Z"},
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Story 1", "status": "pending"},
            ],
        }
        prd_path = resolver.get_prd_path()
        prd_path.parent.mkdir(parents=True, exist_ok=True)
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        # Use ContextRecoveryManager
        manager = ContextRecoveryManager(
            project_root=project_root,
            path_resolver=resolver,
        )

        state = manager.detect_context()

        # Verify stage state info is included
        assert state.stage_state_info is not None
        assert state.current_stage == "analyze"
        assert "intake" in state.stage_completed_stages
