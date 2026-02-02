"""Tests for Orchestrator module."""

import json
import pytest
from pathlib import Path

from plan_cascade.core.orchestrator import Orchestrator, StoryAgent
from plan_cascade.core.prd_generator import create_sample_prd
from plan_cascade.core.stage_state import (
    ExecutionStage,
    StageStatus,
    StageStateMachine,
)


class TestStoryAgent:
    """Tests for StoryAgent class."""

    def test_init(self):
        """Test StoryAgent initialization."""
        agent = StoryAgent(
            name="test-agent",
            command_template='echo "{prompt}"',
            priority=50
        )

        assert agent.name == "test-agent"
        assert agent.priority == 50

    def test_is_available_default(self):
        """Test default availability check."""
        agent = StoryAgent(name="test", command_template="test")
        assert agent.is_available() is True

    def test_is_available_with_check(self):
        """Test availability check with custom function."""
        agent = StoryAgent(
            name="test",
            command_template="test",
            check_available=lambda: False
        )
        assert agent.is_available() is False


class TestOrchestrator:
    """Tests for Orchestrator class."""

    @pytest.fixture
    def orchestrator_with_prd(self, tmp_path: Path) -> Orchestrator:
        """Create orchestrator with sample PRD."""
        prd = create_sample_prd()
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        return Orchestrator(tmp_path)

    def test_init(self, tmp_path: Path):
        """Test Orchestrator initialization."""
        orchestrator = Orchestrator(tmp_path)
        assert orchestrator.project_root == tmp_path

    def test_load_prd(self, orchestrator_with_prd: Orchestrator):
        """Test loading PRD."""
        prd = orchestrator_with_prd.load_prd()

        assert prd is not None
        assert "stories" in prd

    def test_analyze_dependencies(self, orchestrator_with_prd: Orchestrator):
        """Test dependency analysis creates batches."""
        batches = orchestrator_with_prd.analyze_dependencies()

        assert len(batches) > 0
        # First batch should have stories with no dependencies
        for story in batches[0]:
            assert len(story.get("dependencies", [])) == 0

    def test_get_available_agent(self, orchestrator_with_prd: Orchestrator):
        """Test getting available agent."""
        agent = orchestrator_with_prd.get_available_agent()

        # claude-code is always available
        assert agent is not None
        assert agent.name == "claude-code"

    def test_build_story_prompt(self, orchestrator_with_prd: Orchestrator):
        """Test building story prompt."""
        prd = orchestrator_with_prd.load_prd()
        story = prd["stories"][0]
        context = {"dependencies": [], "findings": []}

        prompt = orchestrator_with_prd.build_story_prompt(story)

        assert story["id"] in prompt
        assert story["title"] in prompt
        assert "Acceptance Criteria" in prompt

    def test_execute_story_dry_run(self, orchestrator_with_prd: Orchestrator):
        """Test story execution in dry run mode."""
        prd = orchestrator_with_prd.load_prd()
        story = prd["stories"][0]

        success, message = orchestrator_with_prd.execute_story(story, dry_run=True)

        assert success is True
        assert "DRY RUN" in message

    def test_execute_batch_dry_run(self, orchestrator_with_prd: Orchestrator):
        """Test batch execution in dry run mode."""
        batches = orchestrator_with_prd.analyze_dependencies()
        batch = batches[0]

        results = orchestrator_with_prd.execute_batch(batch, 1, dry_run=True)

        assert len(results) > 0
        for story_id, (success, message) in results.items():
            assert success is True

    def test_execute_all_dry_run(self, orchestrator_with_prd: Orchestrator):
        """Test full execution in dry run mode."""
        results = orchestrator_with_prd.execute_all(dry_run=True)

        assert "success" in results
        assert results["success"] is True
        assert results["total_batches"] > 0

    def test_get_execution_plan(self, orchestrator_with_prd: Orchestrator):
        """Test execution plan generation."""
        plan = orchestrator_with_prd.get_execution_plan()

        assert "EXECUTION PLAN" in plan
        assert "Batch" in plan

    def test_no_prd_returns_empty_batches(self, tmp_path: Path):
        """Test that missing PRD returns empty batches."""
        orchestrator = Orchestrator(tmp_path)
        batches = orchestrator.analyze_dependencies()

        assert batches == []


