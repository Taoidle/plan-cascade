"""Tests for Stage State Machine module."""

import json
import pytest
from pathlib import Path

from plan_cascade.core.stage_state import (
    ExecutionStage,
    StageStatus,
    StageState,
    StageInput,
    StageOutput,
    StageContract,
    StageContractRegistry,
    StageStateMachine,
    get_contract_registry,
)
from plan_cascade.state.path_resolver import PathResolver


class TestExecutionStage:
    """Tests for ExecutionStage enum."""

    def test_all_stages_defined(self):
        """Test that all 8 stages are defined."""
        stages = ExecutionStage.get_order()
        assert len(stages) == 8

        expected = [
            ExecutionStage.INTAKE,
            ExecutionStage.ANALYZE,
            ExecutionStage.PLAN,
            ExecutionStage.DESIGN,
            ExecutionStage.READY_CHECK,
            ExecutionStage.EXECUTE,
            ExecutionStage.VERIFY_REVIEW,
            ExecutionStage.WRAP_UP,
        ]
        assert stages == expected

    def test_stage_values(self):
        """Test that stage values are correct strings."""
        assert ExecutionStage.INTAKE.value == "intake"
        assert ExecutionStage.ANALYZE.value == "analyze"
        assert ExecutionStage.PLAN.value == "plan"
        assert ExecutionStage.DESIGN.value == "design"
        assert ExecutionStage.READY_CHECK.value == "ready_check"
        assert ExecutionStage.EXECUTE.value == "execute"
        assert ExecutionStage.VERIFY_REVIEW.value == "verify_review"
        assert ExecutionStage.WRAP_UP.value == "wrap_up"

    def test_get_index(self):
        """Test getting stage index."""
        assert ExecutionStage.get_index(ExecutionStage.INTAKE) == 0
        assert ExecutionStage.get_index(ExecutionStage.ANALYZE) == 1
        assert ExecutionStage.get_index(ExecutionStage.WRAP_UP) == 7

    def test_next_stage(self):
        """Test getting next stage."""
        assert ExecutionStage.INTAKE.next_stage() == ExecutionStage.ANALYZE
        assert ExecutionStage.ANALYZE.next_stage() == ExecutionStage.PLAN
        assert ExecutionStage.WRAP_UP.next_stage() is None

    def test_previous_stage(self):
        """Test getting previous stage."""
        assert ExecutionStage.INTAKE.previous_stage() is None
        assert ExecutionStage.ANALYZE.previous_stage() == ExecutionStage.INTAKE
        assert ExecutionStage.WRAP_UP.previous_stage() == ExecutionStage.VERIFY_REVIEW


class TestStageStatus:
    """Tests for StageStatus enum."""

    def test_all_statuses_defined(self):
        """Test that all statuses are defined."""
        assert StageStatus.PENDING.value == "pending"
        assert StageStatus.IN_PROGRESS.value == "in_progress"
        assert StageStatus.COMPLETED.value == "completed"
        assert StageStatus.FAILED.value == "failed"
        assert StageStatus.SKIPPED.value == "skipped"


class TestStageState:
    """Tests for StageState dataclass."""

    def test_default_initialization(self):
        """Test StageState with default values."""
        state = StageState(stage=ExecutionStage.INTAKE)

        assert state.stage == ExecutionStage.INTAKE
        assert state.status == StageStatus.PENDING
        assert state.started_at is None
        assert state.completed_at is None
        assert state.outputs == {}
        assert state.errors == []

    def test_full_initialization(self):
        """Test StageState with all values."""
        state = StageState(
            stage=ExecutionStage.EXECUTE,
            status=StageStatus.COMPLETED,
            started_at="2026-02-03T10:00:00Z",
            completed_at="2026-02-03T10:30:00Z",
            outputs={"completed_stories": ["story-001"]},
            errors=[],
        )

        assert state.stage == ExecutionStage.EXECUTE
        assert state.status == StageStatus.COMPLETED
        assert state.started_at == "2026-02-03T10:00:00Z"
        assert state.completed_at == "2026-02-03T10:30:00Z"
        assert state.outputs == {"completed_stories": ["story-001"]}

    def test_to_dict(self):
        """Test JSON serialization."""
        state = StageState(
            stage=ExecutionStage.PLAN,
            status=StageStatus.IN_PROGRESS,
            started_at="2026-02-03T10:00:00Z",
            outputs={"plan_type": "prd"},
        )

        data = state.to_dict()

        assert data["stage"] == "plan"
        assert data["status"] == "in_progress"
        assert data["started_at"] == "2026-02-03T10:00:00Z"
        assert data["completed_at"] is None
        assert data["outputs"] == {"plan_type": "prd"}

    def test_from_dict(self):
        """Test JSON deserialization."""
        data = {
            "stage": "design",
            "status": "completed",
            "started_at": "2026-02-03T10:00:00Z",
            "completed_at": "2026-02-03T10:15:00Z",
            "outputs": {"design_path": "/path/to/design.json"},
            "errors": [],
        }

        state = StageState.from_dict(data)

        assert state.stage == ExecutionStage.DESIGN
        assert state.status == StageStatus.COMPLETED
        assert state.outputs["design_path"] == "/path/to/design.json"

    def test_is_terminal(self):
        """Test terminal state detection."""
        pending = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.PENDING)
        in_progress = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.IN_PROGRESS)
        completed = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.COMPLETED)
        failed = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.FAILED)
        skipped = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.SKIPPED)

        assert not pending.is_terminal()
        assert not in_progress.is_terminal()
        assert completed.is_terminal()
        assert failed.is_terminal()
        assert skipped.is_terminal()

    def test_can_start(self):
        """Test can_start check."""
        pending = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.PENDING)
        in_progress = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.IN_PROGRESS)
        completed = StageState(stage=ExecutionStage.INTAKE, status=StageStatus.COMPLETED)

        assert pending.can_start()
        assert not in_progress.can_start()
        assert not completed.can_start()


