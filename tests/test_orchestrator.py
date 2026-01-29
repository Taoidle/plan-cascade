"""Tests for Orchestrator module."""

import json
import pytest
from pathlib import Path

from plan_cascade.core.orchestrator import Orchestrator, StoryAgent
from plan_cascade.core.prd_generator import create_sample_prd


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
