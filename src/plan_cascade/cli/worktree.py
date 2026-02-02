#!/usr/bin/env python3
"""
Git Worktree Management Commands for Plan Cascade CLI

Provides commands for creating isolated development environments using Git worktrees.
This enables parallel task development with separate branches and planning files.

Commands:
- worktree create <task-name> <target-branch> [description]: Create a new worktree
- worktree complete [target-branch]: Complete and merge worktree task
- worktree list: Show all active worktrees with status
"""

import json
import logging
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING

logger = logging.getLogger(__name__)

try:
    import typer
    from rich.console import Console
    from rich.table import Table

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

if TYPE_CHECKING:
    from .output import OutputManager
    from ..state.path_resolver import PathResolver


class WorktreeStatus(str, Enum):
    """Status of a worktree task."""

    ACTIVE = "active"
    COMPLETE = "complete"
    ABANDONED = "abandoned"


@dataclass
class WorktreeState:
    """Data model for worktree state."""

    task_name: str
    target_branch: str
    worktree_path: Path
    created_at: str
    status: WorktreeStatus = WorktreeStatus.ACTIVE
    description: str = ""
    branch_name: str = ""

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "task_name": self.task_name,
            "target_branch": self.target_branch,
            "worktree_path": str(self.worktree_path),
            "created_at": self.created_at,
            "status": self.status.value,
            "description": self.description,
            "branch_name": self.branch_name,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "WorktreeState":
        """Create from dictionary."""
        return cls(
            task_name=data["task_name"],
            target_branch=data["target_branch"],
            worktree_path=Path(data["worktree_path"]),
            created_at=data["created_at"],
            status=WorktreeStatus(data.get("status", "active")),
            description=data.get("description", ""),
            branch_name=data.get("branch_name", ""),
        )


@dataclass
class WorktreeInfo:
    """Information about a Git worktree."""

    worktree_path: Path
    branch: str
    head: str
    is_detached: bool = False

    # Additional info from planning files
    task_name: str = ""
    target_branch: str = ""
    created_at: str = ""
    stories_total: int = 0
    stories_complete: int = 0
    progress_percentage: float = 0.0


