#!/usr/bin/env python3
"""
Tests for the Context Recovery System.

Tests cover:
- Context detection for different task types
- PRD status analysis
- Progress marker parsing
- Recovery plan generation
- PathResolver integration (new mode vs legacy mode)
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
from src.plan_cascade.state.path_resolver import PathResolver


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


class TestPathResolverIntegration:
    """Tests for PathResolver integration."""

    def test_legacy_mode_default(self, tmp_path: Path):
        """Test that legacy mode is default when no path_resolver provided."""
        manager = ContextRecoveryManager(tmp_path)

        assert manager.is_legacy_mode() is True
        # In legacy mode, paths should be in project root
        assert manager.prd_path == tmp_path / "prd.json"
        assert manager.mega_plan_path == tmp_path / "mega-plan.json"
        assert manager.worktree_dir == tmp_path / ".worktree"

    def test_legacy_mode_explicit(self, tmp_path: Path):
        """Test explicit legacy mode."""
        manager = ContextRecoveryManager(tmp_path, legacy_mode=True)

        assert manager.is_legacy_mode() is True
        assert manager.prd_path == tmp_path / "prd.json"

    def test_new_mode_with_data_dir_override(self, tmp_path: Path):
        """Test new mode with custom data directory."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )
        manager = ContextRecoveryManager(project_root, path_resolver=resolver)

        assert manager.is_legacy_mode() is False
        # PRD should be in data directory
        project_id = resolver.get_project_id()
        expected_prd_path = data_dir / project_id / "prd.json"
        assert manager.prd_path == expected_prd_path

    def test_new_mode_detects_prd_in_user_dir(self, tmp_path: Path):
        """Test that new mode correctly detects prd.json in user directory."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )

        # Create PRD in the user data directory
        project_dir = resolver.get_project_dir()
        project_dir.mkdir(parents=True, exist_ok=True)

        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(project_dir / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = manager.detect_context()

        assert state.context_type == ContextType.HYBRID_AUTO
        assert state.prd_status == PrdStatus.VALID

    def test_new_mode_detects_prd_in_legacy_location(self, tmp_path: Path):
        """Test that new mode falls back to legacy location for prd.json.

        This ensures backward compatibility when prd.json is in project root
        but PathResolver is in new mode.
        """
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )

        # Create PRD in project root (legacy location), NOT in user data dir
        prd = {
            "goal": "Test Legacy Fallback",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(project_root / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = manager.detect_context()

        # Should still detect the PRD in legacy location
        assert state.context_type == ContextType.HYBRID_AUTO
        assert state.prd_status == PrdStatus.VALID
        assert state.task_name == "Test Legacy Fallback"

    def test_new_mode_detects_mega_plan_in_user_dir(self, tmp_path: Path):
        """Test that new mode correctly detects mega-plan.json in user directory."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )

        # Create mega-plan in the user data directory
        project_dir = resolver.get_project_dir()
        project_dir.mkdir(parents=True, exist_ok=True)

        mega_plan = {
            "goal": "Multi-feature project",
            "target_branch": "main",
            "features": [
                {"id": "feature-001", "name": "auth", "title": "Auth", "status": "pending"},
            ],
        }
        with open(project_dir / "mega-plan.json", "w") as f:
            json.dump(mega_plan, f)

        manager = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = manager.detect_context()

        assert state.context_type == ContextType.MEGA_PLAN
        assert state.prd_status == PrdStatus.VALID

    def test_new_mode_worktree_detection(self, tmp_path: Path):
        """Test that new mode correctly detects worktrees in user directory."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )

        # Create worktree directory structure in user data directory
        worktree_dir = resolver.get_worktree_dir()
        worktree_task = worktree_dir / "feature-login"
        worktree_task.mkdir(parents=True)

        # Create config in worktree
        config = {
            "task_name": "feature-login",
            "target_branch": "main",
            "branch_name": "task/feature-login",
        }
        with open(worktree_task / ".planning-config.json", "w") as f:
            json.dump(config, f)

        manager = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = manager.detect_context()

        # Should detect the worktree
        assert state.context_type == ContextType.HYBRID_WORKTREE
        assert state.total_stories == 1  # One worktree found

    def test_shared_path_resolver(self, tmp_path: Path):
        """Test that a shared PathResolver can be used across managers."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        manager = ContextRecoveryManager(tmp_path, path_resolver=resolver)

        # Verify the manager uses the provided resolver
        assert manager.path_resolver is resolver
        assert manager.is_legacy_mode() == resolver.is_legacy_mode()

    def test_show_paths_legacy(self, tmp_path: Path):
        """Test path computation in legacy mode."""
        manager = ContextRecoveryManager(tmp_path, legacy_mode=True)

        assert manager.mega_plan_path == tmp_path / "mega-plan.json"
        assert manager.prd_path == tmp_path / "prd.json"
        assert manager.progress_path == tmp_path / "progress.txt"
        assert manager.worktree_dir == tmp_path / ".worktree"
        assert manager.config_path == tmp_path / ".planning-config.json"

    def test_context_file_in_project_root_new_mode(self, tmp_path: Path):
        """Test that hybrid context files are written to project root even in new mode.

        This ensures consistency with hybrid-context-reminder.py script which
        always writes to project root for user visibility.
        """
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir
        )

        # Create PRD in user data directory
        project_dir = resolver.get_project_dir()
        project_dir.mkdir(parents=True, exist_ok=True)

        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(project_dir / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = manager.detect_context()
        manager.update_context_file(state)

        # Hybrid context file should be in project root (not state dir)
        # for consistency with hybrid-context-reminder.py script
        context_file = project_root / ".hybrid-execution-context.md"
        assert context_file.exists()

        content = context_file.read_text()
        assert "Hybrid Execution Context" in content

    def test_context_file_in_project_root_legacy_mode(self, tmp_path: Path):
        """Test that context files are written to project root in legacy mode."""
        prd = {
            "goal": "Test",
            "stories": [{"id": "story-001", "title": "Test", "status": "pending"}],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        manager = ContextRecoveryManager(tmp_path, legacy_mode=True)
        state = manager.detect_context()
        manager.update_context_file(state)

        # Context file should be in project root
        context_file = tmp_path / ".hybrid-execution-context.md"
        assert context_file.exists()


class TestWorktreePathDetection:
    """Tests for worktree path detection in both modes."""

    def test_is_in_worktree_legacy_config(self, tmp_path: Path):
        """Test worktree detection via config file."""
        config = {
            "branch_name": "task/feature-login",
        }
        with open(tmp_path / ".planning-config.json", "w") as f:
            json.dump(config, f)

        manager = ContextRecoveryManager(tmp_path)
        assert manager._is_in_worktree() is True

    def test_is_not_in_worktree_no_task_branch(self, tmp_path: Path):
        """Test non-worktree detection when branch doesn't start with task/."""
        config = {
            "branch_name": "feature/login",
        }
        with open(tmp_path / ".planning-config.json", "w") as f:
            json.dump(config, f)

        manager = ContextRecoveryManager(tmp_path)
        assert manager._is_in_worktree() is False

    def test_is_in_worktree_parent_path(self, tmp_path: Path):
        """Test worktree detection via parent directory name."""
        # Create a path that looks like it's inside a .worktree directory
        worktree_parent = tmp_path / ".worktree"
        worktree_parent.mkdir()
        worktree_dir = worktree_parent / "my-task"
        worktree_dir.mkdir()

        manager = ContextRecoveryManager(worktree_dir)
        assert manager._is_in_worktree() is True


class TestBackwardCompatibility:
    """Tests for backward compatibility with existing projects."""

    def test_existing_project_detected_in_legacy_mode(self, tmp_path: Path):
        """Test that existing projects with files in root are detected."""
        # Simulate existing project with files in root
        prd = {
            "goal": "Existing project",
            "stories": [{"id": "story-001", "title": "Test", "status": "complete"}],
        }
        with open(tmp_path / "prd.json", "w") as f:
            json.dump(prd, f)

        # Default mode should be legacy
        manager = ContextRecoveryManager(tmp_path)
        state = manager.detect_context()

        assert state.context_type == ContextType.HYBRID_AUTO
        assert state.task_state == TaskState.COMPLETE

    def test_constructor_without_resolver_is_legacy(self, tmp_path: Path):
        """Test that creating manager without resolver defaults to legacy mode."""
        manager = ContextRecoveryManager(tmp_path)
        assert manager.is_legacy_mode() is True

    def test_legacy_mode_none_defaults_to_true(self, tmp_path: Path):
        """Test that legacy_mode=None defaults to True."""
        manager = ContextRecoveryManager(tmp_path, legacy_mode=None)
        assert manager.is_legacy_mode() is True


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
