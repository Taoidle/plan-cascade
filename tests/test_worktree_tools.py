"""Tests for MCP Worktree Tools module.

Tests the four MCP tools for Git worktree lifecycle management:
- worktree_create
- worktree_list
- worktree_remove
- worktree_complete
"""

import importlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict
from unittest.mock import MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Direct module loading to avoid transitive __init__.py imports from
# mcp_server.tools (which pulls in execution_tools with heavy deps).
# ---------------------------------------------------------------------------

_WORKTREE_TOOLS_PATH = (
    Path(__file__).parent.parent / "mcp_server" / "tools" / "worktree_tools.py"
)


def _load_register_function():
    """Load register_worktree_tools without triggering mcp_server.tools.__init__."""
    spec = importlib.util.spec_from_file_location(
        "worktree_tools", str(_WORKTREE_TOOLS_PATH)
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.register_worktree_tools


# ---------------------------------------------------------------------------
# Helpers to register tools and invoke them by name
# ---------------------------------------------------------------------------

def _register_tools(project_root: Path) -> Dict[str, Any]:
    """
    Register worktree tools on a mock MCP server and return a dict of
    tool_name -> callable.
    """
    register_worktree_tools = _load_register_function()

    tools: Dict[str, Any] = {}

    class FakeMCP:
        """Minimal stand-in for FastMCP that captures registered tools."""

        def tool(self):
            def decorator(fn):
                tools[fn.__name__] = fn
                return fn
            return decorator

    fake_mcp = FakeMCP()
    register_worktree_tools(fake_mcp, project_root)
    return tools


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def git_repo(tmp_path: Path) -> Path:
    """Create a minimal Git repo in a temp directory."""
    subprocess.run(
        ["git", "init"], cwd=tmp_path, check=True,
        capture_output=True, text=True,
    )
    subprocess.run(
        ["git", "config", "user.email", "test@test.com"],
        cwd=tmp_path, check=True, capture_output=True, text=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Test"],
        cwd=tmp_path, check=True, capture_output=True, text=True,
    )
    # Create an initial commit so we have a HEAD
    dummy = tmp_path / "README.md"
    dummy.write_text("# Test project\n")
    subprocess.run(
        ["git", "add", "."], cwd=tmp_path, check=True,
        capture_output=True, text=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Initial commit", "--no-gpg-sign"],
        cwd=tmp_path, check=True, capture_output=True, text=True,
    )
    return tmp_path


@pytest.fixture
def tools(git_repo: Path) -> Dict[str, Any]:
    """Register worktree tools against a real git repo."""
    return _register_tools(git_repo)


# ===================================================================
# Tests: worktree_create
# ===================================================================

class TestWorktreeCreate:
    """Tests for the worktree_create MCP tool."""

    def test_creates_worktree_and_planning_files(self, tools, git_repo):
        """worktree_create should create a git worktree with planning files."""
        result = tools["worktree_create"](
            task_name="feature-login",
            target_branch="main",
            description="Implement user login",
        )

        assert result["success"] is True
        assert "feature-login" in result["message"]

        worktree_path = Path(result["worktree_path"])
        assert worktree_path.exists()

        # Verify planning files
        assert (worktree_path / ".planning-config.json").exists()
        assert (worktree_path / "findings.md").exists()
        assert (worktree_path / "progress.txt").exists()

        # Verify config contents
        config = json.loads((worktree_path / ".planning-config.json").read_text())
        assert config["task_name"] == "feature-login"
        assert config["target_branch"] == "main"
        assert config["description"] == "Implement user login"

    def test_default_target_branch(self, tools, git_repo):
        """worktree_create should default target_branch to 'main'."""
        result = tools["worktree_create"](task_name="fix-bug")

        assert result["success"] is True
        config_path = Path(result["worktree_path"]) / ".planning-config.json"
        config = json.loads(config_path.read_text())
        assert config["target_branch"] == "main"

    def test_invalid_task_name_empty(self, tools, git_repo):
        """worktree_create should fail with an empty task name."""
        result = tools["worktree_create"](task_name="")

        assert result["success"] is False
        assert "error" in result

    def test_invalid_task_name_with_slash(self, tools, git_repo):
        """worktree_create should fail when task name contains a slash."""
        result = tools["worktree_create"](task_name="bad/name")

        assert result["success"] is False
        assert "error" in result

    def test_duplicate_worktree(self, tools, git_repo):
        """worktree_create should fail if a worktree with that name already exists."""
        # First create should succeed
        result1 = tools["worktree_create"](task_name="dup-task")
        assert result1["success"] is True

        # Second create with same name should fail
        result2 = tools["worktree_create"](task_name="dup-task")
        assert result2["success"] is False
        assert "error" in result2

    def test_uses_path_resolver_for_directory(self, tools, git_repo):
        """worktree_create should use PathResolver for worktree directory resolution."""
        result = tools["worktree_create"](task_name="resolver-test")
        assert result["success"] is True

        # The worktree_path should be under .worktree directory
        worktree_path = Path(result["worktree_path"])
        assert ".worktree" in str(worktree_path) or "worktree" in str(worktree_path).lower()

    def test_branch_created(self, tools, git_repo):
        """worktree_create should create a task/... branch."""
        result = tools["worktree_create"](task_name="branch-test")
        assert result["success"] is True
        assert result["branch_name"] == "task/branch-test"

        # Verify branch exists in git
        check = subprocess.run(
            ["git", "branch", "--list", "task/branch-test"],
            cwd=git_repo, capture_output=True, text=True,
        )
        assert "task/branch-test" in check.stdout


# ===================================================================
# Tests: worktree_list
# ===================================================================

class TestWorktreeList:
    """Tests for the worktree_list MCP tool."""

    def test_empty_list(self, tools, git_repo):
        """worktree_list should return empty worktrees list when none are created."""
        result = tools["worktree_list"]()

        assert result["success"] is True
        assert isinstance(result["worktrees"], list)
        # Only the main worktree or no task-worktrees
        task_worktrees = [w for w in result["worktrees"] if w.get("task_name")]
        assert len(task_worktrees) == 0

    def test_list_after_create(self, tools, git_repo):
        """worktree_list should show worktrees created by worktree_create."""
        # Create two worktrees
        tools["worktree_create"](task_name="task-a", description="Task A")
        tools["worktree_create"](task_name="task-b", description="Task B")

        result = tools["worktree_list"]()

        assert result["success"] is True
        task_worktrees = [w for w in result["worktrees"] if w.get("task_name")]
        task_names = [w["task_name"] for w in task_worktrees]
        assert "task-a" in task_names
        assert "task-b" in task_names

    def test_list_includes_planning_config(self, tools, git_repo):
        """worktree_list should include planning config info for each worktree."""
        tools["worktree_create"](
            task_name="config-test",
            target_branch="main",
            description="Config test task",
        )

        result = tools["worktree_list"]()
        assert result["success"] is True

        task_wts = [w for w in result["worktrees"] if w.get("task_name") == "config-test"]
        assert len(task_wts) == 1
        wt = task_wts[0]
        assert wt["target_branch"] == "main"

    def test_list_includes_prd_status(self, tools, git_repo):
        """worktree_list should include PRD story progress."""
        create_result = tools["worktree_create"](task_name="prd-test")
        wt_path = Path(create_result["worktree_path"])

        # Write a PRD with stories
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "pending"},
            ],
        }
        (wt_path / "prd.json").write_text(json.dumps(prd))

        result = tools["worktree_list"]()
        assert result["success"] is True

        task_wts = [w for w in result["worktrees"] if w.get("task_name") == "prd-test"]
        assert len(task_wts) == 1
        wt = task_wts[0]
        assert wt["stories_total"] == 2
        assert wt["stories_complete"] == 1


