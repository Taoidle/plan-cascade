#!/usr/bin/env python3
"""
Context Recovery System for Plan Cascade

Provides auto-detection and recovery of interrupted tasks:
- Mega-plan context (multi-feature projects)
- Hybrid-worktree context (isolated worktree development)
- Hybrid-auto context (single PRD execution)

Analyzes state files and progress markers to determine task state
and provide recovery recommendations.

Now integrated with PathResolver for unified path resolution.
"""

from __future__ import annotations

import json
import re
import time
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from plan_cascade.state.path_resolver import PathResolver


class ContextType(str, Enum):
    """Type of context detected."""

    MEGA_PLAN = "mega-plan"
    HYBRID_WORKTREE = "hybrid-worktree"
    HYBRID_AUTO = "hybrid-auto"
    UNKNOWN = "unknown"


class TaskState(str, Enum):
    """State of a task."""

    NEEDS_PRD = "needs_prd"
    NEEDS_APPROVAL = "needs_approval"
    EXECUTING = "executing"
    COMPLETE = "complete"
    FAILED = "failed"


class PrdStatus(str, Enum):
    """Status of a PRD file."""

    MISSING = "missing"
    CORRUPTED = "corrupted"
    EMPTY = "empty"
    VALID = "valid"


@dataclass
class ContextRecoveryState:
    """
    Data class holding the recovery state information.

    Attributes:
        context_type: Type of context detected
        task_state: Current state of the task
        prd_status: Status of the PRD file
        completed_stories: List of completed story IDs
        failed_stories: List of failed story IDs
        in_progress_stories: List of in-progress story IDs
        pending_stories: List of pending story IDs
        last_activity: Timestamp of last activity
        project_path: Path to the project
        worktree_path: Path to worktree (if applicable)
        task_name: Name of the task
        target_branch: Target branch for merging
        total_stories: Total number of stories
        completion_percentage: Percentage of stories completed
        error_message: Any error message from detection
    """

    context_type: ContextType = ContextType.UNKNOWN
    task_state: TaskState = TaskState.NEEDS_PRD
    prd_status: PrdStatus = PrdStatus.MISSING
    completed_stories: list[str] = field(default_factory=list)
    failed_stories: list[str] = field(default_factory=list)
    in_progress_stories: list[str] = field(default_factory=list)
    pending_stories: list[str] = field(default_factory=list)
    last_activity: str = ""
    project_path: Path = field(default_factory=Path)
    worktree_path: Path | None = None
    task_name: str = ""
    target_branch: str = "main"
    total_stories: int = 0
    completion_percentage: float = 0.0
    error_message: str = ""

    # Additional mega-plan specific fields
    mega_plan_features: list[dict] = field(default_factory=list)
    mega_plan_progress: dict = field(default_factory=dict)

    # Stage state fields (from resume_detector integration)
    stage_state_info: dict | None = None
    current_stage: str | None = None
    stage_progress_percent: int = 0
    stage_completed_stages: list[str] = field(default_factory=list)
    stage_failed_stages: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "context_type": self.context_type.value,
            "task_state": self.task_state.value,
            "prd_status": self.prd_status.value,
            "completed_stories": self.completed_stories,
            "failed_stories": self.failed_stories,
            "in_progress_stories": self.in_progress_stories,
            "pending_stories": self.pending_stories,
            "last_activity": self.last_activity,
            "project_path": str(self.project_path),
            "worktree_path": str(self.worktree_path) if self.worktree_path else None,
            "task_name": self.task_name,
            "target_branch": self.target_branch,
            "total_stories": self.total_stories,
            "completion_percentage": self.completion_percentage,
            "error_message": self.error_message,
            "mega_plan_features": self.mega_plan_features,
            "mega_plan_progress": self.mega_plan_progress,
            "stage_state_info": self.stage_state_info,
            "current_stage": self.current_stage,
            "stage_progress_percent": self.stage_progress_percent,
            "stage_completed_stages": self.stage_completed_stages,
            "stage_failed_stages": self.stage_failed_stages,
        }


@dataclass
class RecoveryAction:
    """Represents a recovery action to take."""

    action: str
    description: str
    command: str
    priority: int = 1  # Lower is higher priority

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "action": self.action,
            "description": self.description,
            "command": self.command,
            "priority": self.priority,
        }


@dataclass
class RecoveryPlan:
    """Plan for recovering an interrupted task."""

    state: ContextRecoveryState
    actions: list[RecoveryAction] = field(default_factory=list)
    can_auto_resume: bool = False
    warnings: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "state": self.state.to_dict(),
            "actions": [a.to_dict() for a in self.actions],
            "can_auto_resume": self.can_auto_resume,
            "warnings": self.warnings,
        }


