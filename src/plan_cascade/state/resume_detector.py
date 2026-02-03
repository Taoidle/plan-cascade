#!/usr/bin/env python3
"""
Resume Detector for Plan Cascade

Provides unified detection of incomplete execution state and resume suggestions.
Integrates with StageStateMachine and PathResolver to analyze execution progress
and generate human-readable recovery recommendations.

This module implements the unified resume behavior for Milestone C:
- Detects incomplete stages from stage-state.json
- Provides context about last execution (timestamp, completed work)
- Generates suggested resume points and commands
- Maintains backward compatibility with hybrid-resume and mega-resume
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .path_resolver import PathResolver


class ResumeReason(str, Enum):
    """Reason for resume suggestion."""

    STAGE_IN_PROGRESS = "stage_in_progress"
    STAGE_FAILED = "stage_failed"
    EXECUTION_INCOMPLETE = "execution_incomplete"
    PRD_NEEDS_APPROVAL = "prd_needs_approval"
    MEGA_PLAN_INCOMPLETE = "mega_plan_incomplete"


@dataclass
class IncompleteStateInfo:
    """
    Information about an incomplete execution state.

    Captures the state of an interrupted or incomplete execution,
    including what stage was reached, when it happened, and what
    work was already completed.

    Attributes:
        execution_id: Unique identifier for the execution
        strategy: Execution strategy (DIRECT, HYBRID_AUTO, HYBRID_WORKTREE, MEGA_PLAN)
        flow: Execution flow (quick, standard, full)
        last_stage: The last stage that was active (in_progress or failed)
        last_stage_status: Status of the last stage (in_progress, failed)
        timestamp: ISO-8601 timestamp of last activity
        completed_work: Summary of work already completed
        suggested_resume_point: Stage to resume from
        resume_reason: Why resume is suggested
        progress_percent: Overall progress percentage
        completed_stages: List of completed stage names
        failed_stages: List of failed stage names
        error_messages: Error messages from failed stages
    """

    execution_id: str = ""
    strategy: str | None = None
    flow: str | None = None
    last_stage: str | None = None
    last_stage_status: str | None = None
    timestamp: str = ""
    completed_work: list[str] = field(default_factory=list)
    suggested_resume_point: str | None = None
    resume_reason: ResumeReason | None = None
    progress_percent: int = 0
    completed_stages: list[str] = field(default_factory=list)
    failed_stages: list[str] = field(default_factory=list)
    error_messages: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "execution_id": self.execution_id,
            "strategy": self.strategy,
            "flow": self.flow,
            "last_stage": self.last_stage,
            "last_stage_status": self.last_stage_status,
            "timestamp": self.timestamp,
            "completed_work": self.completed_work,
            "suggested_resume_point": self.suggested_resume_point,
            "resume_reason": self.resume_reason.value if self.resume_reason else None,
            "progress_percent": self.progress_percent,
            "completed_stages": self.completed_stages,
            "failed_stages": self.failed_stages,
            "error_messages": self.error_messages,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> IncompleteStateInfo:
        """Create from dictionary (JSON deserialization)."""
        resume_reason = None
        if data.get("resume_reason"):
            try:
                resume_reason = ResumeReason(data["resume_reason"])
            except ValueError:
                pass

        return cls(
            execution_id=data.get("execution_id", ""),
            strategy=data.get("strategy"),
            flow=data.get("flow"),
            last_stage=data.get("last_stage"),
            last_stage_status=data.get("last_stage_status"),
            timestamp=data.get("timestamp", ""),
            completed_work=data.get("completed_work", []),
            suggested_resume_point=data.get("suggested_resume_point"),
            resume_reason=resume_reason,
            progress_percent=data.get("progress_percent", 0),
            completed_stages=data.get("completed_stages", []),
            failed_stages=data.get("failed_stages", []),
            error_messages=data.get("error_messages", []),
        )

    def has_incomplete_state(self) -> bool:
        """Check if there is incomplete state to resume."""
        return self.last_stage is not None or self.progress_percent > 0

    def is_failed(self) -> bool:
        """Check if execution has failed stages."""
        return len(self.failed_stages) > 0

    def time_since_last_activity(self) -> str | None:
        """
        Get human-readable time since last activity.

        Returns:
            String like "2 hours ago" or None if no timestamp
        """
        if not self.timestamp:
            return None

        try:
            # Parse ISO timestamp
            last_time = datetime.fromisoformat(
                self.timestamp.replace("Z", "+00:00")
            )
            now = datetime.now(last_time.tzinfo)
            delta = now - last_time

            seconds = int(delta.total_seconds())
            if seconds < 60:
                return "just now"
            elif seconds < 3600:
                minutes = seconds // 60
                return f"{minutes} minute{'s' if minutes != 1 else ''} ago"
            elif seconds < 86400:
                hours = seconds // 3600
                return f"{hours} hour{'s' if hours != 1 else ''} ago"
            else:
                days = seconds // 86400
                return f"{days} day{'s' if days != 1 else ''} ago"
        except (ValueError, TypeError):
            return None


@dataclass
class ResumeSuggestion:
    """
    Human-readable resume suggestion with command.

    Provides a formatted message and command for resuming
    an interrupted execution.

    Attributes:
        title: Brief title of the suggestion
        message: Detailed message explaining the state
        command: Command to run to resume
        details: Additional details about what was completed
        priority: Suggestion priority (1 = highest)
        can_auto_resume: Whether auto-resume is safe
    """

    title: str
    message: str
    command: str
    details: list[str] = field(default_factory=list)
    priority: int = 1
    can_auto_resume: bool = False

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "title": self.title,
            "message": self.message,
            "command": self.command,
            "details": self.details,
            "priority": self.priority,
            "can_auto_resume": self.can_auto_resume,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ResumeSuggestion:
        """Create from dictionary."""
        return cls(
            title=data.get("title", ""),
            message=data.get("message", ""),
            command=data.get("command", ""),
            details=data.get("details", []),
            priority=data.get("priority", 1),
            can_auto_resume=data.get("can_auto_resume", False),
        )

    def format_display(self) -> str:
        """
        Format the suggestion for display.

        Returns:
            Multi-line formatted string
        """
        lines = []
        lines.append(f"## {self.title}")
        lines.append("")
        lines.append(self.message)
        lines.append("")

        if self.details:
            lines.append("### Completed Work")
            for detail in self.details:
                lines.append(f"  - {detail}")
            lines.append("")

        lines.append("### Suggested Action")
        lines.append(f"  Run: {self.command}")

        if self.can_auto_resume:
            lines.append("")
            lines.append("  (Auto-resume is safe for this state)")

        return "\n".join(lines)


def detect_incomplete_state(
    path_resolver: PathResolver,
    include_prd_check: bool = True,
) -> IncompleteStateInfo | None:
    """
    Detect incomplete execution state from stage-state.json.

    Analyzes the stage state machine file to determine if there is
    an interrupted or incomplete execution that can be resumed.

    Args:
        path_resolver: PathResolver instance for path resolution
        include_prd_check: Whether to also check for incomplete PRD/mega-plan

    Returns:
        IncompleteStateInfo if incomplete state found, None otherwise
    """
    # Import StageStateMachine here to avoid circular imports
    from ..core.stage_state import StageStateMachine, StageStatus

    # Try to load stage state
    machine = StageStateMachine.load_state(path_resolver)

    if machine is None:
        # No stage state found, check for PRD/mega-plan that needs action
        if include_prd_check:
            return _check_prd_or_mega_state(path_resolver)
        return None

    # Analyze the loaded state
    info = IncompleteStateInfo(
        execution_id=machine.execution_id,
        strategy=machine.strategy,
        flow=machine.flow,
    )

    # Get progress summary
    summary = machine.get_progress_summary()
    info.progress_percent = summary["progress_percent"]

    # Check if execution is complete
    if machine.is_complete():
        return None

    # Collect completed and failed stages
    for stage in machine.stages.values():
        if stage.status == StageStatus.COMPLETED:
            info.completed_stages.append(stage.stage.value)
            # Add stage outputs to completed work
            for key, value in stage.outputs.items():
                if key not in ("skip_reason",):
                    info.completed_work.append(f"{stage.stage.value}: {key}")
        elif stage.status == StageStatus.SKIPPED:
            info.completed_stages.append(f"{stage.stage.value} (skipped)")
        elif stage.status == StageStatus.FAILED:
            info.failed_stages.append(stage.stage.value)
            if stage.errors:
                info.error_messages.extend(stage.errors)

    # Determine last active stage and status
    current = machine.current_stage
    if current:
        info.last_stage = current.value
        info.last_stage_status = "in_progress"
        info.resume_reason = ResumeReason.STAGE_IN_PROGRESS
    elif machine.is_failed():
        # Find the failed stage
        resume_point = machine.get_resume_point()
        if resume_point:
            info.last_stage = resume_point.value
            info.last_stage_status = "failed"
            info.resume_reason = ResumeReason.STAGE_FAILED
    else:
        # Execution incomplete but no stage in progress
        next_stage = machine.next_pending_stage
        if next_stage:
            info.suggested_resume_point = next_stage.value
            info.resume_reason = ResumeReason.EXECUTION_INCOMPLETE

    # Get timestamp from history
    history = machine.stages_history
    if history:
        info.timestamp = history[-1].get("timestamp", "")

    # Set suggested resume point if not set
    if not info.suggested_resume_point:
        resume_point = machine.get_resume_point()
        if resume_point:
            info.suggested_resume_point = resume_point.value

    return info


def _check_prd_or_mega_state(
    path_resolver: PathResolver,
) -> IncompleteStateInfo | None:
    """
    Check for incomplete PRD or mega-plan state.

    Used when no stage-state.json exists but there might be
    PRD or mega-plan files that indicate work was started.

    Args:
        path_resolver: PathResolver for path resolution

    Returns:
        IncompleteStateInfo if incomplete state found
    """
    # Check for mega-plan
    mega_plan_path = path_resolver.get_mega_plan_path()
    if mega_plan_path.exists():
        try:
            with open(mega_plan_path, encoding="utf-8") as f:
                mega_plan = json.load(f)

            features = mega_plan.get("features", [])
            if features:
                completed = sum(1 for f in features if f.get("status") == "complete")
                total = len(features)

                if completed < total:
                    info = IncompleteStateInfo(
                        strategy="MEGA_PLAN",
                        timestamp=mega_plan.get("metadata", {}).get("created_at", ""),
                        progress_percent=int((completed / total) * 100) if total > 0 else 0,
                        resume_reason=ResumeReason.MEGA_PLAN_INCOMPLETE,
                        suggested_resume_point="mega-resume",
                    )

                    for f in features:
                        if f.get("status") == "complete":
                            info.completed_work.append(f"Feature: {f.get('title', f.get('id'))}")
                            info.completed_stages.append(f.get("id"))

                    return info
        except (OSError, json.JSONDecodeError):
            pass

    # Check for PRD
    prd_path = path_resolver.get_prd_path()
    if prd_path.exists():
        try:
            with open(prd_path, encoding="utf-8") as f:
                prd = json.load(f)

            stories = prd.get("stories", [])
            if stories:
                # Check if any stories have status
                has_any_progress = any(
                    s.get("status") in ("complete", "in_progress", "failed")
                    for s in stories
                )

                if has_any_progress:
                    completed = sum(1 for s in stories if s.get("status") == "complete")
                    total = len(stories)

                    info = IncompleteStateInfo(
                        strategy="HYBRID_AUTO",
                        timestamp=prd.get("metadata", {}).get("created_at", ""),
                        progress_percent=int((completed / total) * 100) if total > 0 else 0,
                        resume_reason=ResumeReason.EXECUTION_INCOMPLETE,
                        suggested_resume_point="hybrid-resume",
                    )

                    for s in stories:
                        if s.get("status") == "complete":
                            info.completed_work.append(f"Story: {s.get('title', s.get('id'))}")
                            info.completed_stages.append(s.get("id"))

                    return info
                else:
                    # PRD exists but no execution started - needs approval
                    return IncompleteStateInfo(
                        strategy="HYBRID_AUTO",
                        timestamp=prd.get("metadata", {}).get("created_at", ""),
                        progress_percent=0,
                        resume_reason=ResumeReason.PRD_NEEDS_APPROVAL,
                        suggested_resume_point="approve",
                    )
        except (OSError, json.JSONDecodeError):
            pass

    return None


def get_resume_suggestion(
    incomplete_state: IncompleteStateInfo,
) -> ResumeSuggestion:
    """
    Generate a human-readable resume suggestion.

    Creates a formatted suggestion based on the detected
    incomplete state, including what to run and why.

    Args:
        incomplete_state: The detected incomplete state

    Returns:
        ResumeSuggestion with formatted message and command
    """
    # Format time since last activity
    time_ago = incomplete_state.time_since_last_activity()
    time_str = f" ({time_ago})" if time_ago else ""

    # Determine command based on strategy and reason
    reason = incomplete_state.resume_reason
    strategy = incomplete_state.strategy

    if reason == ResumeReason.PRD_NEEDS_APPROVAL:
        return ResumeSuggestion(
            title="PRD Ready for Approval",
            message=f"A PRD exists but execution has not started{time_str}.",
            command="/plan-cascade:approve",
            details=["PRD generated but not yet approved"],
            priority=1,
            can_auto_resume=True,
        )

    if reason == ResumeReason.MEGA_PLAN_INCOMPLETE:
        return ResumeSuggestion(
            title="Mega Plan Incomplete",
            message=f"Mega plan execution was interrupted{time_str}. {incomplete_state.progress_percent}% complete.",
            command="/plan-cascade:mega-resume --auto-prd",
            details=incomplete_state.completed_work,
            priority=1,
            can_auto_resume=True,
        )

    if reason == ResumeReason.STAGE_FAILED:
        error_msg = incomplete_state.error_messages[0] if incomplete_state.error_messages else "Unknown error"
        return ResumeSuggestion(
            title=f"Stage Failed: {incomplete_state.last_stage}",
            message=f"Execution failed at {incomplete_state.last_stage} stage{time_str}.\nError: {error_msg[:100]}",
            command="/plan-cascade:resume",
            details=incomplete_state.completed_work,
            priority=1,
            can_auto_resume=False,  # Failed state needs review
        )

    if reason == ResumeReason.STAGE_IN_PROGRESS:
        return ResumeSuggestion(
            title=f"Stage In Progress: {incomplete_state.last_stage}",
            message=f"Execution was interrupted during {incomplete_state.last_stage} stage{time_str}. {incomplete_state.progress_percent}% complete.",
            command="/plan-cascade:resume",
            details=incomplete_state.completed_work,
            priority=1,
            can_auto_resume=True,
        )

    # Default: execution incomplete
    resume_point = incomplete_state.suggested_resume_point or "resume"

    if strategy == "MEGA_PLAN":
        command = "/plan-cascade:mega-resume --auto-prd"
    elif strategy in ("HYBRID_AUTO", "HYBRID_WORKTREE"):
        command = "/plan-cascade:hybrid-resume --auto"
    else:
        command = "/plan-cascade:resume"

    return ResumeSuggestion(
        title="Execution Incomplete",
        message=f"Previous execution is {incomplete_state.progress_percent}% complete{time_str}.",
        command=command,
        details=incomplete_state.completed_work,
        priority=1,
        can_auto_resume=True,
    )


def format_resume_display(
    incomplete_state: IncompleteStateInfo | None,
    suggestion: ResumeSuggestion | None = None,
) -> str:
    """
    Format a complete resume detection display.

    Creates a formatted output showing the detected state
    and suggested actions.

    Args:
        incomplete_state: Detected incomplete state (or None)
        suggestion: Optional pre-generated suggestion

    Returns:
        Formatted display string
    """
    lines = []
    lines.append("=" * 60)
    lines.append("PLAN CASCADE - RESUME DETECTION")
    lines.append("=" * 60)
    lines.append("")

    if incomplete_state is None:
        lines.append("No incomplete execution detected.")
        lines.append("")
        lines.append("To start a new task:")
        lines.append("  /plan-cascade:auto \"your task description\"")
        lines.append("")
        lines.append("=" * 60)
        return "\n".join(lines)

    # Show state info
    lines.append(f"Strategy:  {incomplete_state.strategy or 'Unknown'}")
    lines.append(f"Flow:      {incomplete_state.flow or 'Unknown'}")
    lines.append(f"Progress:  {incomplete_state.progress_percent}%")

    time_ago = incomplete_state.time_since_last_activity()
    if time_ago:
        lines.append(f"Last Activity: {time_ago}")

    lines.append("")

    # Show completed work
    if incomplete_state.completed_stages:
        lines.append("## Completed Stages")
        for stage in incomplete_state.completed_stages:
            lines.append(f"  [OK] {stage}")
        lines.append("")

    # Show failed stages
    if incomplete_state.failed_stages:
        lines.append("## Failed Stages")
        for stage in incomplete_state.failed_stages:
            lines.append(f"  [X] {stage}")
        if incomplete_state.error_messages:
            lines.append("")
            lines.append("  Errors:")
            for error in incomplete_state.error_messages[:3]:
                lines.append(f"    - {error[:80]}")
        lines.append("")

    # Show suggestion
    if suggestion is None:
        suggestion = get_resume_suggestion(incomplete_state)

    lines.append("## Suggested Action")
    lines.append(f"  {suggestion.title}")
    lines.append(f"  {suggestion.message}")
    lines.append("")
    lines.append(f"  Command: {suggestion.command}")

    if suggestion.can_auto_resume:
        lines.append("")
        lines.append("  [Auto-resume is safe for this state]")

    lines.append("")
    lines.append("=" * 60)

    return "\n".join(lines)


def check_and_suggest_resume(
    project_root: Path | str,
    legacy_mode: bool = True,
) -> tuple[IncompleteStateInfo | None, ResumeSuggestion | None]:
    """
    Convenience function to check for incomplete state and get suggestion.

    Args:
        project_root: Project root directory
        legacy_mode: Whether to use legacy paths

    Returns:
        Tuple of (IncompleteStateInfo, ResumeSuggestion) or (None, None)
    """
    from .path_resolver import PathResolver

    resolver = PathResolver(
        project_root=Path(project_root),
        legacy_mode=legacy_mode,
    )

    incomplete = detect_incomplete_state(resolver)

    if incomplete is None:
        return None, None

    suggestion = get_resume_suggestion(incomplete)
    return incomplete, suggestion


def main():
    """CLI interface for testing resume detector."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: resume_detector.py <command>")
        print("Commands:")
        print("  detect                      - Detect incomplete state")
        print("  suggest                     - Get resume suggestion")
        print("  display                     - Show formatted display")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    # Check for --legacy flag
    legacy_mode = "--legacy" in sys.argv

    from .path_resolver import PathResolver

    resolver = PathResolver(project_root, legacy_mode=legacy_mode)

    if command == "detect":
        info = detect_incomplete_state(resolver)
        if info:
            print(json.dumps(info.to_dict(), indent=2))
        else:
            print("No incomplete state detected")

    elif command == "suggest":
        info = detect_incomplete_state(resolver)
        if info:
            suggestion = get_resume_suggestion(info)
            print(json.dumps(suggestion.to_dict(), indent=2))
        else:
            print("No incomplete state to suggest resume for")

    elif command == "display":
        info = detect_incomplete_state(resolver)
        print(format_resume_display(info))

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