# ===================================================================
# Tests: worktree_remove
# ===================================================================

class TestWorktreeRemove:
    """Tests for the worktree_remove MCP tool."""

    def test_remove_existing_worktree(self, tools, git_repo):
        """worktree_remove should remove an existing worktree."""
        create_result = tools["worktree_create"](task_name="remove-me")
        assert create_result["success"] is True
        wt_path = Path(create_result["worktree_path"])
        assert wt_path.exists()

        remove_result = tools["worktree_remove"](task_name="remove-me")
        assert remove_result["success"] is True

        # Worktree directory should be gone
        assert not wt_path.exists()

    def test_remove_nonexistent_worktree(self, tools, git_repo):
        """worktree_remove should fail for a nonexistent worktree."""
        result = tools["worktree_remove"](task_name="nonexistent")

        assert result["success"] is False
        assert "error" in result

    def test_remove_with_branch_deletion(self, tools, git_repo):
        """worktree_remove with remove_branch=True should also delete the branch."""
        tools["worktree_create"](task_name="branch-del")

        result = tools["worktree_remove"](task_name="branch-del", remove_branch=True)
        assert result["success"] is True

        # Branch should be gone
        check = subprocess.run(
            ["git", "branch", "--list", "task/branch-del"],
            cwd=git_repo, capture_output=True, text=True,
        )
        assert "task/branch-del" not in check.stdout

    def test_remove_without_branch_deletion(self, tools, git_repo):
        """worktree_remove with remove_branch=False should keep the branch."""
        tools["worktree_create"](task_name="keep-branch")

        result = tools["worktree_remove"](task_name="keep-branch", remove_branch=False)
        assert result["success"] is True

        # Branch should still exist
        check = subprocess.run(
            ["git", "branch", "--list", "task/keep-branch"],
            cwd=git_repo, capture_output=True, text=True,
        )
        assert "task/keep-branch" in check.stdout

    def test_remove_warns_uncommitted_changes(self, tools, git_repo):
        """worktree_remove should warn about uncommitted changes."""
        create_result = tools["worktree_create"](task_name="dirty-wt")
        wt_path = Path(create_result["worktree_path"])

        # Create uncommitted changes in the worktree
        (wt_path / "dirty_file.py").write_text("# uncommitted\n")

        result = tools["worktree_remove"](task_name="dirty-wt")
        assert result["success"] is True
        # Should include a warning about uncommitted changes
        assert result.get("warning") or "uncommitted" in result.get("message", "").lower()


