"""Tests for Worktree Management module."""

import json
import subprocess
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.cli.worktree import (
    WorktreeInfo,
    WorktreeManager,
    WorktreeState,
    WorktreeStatus,
)


class TestWorktreeState:
    """Tests for WorktreeState dataclass."""

    def test_to_dict(self, tmp_path: Path):
        """Test converting WorktreeState to dict."""
        state = WorktreeState(
            task_name="test-task",
            target_branch="main",
            worktree_path=tmp_path / ".worktree" / "test-task",
            created_at="2026-01-31T10:00:00",
            status=WorktreeStatus.ACTIVE,
            description="Test description",
            branch_name="task/test-task",
        )

        result = state.to_dict()

        assert result["task_name"] == "test-task"
        assert result["target_branch"] == "main"
        assert result["status"] == "active"
        assert result["description"] == "Test description"
        assert result["branch_name"] == "task/test-task"

    def test_from_dict(self, tmp_path: Path):
        """Test creating WorktreeState from dict."""
        data = {
            "task_name": "test-task",
            "target_branch": "main",
            "worktree_path": str(tmp_path / ".worktree" / "test-task"),
            "created_at": "2026-01-31T10:00:00",
            "status": "complete",
            "description": "Test description",
            "branch_name": "task/test-task",
        }

        state = WorktreeState.from_dict(data)

        assert state.task_name == "test-task"
        assert state.target_branch == "main"
        assert state.status == WorktreeStatus.COMPLETE
        assert state.worktree_path == Path(tmp_path / ".worktree" / "test-task")


