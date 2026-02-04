"""
Iteration Loop for Plan Cascade

Automatic batch progression until completion with quality gates and retry management.
Provides hands-off execution of PRD stories across multiple batches.
"""

import json
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from .orchestrator import Orchestrator
    from .quality_gate import QualityGate
    from .retry_manager import RetryManager


class IterationMode(Enum):
    """Modes for iteration loop execution."""
    UNTIL_COMPLETE = "until_complete"  # Run until all stories complete
    MAX_ITERATIONS = "max_iterations"  # Run up to N iterations
    BATCH_COMPLETE = "batch_complete"  # Run single batch only


class IterationStatus(Enum):
    """Status of the iteration loop."""
    NOT_STARTED = "not_started"
    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"
    STOPPED = "stopped"


@dataclass
class IterationConfig:
    """Configuration for the iteration loop."""
    mode: IterationMode = IterationMode.UNTIL_COMPLETE
    max_iterations: int = 50
    poll_interval_seconds: int = 10
    batch_timeout_seconds: int = 3600  # 1 hour per batch
    quality_gates_enabled: bool = True
    auto_retry_enabled: bool = True
    stop_on_first_failure: bool = False
    continue_on_optional_failure: bool = True
    dod_gates_enabled: bool = True
    dod_level: str = "standard"  # "standard" or "full"

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "mode": self.mode.value,
            "max_iterations": self.max_iterations,
            "poll_interval_seconds": self.poll_interval_seconds,
            "batch_timeout_seconds": self.batch_timeout_seconds,
            "quality_gates_enabled": self.quality_gates_enabled,
            "auto_retry_enabled": self.auto_retry_enabled,
            "stop_on_first_failure": self.stop_on_first_failure,
            "continue_on_optional_failure": self.continue_on_optional_failure,
            "dod_gates_enabled": self.dod_gates_enabled,
            "dod_level": self.dod_level,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "IterationConfig":
        """Create from dictionary."""
        mode = data.get("mode", "until_complete")
        if isinstance(mode, str):
            mode = IterationMode(mode)

        return cls(
            mode=mode,
            max_iterations=data.get("max_iterations", 50),
            poll_interval_seconds=data.get("poll_interval_seconds", 10),
            batch_timeout_seconds=data.get("batch_timeout_seconds", 3600),
            quality_gates_enabled=data.get("quality_gates_enabled", True),
            auto_retry_enabled=data.get("auto_retry_enabled", True),
            stop_on_first_failure=data.get("stop_on_first_failure", False),
            continue_on_optional_failure=data.get("continue_on_optional_failure", True),
            dod_gates_enabled=data.get("dod_gates_enabled", True),
            dod_level=data.get("dod_level", "standard"),
        )


@dataclass
class BatchResult:
    """Result of executing a single batch."""
    batch_num: int
    started_at: str
    completed_at: str | None = None
    stories_launched: int = 0
    stories_completed: int = 0
    stories_failed: int = 0
    stories_retried: int = 0
    quality_gate_failures: int = 0
    duration_seconds: float = 0.0
    success: bool = False
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "batch_num": self.batch_num,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "stories_launched": self.stories_launched,
            "stories_completed": self.stories_completed,
            "stories_failed": self.stories_failed,
            "stories_retried": self.stories_retried,
            "quality_gate_failures": self.quality_gate_failures,
            "duration_seconds": self.duration_seconds,
            "success": self.success,
            "error": self.error,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "BatchResult":
        """Create from dictionary."""
        return cls(
            batch_num=data["batch_num"],
            started_at=data["started_at"],
            completed_at=data.get("completed_at"),
            stories_launched=data.get("stories_launched", 0),
            stories_completed=data.get("stories_completed", 0),
            stories_failed=data.get("stories_failed", 0),
            stories_retried=data.get("stories_retried", 0),
            quality_gate_failures=data.get("quality_gate_failures", 0),
            duration_seconds=data.get("duration_seconds", 0.0),
            success=data.get("success", False),
            error=data.get("error"),
        )