class WorktreeManager:
    """Manages Git worktrees for Plan Cascade tasks."""

    WORKTREE_DIR_NAME = ".worktree"
    CONFIG_FILE_NAME = ".planning-config.json"

    def __init__(
        self,
        project_root: Path,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the worktree manager.

        Args:
            project_root: Root directory of the Git repository
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for worktree directory (backward compatible).
                If None, defaults to True when path_resolver is not provided for
                backward compatibility.
        """
        self.project_root = Path(project_root).resolve()

        # Set up PathResolver
        if path_resolver is not None:
            self._path_resolver = path_resolver
        else:
            # Default to legacy mode for backward compatibility
            if legacy_mode is None:
                legacy_mode = True
            from ..state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Use PathResolver for worktree directory
        self.worktree_dir = self._path_resolver.get_worktree_dir()

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def _run_git_command(
        self,
        args: list[str],
        cwd: Path | None = None,
        check: bool = True,
        capture_output: bool = True,
    ) -> subprocess.CompletedProcess:
        """
        Run a git command with proper error handling.

        Args:
            args: Git command arguments (without 'git')
            cwd: Working directory for the command
            check: Raise exception on non-zero return code
            capture_output: Capture stdout and stderr

        Returns:
            CompletedProcess instance
        """
        cmd = ["git"] + args
        return subprocess.run(
            cmd,
            cwd=cwd or self.project_root,
            check=check,
            capture_output=capture_output,
            text=True,
            shell=False,  # Cross-platform safety
        )

    def _get_git_root(self) -> Path | None:
        """Get the root of the Git repository."""
        try:
            result = self._run_git_command(["rev-parse", "--show-toplevel"])
            return Path(result.stdout.strip())
        except subprocess.CalledProcessError:
            return None

    def create(
        self,
        task_name: str,
        target_branch: str,
        description: str = "",
    ) -> WorktreeState:
        """
        Create a new worktree for a task.

        Args:
            task_name: Name of the task (used for directory and branch names)
            target_branch: Branch to merge into when complete
            description: Optional task description for auto-generating PRD

        Returns:
            WorktreeState representing the created worktree

        Raises:
            ValueError: If task name is invalid or worktree already exists
            subprocess.CalledProcessError: If git command fails
        """
        # Validate task name
        if not task_name or "/" in task_name or "\\" in task_name:
            raise ValueError(
                f"Invalid task name: '{task_name}'. "
                "Task name cannot be empty or contain path separators."
            )

        # Ensure worktree directory exists
        self.worktree_dir.mkdir(parents=True, exist_ok=True)

        # Define paths
        worktree_path = self.worktree_dir / task_name
        branch_name = f"task/{task_name}"

        # Check if worktree already exists
        if worktree_path.exists():
            raise ValueError(f"Worktree already exists at: {worktree_path}")

        # Check if branch already exists
        try:
            self._run_git_command(["rev-parse", "--verify", branch_name])
            raise ValueError(f"Branch '{branch_name}' already exists")
        except subprocess.CalledProcessError:
            # Branch doesn't exist, which is what we want
            logger.debug("Branch '%s' does not exist (expected for new worktree)", branch_name)

        # Create the worktree with a new branch
        self._run_git_command(
            ["worktree", "add", "-b", branch_name, str(worktree_path)]
        )

        # Create worktree state
        created_at = datetime.now().isoformat()
        state = WorktreeState(
            task_name=task_name,
            target_branch=target_branch,
            worktree_path=worktree_path,
            created_at=created_at,
            status=WorktreeStatus.ACTIVE,
            description=description,
            branch_name=branch_name,
        )

        # Initialize planning files in the worktree
        self._initialize_planning_files(state)

        return state

    def _initialize_planning_files(self, state: WorktreeState) -> None:
        """
        Initialize planning files in the worktree.

        Creates:
        - .planning-config.json: Task metadata
        - prd.json: Empty PRD template
        - progress.txt: Progress log
        - findings.md: Findings document
        """
        worktree_path = state.worktree_path

        # 1. Create .planning-config.json
        config = {
            "version": "1.0.0",
            "task_name": state.task_name,
            "target_branch": state.target_branch,
            "branch_name": state.branch_name,
            "created_at": state.created_at,
            "status": state.status.value,
            "description": state.description,
        }
        config_path = worktree_path / self.CONFIG_FILE_NAME
        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)

        # 2. Create prd.json template
        prd = {
            "metadata": {
                "version": "1.0.0",
                "created_at": state.created_at,
                "task_name": state.task_name,
                "description": state.description,
            },
            "goal": state.description or f"Complete task: {state.task_name}",
            "objectives": [],
            "stories": [],
        }
        prd_path = worktree_path / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(prd, f, indent=2)

        # 3. Create progress.txt
        progress_path = worktree_path / "progress.txt"
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        with open(progress_path, "w", encoding="utf-8") as f:
            f.write(f"[{timestamp}] Task '{state.task_name}' initialized\n")
            f.write(f"[{timestamp}] Target branch: {state.target_branch}\n")
            if state.description:
                f.write(f"[{timestamp}] Description: {state.description}\n")

        # 4. Create findings.md
        findings_path = worktree_path / "findings.md"
        with open(findings_path, "w", encoding="utf-8") as f:
            f.write(f"# Findings: {state.task_name}\n\n")
            f.write(f"**Created:** {state.created_at}\n")
            f.write(f"**Target Branch:** {state.target_branch}\n")
            if state.description:
                f.write(f"**Description:** {state.description}\n")
            f.write("\n---\n\n")
            f.write("## Notes\n\n")

    def complete(
        self,
        task_name: str | None = None,
        target_branch: str | None = None,
        force: bool = False,
    ) -> tuple[bool, str]:
        """
        Complete a worktree task: verify stories, commit, merge, and cleanup.

        If task_name is not provided, attempts to detect from current directory.

        Args:
            task_name: Name of the task to complete (optional)
            target_branch: Branch to merge into (overrides config if provided)
            force: If True, complete even if stories are incomplete

        Returns:
            Tuple of (success: bool, message: str)
        """
        # Detect task name if not provided
        if not task_name:
            task_name = self._detect_current_task()
            if not task_name:
                return False, "Not in a worktree. Please specify task name."

        worktree_path = self.worktree_dir / task_name

        if not worktree_path.exists():
            return False, f"Worktree not found: {worktree_path}"

        # Read config to get target branch
        config_path = worktree_path / self.CONFIG_FILE_NAME
        if not config_path.exists():
            return False, f"Config file not found: {config_path}"

        with open(config_path, encoding="utf-8") as f:
            config = json.load(f)

        if not target_branch:
            target_branch = config.get("target_branch", "main")

        branch_name = config.get("branch_name", f"task/{task_name}")

        # 1. Verify all stories are complete
        stories_complete, story_msg = self._verify_stories_complete(worktree_path)
        incomplete_stories: list[str] = []

        if not stories_complete:
            if not force:
                return False, f"Cannot complete: {story_msg}"
            # Extract incomplete story IDs for commit message
            incomplete_stories = self._get_incomplete_stories(worktree_path)

        # 2. Check for uncommitted changes and commit if needed
        has_changes = self._has_uncommitted_changes(worktree_path)
        if has_changes:
            commit_success, commit_msg = self._commit_changes(
                worktree_path, task_name, incomplete_stories=incomplete_stories
            )
            if not commit_success:
                return False, f"Failed to commit changes: {commit_msg}"

        # 3. Merge branch into target
        merge_success, merge_msg = self._merge_branch(
            branch_name, target_branch, worktree_path
        )
        if not merge_success:
            return False, f"Failed to merge: {merge_msg}"

        # 4. Remove worktree and delete branch
        cleanup_success, cleanup_msg = self._cleanup_worktree(
            task_name, branch_name
        )
        if not cleanup_success:
            # Non-fatal: merge succeeded but cleanup failed
            return True, f"Merged successfully, but cleanup failed: {cleanup_msg}"

        # Build completion message
        if incomplete_stories:
            return True, (
                f"Task '{task_name}' force-completed and merged into '{target_branch}' "
                f"({len(incomplete_stories)} incomplete stories)"
            )
        return True, f"Task '{task_name}' completed and merged into '{target_branch}'"

    def _detect_current_task(self) -> str | None:
        """Detect task name from current working directory."""
        cwd = Path.cwd().resolve()

        # Check if we're in a worktree directory
        if self.worktree_dir in cwd.parents or cwd.parent == self.worktree_dir:
            # Get the task name from the path
            try:
                rel_path = cwd.relative_to(self.worktree_dir)
                return rel_path.parts[0] if rel_path.parts else None
            except ValueError as e:
                logger.debug(
                    "Could not get relative path from '%s' to worktree dir '%s': %s",
                    cwd, self.worktree_dir, e
                )

        # Check for .planning-config.json in current directory
        config_path = cwd / self.CONFIG_FILE_NAME
        if config_path.exists():
            with open(config_path, encoding="utf-8") as f:
                config = json.load(f)
            return config.get("task_name")

        return None

    def _verify_stories_complete(self, worktree_path: Path) -> tuple[bool, str]:
        """Verify all stories in the PRD are complete."""
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return True, "No PRD found (assuming no stories to complete)"

        with open(prd_path, encoding="utf-8") as f:
            prd = json.load(f)

        stories = prd.get("stories", [])
        if not stories:
            return True, "No stories defined"

        incomplete = []
        for story in stories:
            status = story.get("status", "pending")
            if status != "complete":
                incomplete.append(f"{story.get('id', '?')}: {status}")

        if incomplete:
            return False, f"Incomplete stories: {', '.join(incomplete)}"

        return True, f"All {len(stories)} stories complete"

    def _get_incomplete_stories(self, worktree_path: Path) -> list[str]:
        """
        Get list of incomplete story IDs from the PRD.

        Args:
            worktree_path: Path to the worktree

        Returns:
            List of incomplete story IDs with their status (e.g., ["story-001: pending"])
        """
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return []

        with open(prd_path, encoding="utf-8") as f:
            prd = json.load(f)

        stories = prd.get("stories", [])
        incomplete = []
        for story in stories:
            status = story.get("status", "pending")
            if status != "complete":
                story_id = story.get("id", "unknown")
                incomplete.append(f"{story_id}: {status}")

        return incomplete

    def _has_uncommitted_changes(self, worktree_path: Path) -> bool:
        """Check if there are uncommitted changes in the worktree."""
        try:
            result = self._run_git_command(
                ["status", "--porcelain"], cwd=worktree_path
            )
            return bool(result.stdout.strip())
        except subprocess.CalledProcessError:
            return False

    def _commit_changes(
        self,
        worktree_path: Path,
        task_name: str,
        incomplete_stories: list[str] | None = None,
    ) -> tuple[bool, str]:
        """
        Commit changes in the worktree (excluding planning files).

        Args:
            worktree_path: Path to the worktree
            task_name: Name of the task for commit message
            incomplete_stories: List of incomplete story IDs to include in commit message footer

        Returns:
            Tuple of (success: bool, message: str)
        """
        try:
            # Get list of changed files
            result = self._run_git_command(
                ["status", "--porcelain"], cwd=worktree_path
            )
            changed_files = result.stdout.strip().split("\n")

            # Filter out planning files
            planning_files = {
                "prd.json",
                "progress.txt",
                "findings.md",
                ".planning-config.json",
                ".agent-status.json",
                ".iteration-state.json",
                ".retry-state.json",
            }

            files_to_add = []
            for line in changed_files:
                if not line.strip():
                    continue
                # Parse git status output (e.g., " M file.py", "?? new.py")
                parts = line.split(None, 1)
                if len(parts) >= 2:
                    filename = parts[1].strip()
                    if Path(filename).name not in planning_files:
                        files_to_add.append(filename)

            if not files_to_add:
                return True, "No code changes to commit"

            # Stage files
            for f in files_to_add:
                self._run_git_command(["add", f], cwd=worktree_path)

            # Build commit message
            if incomplete_stories:
                # Force completion: include incomplete stories in footer
                commit_msg = f"feat({task_name}): force-complete task implementation\n\n"
                commit_msg += "WARN: Force-completed with incomplete stories:\n"
                for story in incomplete_stories:
                    commit_msg += f"  - {story}\n"
            else:
                commit_msg = f"feat({task_name}): complete task implementation"

            self._run_git_command(
                ["commit", "-m", commit_msg], cwd=worktree_path
            )

            return True, f"Committed {len(files_to_add)} files"

        except subprocess.CalledProcessError as e:
            return False, str(e.stderr or e)

    def _merge_branch(
        self, branch_name: str, target_branch: str, worktree_path: Path
    ) -> tuple[bool, str]:
        """Merge the task branch into the target branch."""
        try:
            # Switch to target branch in main repo
            self._run_git_command(["checkout", target_branch])

            # Merge the task branch
            self._run_git_command(
                ["merge", branch_name, "--no-ff", "-m", f"Merge {branch_name}"]
            )

            return True, f"Merged {branch_name} into {target_branch}"

        except subprocess.CalledProcessError as e:
            # Try to abort merge if it failed
            try:
                self._run_git_command(["merge", "--abort"], check=False)
            except Exception as abort_error:
                logger.warning(
                    "Failed to abort merge after merge failure: %s", abort_error
                )
            return False, str(e.stderr or e)

    def _cleanup_worktree(
        self, task_name: str, branch_name: str
    ) -> tuple[bool, str]:
        """Remove worktree and delete the task branch."""
        worktree_path = self.worktree_dir / task_name

        try:
            # Remove worktree
            self._run_git_command(
                ["worktree", "remove", str(worktree_path), "--force"]
            )

            # Delete branch
            self._run_git_command(["branch", "-d", branch_name])

            return True, "Worktree and branch removed"

        except subprocess.CalledProcessError as e:
            return False, str(e.stderr or e)

    def list_worktrees(self) -> list[WorktreeInfo]:
        """
        List all Git worktrees with their status.

        Returns:
            List of WorktreeInfo objects
        """
        worktrees = []

        try:
            # Get worktree list in porcelain format
            result = self._run_git_command(["worktree", "list", "--porcelain"])
            lines = result.stdout.strip().split("\n")

            current_worktree: dict = {}
            for line in lines:
                if not line.strip():
                    # Empty line means end of worktree entry
                    if current_worktree:
                        info = self._parse_worktree_info(current_worktree)
                        if info:
                            worktrees.append(info)
                    current_worktree = {}
                elif line.startswith("worktree "):
                    current_worktree["path"] = line[9:]
                elif line.startswith("HEAD "):
                    current_worktree["head"] = line[5:]
                elif line.startswith("branch "):
                    current_worktree["branch"] = line[7:]
                elif line == "detached":
                    current_worktree["detached"] = True

            # Handle last entry
            if current_worktree:
                info = self._parse_worktree_info(current_worktree)
                if info:
                    worktrees.append(info)

        except subprocess.CalledProcessError as e:
            logger.warning(
                "Failed to list git worktrees: %s", e.stderr or e
            )

        return worktrees

    def _parse_worktree_info(self, data: dict) -> WorktreeInfo | None:
        """Parse worktree data into WorktreeInfo object."""
        path_str = data.get("path")
        if not path_str:
            return None

        worktree_path = Path(path_str)

        # Extract branch name (remove refs/heads/ prefix)
        branch = data.get("branch", "")
        if branch.startswith("refs/heads/"):
            branch = branch[11:]

        info = WorktreeInfo(
            worktree_path=worktree_path,
            branch=branch,
            head=data.get("head", ""),
            is_detached=data.get("detached", False),
        )

        # Try to read additional info from planning config
        config_path = worktree_path / self.CONFIG_FILE_NAME
        if config_path.exists():
            try:
                with open(config_path, encoding="utf-8") as f:
                    config = json.load(f)
                info.task_name = config.get("task_name", "")
                info.target_branch = config.get("target_branch", "")
                info.created_at = config.get("created_at", "")
            except Exception as e:
                logger.debug(
                    "Failed to read planning config at '%s': %s", config_path, e
                )

        # Try to read story progress from PRD
        prd_path = worktree_path / "prd.json"
        if prd_path.exists():
            try:
                with open(prd_path, encoding="utf-8") as f:
                    prd = json.load(f)
                stories = prd.get("stories", [])
                info.stories_total = len(stories)
                info.stories_complete = sum(
                    1 for s in stories if s.get("status") == "complete"
                )
                if info.stories_total > 0:
                    info.progress_percentage = (
                        info.stories_complete / info.stories_total * 100
                    )
            except Exception as e:
                logger.debug(
                    "Failed to read PRD at '%s': %s", prd_path, e
                )

        return info


# ============================================================================
# Typer CLI Commands
# ============================================================================

if HAS_TYPER:
    worktree_app = typer.Typer(
        name="worktree",
        help="Git worktree management for parallel task development",
        no_args_is_help=True,
    )
    console = Console()

    def _get_output_manager() -> "OutputManager":
        """Get or create OutputManager instance."""
        from .output import OutputManager

        return OutputManager(console)

    @worktree_app.command("create")
    def create_worktree(
        task_name: str = typer.Argument(
            ..., help="Name of the task (used for directory and branch names)"
        ),
        target_branch: str = typer.Argument(
            ..., help="Branch to merge into when complete"
        ),
        description: str = typer.Option(
            "",
            "--description",
            "-d",
            help="Task description for auto-generating PRD",
        ),
        project_path: str | None = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
    ):
        """
        Create a new worktree for isolated task development.

        Creates a new Git worktree at .worktree/<task-name>/ with a new branch
        'task/<task-name>' and initializes planning files.

        Examples:
            plan-cascade worktree create feature-login main
            plan-cascade worktree create fix-auth develop -d "Fix authentication bug"
        """
        output = _get_output_manager()
        project = Path(project_path) if project_path else Path.cwd()

        try:
            manager = WorktreeManager(project)
            state = manager.create(task_name, target_branch, description)

            output.print_success(f"Created worktree: {state.worktree_path}")
            output.print(f"  [dim]Branch:[/dim] {state.branch_name}")
            output.print(f"  [dim]Target:[/dim] {state.target_branch}")

            if description:
                output.print(f"  [dim]Description:[/dim] {description}")

            output.print()
            output.print_info("Planning files initialized:")
            output.print("  - .planning-config.json")
            output.print("  - prd.json")
            output.print("  - progress.txt")
            output.print("  - findings.md")

            output.print()
            output.print(f"[bold]Next steps:[/bold]")
            output.print(f"  cd {state.worktree_path}")
            output.print("  # Edit prd.json to add stories")
            output.print("  # Or run: plan-cascade run '<task description>'")

        except ValueError as e:
            output.print_error(str(e))
            raise typer.Exit(1)
        except subprocess.CalledProcessError as e:
            output.print_error(f"Git error: {e.stderr or e}")
            raise typer.Exit(1)

    @worktree_app.command("complete")
    def complete_worktree(
        task_name: str | None = typer.Argument(
            None,
            help="Task name to complete (auto-detected if in worktree)",
        ),
        target_branch: str | None = typer.Option(
            None,
            "--target",
            "-t",
            help="Override target branch from config",
        ),
        project_path: str | None = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
        force: bool = typer.Option(
            False,
            "--force",
            "-f",
            help="Force completion even if stories are incomplete",
        ),
    ):
        """
        Complete a worktree task: verify, commit, merge, and cleanup.

        Verifies all stories are complete, commits code changes (excluding
        planning files), merges to target branch, and removes the worktree.

        Can be run from within the worktree or from anywhere with task name.

        Examples:
            plan-cascade worktree complete              # Auto-detect from cwd
            plan-cascade worktree complete feature-login
            plan-cascade worktree complete --target main
        """
        output = _get_output_manager()
        project = Path(project_path) if project_path else Path.cwd()

        try:
            manager = WorktreeManager(project)

            output.print_info(
                f"Completing task: {task_name or '(auto-detect)'}"
            )

            # If force is requested, show warning about incomplete stories first
            if force:
                # Detect task name for warning display
                detected_task = task_name or manager._detect_current_task()
                if detected_task:
                    worktree_path = manager.worktree_dir / detected_task
                    incomplete = manager._get_incomplete_stories(worktree_path)
                    if incomplete:
                        output.print_warning(
                            "WARNING: Force-completing with incomplete stories!"
                        )
                        output.print("[yellow]Incomplete stories:[/yellow]")
                        for story in incomplete:
                            output.print(f"  [dim]-[/dim] {story}")
                        output.print()

            success, message = manager.complete(task_name, target_branch, force=force)

            if success:
                output.print_success(message)
            else:
                output.print_error(message)
                output.print("[dim]Use --force to complete anyway[/dim]")
                raise typer.Exit(1)

        except subprocess.CalledProcessError as e:
            output.print_error(f"Git error: {e.stderr or e}")
            raise typer.Exit(1)

    @worktree_app.command("list")
    def list_worktrees(
        project_path: str | None = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
        json_output: bool = typer.Option(
            False,
            "--json",
            "-j",
            help="Output in JSON format",
        ),
    ):
        """
        List all active worktrees with their status.

        Shows worktree path, branch, task info, and story progress.

        Examples:
            plan-cascade worktree list
            plan-cascade worktree list --json
        """
        output = _get_output_manager()
        project = Path(project_path) if project_path else Path.cwd()

        try:
            manager = WorktreeManager(project)
            worktrees = manager.list_worktrees()

            # Filter to only task worktrees (in .worktree directory)
            task_worktrees = [
                w
                for w in worktrees
                if str(manager.worktree_dir) in str(w.worktree_path)
            ]

            if json_output:
                # JSON output
                data = []
                for w in task_worktrees:
                    data.append(
                        {
                            "path": str(w.worktree_path),
                            "branch": w.branch,
                            "task_name": w.task_name,
                            "target_branch": w.target_branch,
                            "created_at": w.created_at,
                            "stories_total": w.stories_total,
                            "stories_complete": w.stories_complete,
                            "progress_percentage": w.progress_percentage,
                        }
                    )
                console.print_json(json.dumps(data, indent=2))
                return

            # Table output
            if not task_worktrees:
                output.print_info("No active worktrees found")
                output.print(
                    "[dim]Create one with: plan-cascade worktree create <name> <branch>[/dim]"
                )
                return

            table = Table(title="Active Worktrees", show_header=True)
            table.add_column("Task", style="cyan")
            table.add_column("Branch", style="white")
            table.add_column("Target", style="dim")
            table.add_column("Progress", justify="right")
            table.add_column("Created", style="dim")

            for w in task_worktrees:
                # Format progress
                if w.stories_total > 0:
                    progress = f"{w.stories_complete}/{w.stories_total} ({w.progress_percentage:.0f}%)"
                    if w.stories_complete == w.stories_total:
                        progress = f"[green]{progress}[/green]"
                    elif w.stories_complete > 0:
                        progress = f"[yellow]{progress}[/yellow]"
                else:
                    progress = "[dim]-[/dim]"

                # Format date
                created = w.created_at[:10] if w.created_at else "-"

                table.add_row(
                    w.task_name or w.worktree_path.name,
                    w.branch or "-",
                    w.target_branch or "-",
                    progress,
                    created,
                )

            console.print(table)

            # Summary
            total = len(task_worktrees)
            complete = sum(
                1
                for w in task_worktrees
                if w.stories_total > 0 and w.stories_complete == w.stories_total
            )
            output.print()
            output.print(
                f"[dim]Total: {total} worktree(s), {complete} ready to complete[/dim]"
            )

        except subprocess.CalledProcessError as e:
            output.print_error(f"Git error: {e.stderr or e}")
            raise typer.Exit(1)

else:
    # Fallback when typer is not installed
    worktree_app = None


def main():
    """CLI entry point for worktree commands."""
    if HAS_TYPER:
        worktree_app()
    else:
        print("Worktree commands require 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