class TestWorktreeManager:
    """Tests for WorktreeManager class."""

    def test_init(self, tmp_path: Path):
        """Test WorktreeManager initialization."""
        manager = WorktreeManager(tmp_path)

        assert manager.project_root == tmp_path.resolve()
        assert manager.worktree_dir == tmp_path.resolve() / ".worktree"

    def test_create_invalid_task_name_empty(self, tmp_path: Path):
        """Test that empty task name raises error."""
        manager = WorktreeManager(tmp_path)

        with pytest.raises(ValueError, match="Invalid task name"):
            manager.create("", "main")

    def test_create_invalid_task_name_with_slash(self, tmp_path: Path):
        """Test that task name with slash raises error."""
        manager = WorktreeManager(tmp_path)

        with pytest.raises(ValueError, match="Invalid task name"):
            manager.create("task/with/slash", "main")

    def test_create_invalid_task_name_with_backslash(self, tmp_path: Path):
        """Test that task name with backslash raises error."""
        manager = WorktreeManager(tmp_path)

        with pytest.raises(ValueError, match="Invalid task name"):
            manager.create("task\\with\\backslash", "main")

    def test_create_worktree_already_exists(self, tmp_path: Path):
        """Test error when worktree directory already exists."""
        manager = WorktreeManager(tmp_path)

        # Create the worktree directory manually
        worktree_dir = tmp_path / ".worktree" / "test-task"
        worktree_dir.mkdir(parents=True)

        with pytest.raises(ValueError, match="Worktree already exists"):
            manager.create("test-task", "main")

    @patch.object(WorktreeManager, "_run_git_command")
    def test_create_worktree_success(self, mock_git, tmp_path: Path):
        """Test successful worktree creation."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"

        # Mock git commands - also create directory when worktree add is called
        def side_effect(args, cwd=None, check=True, capture_output=True):
            if args == ["rev-parse", "--verify", "task/test-task"]:
                # Branch doesn't exist
                raise subprocess.CalledProcessError(1, args)
            if args[0:2] == ["worktree", "add"]:
                # Create directory when git worktree add is called
                worktree_path.mkdir(parents=True, exist_ok=True)
            return MagicMock(stdout="", returncode=0)

        mock_git.side_effect = side_effect

        state = manager.create("test-task", "main", "Test description")

        assert state.task_name == "test-task"
        assert state.target_branch == "main"
        assert state.branch_name == "task/test-task"
        assert state.description == "Test description"
        assert state.status == WorktreeStatus.ACTIVE

        # Verify planning files were created
        assert (worktree_path / ".planning-config.json").exists()
        assert (worktree_path / "prd.json").exists()
        assert (worktree_path / "progress.txt").exists()
        assert (worktree_path / "findings.md").exists()

    def test_initialize_planning_files(self, tmp_path: Path):
        """Test planning files are created correctly."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        state = WorktreeState(
            task_name="test-task",
            target_branch="main",
            worktree_path=worktree_path,
            created_at="2026-01-31T10:00:00",
            status=WorktreeStatus.ACTIVE,
            description="Test description",
            branch_name="task/test-task",
        )

        manager._initialize_planning_files(state)

        # Check .planning-config.json
        config_path = worktree_path / ".planning-config.json"
        assert config_path.exists()
        with open(config_path) as f:
            config = json.load(f)
        assert config["task_name"] == "test-task"
        assert config["target_branch"] == "main"
        assert config["branch_name"] == "task/test-task"
        assert config["status"] == "active"

        # Check prd.json
        prd_path = worktree_path / "prd.json"
        assert prd_path.exists()
        with open(prd_path) as f:
            prd = json.load(f)
        assert prd["metadata"]["task_name"] == "test-task"
        assert prd["goal"] == "Test description"

        # Check progress.txt
        progress_path = worktree_path / "progress.txt"
        assert progress_path.exists()
        content = progress_path.read_text()
        assert "test-task" in content
        assert "main" in content

        # Check findings.md
        findings_path = worktree_path / "findings.md"
        assert findings_path.exists()
        content = findings_path.read_text()
        assert "test-task" in content

    def test_detect_current_task_from_config(self, tmp_path: Path):
        """Test detecting task from .planning-config.json."""
        manager = WorktreeManager(tmp_path)

        # Create config file
        config = {"task_name": "detected-task", "target_branch": "main"}
        config_path = tmp_path / ".planning-config.json"
        with open(config_path, "w") as f:
            json.dump(config, f)

        # Mock cwd to be tmp_path
        with patch("pathlib.Path.cwd", return_value=tmp_path):
            task_name = manager._detect_current_task()

        assert task_name == "detected-task"

    def test_verify_stories_complete_no_prd(self, tmp_path: Path):
        """Test verification when no PRD exists."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        success, message = manager._verify_stories_complete(worktree_path)

        assert success is True
        assert "No PRD found" in message

    def test_verify_stories_complete_no_stories(self, tmp_path: Path):
        """Test verification when PRD has no stories."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        # Create PRD with no stories
        prd = {"metadata": {}, "stories": []}
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(prd, f)

        success, message = manager._verify_stories_complete(worktree_path)

        assert success is True
        assert "No stories defined" in message

    def test_verify_stories_complete_incomplete(self, tmp_path: Path):
        """Test verification when stories are incomplete."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        # Create PRD with incomplete stories
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "pending"},
                {"id": "story-003", "status": "in_progress"},
            ],
        }
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(prd, f)

        success, message = manager._verify_stories_complete(worktree_path)

        assert success is False
        assert "story-002" in message
        assert "story-003" in message

    def test_verify_stories_complete_all_done(self, tmp_path: Path):
        """Test verification when all stories are complete."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        # Create PRD with all stories complete
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "complete"},
            ],
        }
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(prd, f)

        success, message = manager._verify_stories_complete(worktree_path)

        assert success is True
        assert "2 stories complete" in message

    @patch.object(WorktreeManager, "_run_git_command")
    def test_has_uncommitted_changes_true(self, mock_git, tmp_path: Path):
        """Test detecting uncommitted changes."""
        manager = WorktreeManager(tmp_path)
        mock_git.return_value = MagicMock(stdout=" M file.py\n?? new.py")

        result = manager._has_uncommitted_changes(tmp_path)

        assert result is True

    @patch.object(WorktreeManager, "_run_git_command")
    def test_has_uncommitted_changes_false(self, mock_git, tmp_path: Path):
        """Test no uncommitted changes."""
        manager = WorktreeManager(tmp_path)
        mock_git.return_value = MagicMock(stdout="")

        result = manager._has_uncommitted_changes(tmp_path)

        assert result is False

    @patch.object(WorktreeManager, "_run_git_command")
    def test_list_worktrees_empty(self, mock_git, tmp_path: Path):
        """Test listing worktrees when none exist."""
        manager = WorktreeManager(tmp_path)
        mock_git.return_value = MagicMock(
            stdout=f"worktree {tmp_path}\nHEAD abc123\nbranch refs/heads/main\n"
        )

        worktrees = manager.list_worktrees()

        # Only the main worktree
        assert len(worktrees) == 1

    @patch.object(WorktreeManager, "_run_git_command")
    def test_list_worktrees_with_tasks(self, mock_git, tmp_path: Path):
        """Test listing worktrees with tasks."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"

        # Mock git output with two worktrees
        mock_git.return_value = MagicMock(
            stdout=(
                f"worktree {tmp_path}\n"
                "HEAD abc123\n"
                "branch refs/heads/main\n"
                "\n"
                f"worktree {worktree_path}\n"
                "HEAD def456\n"
                "branch refs/heads/task/test-task\n"
                "\n"
            )
        )

        worktrees = manager.list_worktrees()

        assert len(worktrees) == 2

    def test_parse_worktree_info_basic(self, tmp_path: Path):
        """Test parsing basic worktree info."""
        manager = WorktreeManager(tmp_path)

        data = {
            "path": str(tmp_path / ".worktree" / "test-task"),
            "head": "abc123",
            "branch": "refs/heads/task/test-task",
        }

        info = manager._parse_worktree_info(data)

        assert info is not None
        assert info.branch == "task/test-task"
        assert info.head == "abc123"
        assert info.is_detached is False

    def test_parse_worktree_info_detached(self, tmp_path: Path):
        """Test parsing detached worktree info."""
        manager = WorktreeManager(tmp_path)

        data = {
            "path": str(tmp_path / ".worktree" / "test-task"),
            "head": "abc123",
            "detached": True,
        }

        info = manager._parse_worktree_info(data)

        assert info is not None
        assert info.is_detached is True

    def test_parse_worktree_info_with_config(self, tmp_path: Path):
        """Test parsing worktree info with config file."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        # Create config file
        config = {
            "task_name": "test-task",
            "target_branch": "main",
            "created_at": "2026-01-31T10:00:00",
        }
        with open(worktree_path / ".planning-config.json", "w") as f:
            json.dump(config, f)

        data = {
            "path": str(worktree_path),
            "head": "abc123",
            "branch": "refs/heads/task/test-task",
        }

        info = manager._parse_worktree_info(data)

        assert info is not None
        assert info.task_name == "test-task"
        assert info.target_branch == "main"
        assert info.created_at == "2026-01-31T10:00:00"

    def test_parse_worktree_info_with_prd(self, tmp_path: Path):
        """Test parsing worktree info with PRD for story progress."""
        manager = WorktreeManager(tmp_path)
        worktree_path = tmp_path / ".worktree" / "test-task"
        worktree_path.mkdir(parents=True)

        # Create PRD with stories
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "complete"},
                {"id": "story-003", "status": "pending"},
                {"id": "story-004", "status": "pending"},
            ],
        }
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(prd, f)

        data = {
            "path": str(worktree_path),
            "head": "abc123",
            "branch": "refs/heads/task/test-task",
        }

        info = manager._parse_worktree_info(data)

        assert info is not None
        assert info.stories_total == 4
        assert info.stories_complete == 2
        assert info.progress_percentage == 50.0


class TestWorktreeInfo:
    """Tests for WorktreeInfo dataclass."""

    def test_default_values(self, tmp_path: Path):
        """Test WorktreeInfo default values."""
        info = WorktreeInfo(
            worktree_path=tmp_path,
            branch="main",
            head="abc123",
        )

        assert info.is_detached is False
        assert info.task_name == ""
        assert info.target_branch == ""
        assert info.stories_total == 0
        assert info.stories_complete == 0
        assert info.progress_percentage == 0.0