@dataclass
class IterationState:
    """State of the iteration loop."""
    status: IterationStatus = IterationStatus.NOT_STARTED
    started_at: str | None = None
    updated_at: str | None = None
    completed_at: str | None = None
    current_batch: int = 0
    total_batches: int = 0
    current_iteration: int = 0
    total_stories: int = 0
    completed_stories: int = 0
    failed_stories: int = 0
    batch_results: list[BatchResult] = field(default_factory=list)
    error: str | None = None
    pause_reason: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "status": self.status.value,
            "started_at": self.started_at,
            "updated_at": self.updated_at,
            "completed_at": self.completed_at,
            "current_batch": self.current_batch,
            "total_batches": self.total_batches,
            "current_iteration": self.current_iteration,
            "total_stories": self.total_stories,
            "completed_stories": self.completed_stories,
            "failed_stories": self.failed_stories,
            "batch_results": [b.to_dict() for b in self.batch_results],
            "error": self.error,
            "pause_reason": self.pause_reason,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "IterationState":
        """Create from dictionary."""
        status = data.get("status", "not_started")
        if isinstance(status, str):
            status = IterationStatus(status)

        return cls(
            status=status,
            started_at=data.get("started_at"),
            updated_at=data.get("updated_at"),
            completed_at=data.get("completed_at"),
            current_batch=data.get("current_batch", 0),
            total_batches=data.get("total_batches", 0),
            current_iteration=data.get("current_iteration", 0),
            total_stories=data.get("total_stories", 0),
            completed_stories=data.get("completed_stories", 0),
            failed_stories=data.get("failed_stories", 0),
            batch_results=[BatchResult.from_dict(b) for b in data.get("batch_results", [])],
            error=data.get("error"),
            pause_reason=data.get("pause_reason"),
        )

    @property
    def progress_percent(self) -> float:
        """Calculate completion percentage."""
        if self.total_stories == 0:
            return 0.0
        return (self.completed_stories / self.total_stories) * 100


@dataclass
class IterationCallbacks:
    """Callbacks for iteration events."""
    on_batch_start: Callable[[int, list[dict]], None] | None = None
    on_batch_complete: Callable[[BatchResult], None] | None = None
    on_story_complete: Callable[[str, bool], None] | None = None
    on_story_retry: Callable[[str, int], None] | None = None
    on_quality_gate_run: Callable[[str, dict], None] | None = None
    on_dod_gate_run: Callable[[str, Any], None] | None = None
    on_iteration_complete: Callable[[IterationState], None] | None = None
    on_error: Callable[[str, Exception], None] | None = None