class TestStageInput:
    """Tests for StageInput dataclass."""

    def test_basic_input(self):
        """Test basic input definition."""
        inp = StageInput(
            name="task_description",
            description="User's task description",
        )

        assert inp.name == "task_description"
        assert inp.required is True
        assert inp.source_stage is None

    def test_input_with_source(self):
        """Test input with source stage."""
        inp = StageInput(
            name="context",
            description="Collected context",
            source_stage=ExecutionStage.INTAKE,
        )

        assert inp.source_stage == ExecutionStage.INTAKE

    def test_to_dict_from_dict(self):
        """Test serialization roundtrip."""
        inp = StageInput(
            name="strategy",
            description="Selected strategy",
            required=True,
            source_stage=ExecutionStage.ANALYZE,
        )

        data = inp.to_dict()
        restored = StageInput.from_dict(data)

        assert restored.name == inp.name
        assert restored.source_stage == inp.source_stage


class TestStageOutput:
    """Tests for StageOutput dataclass."""

    def test_basic_output(self):
        """Test basic output definition."""
        out = StageOutput(
            name="plan_path",
            description="Path to generated plan",
        )

        assert out.name == "plan_path"
        assert out.required is True

    def test_optional_output(self):
        """Test optional output."""
        out = StageOutput(
            name="warnings",
            description="List of warnings",
            required=False,
        )

        assert not out.required


class TestStageContract:
    """Tests for StageContract dataclass."""

    def test_validate_inputs(self):
        """Test input validation."""
        contract = StageContract(
            stage=ExecutionStage.PLAN,
            required_inputs=[
                StageInput("strategy", "Strategy", required=True),
                StageInput("context", "Context", required=True),
            ],
        )

        # Valid inputs
        valid, missing = contract.validate_inputs({"strategy": "hybrid", "context": {}})
        assert valid
        assert missing == []

        # Missing input
        valid, missing = contract.validate_inputs({"strategy": "hybrid"})
        assert not valid
        assert "context" in missing

    def test_validate_outputs(self):
        """Test output validation."""
        contract = StageContract(
            stage=ExecutionStage.PLAN,
            expected_outputs=[
                StageOutput("plan_type", "Type", required=True),
                StageOutput("warnings", "Warnings", required=False),
            ],
        )

        # Valid outputs (with optional)
        valid, missing = contract.validate_outputs({"plan_type": "prd", "warnings": []})
        assert valid

        # Valid outputs (without optional)
        valid, missing = contract.validate_outputs({"plan_type": "prd"})
        assert valid

        # Missing required
        valid, missing = contract.validate_outputs({"warnings": []})
        assert not valid
        assert "plan_type" in missing

    def test_check_acceptance_default(self):
        """Test default acceptance check (no check defined)."""
        contract = StageContract(stage=ExecutionStage.INTAKE)

        passed, failures = contract.check_acceptance({})
        assert passed
        assert failures == []

    def test_check_acceptance_custom(self):
        """Test custom acceptance check."""
        def check_plan(outputs):
            if outputs.get("stories_count", 0) < 1:
                return False, ["Plan must have at least one story"]
            return True, []

        contract = StageContract(
            stage=ExecutionStage.PLAN,
            acceptance_check=check_plan,
        )

        # Passing
        passed, failures = contract.check_acceptance({"stories_count": 3})
        assert passed

        # Failing
        passed, failures = contract.check_acceptance({"stories_count": 0})
        assert not passed
        assert "at least one story" in failures[0]


