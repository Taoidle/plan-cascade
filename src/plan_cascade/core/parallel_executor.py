#!/usr/bin/env python3
"""
Parallel Executor for Plan Cascade

Executes stories within a batch in parallel using asyncio.
Provides real-time progress tracking, configurable concurrency limits,
and progress persistence for recovery.

Implements:
- ADR-F003: asyncio for parallel story execution
- ADR-F007: Configurable concurrency limit
- ADR-F008: Progress persistence for parallel recovery
"""

import asyncio
import os
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver
    from ..state.state_manager import StateManager
    from .orchestrator import Orchestrator
    from .quality_gate import GateOutput, QualityGate
    from .retry_manager import RetryManager


class StoryStatus(Enum):
    """Status of a story during parallel execution."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETE = "complete"
    FAILED = "failed"
    RETRYING = "retrying"


@dataclass
class ParallelExecutionConfig:
    """Configuration for parallel execution."""
    max_concurrency: int | None = None  # Default to CPU count
    poll_interval_seconds: float = 1.0
    timeout_seconds: int = 3600  # 1 hour per batch
    persist_progress: bool = True
    quality_gates_enabled: bool = True
    auto_retry_enabled: bool = True
    separate_output: bool = True  # Separate output per story
    gate_caching_enabled: bool = True  # Cache gate results across stories in batch
    gate_fail_fast: bool = False  # Stop on first required gate failure
    incremental_gates: bool = True  # Enable incremental checking based on changed files

    def __post_init__(self):
        """Set default concurrency if not specified."""
        if self.max_concurrency is None:
            self.max_concurrency = os.cpu_count() or 4

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "max_concurrency": self.max_concurrency,
            "poll_interval_seconds": self.poll_interval_seconds,
            "timeout_seconds": self.timeout_seconds,
            "persist_progress": self.persist_progress,
            "quality_gates_enabled": self.quality_gates_enabled,
            "auto_retry_enabled": self.auto_retry_enabled,
            "separate_output": self.separate_output,
            "gate_caching_enabled": self.gate_caching_enabled,
            "gate_fail_fast": self.gate_fail_fast,
            "incremental_gates": self.incremental_gates,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ParallelExecutionConfig":
        """Create from dictionary."""
        return cls(
            max_concurrency=data.get("max_concurrency"),
            poll_interval_seconds=data.get("poll_interval_seconds", 1.0),
            timeout_seconds=data.get("timeout_seconds", 3600),
            persist_progress=data.get("persist_progress", True),
            quality_gates_enabled=data.get("quality_gates_enabled", True),
            auto_retry_enabled=data.get("auto_retry_enabled", True),
            separate_output=data.get("separate_output", True),
            gate_caching_enabled=data.get("gate_caching_enabled", True),
            gate_fail_fast=data.get("gate_fail_fast", False),
            incremental_gates=data.get("incremental_gates", True),
        )


@dataclass
class GateProgress:
    """Progress information for gate execution within a story."""
    gate_name: str
    status: str  # "pending", "running", "passed", "failed", "cached", "skipped"
    duration_seconds: float = 0.0
    from_cache: bool = False
    error_summary: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "gate_name": self.gate_name,
            "status": self.status,
            "duration_seconds": self.duration_seconds,
            "from_cache": self.from_cache,
            "error_summary": self.error_summary,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "GateProgress":
        """Create from dictionary."""
        return cls(
            gate_name=data["gate_name"],
            status=data.get("status", "pending"),
            duration_seconds=data.get("duration_seconds", 0.0),
            from_cache=data.get("from_cache", False),
            error_summary=data.get("error_summary"),
        )


@dataclass
class StoryProgress:
    """Progress information for a single story."""
    story_id: str
    status: StoryStatus
    started_at: str | None = None
    completed_at: str | None = None
    error: str | None = None
    retry_count: int = 0
    agent: str | None = None
    output_lines: list[str] = field(default_factory=list)
    gate_progress: dict[str, GateProgress] = field(default_factory=dict)
    changed_files: list[str] = field(default_factory=list)
    cache_time_saved: float = 0.0  # Time saved by using cached gate results

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "story_id": self.story_id,
            "status": self.status.value,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "error": self.error,
            "retry_count": self.retry_count,
            "agent": self.agent,
            "gate_progress": {
                name: gp.to_dict() for name, gp in self.gate_progress.items()
            },
            "changed_files": self.changed_files,
            "cache_time_saved": self.cache_time_saved,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StoryProgress":
        """Create from dictionary."""
        gate_progress = {}
        for name, gp_data in data.get("gate_progress", {}).items():
            gate_progress[name] = GateProgress.from_dict(gp_data)

        return cls(
            story_id=data["story_id"],
            status=StoryStatus(data.get("status", "pending")),
            started_at=data.get("started_at"),
            completed_at=data.get("completed_at"),
            error=data.get("error"),
            retry_count=data.get("retry_count", 0),
            agent=data.get("agent"),
            gate_progress=gate_progress,
            changed_files=data.get("changed_files", []),
            cache_time_saved=data.get("cache_time_saved", 0.0),
        )


@dataclass
class BatchProgress:
    """Progress information for a batch of stories."""
    batch_num: int
    total_stories: int
    running: list[str] = field(default_factory=list)
    completed: list[str] = field(default_factory=list)
    failed: list[str] = field(default_factory=list)
    story_progress: dict[str, StoryProgress] = field(default_factory=dict)

    @property
    def pending_count(self) -> int:
        """Count of pending stories."""
        return self.total_stories - len(self.running) - len(self.completed) - len(self.failed)

    @property
    def progress_percent(self) -> float:
        """Calculate completion percentage."""
        if self.total_stories == 0:
            return 100.0
        return ((len(self.completed) + len(self.failed)) / self.total_stories) * 100

    @property
    def is_complete(self) -> bool:
        """Check if all stories are done."""
        return len(self.completed) + len(self.failed) >= self.total_stories

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "batch_num": self.batch_num,
            "total_stories": self.total_stories,
            "running": list(self.running),
            "completed": list(self.completed),
            "failed": list(self.failed),
            "story_progress": {
                sid: sp.to_dict() for sid, sp in self.story_progress.items()
            },
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "BatchProgress":
        """Create from dictionary."""
        instance = cls(
            batch_num=data["batch_num"],
            total_stories=data["total_stories"],
            running=list(data.get("running", [])),
            completed=list(data.get("completed", [])),
            failed=list(data.get("failed", [])),
        )
        for sid, sp_data in data.get("story_progress", {}).items():
            instance.story_progress[sid] = StoryProgress.from_dict(sp_data)
        return instance


@dataclass
class BatchResult:
    """Result of executing a batch."""
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
    story_results: dict[str, tuple[bool, str]] = field(default_factory=dict)

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


class ParallelExecutor:
    """
    Executes stories within a batch in parallel using asyncio.

    Features:
    - Semaphore-based concurrency control
    - Real-time progress tracking with Rich
    - Progress persistence for recovery
    - Quality gate integration
    - Retry support with exponential backoff
    """

    def __init__(
        self,
        project_root: Path,
        config: ParallelExecutionConfig | None = None,
        orchestrator: Optional["Orchestrator"] = None,
        state_manager: Optional["StateManager"] = None,
        quality_gate: Optional["QualityGate"] = None,
        retry_manager: Optional["RetryManager"] = None,
        path_resolver: Optional["PathResolver"] = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the parallel executor.

        Args:
            project_root: Root directory of the project
            config: Parallel execution configuration
            orchestrator: Orchestrator instance for story execution
            state_manager: StateManager for persisting progress
            quality_gate: QualityGate for verification
            retry_manager: RetryManager for handling failures
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for all paths (backward compatible).
                If None, defaults to True when path_resolver is not provided.
        """
        self.project_root = Path(project_root)
        self.config = config or ParallelExecutionConfig()
        self.orchestrator = orchestrator
        self.quality_gate = quality_gate
        self.retry_manager = retry_manager

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

        # Create or use provided StateManager with PathResolver
        if state_manager is not None:
            self.state_manager = state_manager
        else:
            from ..state.state_manager import StateManager
            self.state_manager = StateManager(
                self.project_root,
                path_resolver=self._path_resolver,
            )

        self._current_progress: BatchProgress | None = None
        self._lock = asyncio.Lock()
        self._story_outputs: dict[str, list[str]] = {}

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

    async def execute_batch(
        self,
        stories: list[dict],
        batch_num: int = 1,
        on_progress: Callable[[StoryProgress], None] | None = None,
        on_batch_progress: Callable[[BatchProgress], None] | None = None,
    ) -> BatchResult:
        """
        Execute a batch of stories in parallel.

        Args:
            stories: List of story dictionaries to execute
            batch_num: Batch number for tracking
            on_progress: Callback for individual story progress updates
            on_batch_progress: Callback for batch-level progress updates

        Returns:
            BatchResult with execution summary
        """
        start_time = datetime.now()
        result = BatchResult(
            batch_num=batch_num,
            started_at=start_time.isoformat(),
            stories_launched=len(stories),
        )

        # Initialize batch progress
        self._current_progress = BatchProgress(
            batch_num=batch_num,
            total_stories=len(stories),
        )

        # Initialize story progress entries
        for story in stories:
            story_id = story.get("id", "unknown")
            self._current_progress.story_progress[story_id] = StoryProgress(
                story_id=story_id,
                status=StoryStatus.PENDING,
            )

        # Persist initial state
        if self.config.persist_progress:
            self._persist_progress()

        # Create semaphore for concurrency control
        semaphore = asyncio.Semaphore(self.config.max_concurrency or 4)

        async def execute_with_limit(story: dict) -> tuple[str, bool, str]:
            """Execute a story with concurrency limiting."""
            story_id = story.get("id", "unknown")

            async with semaphore:
                return await self._execute_story(
                    story,
                    on_progress=on_progress,
                )

        # Launch all stories in parallel with semaphore
        tasks = [
            asyncio.create_task(execute_with_limit(story))
            for story in stories
        ]

        # Wait for all tasks with timeout
        try:
            results = await asyncio.wait_for(
                asyncio.gather(*tasks, return_exceptions=True),
                timeout=self.config.timeout_seconds,
            )
        except (asyncio.TimeoutError, asyncio.CancelledError):
            result.error = f"Batch timeout after {self.config.timeout_seconds}s"
            # Cancel remaining tasks
            for task in tasks:
                if not task.done():
                    task.cancel()

            # Wait for cancellation to complete
            await asyncio.sleep(0.1)

            results = []
            for task in tasks:
                try:
                    if task.done() and not task.cancelled():
                        r = task.result()
                        results.append(r if r else ("unknown", False, "Cancelled due to timeout"))
                    else:
                        results.append(("unknown", False, "Cancelled due to timeout"))
                except (asyncio.CancelledError, Exception) as e:
                    results.append(("unknown", False, f"Cancelled: {type(e).__name__}"))

        # Process results
        for res in results:
            if isinstance(res, BaseException):
                result.stories_failed += 1
                continue

            # res is guaranteed to be tuple[str, bool, str] here
            story_id, success, message = res  # type: ignore[misc]
            result.story_results[story_id] = (success, message)

            if success:
                result.stories_completed += 1
            else:
                result.stories_failed += 1

        # Calculate final metrics
        result.completed_at = datetime.now().isoformat()
        result.duration_seconds = (datetime.now() - start_time).total_seconds()
        result.success = result.stories_failed == 0

        # Get retry and quality gate counts from progress
        if self._current_progress:
            for sp in self._current_progress.story_progress.values():
                if sp.retry_count > 0:
                    result.stories_retried += 1

        # Final progress update
        if on_batch_progress and self._current_progress:
            on_batch_progress(self._current_progress)

        # Persist final state
        if self.config.persist_progress:
            self._persist_progress()

        return result

    async def _execute_story(
        self,
        story: dict,
        on_progress: Callable[[StoryProgress], None] | None = None,
    ) -> tuple[str, bool, str]:
        """
        Execute a single story.

        Args:
            story: Story dictionary
            on_progress: Callback for progress updates

        Returns:
            Tuple of (story_id, success, message)
        """
        story_id = story.get("id", "unknown")

        # Update progress to running
        async with self._lock:
            if self._current_progress:
                self._current_progress.running.append(story_id)
                progress = self._current_progress.story_progress.get(story_id)
                if progress:
                    progress.status = StoryStatus.RUNNING
                    progress.started_at = datetime.now().isoformat()

                    if on_progress:
                        on_progress(progress)

                if self.config.persist_progress:
                    self._persist_progress()

        try:
            # Execute via orchestrator
            if self.orchestrator:
                success, message = self.orchestrator.execute_story(story)
            else:
                # Mock execution for testing
                await asyncio.sleep(0.1)  # Simulate work
                success = True
                message = "Executed successfully (mock)"

            # Run quality gates if enabled and execution succeeded
            gate_results: dict[str, Any] = {}
            if success and self.config.quality_gates_enabled and self.quality_gate:
                # Build context for gate execution
                gate_context: dict[str, Any] = {"story": story}

                # Get changed files for incremental checking if enabled
                changed_files: list[str] = []
                if self.config.incremental_gates:
                    changed_files = self._get_changed_files_for_story(story)
                    gate_context["changed_files"] = changed_files

                    # Update story progress with changed files
                    async with self._lock:
                        if self._current_progress:
                            progress = self._current_progress.story_progress.get(story_id)
                            if progress:
                                progress.changed_files = changed_files

                # Update gate progress to running state
                async with self._lock:
                    if self._current_progress:
                        progress = self._current_progress.story_progress.get(story_id)
                        if progress and self.quality_gate:
                            for gate_config in self.quality_gate.gates:
                                if gate_config.enabled:
                                    progress.gate_progress[gate_config.name] = GateProgress(
                                        gate_name=gate_config.name,
                                        status="running",
                                    )

                # Use async gate execution for parallel gate checking
                gate_results = await self.quality_gate.execute_all_async(story_id, gate_context)

                # Update gate progress with results
                cache_time_saved = 0.0
                async with self._lock:
                    if self._current_progress:
                        progress = self._current_progress.story_progress.get(story_id)
                        if progress:
                            for gate_name, output in gate_results.items():
                                status = "passed" if output.passed else "failed"
                                if output.skipped:
                                    status = "skipped"
                                elif output.from_cache:
                                    status = "cached"
                                    # Estimate time saved (use last cached duration)
                                    cache_time_saved += output.duration_seconds

                                progress.gate_progress[gate_name] = GateProgress(
                                    gate_name=gate_name,
                                    status=status,
                                    duration_seconds=output.duration_seconds,
                                    from_cache=output.from_cache,
                                    error_summary=output.error_summary if not output.passed else None,
                                )
                            progress.cache_time_saved = cache_time_saved

                if not self.quality_gate.should_allow_progression(gate_results):
                    success = False
                    message = "Quality gate failed"

                    # Update quality gate failure count
                    async with self._lock:
                        if self._current_progress:
                            progress = self._current_progress.story_progress.get(story_id)
                            if progress:
                                progress.error = self.quality_gate.get_failure_summary(gate_results)

            # Handle retry if failed and enabled
            if not success and self.config.auto_retry_enabled and self.retry_manager:
                if self.retry_manager.can_retry(story_id):
                    async with self._lock:
                        if self._current_progress:
                            progress = self._current_progress.story_progress.get(story_id)
                            if progress:
                                progress.status = StoryStatus.RETRYING
                                progress.retry_count += 1

                    # Record failure with structured error context from gate results
                    from .retry_manager import ErrorType
                    quality_gate_dict = None
                    if gate_results:
                        quality_gate_dict = {
                            name: {
                                "passed": output.passed,
                                "gate_type": output.gate_type.value,
                                "error_summary": output.error_summary,
                                "structured_errors": [
                                    {"file": e.file, "line": e.line, "message": e.message}
                                    for e in output.structured_errors[:5]  # Limit errors
                                ] if output.structured_errors else [],
                            }
                            for name, output in gate_results.items()
                        }

                    self.retry_manager.record_failure(
                        story_id=story_id,
                        agent="unknown",
                        error_type=ErrorType.QUALITY_GATE if gate_results else ErrorType.EXIT_CODE,
                        error_message=message,
                        quality_gate_results=quality_gate_dict,
                    )

                    # Wait for backoff delay
                    delay = self.retry_manager.get_retry_delay(story_id)
                    await asyncio.sleep(delay)

                    # Invalidate gate cache before retry (files may have changed)
                    if self.quality_gate and hasattr(self.quality_gate, 'invalidate_cache'):
                        self.quality_gate.invalidate_cache()

                    # Retry execution
                    if self.orchestrator:
                        success, message = self.orchestrator.execute_story(story)
                    else:
                        await asyncio.sleep(0.1)
                        success = True
                        message = "Retry succeeded (mock)"

                    # Re-run gates after retry if initially failed on gates
                    if success and gate_results and self.quality_gate:
                        retry_gate_results = await self.quality_gate.execute_all_async(story_id, gate_context)
                        if not self.quality_gate.should_allow_progression(retry_gate_results):
                            success = False
                            message = "Quality gate failed after retry"
                            async with self._lock:
                                if self._current_progress:
                                    progress = self._current_progress.story_progress.get(story_id)
                                    if progress:
                                        progress.error = self.quality_gate.get_failure_summary(retry_gate_results)

            # Update final progress
            async with self._lock:
                if self._current_progress:
                    # Remove from running
                    if story_id in self._current_progress.running:
                        self._current_progress.running.remove(story_id)

                    progress = self._current_progress.story_progress.get(story_id)
                    if progress:
                        progress.completed_at = datetime.now().isoformat()
                        if success:
                            progress.status = StoryStatus.COMPLETE
                            self._current_progress.completed.append(story_id)
                        else:
                            progress.status = StoryStatus.FAILED
                            progress.error = message
                            self._current_progress.failed.append(story_id)

                        if on_progress:
                            on_progress(progress)

                    if self.config.persist_progress:
                        self._persist_progress()

            return story_id, success, message

        except Exception as e:
            error_msg = str(e)

            async with self._lock:
                if self._current_progress:
                    if story_id in self._current_progress.running:
                        self._current_progress.running.remove(story_id)

                    progress = self._current_progress.story_progress.get(story_id)
                    if progress:
                        progress.status = StoryStatus.FAILED
                        progress.completed_at = datetime.now().isoformat()
                        progress.error = error_msg

                    self._current_progress.failed.append(story_id)

                    if on_progress:
                        on_progress(progress)

                    if self.config.persist_progress:
                        self._persist_progress()

            return story_id, False, error_msg

    def _persist_progress(self) -> None:
        """Persist current progress to state manager."""
        if not self.state_manager or not self._current_progress:
            return

        try:
            # Read existing iteration state
            state = self.state_manager.read_iteration_state() or {}

            # Update parallel execution section
            state["parallel_execution"] = {
                "batch_num": self._current_progress.batch_num,
                "running": list(self._current_progress.running),
                "completed": list(self._current_progress.completed),
                "failed": list(self._current_progress.failed),
                "total_stories": self._current_progress.total_stories,
                "story_progress": {
                    sid: sp.to_dict()
                    for sid, sp in self._current_progress.story_progress.items()
                },
            }

            self.state_manager.write_iteration_state(state)
        except Exception:
            pass  # Non-critical failure

    def _get_changed_files_for_story(self, story: dict) -> list[str]:
        """
        Get list of files changed by a story.

        Uses the story's file patterns if available, otherwise
        falls back to git diff for detecting changes.

        Args:
            story: Story dictionary containing file patterns or paths

        Returns:
            List of changed file paths relative to project root
        """
        changed_files: list[str] = []

        # First, try to get files from story metadata
        story_files = story.get("files", [])
        if story_files:
            return story_files

        # Check for affected_files in story
        affected_files = story.get("affected_files", [])
        if affected_files:
            return affected_files

        # Fall back to detecting changes via git
        try:
            from .changed_files import ChangedFilesDetector
            detector = ChangedFilesDetector(self.project_root)
            if detector.is_git_repository():
                changed_files = detector.get_changed_files()
        except Exception:
            pass

        return changed_files

    def get_current_progress(self) -> BatchProgress | None:
        """Get the current batch progress."""
        return self._current_progress

    def recover_progress(self) -> BatchProgress | None:
        """
        Recover progress from persisted state.

        Returns:
            Recovered BatchProgress or None if no state found
        """
        if not self.state_manager:
            return None

        try:
            state = self.state_manager.read_iteration_state()
            if not state:
                return None

            parallel_state = state.get("parallel_execution")
            if not parallel_state:
                return None

            progress = BatchProgress(
                batch_num=parallel_state.get("batch_num", 0),
                total_stories=parallel_state.get("total_stories", 0),
                running=list(parallel_state.get("running", [])),
                completed=list(parallel_state.get("completed", [])),
                failed=list(parallel_state.get("failed", [])),
            )

            for sid, sp_data in parallel_state.get("story_progress", {}).items():
                progress.story_progress[sid] = StoryProgress.from_dict(sp_data)

            self._current_progress = progress
            return progress

        except Exception:
            return None


