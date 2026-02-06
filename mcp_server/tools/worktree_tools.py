#!/usr/bin/env python3
"""
Worktree Tools for Plan Cascade MCP Server

Provides MCP tools for Git worktree lifecycle management:
- worktree_create: Create a new Git worktree with a feature branch
- worktree_list: List all active worktrees managed by Plan Cascade
- worktree_remove: Remove a worktree and optionally its branch
- worktree_complete: Complete a worktree task (verify stories, prepare for merge)
"""

import json
import logging
import subprocess
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

import sys

# Add src directory to path for PathResolver import
PLUGIN_ROOT = Path(__file__).parent.parent.parent
SRC_DIR = PLUGIN_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from plan_cascade.state.path_resolver import PathResolver

logger = logging.getLogger(__name__)


def register_worktree_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all worktree-related tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """
    # Create a PathResolver in legacy mode (worktree dir under project root)
    path_resolver = PathResolver(project_root, legacy_mode=True)

    def _get_worktree_dir() -> Path:
        """Get the worktree base directory via PathResolver."""
        return path_resolver.get_worktree_dir()

    def _run_git(
        args: List[str],
        cwd: Optional[Path] = None,
        check: bool = True,
    ) -> subprocess.CompletedProcess:
        """Run a git command with proper error handling."""
        cmd = ["git"] + args
        return subprocess.run(
            cmd,
            cwd=cwd or project_root,
            check=check,
            capture_output=True,
            text=True,
            shell=False,
        )

    def _has_uncommitted_changes(worktree_path: Path) -> bool:
        """Check if the worktree has uncommitted changes."""
        try:
            result = _run_git(["status", "--porcelain"], cwd=worktree_path)
            return bool(result.stdout.strip())
        except subprocess.CalledProcessError:
            return False

    def _verify_stories_complete(worktree_path: Path) -> tuple:
        """
        Verify all stories in the PRD are complete.

        Returns:
            Tuple of (success: bool, message: str, incomplete: list)
        """
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return True, "No PRD found (no stories to verify)", []

        try:
            with open(prd_path, encoding="utf-8") as f:
                prd = json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            return True, f"Could not read PRD: {e}", []

        stories = prd.get("stories", [])
        if not stories:
            return True, "No stories defined in PRD", []

        incomplete = []
        for story in stories:
            status = story.get("status", "pending")
            if status != "complete":
                incomplete.append({
                    "id": story.get("id", "unknown"),
                    "status": status,
                })

        if incomplete:
            ids = ", ".join(s["id"] for s in incomplete)
            return False, f"Incomplete stories: {ids}", incomplete

        return True, f"All {len(stories)} stories complete", []

    # ------------------------------------------------------------------
    # Tool 1: worktree_create
    # ------------------------------------------------------------------

    @mcp.tool()
    def worktree_create(
        task_name: str,
        target_branch: str = "main",
        description: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Create a new Git worktree with a feature branch.

        Creates a worktree at the PathResolver-determined directory with a new
        branch 'task/<task_name>'. Initializes planning files:
        .planning-config.json, findings.md, and progress.txt.

        Args:
            task_name: Name for the worktree task (no slashes or backslashes)
            target_branch: Branch to eventually merge into (default 'main')
            description: Optional description of the task

        Returns:
            Dict with success, message, worktree_path, and branch_name
        """
        description = description or ""

        # Validate task name
        if not task_name or "/" in task_name or "\\" in task_name:
            return {
                "success": False,
                "error": (
                    f"Invalid task name: '{task_name}'. "
                    "Task name cannot be empty or contain path separators."
                ),
            }

        worktree_dir = _get_worktree_dir()
        worktree_dir.mkdir(parents=True, exist_ok=True)

        worktree_path = worktree_dir / task_name
        branch_name = f"task/{task_name}"

        # Check if worktree already exists
        if worktree_path.exists():
            return {
                "success": False,
                "error": f"Worktree already exists at: {worktree_path}",
            }

        # Check if branch already exists
        try:
            _run_git(["rev-parse", "--verify", branch_name])
            return {
                "success": False,
                "error": f"Branch '{branch_name}' already exists",
            }
        except subprocess.CalledProcessError:
            # Branch does not exist -- good
            pass

        # Create the git worktree with a new branch
        try:
            _run_git(["worktree", "add", "-b", branch_name, str(worktree_path)])
        except subprocess.CalledProcessError as e:
            return {
                "success": False,
                "error": f"Git worktree creation failed: {e.stderr or e}",
            }

        created_at = datetime.now().isoformat()

        # 1. Create .planning-config.json
        config = {
            "version": "1.0.0",
            "task_name": task_name,
            "target_branch": target_branch,
            "branch_name": branch_name,
            "created_at": created_at,
            "status": "active",
            "description": description,
        }
        config_path = worktree_path / ".planning-config.json"
        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)

        # 2. Create findings.md
        findings_path = worktree_path / "findings.md"
        with open(findings_path, "w", encoding="utf-8") as f:
            f.write(f"# Findings: {task_name}\n\n")
            f.write(f"**Created:** {created_at}\n")
            f.write(f"**Target Branch:** {target_branch}\n")
            if description:
                f.write(f"**Description:** {description}\n")
            f.write("\n---\n\n")
            f.write("## Notes\n\n")

        # 3. Create progress.txt
        progress_path = worktree_path / "progress.txt"
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        with open(progress_path, "w", encoding="utf-8") as f:
            f.write(f"[{timestamp}] Task '{task_name}' initialized\n")
            f.write(f"[{timestamp}] Target branch: {target_branch}\n")
            if description:
                f.write(f"[{timestamp}] Description: {description}\n")

        return {
            "success": True,
            "message": f"Worktree '{task_name}' created successfully",
            "worktree_path": str(worktree_path),
            "branch_name": branch_name,
            "target_branch": target_branch,
            "created_at": created_at,
        }

    # ------------------------------------------------------------------
    # Tool 2: worktree_list
    # ------------------------------------------------------------------

    @mcp.tool()
    def worktree_list() -> Dict[str, Any]:
        """
        List all active worktrees managed by Plan Cascade.

        Runs 'git worktree list' and enriches each entry with planning config
        and PRD status information.

        Returns:
            Dict with success and a list of worktree info dicts
        """
        try:
            result = _run_git(["worktree", "list", "--porcelain"])
        except subprocess.CalledProcessError as e:
            return {
                "success": False,
                "error": f"Failed to list git worktrees: {e.stderr or e}",
            }

        lines = result.stdout.strip().split("\n")
        raw_worktrees: List[dict] = []
        current: dict = {}

        for line in lines:
            if not line.strip():
                if current:
                    raw_worktrees.append(current)
                current = {}
            elif line.startswith("worktree "):
                current["path"] = line[9:]
            elif line.startswith("HEAD "):
                current["head"] = line[5:]
            elif line.startswith("branch "):
                current["branch"] = line[7:]
            elif line == "detached":
                current["detached"] = True

        if current:
            raw_worktrees.append(current)

        worktrees_info: List[Dict[str, Any]] = []
        worktree_dir = _get_worktree_dir()

        for wt in raw_worktrees:
            path_str = wt.get("path", "")
            if not path_str:
                continue

            wt_path = Path(path_str)

            # Extract branch name (remove refs/heads/ prefix)
            branch = wt.get("branch", "")
            if branch.startswith("refs/heads/"):
                branch = branch[11:]

            info: Dict[str, Any] = {
                "worktree_path": str(wt_path),
                "branch": branch,
                "head": wt.get("head", ""),
                "is_detached": wt.get("detached", False),
                "task_name": "",
                "target_branch": "",
                "created_at": "",
                "description": "",
                "stories_total": 0,
                "stories_complete": 0,
                "progress_percentage": 0.0,
            }

            # Read planning config if present
            config_path = wt_path / ".planning-config.json"
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    info["task_name"] = config.get("task_name", "")
                    info["target_branch"] = config.get("target_branch", "")
                    info["created_at"] = config.get("created_at", "")
                    info["description"] = config.get("description", "")
                except (json.JSONDecodeError, OSError):
                    pass

            # Read PRD for story progress
            prd_path = wt_path / "prd.json"
            if prd_path.exists():
                try:
                    with open(prd_path, encoding="utf-8") as f:
                        prd = json.load(f)
                    stories = prd.get("stories", [])
                    info["stories_total"] = len(stories)
                    info["stories_complete"] = sum(
                        1 for s in stories if s.get("status") == "complete"
                    )
                    if info["stories_total"] > 0:
                        info["progress_percentage"] = round(
                            info["stories_complete"] / info["stories_total"] * 100, 1
                        )
                except (json.JSONDecodeError, OSError):
                    pass

            worktrees_info.append(info)

        return {
            "success": True,
            "worktrees": worktrees_info,
            "total": len(worktrees_info),
        }

    # ------------------------------------------------------------------
    # Tool 3: worktree_remove
    # ------------------------------------------------------------------

    @mcp.tool()
    def worktree_remove(
        task_name: str,
        remove_branch: bool = False,
    ) -> Dict[str, Any]:
        """
        Remove a worktree and optionally its branch.

        Validates the worktree exists and warns if it has uncommitted changes.

        Args:
            task_name: Name of the worktree task to remove
            remove_branch: If True, also delete the 'task/<task_name>' branch

        Returns:
            Dict with success, message, and optional warning
        """
        worktree_dir = _get_worktree_dir()
        worktree_path = worktree_dir / task_name
        branch_name = f"task/{task_name}"

        if not worktree_path.exists():
            return {
                "success": False,
                "error": f"Worktree not found: {worktree_path}",
            }

        # Check for uncommitted changes
        warning = None
        if _has_uncommitted_changes(worktree_path):
            warning = f"Worktree '{task_name}' has uncommitted changes"

        # Remove the worktree via git
        try:
            _run_git(["worktree", "remove", str(worktree_path), "--force"])
        except subprocess.CalledProcessError as e:
            return {
                "success": False,
                "error": f"Failed to remove worktree: {e.stderr or e}",
            }

        # Optionally delete the branch
        branch_deleted = False
        if remove_branch:
            try:
                _run_git(["branch", "-D", branch_name])
                branch_deleted = True
            except subprocess.CalledProcessError as e:
                # Non-fatal: worktree removed but branch deletion failed
                logger.warning(
                    "Worktree removed but branch deletion failed: %s",
                    e.stderr or e,
                )

        message_parts = [f"Worktree '{task_name}' removed"]
        if branch_deleted:
            message_parts.append(f"branch '{branch_name}' deleted")
        if warning:
            message_parts.append(f"(warning: {warning})")

        result: Dict[str, Any] = {
            "success": True,
            "message": "; ".join(message_parts),
        }
        if warning:
            result["warning"] = warning

        return result

    # ------------------------------------------------------------------
    # Tool 4: worktree_complete
    # ------------------------------------------------------------------

    @mcp.tool()
    def worktree_complete(
        task_name: str,
        merge: bool = False,
    ) -> Dict[str, Any]:
        """
        Complete a worktree task: verify all stories are done, prepare for merge.

        Validates that all stories in the PRD are marked 'complete'. If merge is
        True, also merges the task branch into the target branch and cleans up.

        Args:
            task_name: Name of the worktree task to complete
            merge: If True, merge the branch and remove the worktree

        Returns:
            Dict with success, message, and details about completion status
        """
        worktree_dir = _get_worktree_dir()
        worktree_path = worktree_dir / task_name

        if not worktree_path.exists():
            return {
                "success": False,
                "error": f"Worktree not found: {worktree_path}",
            }

        # Read config for branch and target info
        config_path = worktree_path / ".planning-config.json"
        branch_name = f"task/{task_name}"
        target_branch = "main"

        if config_path.exists():
            try:
                with open(config_path, encoding="utf-8") as f:
                    config = json.load(f)
                branch_name = config.get("branch_name", branch_name)
                target_branch = config.get("target_branch", target_branch)
            except (json.JSONDecodeError, OSError):
                pass

        # Verify stories are complete
        stories_ok, story_msg, incomplete = _verify_stories_complete(worktree_path)
        if not stories_ok:
            incomplete_ids = [s["id"] for s in incomplete]
            return {
                "success": False,
                "error": f"Cannot complete: {story_msg}",
                "incomplete_stories": incomplete_ids,
            }

        # If not merging, just mark as ready for merge
        if not merge:
            # Update config status to complete
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    config["status"] = "complete"
                    with open(config_path, "w", encoding="utf-8") as f:
                        json.dump(config, f, indent=2)
                except (json.JSONDecodeError, OSError):
                    pass

            return {
                "success": True,
                "message": (
                    f"Task '{task_name}' verified complete. "
                    f"{story_msg}. Ready for merge into '{target_branch}'."
                ),
                "branch_name": branch_name,
                "target_branch": target_branch,
                "merged": False,
            }

        # Merge flow: commit, merge, cleanup
        try:
            # Check for uncommitted changes and commit
            if _has_uncommitted_changes(worktree_path):
                _run_git(["add", "."], cwd=worktree_path)
                _run_git(
                    ["commit", "-m", f"feat({task_name}): complete task implementation",
                     "--no-gpg-sign"],
                    cwd=worktree_path,
                )

            # Switch to target branch
            _run_git(["checkout", target_branch])

            # Merge
            _run_git(
                ["merge", branch_name, "--no-ff", "-m", f"Merge {branch_name}"]
            )

            # Remove worktree and branch
            _run_git(["worktree", "remove", str(worktree_path), "--force"])
            _run_git(["branch", "-d", branch_name], check=False)

            return {
                "success": True,
                "message": (
                    f"Task '{task_name}' completed and merged into '{target_branch}'"
                ),
                "branch_name": branch_name,
                "target_branch": target_branch,
                "merged": True,
            }

        except subprocess.CalledProcessError as e:
            # Try to abort merge if it failed
            try:
                _run_git(["merge", "--abort"], check=False)
            except Exception:
                pass

            return {
                "success": False,
                "error": f"Merge failed: {e.stderr or e}",
            }
