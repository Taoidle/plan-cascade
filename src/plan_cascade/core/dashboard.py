#!/usr/bin/env python3
# Dashboard Status Aggregation for Plan Cascade
# Implements ADR-004: Dashboard aggregates existing state files

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver
    from ..state.state_manager import StateManager


class ExecutionStatus(str, Enum):
    NOT_STARTED = "not_started"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"
    PAUSED = "paused"


class StoryStatus(str, Enum):
    PENDING = "pending"
    IN_PROGRESS = "in_progress"
    COMPLETE = "complete"
    FAILED = "failed"
    SKIPPED = "skipped"


class ActionType(str, Enum):
    RETRY = "retry"
    SWITCH_AGENT = "switch_agent"
    MANUAL_FIX = "manual_fix"
    CONTINUE = "continue"
    RESUME = "resume"
    COMPLETE = "complete"
    REVIEW = "review"


@dataclass
class StoryInfo:
    story_id: str
    title: str
    status: StoryStatus
    agent: str | None = None
    started_at: str | None = None
    completed_at: str | None = None
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "story_id": self.story_id,
            "title": self.title,
            "status": self.status.value,
            "agent": self.agent,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "error": self.error,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StoryInfo":
        return cls(
            story_id=data["story_id"],
            title=data["title"],
            status=StoryStatus(data["status"]),
            agent=data.get("agent"),
            started_at=data.get("started_at"),
            completed_at=data.get("completed_at"),
            error=data.get("error"),
        )


@dataclass
class BatchStatus:
    batch_id: int
    stories: list[StoryInfo]
    status: ExecutionStatus = ExecutionStatus.NOT_STARTED
    started_at: str | None = None
    completed_at: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "batch_id": self.batch_id,
            "stories": [s.to_dict() for s in self.stories],
            "status": self.status.value,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "BatchStatus":
        return cls(
            batch_id=data["batch_id"],
            stories=[StoryInfo.from_dict(s) for s in data["stories"]],
            status=ExecutionStatus(data["status"]),
            started_at=data.get("started_at"),
            completed_at=data.get("completed_at"),
        )