class TestStageContractRegistry:
    """Tests for StageContractRegistry."""

    def test_default_contracts(self):
        """Test that all stages have default contracts."""
        registry = StageContractRegistry()

        for stage in ExecutionStage.get_order():
            contract = registry.get_contract(stage)
            assert contract is not None
            assert contract.stage == stage

    def test_get_contract_registry_singleton(self):
        """Test singleton pattern for default registry."""
        registry1 = get_contract_registry()
        registry2 = get_contract_registry()

        assert registry1 is registry2

    def test_register_custom_contract(self):
        """Test registering custom contract."""
        registry = StageContractRegistry()

        custom_contract = StageContract(
            stage=ExecutionStage.INTAKE,
            required_inputs=[
                StageInput("custom_input", "Custom"),
            ],
        )

        registry.register_contract(custom_contract)

        retrieved = registry.get_contract(ExecutionStage.INTAKE)
        assert len(retrieved.required_inputs) == 1
        assert retrieved.required_inputs[0].name == "custom_input"


class TestStageStateMachine:
    """Tests for StageStateMachine."""

    def test_initialization(self):
        """Test state machine initialization."""
        machine = StageStateMachine(
            execution_id="test-001",
            strategy="hybrid_auto",
            flow="standard",
        )

        assert machine.execution_id == "test-001"
        assert machine.strategy == "hybrid_auto"
        assert machine.flow == "standard"
        assert machine.current_stage is None
        assert machine.is_complete() is False

    def test_all_stages_initialized_pending(self):
        """Test that all stages start as pending."""
        machine = StageStateMachine(execution_id="test-001")

        for stage in ExecutionStage.get_order():
            state = machine.get_stage_state(stage)
            assert state.status == StageStatus.PENDING

    def test_start_stage(self):
        """Test starting a stage."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)

        assert machine.current_stage == ExecutionStage.INTAKE
        state = machine.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.IN_PROGRESS
        assert state.started_at is not None

    def test_complete_stage(self):
        """Test completing a stage."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(
            ExecutionStage.INTAKE,
            outputs={"context": {}, "task_normalized": "test task"},
        )

        state = machine.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.COMPLETED
        assert state.completed_at is not None
        assert state.outputs["context"] == {}

    def test_fail_stage(self):
        """Test failing a stage."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.fail_stage(ExecutionStage.INTAKE, errors=["Test error"])

        state = machine.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.FAILED
        assert "Test error" in state.errors
        assert machine.is_failed()

    def test_cannot_start_non_sequential_stage(self):
        """Test that stages must follow sequence."""
        machine = StageStateMachine(execution_id="test-001")

        # Cannot start PLAN without completing INTAKE and ANALYZE
        with pytest.raises(ValueError, match="not complete"):
            machine.start_stage(ExecutionStage.PLAN)

    def test_can_transition_to(self):
        """Test transition validation."""
        machine = StageStateMachine(execution_id="test-001")

        # Can start first stage
        can, reason = machine.can_transition_to(ExecutionStage.INTAKE)
        assert can

        machine.start_stage(ExecutionStage.INTAKE)

        # Cannot start another stage while one is in progress
        can, reason = machine.can_transition_to(ExecutionStage.ANALYZE)
        assert not can
        assert "in progress" in reason

    def test_skip_stage(self):
        """Test skipping a skippable stage."""
        machine = StageStateMachine(
            execution_id="test-001",
            skippable_stages=[ExecutionStage.DESIGN],
        )

        # Complete up to DESIGN
        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.complete_stage(ExecutionStage.ANALYZE, outputs={
            "strategy": "hybrid", "flow": "quick", "confidence": 0.9, "reasoning": "test"
        })

        machine.start_stage(ExecutionStage.PLAN)
        machine.complete_stage(ExecutionStage.PLAN, outputs={
            "plan_type": "prd", "plan_path": "/path/to/prd.json"
        })

        # Skip DESIGN
        machine.skip_stage(ExecutionStage.DESIGN, reason="Quick Flow")

        state = machine.get_stage_state(ExecutionStage.DESIGN)
        assert state.status == StageStatus.SKIPPED
        assert state.outputs["skip_reason"] == "Quick Flow"

    def test_last_completed_stage(self):
        """Test getting last completed stage."""
        machine = StageStateMachine(execution_id="test-001")

        assert machine.last_completed_stage is None

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        assert machine.last_completed_stage == ExecutionStage.INTAKE

    def test_next_pending_stage(self):
        """Test getting next pending stage."""
        machine = StageStateMachine(execution_id="test-001")

        assert machine.next_pending_stage == ExecutionStage.INTAKE

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        assert machine.next_pending_stage == ExecutionStage.ANALYZE

    def test_get_resume_point_after_failure(self):
        """Test resume point after failure."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.fail_stage(ExecutionStage.ANALYZE, errors=["Analysis failed"])

        resume_point = machine.get_resume_point()
        assert resume_point == ExecutionStage.ANALYZE

    def test_reset_stage(self):
        """Test resetting a failed stage."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.fail_stage(ExecutionStage.INTAKE, errors=["Error"])

        machine.reset_stage(ExecutionStage.INTAKE)

        state = machine.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.PENDING
        assert state.errors == []

    def test_resume_from(self):
        """Test resuming from a specific stage."""
        machine = StageStateMachine(execution_id="test-001")

        # Complete some stages
        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.complete_stage(ExecutionStage.ANALYZE, outputs={
            "strategy": "hybrid", "flow": "standard", "confidence": 0.9, "reasoning": "test"
        })

        # Resume from ANALYZE
        machine.resume_from(ExecutionStage.ANALYZE)

        # ANALYZE and subsequent stages should be reset
        assert machine.get_stage_state(ExecutionStage.INTAKE).status == StageStatus.COMPLETED
        assert machine.get_stage_state(ExecutionStage.ANALYZE).status == StageStatus.PENDING

    def test_is_resumable(self):
        """Test is_resumable check."""
        machine = StageStateMachine(execution_id="test-001")

        # Not started - resumable (can start from beginning)
        assert machine.is_resumable()

        # Complete all stages
        for stage in ExecutionStage.get_order():
            machine.start_stage(stage)
            machine.complete_stage(stage, outputs={})

        # All complete - not resumable
        assert not machine.is_resumable()

    def test_get_all_outputs(self):
        """Test getting combined outputs."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {"files": ["a.py"]},
            "task_normalized": "test"
        })

        machine.start_stage(ExecutionStage.ANALYZE)
        machine.complete_stage(ExecutionStage.ANALYZE, outputs={
            "strategy": "hybrid",
            "flow": "standard",
            "confidence": 0.9,
            "reasoning": "test"
        })

        outputs = machine.get_all_outputs()

        assert outputs["context"] == {"files": ["a.py"]}
        assert outputs["strategy"] == "hybrid"
        assert outputs["confidence"] == 0.9

    def test_stages_history(self):
        """Test that history is recorded."""
        machine = StageStateMachine(execution_id="test-001")

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        history = machine.stages_history
        assert len(history) == 2  # start + complete

        assert history[0]["stage"] == "intake"
        assert history[0]["new_status"] == "in_progress"
        assert history[1]["new_status"] == "completed"

    def test_to_dict_from_dict(self):
        """Test serialization roundtrip."""
        machine = StageStateMachine(
            execution_id="test-001",
            strategy="hybrid_auto",
            flow="standard",
        )

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {"data": 123},
            "task_normalized": "test"
        })

        data = machine.to_dict()
        restored = StageStateMachine.from_dict(data)

        assert restored.execution_id == machine.execution_id
        assert restored.strategy == machine.strategy
        assert restored.flow == machine.flow

        state = restored.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.COMPLETED
        assert state.outputs["context"]["data"] == 123

    def test_get_progress_summary(self):
        """Test progress summary generation."""
        machine = StageStateMachine(
            execution_id="test-001",
            strategy="hybrid_auto",
            flow="standard",
        )

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={"context": {}, "task_normalized": "test"})

        machine.start_stage(ExecutionStage.ANALYZE)

        summary = machine.get_progress_summary()

        assert summary["execution_id"] == "test-001"
        assert summary["strategy"] == "hybrid_auto"
        assert summary["current_stage"] == "analyze"
        assert summary["total_stages"] == 8
        assert summary["completed_stages"] == 1
        assert summary["progress_percent"] == 12  # 1/8