class TestOrchestratorWithStageMachine:
    """Tests for Orchestrator integration with StageStateMachine."""

    @pytest.fixture
    def stage_machine(self) -> StageStateMachine:
        """Create a stage machine ready for EXECUTE stage."""
        machine = StageStateMachine(
            execution_id="test-001",
            strategy="hybrid_auto",
            flow="standard",
        )
        # Complete prerequisite stages
        for stage in [
            ExecutionStage.INTAKE,
            ExecutionStage.ANALYZE,
            ExecutionStage.PLAN,
            ExecutionStage.DESIGN,
            ExecutionStage.READY_CHECK,
        ]:
            machine.start_stage(stage)
            machine.complete_stage(stage, outputs={})
        return machine

    @pytest.fixture
    def orchestrator_with_stage_machine(
        self, tmp_path: Path, stage_machine: StageStateMachine
    ) -> Orchestrator:
        """Create orchestrator with sample PRD and stage machine."""
        prd = create_sample_prd()
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        return Orchestrator(tmp_path, stage_machine=stage_machine)

    def test_init_with_stage_machine(self, tmp_path: Path, stage_machine: StageStateMachine):
        """Test Orchestrator initialization with stage machine."""
        orchestrator = Orchestrator(tmp_path, stage_machine=stage_machine)

        assert orchestrator.stage_machine is stage_machine
        assert orchestrator.get_stage_status() is not None

    def test_init_without_stage_machine(self, tmp_path: Path):
        """Test Orchestrator initialization without stage machine (backward compatible)."""
        orchestrator = Orchestrator(tmp_path)

        assert orchestrator.stage_machine is None
        assert orchestrator.get_stage_status() is None

    def test_set_stage_machine(self, tmp_path: Path, stage_machine: StageStateMachine):
        """Test setting stage machine after initialization."""
        orchestrator = Orchestrator(tmp_path)
        assert orchestrator.stage_machine is None

        orchestrator.set_stage_machine(stage_machine)
        assert orchestrator.stage_machine is stage_machine

    def test_get_stage_status(
        self, orchestrator_with_stage_machine: Orchestrator, stage_machine: StageStateMachine
    ):
        """Test getting stage status."""
        status = orchestrator_with_stage_machine.get_stage_status()

        assert status is not None
        assert status["execution_id"] == "test-001"
        assert status["strategy"] == "hybrid_auto"
        assert status["completed_stages"] == 5

    def test_execute_all_updates_stage_on_success(
        self, orchestrator_with_stage_machine: Orchestrator, stage_machine: StageStateMachine
    ):
        """Test that execute_all updates EXECUTE stage on success."""
        results = orchestrator_with_stage_machine.execute_all(dry_run=True)

        assert results["success"] is True

        # EXECUTE stage should be completed
        execute_state = stage_machine.get_stage_state(ExecutionStage.EXECUTE)
        assert execute_state.status == StageStatus.COMPLETED
        assert "completed_stories" in execute_state.outputs
        assert "batches_executed" in execute_state.outputs

    def test_execute_all_without_stage_machine_works(self, tmp_path: Path):
        """Test that execute_all works without stage machine (backward compatible)."""
        prd = create_sample_prd()
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        orchestrator = Orchestrator(tmp_path)  # No stage machine

        results = orchestrator.execute_all(dry_run=True)

        assert results["success"] is True

    def test_execute_batch_records_to_stage(
        self, orchestrator_with_stage_machine: Orchestrator, stage_machine: StageStateMachine
    ):
        """Test that batch execution records results to stage outputs."""
        # Start EXECUTE stage
        stage_machine.start_stage(ExecutionStage.EXECUTE)

        batches = orchestrator_with_stage_machine.analyze_dependencies()
        batch = batches[0]

        orchestrator_with_stage_machine.execute_batch(batch, 1, dry_run=True)

        # Check batch results were recorded
        execute_state = stage_machine.get_stage_state(ExecutionStage.EXECUTE)
        assert "batch_results" in execute_state.outputs
        assert "batch_1" in execute_state.outputs["batch_results"]
        assert execute_state.outputs["batches_executed"] == 1

    def test_execute_all_with_callback(
        self, orchestrator_with_stage_machine: Orchestrator
    ):
        """Test execute_all with callback still works with stage machine."""
        callbacks_received = []

        def callback(batch_num, batch, results):
            callbacks_received.append((batch_num, len(batch)))

        results = orchestrator_with_stage_machine.execute_all(
            dry_run=True, callback=callback
        )

        assert results["success"] is True
        assert len(callbacks_received) > 0

    def test_stage_status_shows_progress(
        self, orchestrator_with_stage_machine: Orchestrator, stage_machine: StageStateMachine
    ):
        """Test that stage status reflects execution progress."""
        # Before execution
        status_before = orchestrator_with_stage_machine.get_stage_status()
        assert status_before["current_stage"] is None  # EXECUTE not started

        # During execution (start EXECUTE manually)
        stage_machine.start_stage(ExecutionStage.EXECUTE)
        status_during = orchestrator_with_stage_machine.get_stage_status()
        assert status_during["current_stage"] == "execute"

        # After execution
        stage_machine.complete_stage(ExecutionStage.EXECUTE, outputs={
            "completed_stories": ["story-001"],
            "batches_executed": 1,
        })
        status_after = orchestrator_with_stage_machine.get_stage_status()
        assert status_after["completed_stages"] == 6  # Previous 5 + EXECUTE
