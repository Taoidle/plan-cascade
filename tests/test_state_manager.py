"""Tests for StateManager module."""

import json
import tempfile
from pathlib import Path
import pytest

from plan_cascade.state.state_manager import StateManager, FileLock
from plan_cascade.state.path_resolver import PathResolver


class TestFileLock:
    """Tests for FileLock class."""

    def test_acquire_release(self, tmp_path: Path):
        """Test basic lock acquire and release."""
        lock_file = tmp_path / "test.lock"
        lock = FileLock(lock_file)

        assert lock.acquire() is True
        lock.release()

    def test_context_manager(self, tmp_path: Path):
        """Test lock as context manager."""
        lock_file = tmp_path / "test.lock"

        with FileLock(lock_file):
            assert lock_file.exists()

    def test_lock_creates_directory(self, tmp_path: Path):
        """Test that lock creates parent directory."""
        lock_file = tmp_path / "subdir" / "test.lock"
        lock = FileLock(lock_file)

        lock.acquire()
        assert lock_file.parent.exists()
        lock.release()


class TestStateManager:
    """Tests for StateManager class."""

    def test_init(self, tmp_path: Path):
        """Test StateManager initialization."""
        sm = StateManager(tmp_path)
        assert sm.project_root == tmp_path

    def test_read_prd_not_found(self, tmp_path: Path):
        """Test reading PRD when file doesn't exist."""
        sm = StateManager(tmp_path)
        result = sm.read_prd()
        assert result is None

    def test_write_read_prd(self, tmp_path: Path):
        """Test writing and reading PRD."""
        sm = StateManager(tmp_path)
        prd = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test goal",
            "stories": []
        }

        sm.write_prd(prd)
        result = sm.read_prd()

        assert result is not None
        assert result["goal"] == "Test goal"

    def test_update_story_status(self, tmp_path: Path):
        """Test updating story status in PRD."""
        sm = StateManager(tmp_path)
        prd = {
            "metadata": {"version": "1.0.0"},
            "stories": [
                {"id": "story-001", "status": "pending"},
                {"id": "story-002", "status": "pending"}
            ]
        }
        sm.write_prd(prd)

        sm.update_story_status("story-001", "complete")

        result = sm.read_prd()
        assert result["stories"][0]["status"] == "complete"
        assert result["stories"][1]["status"] == "pending"

    def test_append_findings(self, tmp_path: Path):
        """Test appending to findings."""
        sm = StateManager(tmp_path)

        sm.append_findings("Test finding 1", tags=["story-001"])
        sm.append_findings("Test finding 2", tags=["story-001", "story-002"])

        content = sm.read_findings()
        assert "Test finding 1" in content
        assert "Test finding 2" in content
        assert "@tags: story-001" in content

    def test_append_progress(self, tmp_path: Path):
        """Test appending to progress."""
        sm = StateManager(tmp_path)

        sm.append_progress("Started work", story_id="story-001")
        sm.mark_story_in_progress("story-002")
        sm.mark_story_complete("story-001")

        content = sm.read_progress()
        assert "story-001" in content
        assert "story-002" in content
        assert "[COMPLETE]" in content
        assert "[IN_PROGRESS]" in content

    def test_get_all_story_statuses(self, tmp_path: Path):
        """Test getting all story statuses from progress."""
        sm = StateManager(tmp_path)

        sm.mark_story_complete("story-001")
        sm.mark_story_in_progress("story-002")

        statuses = sm.get_all_story_statuses()
        assert statuses.get("story-001") == "complete"
        assert statuses.get("story-002") == "in_progress"

    def test_agent_status_tracking(self, tmp_path: Path):
        """Test agent status tracking."""
        sm = StateManager(tmp_path)

        sm.record_agent_start("story-001", "claude-code", pid=12345)
        status = sm.read_agent_status()
        assert len(status["running"]) == 1
        assert status["running"][0]["story_id"] == "story-001"

        sm.record_agent_complete("story-001", "claude-code")
        status = sm.read_agent_status()
        assert len(status["running"]) == 0
        assert len(status["completed"]) == 1

    def test_agent_failure_tracking(self, tmp_path: Path):
        """Test agent failure tracking."""
        sm = StateManager(tmp_path)

        sm.record_agent_start("story-001", "aider", pid=12345)
        sm.record_agent_failure("story-001", "aider", "Command not found")

        status = sm.read_agent_status()
        assert len(status["running"]) == 0
        assert len(status["failed"]) == 1
        assert status["failed"][0]["error"] == "Command not found"

    def test_iteration_state(self, tmp_path: Path):
        """Test iteration state management."""
        sm = StateManager(tmp_path)

        state = {
            "status": "running",
            "current_batch": 1,
            "total_batches": 3,
            "completed_stories": 2,
            "total_stories": 6
        }
        sm.write_iteration_state(state)

        result = sm.read_iteration_state()
        assert result["status"] == "running"
        assert result["current_batch"] == 1

        sm.clear_iteration_state()
        assert sm.read_iteration_state() is None

    def test_retry_state(self, tmp_path: Path):
        """Test retry state management."""
        sm = StateManager(tmp_path)

        sm.record_retry_attempt("story-001", "claude-code", "quality_gate", "Tests failed")

        summary = sm.get_retry_summary("story-001")
        assert summary["story_id"] == "story-001"
        assert summary["current_attempt"] == 1
        assert summary["failures"] == 1