class TestStageStateMachinePersistence:
    """Tests for StageStateMachine persistence."""

    def test_save_and_load_state(self, tmp_path: Path):
        """Test saving and loading state."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create and populate machine
        machine = StageStateMachine(
            execution_id="persist-test",
            strategy="mega_plan",
            flow="full",
        )

        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {"key": "value"},
            "task_normalized": "test task"
        })

        # Save
        saved_path = machine.save_state(resolver)
        assert saved_path.exists()

        # Load
        loaded = StageStateMachine.load_state(resolver)
        assert loaded is not None
        assert loaded.execution_id == "persist-test"
        assert loaded.strategy == "mega_plan"

        state = loaded.get_stage_state(ExecutionStage.INTAKE)
        assert state.status == StageStatus.COMPLETED
        assert state.outputs["context"]["key"] == "value"

    def test_load_state_not_found(self, tmp_path: Path):
        """Test loading when file doesn't exist."""
        resolver = PathResolver(
            project_root=tmp_path,
            legacy_mode=True,
        )

        loaded = StageStateMachine.load_state(resolver)
        assert loaded is None

    def test_state_file_location(self, tmp_path: Path):
        """Test that state file is in correct location per ADR-002."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        machine = StageStateMachine(execution_id="location-test")
        saved_path = machine.save_state(resolver)

        # Should be in .state directory
        assert ".state" in str(saved_path)
        assert saved_path.name == "stage-state.json"


class TestStageStateMachineIntegration:
    """Integration tests for full workflow."""

    def test_full_workflow_quick_flow(self):
        """Test a complete Quick Flow execution."""
        machine = StageStateMachine(
            execution_id="quick-001",
            strategy="direct",
            flow="quick",
            skippable_stages=[ExecutionStage.DESIGN, ExecutionStage.READY_CHECK],
        )

        # INTAKE
        machine.start_stage(ExecutionStage.INTAKE)
        machine.complete_stage(ExecutionStage.INTAKE, outputs={
            "context": {},
            "task_normalized": "simple task"
        })

        # ANALYZE
        machine.start_stage(ExecutionStage.ANALYZE)
        machine.complete_stage(ExecutionStage.ANALYZE, outputs={
            "strategy": "direct",
            "flow": "quick",
            "confidence": 0.95,
            "reasoning": "Simple task"
        })

        # PLAN
        machine.start_stage(ExecutionStage.PLAN)
        machine.complete_stage(ExecutionStage.PLAN, outputs={
            "plan_type": "minimal",
            "plan_path": "/tmp/plan.json"
        })

        # Skip DESIGN and READY_CHECK
        machine.skip_stage(ExecutionStage.DESIGN, "Quick Flow")
        machine.skip_stage(ExecutionStage.READY_CHECK, "Quick Flow")

        # EXECUTE
        machine.start_stage(ExecutionStage.EXECUTE)
        machine.complete_stage(ExecutionStage.EXECUTE, outputs={
            "completed_stories": ["task-001"],
            "batches_executed": 1
        })

        # VERIFY_REVIEW
        machine.start_stage(ExecutionStage.VERIFY_REVIEW)
        machine.complete_stage(ExecutionStage.VERIFY_REVIEW, outputs={
            "quality_gate_passed": True,
            "verification_passed": True
        })

        # WRAP_UP
        machine.start_stage(ExecutionStage.WRAP_UP)
        machine.complete_stage(ExecutionStage.WRAP_UP, outputs={
            "summary": "Task completed successfully"
        })

        # Verify final state
        assert machine.is_complete()
        assert not machine.is_failed()

        summary = machine.get_progress_summary()
        assert summary["completed_stages"] == 8  # 6 completed + 2 skipped
        assert summary["progress_percent"] == 100

    def test_workflow_with_retry(self):
        """Test workflow with a failed stage that gets retried."""
        machine = StageStateMachine(
            execution_id="retry-001",
            strategy="hybrid_auto",
            flow="standard",
        )

        # Complete through EXECUTE
        for stage in [ExecutionStage.INTAKE, ExecutionStage.ANALYZE, ExecutionStage.PLAN,
                      ExecutionStage.DESIGN, ExecutionStage.READY_CHECK, ExecutionStage.EXECUTE]:
            machine.start_stage(stage)
            machine.complete_stage(stage, outputs={})

        # VERIFY_REVIEW fails
        machine.start_stage(ExecutionStage.VERIFY_REVIEW)
        machine.fail_stage(ExecutionStage.VERIFY_REVIEW, errors=["Tests failed"])

        assert machine.is_failed()
        assert machine.get_resume_point() == ExecutionStage.VERIFY_REVIEW

        # Reset and retry
        machine.reset_stage(ExecutionStage.VERIFY_REVIEW)
        machine.start_stage(ExecutionStage.VERIFY_REVIEW)
        machine.complete_stage(ExecutionStage.VERIFY_REVIEW, outputs={
            "quality_gate_passed": True,
            "verification_passed": True
        })

        # Complete WRAP_UP
        machine.start_stage(ExecutionStage.WRAP_UP)
        machine.complete_stage(ExecutionStage.WRAP_UP, outputs={"summary": "Done"})

        assert machine.is_complete()
        assert not machine.is_failed()