class ContextRecoveryManager:
    """
    Manages context detection and recovery for Plan Cascade.

    Detects the type of interrupted task and provides recovery recommendations.

    Can operate in two modes:
    - New mode (with PathResolver): Files stored in ~/.plan-cascade/<project-id>/
    - Legacy mode: Files stored in project root (backward compatible)
    """

    # Progress markers (both old-style and new-style)
    COMPLETE_MARKERS = [
        r"\[COMPLETE\]",
        r"\[STORY_COMPLETE:\s*(story-[\w-]+)\]",
        r"\[FEATURE_COMPLETE:\s*(feature-[\w-]+)\]",
    ]

    IN_PROGRESS_MARKERS = [
        r"\[IN_PROGRESS\]",
        r"\[START\]",
        r"\[EXECUTING\]",
    ]

    FAILED_MARKERS = [
        r"\[FAILED\]",
        r"\[ERROR\]",
    ]

    def __init__(
        self,
        project_root: Path,
        path_resolver: PathResolver | None = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the context recovery manager.

        Args:
            project_root: Root directory of the project
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for all paths (backward compatible).
                If None, defaults to True when path_resolver is not provided for
                backward compatibility. If False, uses new ~/.plan-cascade/<project-id>/
                structure.
        """
        self.project_root = Path(project_root).resolve()

        # Determine legacy mode and set up PathResolver
        if path_resolver is not None:
            # Use provided resolver's mode
            self._path_resolver = path_resolver
        else:
            # Create default resolver
            # Default to legacy mode for backward compatibility when no resolver provided
            if legacy_mode is None:
                legacy_mode = True
            from plan_cascade.state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Initialize paths using PathResolver
        self._init_paths()

    def _init_paths(self) -> None:
        """Initialize all file paths using PathResolver."""
        resolver = self._path_resolver

        # Worktree directory
        self.worktree_dir = resolver.get_worktree_dir()

        # Mega plan path
        self.mega_plan_path = resolver.get_mega_plan_path()

        # PRD path
        self.prd_path = resolver.get_prd_path()

        # Mega status path
        self.mega_status_path = resolver.get_mega_status_path()

        # Config path - in legacy mode it's in project root
        # In new mode, we check both locations for compatibility
        if resolver.is_legacy_mode():
            self.config_path = self.project_root / ".planning-config.json"
        else:
            # New mode: check state directory first
            self.config_path = resolver.get_state_file_path("planning-config.json")

        # Progress file - always in project root (user-visible)
        self.progress_path = self.project_root / "progress.txt"

    @property
    def path_resolver(self) -> PathResolver:
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def detect_context(self) -> ContextRecoveryState:
        """
        Detect the context type and state from project files.

        Returns:
            ContextRecoveryState with detected information
        """
        state = ContextRecoveryState(project_path=self.project_root)

        # First, enrich state with stage state info if available
        self._enrich_with_stage_state(state)

        # Check for mega-plan context first (using PathResolver path)
        if self.mega_plan_path.exists():
            return self._detect_mega_plan_context(state)

        # Check for hybrid-worktree context
        if self._is_in_worktree():
            return self._detect_hybrid_worktree_context(state)

        # Check for worktree directories in project
        if self.worktree_dir.exists() and any(self.worktree_dir.iterdir()):
            # There are worktrees but we're not in one
            state.context_type = ContextType.HYBRID_WORKTREE
            state.worktree_path = None
            return self._scan_worktrees_for_recovery(state)

        # Check for hybrid-auto context (prd.json using PathResolver path)
        # Also check legacy location if in new mode for backward compatibility
        prd_exists = self.prd_path.exists()
        if not prd_exists and not self.is_legacy_mode():
            # In new mode, also check legacy location (project root)
            legacy_prd = self.project_root / "prd.json"
            if legacy_prd.exists():
                prd_exists = True
                # Update prd_path to use the legacy location for subsequent operations
                self.prd_path = legacy_prd

        if prd_exists:
            return self._detect_hybrid_auto_context(state)

        # Check for .planning-config.json (using computed config path)
        # Also check legacy location if in new mode
        config_exists = self.config_path.exists()
        if not config_exists and not self.is_legacy_mode():
            # In new mode, also check legacy location for backward compatibility
            legacy_config = self.project_root / ".planning-config.json"
            config_exists = legacy_config.exists()

        if config_exists:
            return self._detect_from_config(state)

        # Check if we have stage state even without other context files
        # This can happen if execution started but planning files were cleaned up
        if state.stage_state_info is not None:
            state.context_type = ContextType.HYBRID_AUTO
            state.task_state = TaskState.EXECUTING
            return state

        # No context found
        state.context_type = ContextType.UNKNOWN
        state.task_state = TaskState.NEEDS_PRD
        state.prd_status = PrdStatus.MISSING
        state.error_message = "No task context found in this directory"

        return state

    def _enrich_with_stage_state(self, state: ContextRecoveryState) -> None:
        """
        Enrich the recovery state with stage state information.

        Uses the resume_detector module to detect incomplete stage state
        and adds that information to the ContextRecoveryState.

        Args:
            state: ContextRecoveryState to enrich
        """
        try:
            from .resume_detector import detect_incomplete_state

            incomplete = detect_incomplete_state(
                self._path_resolver,
                include_prd_check=False,  # PRD check is done separately
            )

            if incomplete is not None:
                state.stage_state_info = incomplete.to_dict()
                state.current_stage = incomplete.last_stage
                state.stage_progress_percent = incomplete.progress_percent
                state.stage_completed_stages = incomplete.completed_stages
                state.stage_failed_stages = incomplete.failed_stages

                # Update last_activity if stage state has more recent timestamp
                if incomplete.timestamp and (
                    not state.last_activity or incomplete.timestamp > state.last_activity
                ):
                    state.last_activity = incomplete.timestamp

        except Exception:
            # Non-critical - continue without stage state info
            pass

    def _is_in_worktree(self) -> bool:
        """Check if current directory is inside a worktree.

        Detects worktrees in both legacy (.worktree in project) and new
        (~/.plan-cascade/<project-id>/.worktree/) directory structures.
        """
        # Check for .planning-config.json with worktree metadata
        # Check both computed config path and legacy location
        config_paths_to_check = [self.config_path]
        if not self.is_legacy_mode():
            config_paths_to_check.append(self.project_root / ".planning-config.json")

        for config_path in config_paths_to_check:
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    # If it has branch_name like task/* it's a worktree
                    if config.get("branch_name", "").startswith("task/"):
                        return True
                except (json.JSONDecodeError, OSError):
                    pass

        # Check if we're physically inside .worktree directory
        try:
            # Check if any parent is named .worktree
            for parent in self.project_root.parents:
                if parent.name == ".worktree":
                    return True
        except Exception:
            pass

        # Check if we're inside the new user directory worktree structure
        # Path pattern: ~/.plan-cascade/<project-id>/.worktree/<task-name>
        if not self.is_legacy_mode():
            try:
                data_dir = self._path_resolver.get_data_dir()
                # Check if project_root is under the data directory's .worktree
                project_root_str = str(self.project_root)
                data_dir_str = str(data_dir)
                if project_root_str.startswith(data_dir_str):
                    # We're under the data directory, check for .worktree pattern
                    relative = self.project_root.relative_to(data_dir)
                    parts = relative.parts
                    # Pattern should be: <project-id>/.worktree/<task-name>
                    if len(parts) >= 3 and parts[1] == ".worktree":
                        return True
            except (ValueError, Exception):
                pass

        return False

    def _detect_mega_plan_context(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Detect mega-plan context and state."""
        state.context_type = ContextType.MEGA_PLAN

        # Use PathResolver-resolved path
        try:
            with open(self.mega_plan_path, encoding="utf-8") as f:
                mega_plan = json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            state.prd_status = PrdStatus.CORRUPTED
            state.task_state = TaskState.NEEDS_PRD
            state.error_message = f"Could not read mega-plan.json: {e}"
            return state

        # Analyze features
        features = mega_plan.get("features", [])
        if not features:
            state.prd_status = PrdStatus.EMPTY
            state.task_state = TaskState.NEEDS_PRD
            return state

        state.prd_status = PrdStatus.VALID
        state.mega_plan_features = features
        state.target_branch = mega_plan.get("target_branch", "main")
        state.task_name = mega_plan.get("goal", "Mega Plan")[:50]

        # Calculate progress
        total = len(features)
        complete = sum(1 for f in features if f.get("status") == "complete")
        in_progress = sum(1 for f in features if f.get("status") in ["in_progress", "approved", "prd_generated"])
        failed = sum(1 for f in features if f.get("status") == "failed")
        pending = sum(1 for f in features if f.get("status") == "pending")

        state.mega_plan_progress = {
            "total": total,
            "complete": complete,
            "in_progress": in_progress,
            "failed": failed,
            "pending": pending,
            "percentage": (complete / total * 100) if total > 0 else 0,
        }

        # Populate story lists with feature IDs
        state.completed_stories = [f["id"] for f in features if f.get("status") == "complete"]
        state.failed_stories = [f["id"] for f in features if f.get("status") == "failed"]
        state.in_progress_stories = [f["id"] for f in features if f.get("status") in ["in_progress", "approved", "prd_generated"]]
        state.pending_stories = [f["id"] for f in features if f.get("status") == "pending"]
        state.total_stories = total
        state.completion_percentage = state.mega_plan_progress["percentage"]

        # Determine task state
        if complete == total:
            state.task_state = TaskState.COMPLETE
        elif failed > 0 and in_progress == 0 and pending == 0:
            state.task_state = TaskState.FAILED
        elif in_progress > 0 or complete > 0:
            state.task_state = TaskState.EXECUTING
        elif pending == total:
            state.task_state = TaskState.NEEDS_APPROVAL
        else:
            state.task_state = TaskState.EXECUTING

        # Also check progress.txt for completion markers (may override mega-plan.json status)
        self._scan_progress_file_for_features(state, features)

        # Recalculate progress after scanning progress file
        complete = len(state.completed_stories)
        in_progress = len(state.in_progress_stories)
        failed = len(state.failed_stories)
        pending = len(state.pending_stories)

        state.mega_plan_progress = {
            "total": total,
            "complete": complete,
            "in_progress": in_progress,
            "failed": failed,
            "pending": pending,
            "percentage": (complete / total * 100) if total > 0 else 0,
        }
        state.completion_percentage = state.mega_plan_progress["percentage"]

        # Re-determine task state after scanning progress
        if complete == total:
            state.task_state = TaskState.COMPLETE
        elif failed > 0 and in_progress == 0 and pending == 0:
            state.task_state = TaskState.FAILED
        elif in_progress > 0 or complete > 0:
            state.task_state = TaskState.EXECUTING
        elif pending == total:
            state.task_state = TaskState.NEEDS_APPROVAL
        else:
            state.task_state = TaskState.EXECUTING

        # Get last activity from mega-status (using PathResolver-resolved path)
        if self.mega_status_path.exists():
            try:
                with open(self.mega_status_path, encoding="utf-8") as f:
                    status = json.load(f)
                state.last_activity = status.get("updated_at", "")
            except (json.JSONDecodeError, OSError):
                pass

        return state

    def _scan_progress_file_for_features(self, state: ContextRecoveryState, features: list[dict]) -> None:
        """Scan progress.txt for feature completion markers and update state."""
        # Use instance progress_path (always in project root)
        if not self.progress_path.exists():
            return

        try:
            with open(self.progress_path, encoding="utf-8") as f:
                content = f.read()
        except OSError:
            return

        # Build feature ID set for validation
        feature_ids = {f["id"] for f in features}

        # Parse progress file for feature completion markers
        lines = content.split("\n")
        last_timestamp = ""

        for line in lines:
            # Extract timestamp if present
            timestamp_match = re.match(r"\[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\]", line)
            if timestamp_match:
                last_timestamp = timestamp_match.group(1)

            # Check for feature completion markers
            feature_complete_match = re.search(r"\[FEATURE_COMPLETE:\s*(feature-[\w-]+)\]", line)
            if feature_complete_match:
                feature_id = feature_complete_match.group(1)
                if feature_id in feature_ids:
                    # Move from other lists to completed
                    if feature_id in state.pending_stories:
                        state.pending_stories.remove(feature_id)
                    if feature_id in state.in_progress_stories:
                        state.in_progress_stories.remove(feature_id)
                    if feature_id in state.failed_stories:
                        state.failed_stories.remove(feature_id)
                    if feature_id not in state.completed_stories:
                        state.completed_stories.append(feature_id)

        if last_timestamp:
            state.last_activity = last_timestamp

    def _detect_hybrid_worktree_context(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Detect hybrid-worktree context and state."""
        state.context_type = ContextType.HYBRID_WORKTREE
        state.worktree_path = self.project_root

        # Read config - check both computed path and legacy location
        config_found = False
        for config_path in [self.config_path, self.project_root / ".planning-config.json"]:
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    state.task_name = config.get("task_name", "")
                    state.target_branch = config.get("target_branch", "main")
                    config_found = True
                    break
                except (json.JSONDecodeError, OSError):
                    pass

        # Analyze PRD
        return self._analyze_prd_and_progress(state)

    def _detect_hybrid_auto_context(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Detect hybrid-auto context and state."""
        state.context_type = ContextType.HYBRID_AUTO

        # Read task name from PRD or config - check both computed path and legacy location
        for config_path in [self.config_path, self.project_root / ".planning-config.json"]:
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    state.task_name = config.get("task_name", "")
                    state.target_branch = config.get("target_branch", "main")
                    break
                except (json.JSONDecodeError, OSError):
                    pass

        # Analyze PRD
        return self._analyze_prd_and_progress(state)

    def _detect_from_config(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Detect context from .planning-config.json."""
        # Try computed config path first, then legacy location
        config = None
        for config_path in [self.config_path, self.project_root / ".planning-config.json"]:
            if config_path.exists():
                try:
                    with open(config_path, encoding="utf-8") as f:
                        config = json.load(f)
                    break
                except (json.JSONDecodeError, OSError):
                    pass

        if config is None:
            state.error_message = "Could not read config file"
            return state

        # Determine context type from config
        if config.get("branch_name", "").startswith("task/"):
            state.context_type = ContextType.HYBRID_WORKTREE
            state.worktree_path = self.project_root
        else:
            state.context_type = ContextType.HYBRID_AUTO

        state.task_name = config.get("task_name", "")
        state.target_branch = config.get("target_branch", "main")

        return self._analyze_prd_and_progress(state)

    def _analyze_prd_and_progress(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Analyze PRD and progress files for state."""
        # Use PathResolver-resolved path
        # Check PRD status
        if not self.prd_path.exists():
            state.prd_status = PrdStatus.MISSING
            state.task_state = TaskState.NEEDS_PRD
            return state

        try:
            with open(self.prd_path, encoding="utf-8") as f:
                prd = json.load(f)
        except json.JSONDecodeError:
            state.prd_status = PrdStatus.CORRUPTED
            state.task_state = TaskState.NEEDS_PRD
            return state
        except OSError as e:
            state.prd_status = PrdStatus.MISSING
            state.task_state = TaskState.NEEDS_PRD
            state.error_message = str(e)
            return state

        # Check if PRD has stories
        stories = prd.get("stories", [])
        if not stories:
            state.prd_status = PrdStatus.EMPTY
            state.task_state = TaskState.NEEDS_PRD
            return state

        state.prd_status = PrdStatus.VALID

        # Get task name if not set
        if not state.task_name:
            state.task_name = prd.get("goal", "")[:50] or prd.get("metadata", {}).get("task_name", "")

        # Analyze stories
        state.total_stories = len(stories)
        state.completed_stories = []
        state.failed_stories = []
        state.in_progress_stories = []
        state.pending_stories = []

        for story in stories:
            story_id = story.get("id", "")
            status = story.get("status", "pending")

            if status == "complete":
                state.completed_stories.append(story_id)
            elif status == "failed":
                state.failed_stories.append(story_id)
            elif status in ["in_progress", "running"]:
                state.in_progress_stories.append(story_id)
            else:
                state.pending_stories.append(story_id)

        # Calculate completion percentage
        if state.total_stories > 0:
            state.completion_percentage = len(state.completed_stories) / state.total_stories * 100

        # Also check progress.txt for markers
        self._scan_progress_file(state)

        # Determine task state
        if len(state.completed_stories) == state.total_stories:
            state.task_state = TaskState.COMPLETE
        elif state.failed_stories and not state.in_progress_stories and not state.pending_stories:
            state.task_state = TaskState.FAILED
        elif state.in_progress_stories or state.completed_stories:
            state.task_state = TaskState.EXECUTING
        else:
            state.task_state = TaskState.NEEDS_APPROVAL

        return state

    def _scan_progress_file(self, state: ContextRecoveryState) -> None:
        """Scan progress.txt for completion markers."""
        # Use instance progress_path (always in project root)
        if not self.progress_path.exists():
            return

        try:
            with open(self.progress_path, encoding="utf-8") as f:
                content = f.read()
        except OSError:
            return

        # Parse progress file for markers
        lines = content.split("\n")
        last_timestamp = ""

        for line in lines:
            # Extract timestamp if present
            timestamp_match = re.match(r"\[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\]", line)
            if timestamp_match:
                last_timestamp = timestamp_match.group(1)

            # Check for story completion markers (new style)
            story_complete_match = re.search(r"\[STORY_COMPLETE:\s*(story-[\w-]+)\]", line)
            if story_complete_match:
                story_id = story_complete_match.group(1)
                if story_id not in state.completed_stories:
                    state.completed_stories.append(story_id)
                continue

            # Check for old-style complete markers
            if "[COMPLETE]" in line:
                # Try to extract story ID
                story_match = re.search(r"(story-[\w-]+)", line)
                if story_match:
                    story_id = story_match.group(1)
                    if story_id not in state.completed_stories:
                        state.completed_stories.append(story_id)

            # Check for feature completion markers
            feature_complete_match = re.search(r"\[FEATURE_COMPLETE:\s*(feature-[\w-]+)\]", line)
            if feature_complete_match:
                feature_id = feature_complete_match.group(1)
                # Add to completed if not already there
                if feature_id not in state.completed_stories:
                    state.completed_stories.append(feature_id)

        state.last_activity = last_timestamp

    def _scan_worktrees_for_recovery(self, state: ContextRecoveryState) -> ContextRecoveryState:
        """Scan worktree directory for interrupted tasks.

        Uses PathResolver-resolved worktree directory which may be in:
        - Legacy mode: <project-root>/.worktree/
        - New mode: ~/.plan-cascade/<project-id>/.worktree/
        """
        active_worktrees = []

        if not self.worktree_dir.exists():
            return state

        for worktree_path in self.worktree_dir.iterdir():
            if not worktree_path.is_dir():
                continue

            # Check both .planning-config.json locations for worktree config
            config = None
            for config_path in [
                worktree_path / ".planning-config.json",
                # In new mode, config might be in state subdirectory
                worktree_path / ".state" / "planning-config.json"
            ]:
                if config_path.exists():
                    try:
                        with open(config_path, encoding="utf-8") as f:
                            config = json.load(f)
                        break
                    except (json.JSONDecodeError, OSError):
                        pass

            if config is not None:
                active_worktrees.append({
                    "path": worktree_path,
                    "task_name": config.get("task_name", worktree_path.name),
                    "target_branch": config.get("target_branch", "main"),
                    "status": config.get("status", "active"),
                })
            else:
                # Still count as active worktree even without config
                active_worktrees.append({
                    "path": worktree_path,
                    "task_name": worktree_path.name,
                    "target_branch": "main",
                    "status": "unknown",
                })

        if active_worktrees:
            state.context_type = ContextType.HYBRID_WORKTREE
            # Store worktree info in a way that can be accessed
            state.mega_plan_features = active_worktrees  # Reuse this field
            state.total_stories = len(active_worktrees)
            state.task_state = TaskState.EXECUTING

        return state

    def generate_recovery_plan(self, state: ContextRecoveryState | None = None) -> RecoveryPlan:
        """
        Generate a recovery plan based on detected state.

        Args:
            state: Optional pre-detected state, will detect if not provided

        Returns:
            RecoveryPlan with recommended actions
        """
        if state is None:
            state = self.detect_context()

        plan = RecoveryPlan(state=state)

        # Add stage-level information to the plan if available
        self._enrich_plan_with_stage_info(state, plan)

        # Handle each context type
        if state.context_type == ContextType.UNKNOWN:
            plan.actions.append(RecoveryAction(
                action="start_new",
                description="No task found. Start a new task.",
                command="plan-cascade run '<description>'",
                priority=1,
            ))
            plan.can_auto_resume = False
            return plan

        if state.context_type == ContextType.MEGA_PLAN:
            return self._plan_mega_recovery(state, plan)

        if state.context_type == ContextType.HYBRID_WORKTREE:
            return self._plan_worktree_recovery(state, plan)

        if state.context_type == ContextType.HYBRID_AUTO:
            return self._plan_hybrid_auto_recovery(state, plan)

        return plan

    def _plan_mega_recovery(self, state: ContextRecoveryState, plan: RecoveryPlan) -> RecoveryPlan:
        """Generate recovery plan for mega-plan context."""
        if state.task_state == TaskState.COMPLETE:
            plan.actions.append(RecoveryAction(
                action="complete",
                description="All features complete. Run completion to finalize.",
                command="plan-cascade mega complete",
                priority=1,
            ))
            plan.can_auto_resume = True

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            plan.actions.append(RecoveryAction(
                action="approve",
                description="Mega-plan needs approval to start execution.",
                command="plan-cascade mega approve",
                priority=1,
            ))
            plan.can_auto_resume = True

        elif state.task_state == TaskState.NEEDS_PRD:
            if state.prd_status == PrdStatus.CORRUPTED:
                plan.warnings.append("mega-plan.json is corrupted and needs to be regenerated")
            plan.actions.append(RecoveryAction(
                action="regenerate_plan",
                description="Generate or fix mega-plan.",
                command="plan-cascade mega plan '<description>'",
                priority=1,
            ))
            plan.can_auto_resume = False

        elif state.task_state == TaskState.EXECUTING:
            plan.actions.append(RecoveryAction(
                action="resume_mega",
                description=f"Resume execution. Progress: {state.completion_percentage:.0f}%",
                command="plan-cascade mega resume",
                priority=1,
            ))
            plan.can_auto_resume = True

            if state.failed_stories:
                plan.warnings.append(f"{len(state.failed_stories)} feature(s) failed: {', '.join(state.failed_stories[:3])}")
                plan.actions.append(RecoveryAction(
                    action="view_status",
                    description="View detailed status to investigate failures.",
                    command="plan-cascade mega status --verbose",
                    priority=2,
                ))

        elif state.task_state == TaskState.FAILED:
            plan.warnings.append("All remaining features have failed")
            plan.actions.append(RecoveryAction(
                action="view_status",
                description="View status to investigate failures.",
                command="plan-cascade mega status --verbose",
                priority=1,
            ))
            plan.actions.append(RecoveryAction(
                action="force_complete",
                description="Force completion despite failures.",
                command="plan-cascade mega complete --force",
                priority=2,
            ))
            plan.can_auto_resume = False

        return plan

    def _plan_worktree_recovery(self, state: ContextRecoveryState, plan: RecoveryPlan) -> RecoveryPlan:
        """Generate recovery plan for hybrid-worktree context."""
        if state.worktree_path is None:
            # Multiple worktrees found, not in one
            worktrees = state.mega_plan_features  # We stored worktree info here
            if worktrees:
                plan.warnings.append(f"Found {len(worktrees)} active worktree(s). Change to a worktree directory to resume.")
                for wt in worktrees[:3]:
                    plan.actions.append(RecoveryAction(
                        action="change_dir",
                        description=f"Resume task: {wt.get('task_name', 'unknown')}",
                        command=f"cd {wt['path']}",
                        priority=1,
                    ))
            plan.can_auto_resume = False
            return plan

        if state.task_state == TaskState.COMPLETE:
            plan.actions.append(RecoveryAction(
                action="complete_worktree",
                description="All stories complete. Run completion to merge and cleanup.",
                command="plan-cascade worktree complete",
                priority=1,
            ))
            plan.can_auto_resume = True

        elif state.task_state == TaskState.NEEDS_PRD:
            plan.actions.append(RecoveryAction(
                action="generate_prd",
                description="Generate PRD for this worktree task.",
                command=f"plan-cascade run '<description>' --project {state.worktree_path}",
                priority=1,
            ))
            plan.can_auto_resume = False

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            plan.actions.append(RecoveryAction(
                action="approve",
                description="PRD ready. Start execution.",
                command="plan-cascade auto-run",
                priority=1,
            ))
            plan.can_auto_resume = True

        elif state.task_state == TaskState.EXECUTING:
            plan.actions.append(RecoveryAction(
                action="resume_execution",
                description=f"Resume execution. Progress: {state.completion_percentage:.0f}%",
                command="plan-cascade auto-run",
                priority=1,
            ))
            plan.can_auto_resume = True

            if state.failed_stories:
                plan.warnings.append(f"{len(state.failed_stories)} story(ies) failed")

        elif state.task_state == TaskState.FAILED:
            plan.warnings.append("All remaining stories have failed")
            plan.actions.append(RecoveryAction(
                action="view_status",
                description="View status to investigate failures.",
                command="plan-cascade status",
                priority=1,
            ))
            plan.can_auto_resume = False

        return plan

    def _plan_hybrid_auto_recovery(self, state: ContextRecoveryState, plan: RecoveryPlan) -> RecoveryPlan:
        """Generate recovery plan for hybrid-auto context."""
        if state.task_state == TaskState.COMPLETE:
            plan.actions.append(RecoveryAction(
                action="view_summary",
                description="All stories complete. View summary.",
                command="plan-cascade status",
                priority=1,
            ))
            plan.can_auto_resume = False

        elif state.task_state == TaskState.NEEDS_PRD:
            if state.prd_status == PrdStatus.CORRUPTED:
                plan.warnings.append("prd.json is corrupted and needs to be regenerated")
            plan.actions.append(RecoveryAction(
                action="generate_prd",
                description="Generate or fix PRD.",
                command="plan-cascade run '<description>'",
                priority=1,
            ))
            plan.can_auto_resume = False

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            plan.actions.append(RecoveryAction(
                action="approve_and_run",
                description="PRD ready. Start execution.",
                command="plan-cascade auto-run",
                priority=1,
            ))
            plan.can_auto_resume = True

        elif state.task_state == TaskState.EXECUTING:
            plan.actions.append(RecoveryAction(
                action="resume_execution",
                description=f"Resume execution. Progress: {state.completion_percentage:.0f}%",
                command="plan-cascade auto-run",
                priority=1,
            ))
            plan.can_auto_resume = True

            if state.failed_stories:
                plan.warnings.append(f"{len(state.failed_stories)} story(ies) failed: {', '.join(state.failed_stories[:3])}")

        elif state.task_state == TaskState.FAILED:
            plan.warnings.append("All remaining stories have failed")
            plan.actions.append(RecoveryAction(
                action="view_status",
                description="View status to investigate failures.",
                command="plan-cascade status",
                priority=1,
            ))
            plan.can_auto_resume = False

        return plan

    def _enrich_plan_with_stage_info(
        self,
        state: ContextRecoveryState,
        plan: RecoveryPlan,
    ) -> None:
        """
        Enrich recovery plan with stage-level information.

        Adds warnings and actions based on stage state if available.

        Args:
            state: Recovery state with stage info
            plan: Recovery plan to enrich
        """
        if state.stage_state_info is None:
            return

        # Add stage progress information
        if state.stage_progress_percent > 0:
            stage_info = state.stage_state_info
            current_stage = stage_info.get("last_stage")

            if current_stage:
                # Add information about current stage
                if stage_info.get("last_stage_status") == "failed":
                    errors = stage_info.get("error_messages", [])
                    error_preview = errors[0][:50] if errors else "Unknown error"
                    plan.warnings.append(
                        f"Stage '{current_stage}' failed: {error_preview}..."
                    )

        # Add completed stages info to warnings
        if state.stage_completed_stages:
            stages_str = ", ".join(state.stage_completed_stages[:3])
            if len(state.stage_completed_stages) > 3:
                stages_str += f" (+{len(state.stage_completed_stages) - 3} more)"
            plan.warnings.insert(
                0,
                f"Completed stages: {stages_str}"
            )

    def update_context_file(self, state: ContextRecoveryState) -> None:
        """
        Update context file after resume.

        Writes to .hybrid-execution-context.md or .mega-execution-context.md
        depending on context type.

        For mega-plan: In new mode, written to state directory; in legacy mode,
        written to project root.

        For hybrid (auto/worktree): Always written to project/worktree root
        to stay consistent with hybrid-context-reminder.py script and keep
        the file visible to users.

        Args:
            state: Current recovery state
        """
        if state.context_type == ContextType.MEGA_PLAN:
            # For mega-plan context files, write to state directory in new mode
            # or project root in legacy mode
            if self.is_legacy_mode():
                context_file = self.project_root / ".mega-execution-context.md"
            else:
                # In new mode, write to state directory
                context_file = self._path_resolver.get_state_file_path("mega-execution-context.md")
                # Ensure directory exists
                context_file.parent.mkdir(parents=True, exist_ok=True)
            content = self._generate_mega_context(state)
        else:
            # For hybrid context files - always write to project/worktree root
            # This keeps the file visible to users and consistent with
            # hybrid-context-reminder.py script which also writes to root
            target_path = state.worktree_path or self.project_root
            context_file = target_path / ".hybrid-execution-context.md"
            content = self._generate_hybrid_context(state)

        try:
            with open(context_file, "w", encoding="utf-8") as f:
                f.write(content)
        except OSError:
            pass  # Non-critical operation

    def _generate_mega_context(self, state: ContextRecoveryState) -> str:
        """Generate mega-plan execution context markdown."""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")

        progress = state.mega_plan_progress
        content = f"""# Mega-Plan Execution Context

**Last Updated:** {timestamp}
**Task:** {state.task_name}
**Target Branch:** {state.target_branch}

## Progress

- **Total Features:** {progress.get('total', 0)}
- **Complete:** {progress.get('complete', 0)}
- **In Progress:** {progress.get('in_progress', 0)}
- **Failed:** {progress.get('failed', 0)}
- **Pending:** {progress.get('pending', 0)}
- **Completion:** {progress.get('percentage', 0):.1f}%

## Feature Status

"""
        for feature in state.mega_plan_features:
            status = feature.get("status", "pending")
            icon = {"complete": "v", "failed": "x", "in_progress": ">", "pending": "o"}.get(status, "?")
            content += f"- [{icon}] **{feature.get('id')}**: {feature.get('title', feature.get('name', 'Unknown'))} ({status})\n"

        if state.failed_stories:
            content += "\n## Failed Features\n\n"
            for fid in state.failed_stories:
                content += f"- {fid}\n"

        return content

    def _generate_hybrid_context(self, state: ContextRecoveryState) -> str:
        """Generate hybrid execution context markdown."""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")

        content = f"""# Hybrid Execution Context

**Last Updated:** {timestamp}
**Task:** {state.task_name}
**Context Type:** {state.context_type.value}
**Target Branch:** {state.target_branch}

## Progress

- **Total Stories:** {state.total_stories}
- **Complete:** {len(state.completed_stories)}
- **In Progress:** {len(state.in_progress_stories)}
- **Failed:** {len(state.failed_stories)}
- **Pending:** {len(state.pending_stories)}
- **Completion:** {state.completion_percentage:.1f}%

## Story Status

"""
        for sid in state.completed_stories:
            content += f"- [v] {sid} (complete)\n"
        for sid in state.in_progress_stories:
            content += f"- [>] {sid} (in_progress)\n"
        for sid in state.failed_stories:
            content += f"- [x] {sid} (failed)\n"
        for sid in state.pending_stories:
            content += f"- [o] {sid} (pending)\n"

        if state.failed_stories:
            content += "\n## Failed Stories\n\n"
            for sid in state.failed_stories:
                content += f"- {sid}\n"

        return content


def main():
    """CLI interface for testing context recovery manager."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: context_recovery.py [--legacy] <command> [args]")
        print("Options:")
        print("  --legacy                    - Use legacy mode (files in project root)")
        print("Commands:")
        print("  detect                      - Detect current context")
        print("  plan                        - Generate recovery plan")
        print("  update-context              - Update context file")
        print("  show-paths                  - Show all file paths")
        sys.exit(1)

    # Check for --legacy flag (None means use default, which is legacy mode)
    legacy_mode = True if "--legacy" in sys.argv else None
    args = [a for a in sys.argv[1:] if a != "--legacy"]
    command = args[0] if args else ""

    project_root = Path.cwd()

    manager = ContextRecoveryManager(project_root, legacy_mode=legacy_mode)

    if command == "detect":
        state = manager.detect_context()
        print(json.dumps(state.to_dict(), indent=2, default=str))

    elif command == "plan":
        plan = manager.generate_recovery_plan()
        print(json.dumps(plan.to_dict(), indent=2, default=str))

    elif command == "update-context":
        state = manager.detect_context()
        manager.update_context_file(state)
        print("Context file updated")

    elif command == "show-paths":
        print(f"Mode: {'legacy' if manager.is_legacy_mode() else 'new'}")
        print(f"Project root: {manager.project_root}")
        print(f"Mega plan path: {manager.mega_plan_path}")
        print(f"PRD path: {manager.prd_path}")
        print(f"Mega status path: {manager.mega_status_path}")
        print(f"Config path: {manager.config_path}")
        print(f"Progress path: {manager.progress_path}")
        print(f"Worktree directory: {manager.worktree_dir}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
