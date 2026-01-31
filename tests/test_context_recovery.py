#!/usr/bin/env python3
"""
Tests for the Context Recovery System.

Tests cover:
- Context detection for different task types
- PRD status analysis
- Progress marker parsing
- Recovery plan generation
"""

import json
import tempfile
from pathlib import Path

import pytest

from src.plan_cascade.state.context_recovery import (
    ContextRecoveryManager,
    ContextRecoveryState,
    ContextType,
    PrdStatus,
    RecoveryPlan,
    TaskState,
)


class TestContextDetection:
    """Tests for context type detection."""

    def test_detect_no_context(self, tmp_path: Path):
        """Test detection when no task files exist."""
        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.context_type == ContextType.UNKNOWN
        assert state.task_state == TaskState.NEEDS_PRD
        assert state.prd_status == PrdStatus.MISSING

    def test_detect_hybrid_auto_context(self, tmp_path: Path):
        """Test detection of hybrid-auto context from prd.json."""
        # Create a valid prd.json
        prd = {
            "goal": "Test task",
            "stories": [
                {"id": "story-001", "title": "Test story", "status": "pending"},
            ],
        }
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.context_type == ContextType.HYBRID_AUTO
        assert state.prd_status == PrdStatus.VALID
        assert state.task_state == TaskState.NEEDS_APPROVAL
        assert state.total_stories == 1
        assert "story-001" in state.pending_stories

    def test_detect_mega_plan_context(self, tmp_path: Path):
        """Test detection of mega-plan context."""
        # Create a mega-plan.json
        mega_plan = {
            "goal": "Multi-feature project",
            "target_branch": "main",
            "features": [
                {"id": "feature-001", "name": "auth", "title": "Auth System", "status": "pending"},
                {"id": "feature-002", "name": "api", "title": "API", "status": "complete"},
            ],
        }
        mega_path = tmp_path / "mega-plan.json"
        with open(mega_path, "w") as f:
            json.dump(mega_plan, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.context_type == ContextType.MEGA_PLAN
        assert state.prd_status == PrdStatus.VALID
        assert state.task_state == TaskState.EXECUTING  # One complete, one pending
        assert len(state.mega_plan_features) == 2
        assert "feature-002" in state.completed_stories
        assert "feature-001" in state.pending_stories

    def test_detect_hybrid_worktree_context(self, tmp_path: Path):
        """Test detection of hybrid-worktree context."""
        # Create .planning-config.json with worktree metadata
        config = {
            "version": "1.0.0",
            "task_name": "feature-login",
            "target_branch": "main",
            "branch_name": "task/feature-login",
            "status": "active",
        }
        config_path = tmp_path / ".planning-config.json"
        with open(config_path, "w") as f:
            json.dump(config, f)

        # Create a prd.json
        prd = {
            "goal": "Feature login",
            "stories": [
                {"id": "story-001", "title": "Login form", "status": "complete"},
                {"id": "story-002", "title": "Auth logic", "status": "in_progress"},
            ],
        }
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.context_type == ContextType.HYBRID_WORKTREE
        assert state.task_name == "feature-login"
        assert state.target_branch == "main"
        assert state.task_state == TaskState.EXECUTING
        assert "story-001" in state.completed_stories
        assert "story-002" in state.in_progress_stories


class TestPrdStatusAnalysis:
    """Tests for PRD status analysis."""

    def test_prd_missing(self, tmp_path: Path):
        """Test detection of missing PRD."""
        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.prd_status == PrdStatus.MISSING

    def test_prd_corrupted(self, tmp_path: Path):
        """Test detection of corrupted PRD."""
        # Create an invalid JSON file
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            f.write("{ invalid json }")

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.prd_status == PrdStatus.CORRUPTED

    def test_prd_empty(self, tmp_path: Path):
        """Test detection of empty PRD (no stories)."""
        prd = {"goal": "Test", "stories": []}
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.prd_status == PrdStatus.EMPTY

    def test_prd_valid(self, tmp_path: Path):
        """Test detection of valid PRD."""
        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.prd_status == PrdStatus.VALID


class TestProgressMarkerParsing:
    """Tests for progress marker parsing."""

    def test_parse_story_complete_marker(self, tmp_path: Path):
        """Test parsing of [STORY_COMPLETE: story-XXX] marker."""
        # Create prd.json
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "pending"},
                {"id": "story-002", "title": "Test 2", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        # Create progress.txt with completion marker
        progress_content = """
[2024-01-15 10:00:00] story-001: Started
[2024-01-15 10:30:00] [STORY_COMPLETE: story-001] Finished successfully
"""
        with open(tmp_path / "progress.txt", "w") as f:
            f.write(progress_content)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert "story-001" in state.completed_stories

    def test_parse_old_style_complete_marker(self, tmp_path: Path):
        """Test parsing of old-style [COMPLETE] marker."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        progress_content = """
[2024-01-15 10:30:00] story-001: [COMPLETE] story-001 finished
"""
        with open(tmp_path / "progress.txt", "w") as f:
            f.write(progress_content)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert "story-001" in state.completed_stories

    def test_parse_feature_complete_marker(self, tmp_path: Path):
        """Test parsing of [FEATURE_COMPLETE: feature-XXX] marker."""
        mega_plan = {
            "goal": "Test",
            "features": [
                {"id": "feature-001", "name": "auth", "title": "Auth", "status": "pending"},
            ],
        }
        with open(tmp_path / "mega-plan.json", "w") as f:
            json.dump(mega_plan, f)

        progress_content = """
[2024-01-15 11:00:00] [FEATURE_COMPLETE: feature-001] Auth completed
"""
        with open(tmp_path / "progress.txt", "w") as f:
            f.write(progress_content)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        # Progress file markers should be detected too
        assert "feature-001" in state.completed_stories

    def test_extract_last_activity_timestamp(self, tmp_path: Path):
        """Test extraction of last activity timestamp."""
        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        progress_content = """
[2024-01-15 09:00:00] Started
[2024-01-15 10:30:00] In progress
[2024-01-15 11:45:30] Last action
"""
        with open(tmp_path / "progress.txt", "w") as f:
            f.write(progress_content)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.last_activity == "2024-01-15 11:45:30"


class TestTaskStateDetection:
    """Tests for task state detection."""

    def test_state_needs_prd(self, tmp_path: Path):
        """Test detection of needs_prd state."""
        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.task_state == TaskState.NEEDS_PRD

    def test_state_needs_approval(self, tmp_path: Path):
        """Test detection of needs_approval state (all stories pending)."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test", "status": "pending"},
                {"id": "story-002", "title": "Test 2", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.task_state == TaskState.NEEDS_APPROVAL

    def test_state_executing(self, tmp_path: Path):
        """Test detection of executing state."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "in_progress"},
                {"id": "story-003", "title": "Test 3", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.task_state == TaskState.EXECUTING

    def test_state_complete(self, tmp_path: Path):
        """Test detection of complete state."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "complete"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.task_state == TaskState.COMPLETE

    def test_state_failed(self, tmp_path: Path):
        """Test detection of failed state (only failed stories remain)."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "failed"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        # When there's only failed remaining (no in_progress, no pending)
        assert state.task_state == TaskState.FAILED


class TestRecoveryPlanGeneration:
    """Tests for recovery plan generation."""

    def test_plan_for_no_context(self, tmp_path: Path):
        """Test recovery plan when no context exists."""
        manager = ContextRecoveryManager(tmp_path)
        plan = manager.generate_recovery_plan()

        assert not plan.can_auto_resume
        assert len(plan.actions) >= 1
        assert plan.actions[0].action == "start_new"

    def test_plan_for_hybrid_auto_needs_approval(self, tmp_path: Path):
        """Test recovery plan for hybrid-auto needing approval."""
        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        plan = manager.generate_recovery_plan()

        assert plan.can_auto_resume
        assert plan.actions[0].action == "approve_and_run"
        assert "auto-run" in plan.actions[0].command

    def test_plan_for_mega_plan_executing(self, tmp_path: Path):
        """Test recovery plan for mega-plan in execution."""
        mega_plan = {
            "goal": "Test",
            "target_branch": "main",
            "features": [
                {"id": "feature-001", "name": "auth", "title": "Auth", "status": "complete"},
                {"id": "feature-002", "name": "api", "title": "API", "status": "in_progress"},
            ],
        }
        with open(tmp_path / "mega-plan.json", "w") as f:
            json.dump(mega_plan, f)

        manager = ContextRecoveryManager(tmp_path)
        plan = manager.generate_recovery_plan()

        assert plan.can_auto_resume
        assert plan.actions[0].action == "resume_mega"

    def test_plan_with_failed_stories_warning(self, tmp_path: Path):
        """Test recovery plan includes warning for failed stories."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "failed"},
                {"id": "story-003", "title": "Test 3", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        plan = manager.generate_recovery_plan()

        assert len(plan.warnings) >= 1
        assert any("failed" in w.lower() for w in plan.warnings)


class TestContextFileUpdate:
    """Tests for context file updates after resume."""

    def test_hybrid_context_file_created(self, tmp_path: Path):
        """Test .hybrid-execution-context.md is created."""
        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()
        manager.update_context_file(state)

        context_file = tmp_path / ".hybrid-execution-context.md"
        assert context_file.exists()

        content = context_file.read_text()
        assert "Hybrid Execution Context" in content
        assert "Progress" in content

    def test_mega_context_file_created(self, tmp_path: Path):
        """Test .mega-execution-context.md is created."""
        mega_plan = {
            "goal": "Test",
            "features": [{"id": "feature-001", "name": "auth", "title": "Auth", "status": "pending"}],
        }
        with open(tmp_path / "mega-plan.json", "w") as f:
            json.dump(mega_plan, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()
        manager.update_context_file(state)

        context_file = tmp_path / ".mega-execution-context.md"
        assert context_file.exists()

        content = context_file.read_text()
        assert "Mega-Plan Execution Context" in content
        assert "feature-001" in content


class TestCompletionPercentage:
    """Tests for completion percentage calculation."""

    def test_zero_percent_completion(self, tmp_path: Path):
        """Test 0% completion when all stories pending."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "pending"},
                {"id": "story-002", "title": "Test 2", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.completion_percentage == 0.0

    def test_fifty_percent_completion(self, tmp_path: Path):
        """Test 50% completion."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "pending"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.completion_percentage == 50.0

    def test_hundred_percent_completion(self, tmp_path: Path):
        """Test 100% completion."""
        prd = {
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "status": "complete"},
                {"id": "story-002", "title": "Test 2", "status": "complete"},
            ],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.completion_percentage == 100.0


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