class TestStateManagerWithPathResolver:
    """Tests for StateManager integration with PathResolver."""

    def test_init_with_default_legacy_mode(self, tmp_path: Path):
        """Test that default initialization uses legacy mode for backward compatibility."""
        sm = StateManager(tmp_path)

        # Default should be legacy mode for backward compatibility
        assert sm.is_legacy_mode() is True
        assert sm.prd_path == tmp_path / "prd.json"
        assert sm.locks_dir == tmp_path / ".locks"

    def test_init_with_explicit_legacy_mode(self, tmp_path: Path):
        """Test explicit legacy mode initialization."""
        sm = StateManager(tmp_path, legacy_mode=True)

        assert sm.is_legacy_mode() is True
        assert sm.prd_path == tmp_path / "prd.json"
        assert sm.findings_path == tmp_path / "findings.md"
        assert sm.progress_path == tmp_path / "progress.txt"
        assert sm.agent_status_path == tmp_path / ".agent-status.json"

    def test_init_with_new_mode(self, tmp_path: Path):
        """Test StateManager with new mode (user directory structure)."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(
            project_root=tmp_path / "project",
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(tmp_path / "project", path_resolver=resolver)

        assert sm.is_legacy_mode() is False
        # PRD should be in data directory
        assert data_dir in sm.prd_path.parents or data_dir == sm.prd_path.parent.parent
        # Locks should be in data directory
        assert data_dir in sm.locks_dir.parents or data_dir == sm.locks_dir.parent.parent

    def test_init_with_legacy_mode_false(self, tmp_path: Path):
        """Test initialization with legacy_mode=False creates PathResolver in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        # Create a custom PathResolver with data_dir_override for testing
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)

        assert sm.is_legacy_mode() is False
        # State files should be in the data directory structure
        assert ".state" in str(sm.agent_status_path)
        assert ".state" in str(sm.iteration_state_path)
        assert ".state" in str(sm.retry_state_path)

    def test_path_resolver_property(self, tmp_path: Path):
        """Test that path_resolver property returns the resolver."""
        sm = StateManager(tmp_path)

        assert sm.path_resolver is not None
        assert isinstance(sm.path_resolver, PathResolver)

    def test_injected_path_resolver_is_used(self, tmp_path: Path):
        """Test that injected PathResolver is used."""
        data_dir = tmp_path / "custom_data"
        resolver = PathResolver(
            project_root=tmp_path / "project",
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(tmp_path / "project", path_resolver=resolver)

        assert sm.path_resolver is resolver
        assert sm.is_legacy_mode() is False

    def test_new_mode_prd_operations(self, tmp_path: Path):
        """Test PRD read/write operations in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)

        prd = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test new mode",
            "stories": []
        }

        sm.write_prd(prd)
        result = sm.read_prd()

        assert result is not None
        assert result["goal"] == "Test new mode"
        # Verify file was written to new location
        assert sm.prd_path.exists()
        assert data_dir in sm.prd_path.parents

    def test_new_mode_state_file_operations(self, tmp_path: Path):
        """Test state file operations in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)

        # Test iteration state
        state = {
            "status": "running",
            "current_batch": 1,
            "total_batches": 3,
        }
        sm.write_iteration_state(state)
        result = sm.read_iteration_state()

        assert result["status"] == "running"
        # Verify file was written to state directory
        assert sm.iteration_state_path.exists()
        assert ".state" in str(sm.iteration_state_path)

    def test_new_mode_locks_in_user_directory(self, tmp_path: Path):
        """Test that locks are created in user directory in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)

        # Perform an operation that creates a lock
        prd = {"metadata": {"version": "1.0.0"}, "stories": []}
        sm.write_prd(prd)

        # Locks directory should be in data directory
        assert ".locks" in str(sm.locks_dir)
        assert data_dir in sm.locks_dir.parents

    def test_findings_and_progress_in_project_root_new_mode(self, tmp_path: Path):
        """Test that findings.md and progress.txt remain in project root even in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)

        # User-facing files should stay in project root
        assert sm.findings_path == project_root / "findings.md"
        assert sm.progress_path == project_root / "progress.txt"

        # Test operations
        sm.append_findings("Test finding", tags=["test"])
        sm.append_progress("Test progress")

        assert sm.findings_path.exists()
        assert sm.progress_path.exists()

    def test_ensure_directories(self, tmp_path: Path):
        """Test ensure_directories creates all necessary directories."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        sm = StateManager(project_root, path_resolver=resolver)
        sm.ensure_directories()

        # Verify directories were created
        assert sm.locks_dir.exists()
        project_dir = resolver.get_project_dir()
        assert project_dir.exists()
        assert resolver.get_state_dir().exists()
        assert resolver.get_worktree_dir().exists()

    def test_backward_compatibility_existing_code(self, tmp_path: Path):
        """Test that existing code using StateManager(project_root) continues to work."""
        # This simulates existing code that only passes project_root
        sm = StateManager(tmp_path)

        # All operations should work in legacy mode
        prd = {"metadata": {"version": "1.0.0"}, "goal": "Test", "stories": []}
        sm.write_prd(prd)
        result = sm.read_prd()
        assert result["goal"] == "Test"

        sm.append_findings("Finding", tags=["test"])
        assert "Finding" in sm.read_findings()

        sm.append_progress("Progress")
        assert "Progress" in sm.read_progress()

        # Verify files are in project root (legacy behavior)
        assert (tmp_path / "prd.json").exists()
        assert (tmp_path / "findings.md").exists()
        assert (tmp_path / "progress.txt").exists()

    def test_cleanup_locks_works_in_both_modes(self, tmp_path: Path):
        """Test cleanup_locks works in both legacy and new modes."""
        # Legacy mode
        sm_legacy = StateManager(tmp_path / "legacy", legacy_mode=True)
        (tmp_path / "legacy").mkdir(parents=True)
        sm_legacy.cleanup_locks()  # Should not raise

        # New mode
        data_dir = tmp_path / "data"
        resolver = PathResolver(
            project_root=tmp_path / "new",
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        (tmp_path / "new").mkdir(parents=True)
        sm_new = StateManager(tmp_path / "new", path_resolver=resolver)
        sm_new.cleanup_locks()  # Should not raise
