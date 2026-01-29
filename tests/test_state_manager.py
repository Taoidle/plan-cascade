"""Tests for StateManager module."""

import json
import tempfile
from pathlib import Path
import pytest

from plan_cascade.state.state_manager import StateManager, FileLock


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