# ===================================================================
# Tests: worktree_complete
# ===================================================================

class TestWorktreeComplete:
    """Tests for the worktree_complete MCP tool."""

    def test_complete_all_stories_done(self, tools, git_repo):
        """worktree_complete should succeed when all stories are complete."""
        create_result = tools["worktree_create"](task_name="complete-me")
        wt_path = Path(create_result["worktree_path"])

        # Write a PRD with all stories complete
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "complete"},
            ],
        }
        (wt_path / "prd.json").write_text(json.dumps(prd))

        result = tools["worktree_complete"](task_name="complete-me")
        assert result["success"] is True

    def test_complete_with_incomplete_stories(self, tools, git_repo):
        """worktree_complete should fail when stories are incomplete."""
        create_result = tools["worktree_create"](task_name="incomplete-task")
        wt_path = Path(create_result["worktree_path"])

        # Write a PRD with incomplete stories
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "pending"},
            ],
        }
        (wt_path / "prd.json").write_text(json.dumps(prd))

        result = tools["worktree_complete"](task_name="incomplete-task")
        assert result["success"] is False
        assert "story-002" in result.get("error", "") or "incomplete" in result.get("error", "").lower()

    def test_complete_nonexistent_worktree(self, tools, git_repo):
        """worktree_complete should fail for a nonexistent worktree."""
        result = tools["worktree_complete"](task_name="ghost")

        assert result["success"] is False
        assert "error" in result

    def test_complete_no_prd(self, tools, git_repo):
        """worktree_complete should succeed when there is no PRD (nothing to verify)."""
        create_result = tools["worktree_create"](task_name="no-prd-task")
        wt_path = Path(create_result["worktree_path"])

        # Remove the auto-generated prd.json
        prd_path = wt_path / "prd.json"
        if prd_path.exists():
            prd_path.unlink()

        result = tools["worktree_complete"](task_name="no-prd-task")
        assert result["success"] is True

    def test_complete_with_merge_false(self, tools, git_repo):
        """worktree_complete with merge=False should not merge."""
        create_result = tools["worktree_create"](task_name="nomerge-task")
        wt_path = Path(create_result["worktree_path"])

        # All stories complete
        prd = {
            "metadata": {},
            "stories": [
                {"id": "story-001", "status": "complete"},
            ],
        }
        (wt_path / "prd.json").write_text(json.dumps(prd))

        result = tools["worktree_complete"](task_name="nomerge-task", merge=False)
        assert result["success"] is True
        # The worktree should still exist (no merge/cleanup)
        assert wt_path.exists()

    def test_complete_returns_dict_with_success_and_message(self, tools, git_repo):
        """worktree_complete should return Dict with 'success' and 'message' keys."""
        create_result = tools["worktree_create"](task_name="dict-test")
        wt_path = Path(create_result["worktree_path"])

        # Remove PRD so it passes verification
        prd_path = wt_path / "prd.json"
        if prd_path.exists():
            prd_path.unlink()

        result = tools["worktree_complete"](task_name="dict-test")
        assert isinstance(result, dict)
        assert "success" in result
        assert "message" in result or "error" in result


# ===================================================================
# Tests: Registration & Integration
# ===================================================================

class TestRegistration:
    """Tests for tool registration in the MCP server."""

    def test_all_four_tools_registered(self, tools):
        """All four worktree tools should be registered."""
        assert "worktree_create" in tools
        assert "worktree_list" in tools
        assert "worktree_remove" in tools
        assert "worktree_complete" in tools

    def test_register_worktree_tools_importable(self):
        """register_worktree_tools should be importable from worktree_tools module."""
        register_fn = _load_register_function()
        assert callable(register_fn)

    def test_tools_return_dict_with_success(self, tools, git_repo):
        """Every tool should return a dict with a 'success' boolean."""
        # worktree_list always works
        result = tools["worktree_list"]()
        assert isinstance(result, dict)
        assert isinstance(result["success"], bool)

        # worktree_create with valid params
        result = tools["worktree_create"](task_name="success-test")
        assert isinstance(result, dict)
        assert isinstance(result["success"], bool)