class IterationLoop:
    """
    Automatic iteration loop for PRD execution.

    Manages batch progression, quality gates, retries, and completion detection.
    Provides a hands-off execution mode for running all PRD stories.
    """

    def __init__(
        self,
        project_root: Path,
        config: IterationConfig | None = None,
        orchestrator: Optional["Orchestrator"] = None,
        quality_gate: Optional["QualityGate"] = None,
        retry_manager: Optional["RetryManager"] = None,
        state_file: Path | None = None,
    ):
        """
        Initialize the iteration loop.

        Args:
            project_root: Root directory of the project
            config: Iteration configuration
            orchestrator: Orchestrator instance for batch execution
            quality_gate: QualityGate instance for verification
            retry_manager: RetryManager for handling failures
            state_file: Path to state file (defaults to .iteration-state.json)
        """
        self.project_root = Path(project_root)
        self.config = config or IterationConfig()
        self.orchestrator = orchestrator
        self.quality_gate = quality_gate
        self.retry_manager = retry_manager
        self.state_file = state_file or (self.project_root / ".iteration-state.json")

        self._state = IterationState()
        self._stop_requested = False
        self._pause_requested = False
        self._callbacks: IterationCallbacks | None = None

        # Initialize DoD gate if enabled
        self._dod_gate = None
        if self.config.dod_gates_enabled:
            try:
                from .done_gate import DoneGate
                self._dod_gate = DoneGate.from_flow(self.config.dod_level)
            except ImportError:
                pass  # DoD gate not available

        # Load existing state
        self._load_state()

    def start(
        self,
        callbacks: IterationCallbacks | None = None,
        dry_run: bool = False,
    ) -> IterationState:
        """
        Start the iteration loop.

        Args:
            callbacks: Optional callbacks for iteration events
            dry_run: If True, don't actually execute (for testing)

        Returns:
            Final iteration state
        """
        if not self.orchestrator:
            raise ValueError("Orchestrator not set")

        self._callbacks = callbacks or IterationCallbacks()
        self._stop_requested = False
        self._pause_requested = False

        # Initialize state
        now = datetime.now().isoformat()
        self._state = IterationState(
            status=IterationStatus.RUNNING,
            started_at=now,
            updated_at=now,
        )

        try:
            # Analyze dependencies and get batches
            batches = self.orchestrator.analyze_dependencies()
            self._state.total_batches = len(batches)

            # Count total stories
            self._state.total_stories = sum(len(batch) for batch in batches)

            self._save_state()

            # Execute iteration loop
            self._run_loop(batches, dry_run)

        except Exception as e:
            self._state.status = IterationStatus.FAILED
            self._state.error = str(e)
            self._state.completed_at = datetime.now().isoformat()
            self._save_state()

            if self._callbacks.on_error:
                self._callbacks.on_error("iteration_loop", e)

            raise

        return self._state

    def pause(self, reason: str | None = None) -> None:
        """Pause the iteration loop."""
        self._pause_requested = True
        self._state.pause_reason = reason
        self._state.status = IterationStatus.PAUSED
        self._save_state()

    def resume(self) -> IterationState:
        """Resume a paused iteration loop."""
        if self._state.status != IterationStatus.PAUSED:
            raise ValueError("Cannot resume - iteration is not paused")

        self._pause_requested = False
        self._state.status = IterationStatus.RUNNING
        self._state.pause_reason = None
        self._save_state()

        # Continue from where we left off
        if self.orchestrator:
            batches = self.orchestrator.analyze_dependencies()
            self._run_loop(batches, dry_run=False)

        return self._state

    def stop(self) -> None:
        """Stop the iteration loop."""
        self._stop_requested = True
        self._state.status = IterationStatus.STOPPED
        self._state.completed_at = datetime.now().isoformat()
        self._save_state()

    def get_state(self) -> IterationState:
        """Get the current iteration state."""
        return self._state

    def _run_loop(self, batches: list[list[dict]], dry_run: bool) -> None:
        """Run the main iteration loop."""
        while self._should_continue(batches):
            self._state.current_iteration += 1
            self._state.updated_at = datetime.now().isoformat()
            self._save_state()

            # Execute each pending batch
            for batch_num, batch in enumerate(batches, 1):
                if self._stop_requested or self._pause_requested:
                    break

                # Skip completed batches
                if batch_num <= len(self._state.batch_results):
                    continue

                self._state.current_batch = batch_num

                # Get pending stories in batch
                pending_stories = self._get_pending_stories(batch)
                if not pending_stories:
                    continue

                # Execute batch
                result = self._execute_batch(batch_num, pending_stories, dry_run)
                self._state.batch_results.append(result)
                self._save_state()

                if self._callbacks.on_batch_complete:
                    self._callbacks.on_batch_complete(result)

                # Check for stop conditions
                if not result.success and self.config.stop_on_first_failure:
                    self._state.status = IterationStatus.FAILED
                    self._state.error = f"Batch {batch_num} failed"
                    break

            # Check completion
            if self._check_completion():
                self._state.status = IterationStatus.COMPLETED
                self._state.completed_at = datetime.now().isoformat()
                break

            # Check iteration limit
            if self.config.mode == IterationMode.MAX_ITERATIONS:
                if self._state.current_iteration >= self.config.max_iterations:
                    self._state.status = IterationStatus.STOPPED
                    self._state.error = f"Max iterations ({self.config.max_iterations}) reached"
                    break

            # Single batch mode
            if self.config.mode == IterationMode.BATCH_COMPLETE:
                break

        self._state.updated_at = datetime.now().isoformat()
        self._save_state()

        if self._callbacks.on_iteration_complete:
            self._callbacks.on_iteration_complete(self._state)

    def _execute_batch(
        self,
        batch_num: int,
        stories: list[dict],
        dry_run: bool,
    ) -> BatchResult:
        """Execute a single batch of stories."""
        start_time = datetime.now()
        result = BatchResult(
            batch_num=batch_num,
            started_at=start_time.isoformat(),
            stories_launched=len(stories),
        )

        if self._callbacks.on_batch_start:
            self._callbacks.on_batch_start(batch_num, stories)

        if dry_run:
            result.success = True
            result.completed_at = datetime.now().isoformat()
            result.duration_seconds = 0.0
            return result

        try:
            # Launch batch via orchestrator
            if self.orchestrator:
                self.orchestrator.execute_batch(
                    batch=stories,
                    batch_num=batch_num,
                    dry_run=False,
                )

            # Wait for batch completion with polling
            self._wait_for_batch_completion(batch_num, stories, result, start_time)

        except Exception as e:
            result.error = str(e)
            result.success = False

        result.completed_at = datetime.now().isoformat()
        result.duration_seconds = (datetime.now() - start_time).total_seconds()
        result.success = result.stories_failed == 0 or (
            self.config.continue_on_optional_failure and result.success
        )

        return result

    def _wait_for_batch_completion(
        self,
        batch_num: int,
        stories: list[dict],
        result: BatchResult,
        start_time: datetime,
    ) -> None:
        """Wait for batch completion with polling."""
        timeout = timedelta(seconds=self.config.batch_timeout_seconds)
        story_ids = {s["id"] for s in stories}
        completed_ids = set()
        failed_ids = set()

        while True:
            # Check timeout
            if datetime.now() - start_time > timeout:
                result.error = f"Batch timeout after {self.config.batch_timeout_seconds}s"
                result.success = False
                break

            # Check stop/pause
            if self._stop_requested or self._pause_requested:
                break

            # Poll for completion
            if self.orchestrator:
                self.orchestrator.check_batch_complete(stories)

                # Check each story status
                for story in stories:
                    story_id = story["id"]
                    if story_id in completed_ids or story_id in failed_ids:
                        continue

                    story_status = self._get_story_status(story_id)

                    if story_status == "complete":
                        # Run quality gates if enabled
                        gate_outputs = None
                        changed_files: list[str] | None = None
                        if self.config.quality_gates_enabled and self.quality_gate:
                            gate_context: dict[str, Any] = {"story": story}

                            # Include PRD so gates can read flow/tdd configuration
                            try:
                                if self.orchestrator:
                                    prd = self.orchestrator.load_prd()
                                    if prd:
                                        gate_context["prd"] = prd
                            except Exception:
                                pass

                            # Best-effort changed files detection for incremental gates (TDD, review, etc.)
                            try:
                                from .changed_files import ChangedFilesDetector

                                detector = ChangedFilesDetector(self.project_root)
                                if detector.is_git_repository():
                                    changed_files = detector.get_changed_files()
                                    gate_context["changed_files"] = changed_files
                            except Exception:
                                pass

                            gate_outputs = self.quality_gate.execute_all(story_id, gate_context)
                            if self._callbacks.on_quality_gate_run:
                                self._callbacks.on_quality_gate_run(story_id, gate_outputs)
                            gate_passed = self.quality_gate.should_allow_progression(gate_outputs)
                            if not gate_passed:
                                result.quality_gate_failures += 1
                                if self.config.auto_retry_enabled:
                                    if self._handle_retry(story_id, story, "quality_gate"):
                                        result.stories_retried += 1
                                        continue
                                    else:
                                        failed_ids.add(story_id)
                                        result.stories_failed += 1
                                else:
                                    failed_ids.add(story_id)
                                    result.stories_failed += 1
                                continue

                        # Run DoD gates after quality gates pass
                        if self.config.dod_gates_enabled:
                            dod_passed = self._run_dod_gates(story_id, story, gate_outputs, changed_files)
                            if not dod_passed:
                                if self.config.auto_retry_enabled:
                                    if self._handle_retry(story_id, story, "dod_gate"):
                                        result.stories_retried += 1
                                        continue
                                # If no retry or retry failed, mark as failed
                                failed_ids.add(story_id)
                                result.stories_failed += 1
                                self._state.failed_stories += 1
                                if self._callbacks.on_story_complete:
                                    self._callbacks.on_story_complete(story_id, False)
                                continue

                        completed_ids.add(story_id)
                        result.stories_completed += 1
                        self._state.completed_stories += 1

                        if self._callbacks.on_story_complete:
                            self._callbacks.on_story_complete(story_id, True)

                    elif story_status == "failed":
                        if self.config.auto_retry_enabled:
                            if self._handle_retry(story_id, story, "execution"):
                                result.stories_retried += 1
                                continue

                        failed_ids.add(story_id)
                        result.stories_failed += 1
                        self._state.failed_stories += 1

                        if self._callbacks.on_story_complete:
                            self._callbacks.on_story_complete(story_id, False)

                # Check if all stories are done
                if len(completed_ids) + len(failed_ids) >= len(story_ids):
                    result.success = len(failed_ids) == 0
                    break

            time.sleep(self.config.poll_interval_seconds)

    def _run_quality_gates(self, story_id: str, story: dict) -> bool:
        """Run quality gates for a completed story."""
        if not self.quality_gate:
            return True

        results = self.quality_gate.execute_all(story_id, {"story": story})

        if self._callbacks.on_quality_gate_run:
            self._callbacks.on_quality_gate_run(story_id, results)

        return self.quality_gate.should_allow_progression(results)

    def _run_dod_gates(
        self,
        story_id: str,
        story: dict,
        gate_outputs: dict | None = None,
        changed_files: list[str] | None = None,
    ) -> bool:
        """
        Run Definition of Done (DoD) check after story completion.

        Args:
            story_id: ID of the completed story
            story: Story dictionary
            gate_outputs: Optional quality gate outputs to include in DoD check
            changed_files: Optional list of changed files (for DoD checks like test changes)

        Returns:
            True if DoD check passed, False otherwise
        """
        if not self._dod_gate:
            return True

        # Build context for DoD check
        result = self._dod_gate.check(
            gate_outputs=gate_outputs,
            verification_result=None,  # Could be added if AI verification is available
            changed_files=changed_files,
        )

        if self._callbacks and self._callbacks.on_dod_gate_run:
            self._callbacks.on_dod_gate_run(story_id, result)

        # Log to progress.txt
        try:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            progress_path = self.project_root / "progress.txt"
            with open(progress_path, "a", encoding="utf-8") as f:
                if result.passed:
                    f.write(f"[{timestamp}] [DoD_PASSED] {story_id}: Definition of Done check passed\n")
                else:
                    summary = "; ".join(result.errors[:2]) if result.errors else "Check failed"
                    f.write(f"[{timestamp}] [DoD_FAILED] {story_id}: {summary}\n")
        except IOError:
            pass  # Non-critical

        return result.passed

    def _handle_retry(
        self,
        story_id: str,
        story: dict,
        failure_type: str,
    ) -> bool:
        """Handle retry for a failed story. Returns True if retry was initiated."""
        if not self.retry_manager:
            return False

        if not self.retry_manager.can_retry(story_id):
            return False

        # Record the failure
        from .retry_manager import ErrorType
        if failure_type == "quality_gate":
            error_type = ErrorType.QUALITY_GATE
        elif failure_type == "dod_gate":
            error_type = ErrorType.QUALITY_GATE  # DoD is similar to quality gate
        else:
            error_type = ErrorType.EXIT_CODE
        self.retry_manager.record_failure(
            story_id=story_id,
            agent="unknown",
            error_type=error_type,
            error_message=f"Failed during {failure_type}",
        )

        if self._callbacks.on_story_retry:
            retry_count = self.retry_manager.get_retry_count(story_id)
            self._callbacks.on_story_retry(story_id, retry_count)

        return True

    def _get_story_status(self, story_id: str) -> str:
        """Get the current status of a story."""
        if not self.orchestrator:
            return "unknown"

        from ..state.state_manager import StateManager
        state_manager = StateManager(self.project_root)
        statuses = state_manager.get_all_story_statuses()

        return statuses.get(story_id, "pending")

    def _get_pending_stories(self, batch: list[dict]) -> list[dict]:
        """Get stories in batch that are still pending."""
        from ..state.state_manager import StateManager
        state_manager = StateManager(self.project_root)
        statuses = state_manager.get_all_story_statuses()

        pending = []
        for story in batch:
            story_id = story["id"]
            status = statuses.get(story_id, "pending")
            if status not in ["complete", "completed"]:
                pending.append(story)

        return pending

    def _should_continue(self, batches: list[list[dict]]) -> bool:
        """Check if the iteration loop should continue."""
        if self._stop_requested:
            return False

        if self._pause_requested:
            return False

        if self._state.status in [IterationStatus.COMPLETED, IterationStatus.FAILED, IterationStatus.STOPPED]:
            return False

        return not self._check_completion()

    def _check_completion(self) -> bool:
        """Check if all stories are complete."""
        return self._state.completed_stories >= self._state.total_stories

    def _load_state(self) -> None:
        """Load state from disk."""
        if not self.state_file.exists():
            return

        try:
            with open(self.state_file, encoding="utf-8") as f:
                data = json.load(f)
            self._state = IterationState.from_dict(data)
        except (json.JSONDecodeError, KeyError, TypeError):
            self._state = IterationState()

    def _save_state(self) -> None:
        """Save state to disk."""
        data = self._state.to_dict()
        data["config"] = self.config.to_dict()
        data["version"] = "1.0.0"

        try:
            with open(self.state_file, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
        except OSError:
            pass  # State write failure is non-critical

    def reset(self) -> None:
        """Reset iteration state."""
        self._state = IterationState()
        self._stop_requested = False
        self._pause_requested = False

        if self.state_file.exists():
            try:
                self.state_file.unlink()
            except OSError:
                pass

    def get_progress_summary(self) -> dict[str, Any]:
        """Get a summary of iteration progress."""
        return {
            "status": self._state.status.value,
            "progress_percent": self._state.progress_percent,
            "current_batch": self._state.current_batch,
            "total_batches": self._state.total_batches,
            "current_iteration": self._state.current_iteration,
            "completed_stories": self._state.completed_stories,
            "failed_stories": self._state.failed_stories,
            "total_stories": self._state.total_stories,
            "batch_results": [b.to_dict() for b in self._state.batch_results],
        }