@dataclass
class GateSummary:
    passed: int = 0
    failed: int = 0
    cached: int = 0
    skipped: int = 0
    total: int = 0
    details: list[dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "passed": self.passed,
            "failed": self.failed,
            "cached": self.cached,
            "skipped": self.skipped,
            "total": self.total,
            "details": self.details,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "GateSummary":
        return cls(
            passed=data.get("passed", 0),
            failed=data.get("failed", 0),
            cached=data.get("cached", 0),
            skipped=data.get("skipped", 0),
            total=data.get("total", 0),
            details=data.get("details", []),
        )


@dataclass
class FailureInfo:
    story_id: str
    error: str
    timestamp: str | None = None
    agent: str | None = None
    attempt: int = 1
    error_type: str = "unknown"

    def to_dict(self) -> dict[str, Any]:
        return {
            "story_id": self.story_id,
            "error": self.error,
            "timestamp": self.timestamp,
            "agent": self.agent,
            "attempt": self.attempt,
            "error_type": self.error_type,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "FailureInfo":
        return cls(
            story_id=data["story_id"],
            error=data["error"],
            timestamp=data.get("timestamp"),
            agent=data.get("agent"),
            attempt=data.get("attempt", 1),
            error_type=data.get("error_type", "unknown"),
        )


@dataclass
class RecommendedAction:
    action_type: ActionType
    description: str
    command: str | None = None
    reason: str | None = None
    priority: int = 1

    def to_dict(self) -> dict[str, Any]:
        return {
            "action_type": self.action_type.value,
            "description": self.description,
            "command": self.command,
            "reason": self.reason,
            "priority": self.priority,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RecommendedAction":
        return cls(
            action_type=ActionType(data["action_type"]),
            description=data["description"],
            command=data.get("command"),
            reason=data.get("reason"),
            priority=data.get("priority", 1),
        )


@dataclass
class DashboardState:
    # Overall status
    status: ExecutionStatus = ExecutionStatus.NOT_STARTED
    strategy: str | None = None
    flow: str | None = None

    # Stage progress
    current_stage: str | None = None
    completed_stages: list[str] = field(default_factory=list)
    stage_progress_percent: int = 0

    # Story/batch progress
    current_batch: int = 0
    total_batches: int = 0
    completed_stories: int = 0
    failed_stories: int = 0
    total_stories: int = 0
    batches: list[BatchStatus] = field(default_factory=list)

    # Failures
    recent_failures: list[FailureInfo] = field(default_factory=list)
    has_failures: bool = False

    # Gate summary
    gate_summary: GateSummary = field(default_factory=GateSummary)

    # Recommendations
    recommended_actions: list[RecommendedAction] = field(default_factory=list)

    # Metadata
    updated_at: str | None = None
    project_root: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "strategy": self.strategy,
            "flow": self.flow,
            "current_stage": self.current_stage,
            "completed_stages": self.completed_stages,
            "stage_progress_percent": self.stage_progress_percent,
            "current_batch": self.current_batch,
            "total_batches": self.total_batches,
            "completed_stories": self.completed_stories,
            "failed_stories": self.failed_stories,
            "total_stories": self.total_stories,
            "batches": [b.to_dict() for b in self.batches],
            "recent_failures": [f.to_dict() for f in self.recent_failures],
            "has_failures": self.has_failures,
            "gate_summary": self.gate_summary.to_dict(),
            "recommended_actions": [a.to_dict() for a in self.recommended_actions],
            "updated_at": self.updated_at,
            "project_root": self.project_root,
        }


    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "DashboardState":
        return cls(
            status=ExecutionStatus(data.get("status", "not_started")),
            strategy=data.get("strategy"),
            flow=data.get("flow"),
            current_stage=data.get("current_stage"),
            completed_stages=data.get("completed_stages", []),
            stage_progress_percent=data.get("stage_progress_percent", 0),
            current_batch=data.get("current_batch", 0),
            total_batches=data.get("total_batches", 0),
            completed_stories=data.get("completed_stories", 0),
            failed_stories=data.get("failed_stories", 0),
            total_stories=data.get("total_stories", 0),
            batches=[BatchStatus.from_dict(b) for b in data.get("batches", [])],
            recent_failures=[FailureInfo.from_dict(f) for f in data.get("recent_failures", [])],
            has_failures=data.get("has_failures", False),
            gate_summary=GateSummary.from_dict(data.get("gate_summary", {})),
            recommended_actions=[RecommendedAction.from_dict(a) for a in data.get("recommended_actions", [])],
            updated_at=data.get("updated_at"),
            project_root=data.get("project_root"),
        )

    @property
    def progress_percent(self) -> int:
        if self.total_stories == 0:
            return 0
        return int((self.completed_stories / self.total_stories) * 100)

    @property
    def is_complete(self) -> bool:
        return self.status == ExecutionStatus.COMPLETED

    @property
    def is_failed(self) -> bool:
        return self.status == ExecutionStatus.FAILED

    @property
    def primary_action(self) -> RecommendedAction | None:
        if not self.recommended_actions:
            return None
        sorted_actions = sorted(self.recommended_actions, key=lambda a: a.priority)
        return sorted_actions[0]


# ==============================================================================
# State File Readers
# ==============================================================================


def read_progress_status(
    project_root: Path,
    progress_path: Path | None = None,
) -> dict[str, StoryStatus]:
    """
    Read story statuses from progress.txt.

    Args:
        project_root: Project root directory
        progress_path: Optional custom path to progress.txt

    Returns:
        Dictionary mapping story_id to StoryStatus
    """
    path = progress_path or project_root / "progress.txt"
    statuses: dict[str, StoryStatus] = {}

    if not path.exists():
        return statuses

    try:
        content = path.read_text(encoding="utf-8")
        for line in content.split(chr(10)):
            line = line.strip()
            if not line:
                continue

            # Parse status markers
            story_id = None
            status = None

            if "[COMPLETE]" in line or "[STORY_COMPLETE]" in line:
                status = StoryStatus.COMPLETE
            elif "[FAILED]" in line or "[STORY_FAILED]" in line or "[ERROR]" in line:
                status = StoryStatus.FAILED
            elif "[IN_PROGRESS]" in line:
                status = StoryStatus.IN_PROGRESS

            if status:
                # Extract story ID (story-XXX pattern)
                match = re.search(r"(story-\d+)", line, re.IGNORECASE)
                if match:
                    story_id = match.group(1).lower()
                    statuses[story_id] = status

    except (OSError, UnicodeDecodeError):
        pass

    return statuses


def read_agent_status(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read .agent-status.json file.

    Args:
        path: Path to .agent-status.json

    Returns:
        Parsed JSON data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def read_iteration_state(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read .iteration-state.json file.

    Args:
        path: Path to .iteration-state.json

    Returns:
        Parsed JSON data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def read_retry_state(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read .retry-state.json file.

    Args:
        path: Path to .retry-state.json

    Returns:
        Parsed JSON data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def read_mega_status(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read .mega-status.json file.

    Args:
        path: Path to .mega-status.json

    Returns:
        Parsed JSON data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def read_stage_state(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read .state/stage-state.json file.

    Args:
        path: Path to stage-state.json

    Returns:
        Parsed JSON data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def read_agent_outputs(
    output_dir: Path,
) -> list[dict[str, Any]]:
    """
    Read all .result.json files from .agent-outputs/ directory.

    Args:
        output_dir: Path to .agent-outputs/ directory

    Returns:
        List of parsed gate result dictionaries
    """
    results: list[dict[str, Any]] = []

    if not output_dir.exists():
        return results

    try:
        for result_file in output_dir.glob("*.result.json"):
            try:
                with open(result_file, encoding="utf-8") as f:
                    data = json.load(f)
                    data["_file"] = str(result_file.name)
                    results.append(data)
            except (OSError, json.JSONDecodeError):
                continue
    except OSError:
        pass

    return results


def read_prd(
    path: Path,
) -> dict[str, Any] | None:
    """
    Read prd.json file.

    Args:
        path: Path to prd.json

    Returns:
        Parsed PRD data or None if file doesn't exist
    """
    if not path.exists():
        return None

    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


# ==============================================================================
# State Aggregator
# ==============================================================================


class DashboardAggregator:
    """
    Aggregates state from multiple source files into unified DashboardState.

    Follows ADR-004: Aggregates existing state files without creating
    new persistent state.
    """

    def __init__(
        self,
        project_root: Path,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool = True,
    ) -> None:
        """
        Initialize the aggregator.

        Args:
            project_root: Project root directory
            path_resolver: Optional PathResolver instance
            legacy_mode: Whether to use legacy paths (default True)
        """
        self.project_root = Path(project_root)
        self._legacy_mode = legacy_mode

        if path_resolver:
            self._path_resolver = path_resolver
        else:
            from ..state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

    def _get_agent_status_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / ".agent-status.json"
        return self._path_resolver.get_state_file_path("agent-status.json")

    def _get_iteration_state_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / ".iteration-state.json"
        return self._path_resolver.get_state_file_path("iteration-state.json")

    def _get_retry_state_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / ".retry-state.json"
        return self._path_resolver.get_state_file_path("retry-state.json")

    def _get_mega_status_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / ".mega-status.json"
        return self._path_resolver.get_mega_status_path()

    def _get_stage_state_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / ".state" / "stage-state.json"
        return self._path_resolver.get_state_file_path("stage-state.json")

    def _get_prd_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / "prd.json"
        return self._path_resolver.get_prd_path()

    def _get_mega_plan_path(self) -> Path:
        if self._legacy_mode:
            return self.project_root / "mega-plan.json"
        return self._path_resolver.get_mega_plan_path()

    def _get_agent_outputs_dir(self) -> Path:
        return self.project_root / ".agent-outputs"


    def _detect_strategy(self) -> str | None:
        """
        Detect current execution strategy from available files.

        Returns:
            Strategy name: 'MEGA', 'HYBRID', 'DIRECT', or None
        """
        mega_plan = self._get_mega_plan_path()
        mega_status = self._get_mega_status_path()
        prd = self._get_prd_path()

        if mega_plan.exists() or mega_status.exists():
            return "MEGA"
        elif prd.exists():
            return "HYBRID"
        else:
            # Check for stage state indicating DIRECT
            stage_state = read_stage_state(self._get_stage_state_path())
            if stage_state and stage_state.get("strategy"):
                return stage_state["strategy"].upper()
        return None

    def _aggregate_stage_info(
        self,
        stage_state: dict[str, Any] | None,
    ) -> tuple[str | None, list[str], int, str | None]:
        """
        Aggregate stage information.

        Returns:
            Tuple of (current_stage, completed_stages, progress_percent, flow)
        """
        if not stage_state:
            return None, [], 0, None

        current_stage = stage_state.get("current_stage")
        flow = stage_state.get("flow")
        stages = stage_state.get("stages", {})

        completed = []
        total = len(stages)
        completed_count = 0

        for stage_name, stage_data in stages.items():
            status = stage_data.get("status", "pending")
            if status in ("completed", "skipped"):
                completed.append(stage_name)
                completed_count += 1
            elif status == "in_progress" and not current_stage:
                current_stage = stage_name

        progress = int((completed_count / total) * 100) if total > 0 else 0

        return current_stage, completed, progress, flow


    def _aggregate_story_info(
        self,
        prd_data: dict[str, Any] | None,
        progress_statuses: dict[str, StoryStatus],
        agent_status: dict[str, Any] | None,
    ) -> tuple[list[BatchStatus], int, int, int, int, int]:
        """
        Aggregate story and batch information from PRD and progress.

        Returns:
            Tuple of (batches, current_batch, total_batches, completed, failed, total)
        """
        if not prd_data:
            return [], 0, 0, 0, 0, 0

        stories = prd_data.get("stories", [])
        total_stories = len(stories)
        completed_stories = 0
        failed_stories = 0
        current_batch = 0
        in_progress_batch = 0

        # Build story info objects
        story_infos: dict[str, StoryInfo] = {}
        for story in stories:
            story_id = story.get("id", "").lower()
            title = story.get("title", "Untitled")

            # Get status from progress or default to pending
            status = progress_statuses.get(story_id, StoryStatus.PENDING)

            # Get agent info if available
            agent = None
            if agent_status:
                for running in agent_status.get("running", []):
                    if running.get("story_id", "").lower() == story_id:
                        agent = running.get("agent")
                        if status == StoryStatus.PENDING:
                            status = StoryStatus.IN_PROGRESS

            story_infos[story_id] = StoryInfo(
                story_id=story_id,
                title=title,
                status=status,
                agent=agent,
            )

            if status == StoryStatus.COMPLETE:
                completed_stories += 1
            elif status == StoryStatus.FAILED:
                failed_stories += 1

        # Build batches from dependencies
        batches = self._build_batches(stories, story_infos)
        total_batches = len(batches)

        # Find current batch
        for i, batch in enumerate(batches, 1):
            batch_complete = all(
                s.status in (StoryStatus.COMPLETE, StoryStatus.SKIPPED)
                for s in batch.stories
            )
            batch_has_progress = any(
                s.status in (StoryStatus.IN_PROGRESS, StoryStatus.COMPLETE)
                for s in batch.stories
            )

            if not batch_complete and batch_has_progress:
                current_batch = i
                in_progress_batch = i
                break
            elif not batch_complete:
                current_batch = i
                break

        if current_batch == 0 and total_batches > 0:
            current_batch = total_batches  # All complete

        return (
            batches,
            current_batch,
            total_batches,
            completed_stories,
            failed_stories,
            total_stories,
        )


    def _build_batches(
        self,
        stories: list[dict[str, Any]],
        story_infos: dict[str, StoryInfo],
    ) -> list[BatchStatus]:
        """
        Build execution batches from story dependencies.

        Args:
            stories: List of story dictionaries from PRD
            story_infos: Dictionary of StoryInfo objects

        Returns:
            List of BatchStatus objects
        """
        if not stories:
            return []

        # Build dependency graph
        deps: dict[str, set[str]] = {}
        for story in stories:
            story_id = story.get("id", "").lower()
            story_deps = story.get("dependencies", [])
            deps[story_id] = set(d.lower() for d in story_deps)

        # Topological sort into batches
        batches: list[BatchStatus] = []
        assigned: set[str] = set()
        batch_num = 0

        while len(assigned) < len(stories):
            batch_num += 1
            batch_stories: list[StoryInfo] = []

            for story in stories:
                story_id = story.get("id", "").lower()
                if story_id in assigned:
                    continue

                # Check if all dependencies are assigned
                story_deps = deps.get(story_id, set())
                if story_deps.issubset(assigned):
                    if story_id in story_infos:
                        batch_stories.append(story_infos[story_id])
                    assigned.add(story_id)

            if not batch_stories:
                # Circular dependency or error - add remaining
                for story in stories:
                    story_id = story.get("id", "").lower()
                    if story_id not in assigned and story_id in story_infos:
                        batch_stories.append(story_infos[story_id])
                        assigned.add(story_id)

            if batch_stories:
                # Determine batch status
                all_complete = all(
                    s.status in (StoryStatus.COMPLETE, StoryStatus.SKIPPED)
                    for s in batch_stories
                )
                any_failed = any(s.status == StoryStatus.FAILED for s in batch_stories)
                any_in_progress = any(s.status == StoryStatus.IN_PROGRESS for s in batch_stories)

                if all_complete:
                    status = ExecutionStatus.COMPLETED
                elif any_failed:
                    status = ExecutionStatus.FAILED
                elif any_in_progress:
                    status = ExecutionStatus.IN_PROGRESS
                else:
                    status = ExecutionStatus.NOT_STARTED

                batches.append(BatchStatus(
                    batch_id=batch_num,
                    stories=batch_stories,
                    status=status,
                ))

        return batches


    def _aggregate_failures(
        self,
        retry_state: dict[str, Any] | None,
        agent_status: dict[str, Any] | None,
    ) -> list[FailureInfo]:
        """
        Aggregate recent failures from retry state and agent status.

        Returns:
            List of FailureInfo objects (most recent first)
        """
        failures: list[FailureInfo] = []

        # From retry state
        if retry_state:
            stories = retry_state.get("stories", {})
            for story_id, story_state in stories.items():
                for failure in story_state.get("failures", []):
                    failures.append(FailureInfo(
                        story_id=story_id,
                        error=failure.get("error_message", "Unknown error"),
                        timestamp=failure.get("timestamp"),
                        agent=failure.get("agent"),
                        attempt=failure.get("attempt", 1),
                        error_type=failure.get("error_type", "unknown"),
                    ))

        # From agent status
        if agent_status:
            for failed in agent_status.get("failed", []):
                failures.append(FailureInfo(
                    story_id=failed.get("story_id", "unknown"),
                    error=failed.get("error", "Unknown error"),
                    timestamp=failed.get("failed_at"),
                    agent=failed.get("agent"),
                    error_type="agent_failure",
                ))

        # Sort by timestamp (most recent first)
        failures.sort(
            key=lambda f: f.timestamp or "",
            reverse=True,
        )

        # Return only recent failures (last 10)
        return failures[:10]


    def _aggregate_gate_summary(
        self,
        agent_outputs: list[dict[str, Any]],
    ) -> GateSummary:
        """
        Aggregate gate summary from agent output files.

        Returns:
            GateSummary object
        """
        passed = 0
        failed = 0
        cached = 0
        skipped = 0
        details: list[dict[str, Any]] = []

        for output in agent_outputs:
            gates = output.get("quality_gates", output.get("gates", {}))

            if isinstance(gates, dict):
                for gate_name, gate_result in gates.items():
                    if isinstance(gate_result, dict):
                        gate_passed = gate_result.get("passed", False)
                        from_cache = gate_result.get("from_cache", False)
                        gate_skipped = gate_result.get("skipped", False)

                        if gate_skipped:
                            skipped += 1
                        elif from_cache:
                            cached += 1
                            if gate_passed:
                                passed += 1
                            else:
                                failed += 1
                        elif gate_passed:
                            passed += 1
                        else:
                            failed += 1

                        details.append({
                            "name": gate_name,
                            "passed": gate_passed,
                            "cached": from_cache,
                            "skipped": gate_skipped,
                        })

        return GateSummary(
            passed=passed,
            failed=failed,
            cached=cached,
            skipped=skipped,
            total=passed + failed,
            details=details,
        )


    def _generate_recommendations(
        self,
        state: DashboardState,
        failures: list[FailureInfo],
    ) -> list[RecommendedAction]:
        """
        Generate recommended actions based on current state.

        Returns:
            List of RecommendedAction objects sorted by priority
        """
        actions: list[RecommendedAction] = []

        # If complete, suggest completion
        if state.status == ExecutionStatus.COMPLETED:
            if state.strategy == "HYBRID":
                actions.append(RecommendedAction(
                    action_type=ActionType.COMPLETE,
                    description="Complete and merge changes",
                    command="/plan-cascade:hybrid-complete",
                    reason="All stories completed successfully",
                    priority=1,
                ))
            elif state.strategy == "MEGA":
                actions.append(RecommendedAction(
                    action_type=ActionType.COMPLETE,
                    description="Complete the mega-plan",
                    command="/plan-cascade:mega-complete",
                    reason="All features completed successfully",
                    priority=1,
                ))
            return actions

        # If there are failures
        if failures:
            latest = failures[0]

            # Check retry count
            if latest.attempt < 3:
                actions.append(RecommendedAction(
                    action_type=ActionType.RETRY,
                    description=f"Retry {latest.story_id}",
                    command=f"/plan-cascade:hybrid-resume --retry {latest.story_id}",
                    reason=f"Attempt {latest.attempt}/3 failed: {latest.error[:50]}...",
                    priority=1,
                ))

            # Suggest agent switch
            if latest.attempt >= 2:
                actions.append(RecommendedAction(
                    action_type=ActionType.SWITCH_AGENT,
                    description="Try a different agent",
                    command="/plan-cascade:hybrid-resume --retry-agent opus",
                    reason="Multiple failures suggest trying a different approach",
                    priority=2,
                ))

            # Always offer manual fix option
            actions.append(RecommendedAction(
                action_type=ActionType.MANUAL_FIX,
                description="Fix manually and resume",
                command="/plan-cascade:hybrid-resume",
                reason="Review the error and fix the issue manually",
                priority=3,
            ))

        # If paused or not started
        elif state.status in (ExecutionStatus.PAUSED, ExecutionStatus.NOT_STARTED):
            actions.append(RecommendedAction(
                action_type=ActionType.RESUME,
                description="Resume execution",
                command="/plan-cascade:resume",
                reason="Continue from where you left off",
                priority=1,
            ))

        # If in progress, suggest monitoring
        elif state.status == ExecutionStatus.IN_PROGRESS:
            actions.append(RecommendedAction(
                action_type=ActionType.CONTINUE,
                description="Continue monitoring",
                command="/plan-cascade:dashboard",
                reason=f"Batch {state.current_batch}/{state.total_batches} in progress",
                priority=1,
            ))

        return actions


    def aggregate(self) -> DashboardState:
        """
        Aggregate all state sources into a DashboardState.

        Returns:
            DashboardState with all aggregated information
        """
        # Read all state files
        prd_data = read_prd(self._get_prd_path())
        progress_statuses = read_progress_status(self.project_root)
        agent_status = read_agent_status(self._get_agent_status_path())
        iteration_state = read_iteration_state(self._get_iteration_state_path())
        retry_state = read_retry_state(self._get_retry_state_path())
        stage_state_data = read_stage_state(self._get_stage_state_path())
        agent_outputs = read_agent_outputs(self._get_agent_outputs_dir())

        # Detect strategy
        strategy = self._detect_strategy()

        # Aggregate stage info
        current_stage, completed_stages, stage_progress, flow = (
            self._aggregate_stage_info(stage_state_data)
        )

        # Aggregate story info
        (
            batches,
            current_batch,
            total_batches,
            completed_stories,
            failed_stories,
            total_stories,
        ) = self._aggregate_story_info(prd_data, progress_statuses, agent_status)

        # Aggregate failures
        failures = self._aggregate_failures(retry_state, agent_status)

        # Aggregate gate summary
        gate_summary = self._aggregate_gate_summary(agent_outputs)

        # Determine overall status
        if total_stories == 0 and not stage_state_data:
            status = ExecutionStatus.NOT_STARTED
        elif completed_stories == total_stories and total_stories > 0:
            status = ExecutionStatus.COMPLETED
        elif failed_stories > 0:
            status = ExecutionStatus.FAILED
        elif completed_stories > 0 or current_stage:
            status = ExecutionStatus.IN_PROGRESS
        else:
            status = ExecutionStatus.NOT_STARTED

        # Create state
        state = DashboardState(
            status=status,
            strategy=strategy,
            flow=flow,
            current_stage=current_stage,
            completed_stages=completed_stages,
            stage_progress_percent=stage_progress,
            current_batch=current_batch,
            total_batches=total_batches,
            completed_stories=completed_stories,
            failed_stories=failed_stories,
            total_stories=total_stories,
            batches=batches,
            recent_failures=failures,
            has_failures=len(failures) > 0,
            gate_summary=gate_summary,
            updated_at=datetime.now().isoformat(),
            project_root=str(self.project_root),
        )

        # Generate recommendations
        state.recommended_actions = self._generate_recommendations(state, failures)

        return state



class DashboardFormatter:
    COMPLETE = "[OK]"
    FAILED = "[X]"
    IN_PROGRESS = "[..]"
    PENDING = "[ ]"
    SKIPPED = "[-]"

    def __init__(self, use_unicode: bool = True) -> None:
        self.use_unicode = use_unicode

    def _progress_bar(self, percent: int, width: int = 20) -> str:
        filled = int(width * percent / 100)
        bar="="*filled+"-"*(width-filled)
        return f"[{bar}] {percent}%"

    def _status_indicator(self, status) -> str:
        s = status.value if hasattr(status, "value") else str(status)
        return {"complete": self.COMPLETE, "completed": self.COMPLETE, "failed": self.FAILED, "in_progress": self.IN_PROGRESS, "skipped": self.SKIPPED}.get(s, self.PENDING)


    def format_concise(self, state: DashboardState) -> str:
        icon = self._status_indicator(state.status)
        prog = self._progress_bar(state.progress_percent, 15)
        line = f"{icon} {state.strategy or 'N/A'} | {prog}"
        if state.total_stories > 0:
            line += f" | {state.completed_stories}/{state.total_stories} stories"
        if state.has_failures:
            line += f" | {state.failed_stories} failed"
        if state.primary_action:
            line += f" | Next: {state.primary_action.description}"
        return line


    def format_verbose(self, state: DashboardState) -> str:
        lines = []
        sep = "=" * 60
        lines.append(sep)
        lines.append("PLAN CASCADE DASHBOARD")
        lines.append(sep)
        lines.append("")
        lines.append("## Overview")
        lines.append(f"  Status:   {self._status_indicator(state.status)} {state.status.value.upper()}")
        lines.append(f"  Strategy: {state.strategy or 'Not detected'}")
        lines.append(f"  Flow:     {state.flow or 'Not set'}")
        lines.append("")
        lines.append("## Progress")
        lines.append(f"  {self._progress_bar(state.progress_percent)}")
        lines.append(f"  Stories: {state.completed_stories}/{state.total_stories} complete")
        if state.failed_stories > 0:
            lines.append(f"  Failed:  {state.failed_stories}")
        lines.append("")
        if state.batches:
            lines.append("## Batches")
            for batch in state.batches:
                bi = self._status_indicator(batch.status)
                lines.append(f"  Batch {batch.batch_id}: {bi} ({len(batch.stories)} stories)")
                for story in batch.stories:
                    si = self._status_indicator(story.status)
                    lines.append(f"    {si} {story.story_id}: {story.title}")
            lines.append("")
        if state.gate_summary.total > 0:
            gs = state.gate_summary
            lines.append("## Quality Gates")
            lines.append(f"  Passed: {gs.passed}, Failed: {gs.failed}, Cached: {gs.cached}")
            lines.append("")
        if state.recent_failures:
            lines.append("## Recent Failures")
            for f in state.recent_failures[:3]:
                lines.append(f"  {self.FAILED} {f.story_id}: {f.error[:40]}...")
            lines.append("")
        if state.recommended_actions:
            lines.append("## Next Actions")
            for a in state.recommended_actions:
                lines.append(f"  [{a.priority}] {a.description}")
                if a.command:
                    lines.append(f"      {a.command}")
            lines.append("")
        lines.append(sep)
        return chr(10).join(lines)

    def format(self, state: DashboardState, verbose: bool = False) -> str:
        return self.format_verbose(state) if verbose else self.format_concise(state)



# ==============================================================================
# Public API
# ==============================================================================


def get_dashboard(
    project_root: Path | str,
    path_resolver: "PathResolver | None" = None,
    legacy_mode: bool = True,
) -> DashboardState:
    """
    Get aggregated dashboard state for a project.

    Args:
        project_root: Project root directory
        path_resolver: Optional PathResolver instance
        legacy_mode: Whether to use legacy paths

    Returns:
        Aggregated DashboardState
    """
    aggregator = DashboardAggregator(
        project_root=Path(project_root),
        path_resolver=path_resolver,
        legacy_mode=legacy_mode,
    )
    return aggregator.aggregate()


def format_dashboard(
    state: DashboardState,
    verbose: bool = False,
    use_unicode: bool = True,
) -> str:
    """
    Format dashboard state for display.

    Args:
        state: Dashboard state to format
        verbose: Whether to use verbose format
        use_unicode: Whether to use Unicode symbols

    Returns:
        Formatted string
    """
    formatter = DashboardFormatter(use_unicode=use_unicode)
    return formatter.format(state, verbose=verbose)


def show_dashboard(
    project_root: Path | str | None = None,
    verbose: bool = False,
    legacy_mode: bool = True,
) -> str:
    """
    Convenience function to get and format dashboard in one call.

    Args:
        project_root: Project root directory (default: current directory)
        verbose: Whether to use verbose format
        legacy_mode: Whether to use legacy paths

    Returns:
        Formatted dashboard string
    """
    root = Path(project_root) if project_root else Path.cwd()
    state = get_dashboard(root, legacy_mode=legacy_mode)
    return format_dashboard(state, verbose=verbose)