class ParallelProgressDisplay:
    """
    Real-time progress display for parallel execution using Rich.

    Shows a live table with all running stories and their status.
    """

    def __init__(self, console=None):
        """
        Initialize the progress display.

        Args:
            console: Rich Console instance (creates one if not provided)
        """
        try:
            from rich.console import Console
            from rich.live import Live
            from rich.table import Table
            self._has_rich = True
            self._console = console or Console()
        except ImportError:
            self._has_rich = False
            self._console = None

        self._live = None
        self._current_progress: BatchProgress | None = None

    def _generate_table(self) -> Any:
        """Generate the progress table."""
        if not self._has_rich or not self._current_progress:
            return None

        from rich.table import Table

        progress = self._current_progress

        table = Table(
            title=f"Batch {progress.batch_num} Progress ({progress.progress_percent:.0f}%)",
            show_header=True,
            header_style="bold cyan",
        )
        table.add_column("Story", style="cyan", width=15)
        table.add_column("Status", width=12)
        table.add_column("Gates", width=20)
        table.add_column("Duration", style="dim", width=12)
        table.add_column("Info", width=30)

        # Status styling
        status_styles = {
            StoryStatus.PENDING: "[dim]pending[/dim]",
            StoryStatus.RUNNING: "[yellow]running[/yellow]",
            StoryStatus.COMPLETE: "[green]complete[/green]",
            StoryStatus.FAILED: "[red]failed[/red]",
            StoryStatus.RETRYING: "[yellow]retrying[/yellow]",
        }

        # Sort: running first, then pending, then completed, then failed
        story_order = []
        for sid in progress.running:
            if sid in progress.story_progress:
                story_order.append((sid, 0))
        for sid, sp in progress.story_progress.items():
            if sp.status == StoryStatus.PENDING and sid not in progress.running:
                story_order.append((sid, 1))
        for sid in progress.completed:
            story_order.append((sid, 2))
        for sid in progress.failed:
            story_order.append((sid, 3))

        # Deduplicate while preserving order
        seen = set()
        for sid, priority in sorted(story_order, key=lambda x: x[1]):
            if sid in seen:
                continue
            seen.add(sid)

            sp = progress.story_progress.get(sid)
            if not sp:
                continue

            # Calculate duration
            duration = ""
            if sp.started_at:
                start = datetime.fromisoformat(sp.started_at)
                if sp.completed_at:
                    end = datetime.fromisoformat(sp.completed_at)
                    duration = f"{(end - start).total_seconds():.1f}s"
                else:
                    duration = f"{(datetime.now() - start).total_seconds():.1f}s..."

            # Gate status column
            gate_info = self._format_gate_status(sp)

            # Info column
            info = ""
            if sp.retry_count > 0:
                info = f"Retry #{sp.retry_count}"
            if sp.cache_time_saved > 0:
                info = f"Cache saved {sp.cache_time_saved:.1f}s"
            if sp.error:
                info = sp.error[:30] + "..." if len(sp.error) > 30 else sp.error

            table.add_row(
                sid,
                status_styles.get(sp.status, str(sp.status.value)),
                gate_info,
                duration,
                info,
            )

        # Summary row
        table.add_section()
        summary = (
            f"[green]{len(progress.completed)}[/green] complete, "
            f"[yellow]{len(progress.running)}[/yellow] running, "
            f"[red]{len(progress.failed)}[/red] failed, "
            f"[dim]{progress.pending_count}[/dim] pending"
        )
        table.add_row("", "", "", "", summary)

        return table

    def _format_gate_status(self, story_progress: StoryProgress) -> str:
        """
        Format gate execution status for display.

        Args:
            story_progress: Story progress containing gate status

        Returns:
            Formatted string showing gate status
        """
        if not story_progress.gate_progress:
            return "-"

        status_icons = {
            "pending": "[dim].[/dim]",
            "running": "[yellow]~[/yellow]",
            "passed": "[green]+[/green]",
            "failed": "[red]x[/red]",
            "cached": "[cyan]c[/cyan]",
            "skipped": "[dim]s[/dim]",
        }

        parts = []
        for gate_name, gp in story_progress.gate_progress.items():
            # Use first letter of gate name + status icon
            short_name = gate_name[0].upper() if gate_name else "?"
            icon = status_icons.get(gp.status, "?")
            parts.append(f"{short_name}:{icon}")

        return " ".join(parts)

    def start(self) -> None:
        """Start the live display."""
        if not self._has_rich:
            return

        from rich.live import Live
        self._live = Live(
            self._generate_table(),
            console=self._console,
            refresh_per_second=2,
        )
        self._live.start()

    def stop(self) -> None:
        """Stop the live display."""
        if self._live:
            self._live.stop()
            self._live = None

    def update(self, progress: BatchProgress) -> None:
        """
        Update the display with new progress.

        Args:
            progress: Current batch progress
        """
        self._current_progress = progress
        if self._live:
            self._live.update(self._generate_table())

    def __enter__(self):
        """Context manager entry."""
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.stop()


async def run_parallel_batch(
    project_root: Path,
    stories: list[dict],
    batch_num: int = 1,
    config: ParallelExecutionConfig | None = None,
    orchestrator: Optional["Orchestrator"] = None,
    state_manager: Optional["StateManager"] = None,
    quality_gate: Optional["QualityGate"] = None,
    retry_manager: Optional["RetryManager"] = None,
    show_progress: bool = True,
    path_resolver: Optional["PathResolver"] = None,
    legacy_mode: bool | None = None,
) -> BatchResult:
    """
    Convenience function to run a batch in parallel with progress display.

    Args:
        project_root: Root directory of the project
        stories: List of story dictionaries
        batch_num: Batch number
        config: Parallel execution configuration
        orchestrator: Orchestrator instance
        state_manager: StateManager instance
        quality_gate: QualityGate instance
        retry_manager: RetryManager instance
        show_progress: Whether to show live progress display
        path_resolver: Optional PathResolver instance
        legacy_mode: If True, use legacy paths

    Returns:
        BatchResult with execution summary
    """
    executor = ParallelExecutor(
        project_root=project_root,
        config=config,
        orchestrator=orchestrator,
        state_manager=state_manager,
        quality_gate=quality_gate,
        retry_manager=retry_manager,
        path_resolver=path_resolver,
        legacy_mode=legacy_mode,
    )

    if show_progress:
        display = ParallelProgressDisplay()

        def on_batch_progress(progress: BatchProgress):
            display.update(progress)

        with display:
            result = await executor.execute_batch(
                stories=stories,
                batch_num=batch_num,
                on_batch_progress=on_batch_progress,
            )
    else:
        result = await executor.execute_batch(
            stories=stories,
            batch_num=batch_num,
        )

    return result
