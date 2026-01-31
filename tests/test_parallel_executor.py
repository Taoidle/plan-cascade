"""Tests for ParallelExecutor module."""

import asyncio
import json
import pytest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

from plan_cascade.core.parallel_executor import (
    BatchProgress,
    BatchResult,
    ParallelExecutionConfig,
    ParallelExecutor,
    ParallelProgressDisplay,
    StoryProgress,
    StoryStatus,
    run_parallel_batch,
)
from plan_cascade.core.prd_generator import create_sample_prd


class TestParallelExecutionConfig:
    """Tests for ParallelExecutionConfig class."""

    def test_default_config(self):
        """Test default configuration values."""
        config = ParallelExecutionConfig()

        assert config.max_concurrency is not None  # Should default to CPU count
        assert config.poll_interval_seconds == 1.0
        assert config.timeout_seconds == 3600
        assert config.persist_progress is True
        assert config.quality_gates_enabled is True
        assert config.auto_retry_enabled is True
        assert config.separate_output is True

    def test_custom_config(self):
        """Test custom configuration values."""
        config = ParallelExecutionConfig(
            max_concurrency=4,
            poll_interval_seconds=0.5,
            timeout_seconds=1800,
            persist_progress=False,
        )

        assert config.max_concurrency == 4
        assert config.poll_interval_seconds == 0.5
        assert config.timeout_seconds == 1800
        assert config.persist_progress is False

    def test_to_dict(self):
        """Test configuration serialization."""
        config = ParallelExecutionConfig(max_concurrency=2)
        data = config.to_dict()

        assert data["max_concurrency"] == 2
        assert "poll_interval_seconds" in data
        assert "timeout_seconds" in data

    def test_from_dict(self):
        """Test configuration deserialization."""
        data = {
            "max_concurrency": 8,
            "poll_interval_seconds": 2.0,
            "timeout_seconds": 7200,
        }
        config = ParallelExecutionConfig.from_dict(data)

        assert config.max_concurrency == 8
        assert config.poll_interval_seconds == 2.0
        assert config.timeout_seconds == 7200


class TestStoryProgress:
    """Tests for StoryProgress class."""

    def test_init(self):
        """Test StoryProgress initialization."""
        progress = StoryProgress(
            story_id="story-001",
            status=StoryStatus.PENDING,
        )

        assert progress.story_id == "story-001"
        assert progress.status == StoryStatus.PENDING
        assert progress.started_at is None
        assert progress.completed_at is None
        assert progress.error is None
        assert progress.retry_count == 0

    def test_to_dict(self):
        """Test StoryProgress serialization."""
        progress = StoryProgress(
            story_id="story-001",
            status=StoryStatus.RUNNING,
            started_at="2024-01-01T00:00:00",
            agent="claude-code",
        )
        data = progress.to_dict()

        assert data["story_id"] == "story-001"
        assert data["status"] == "running"
        assert data["started_at"] == "2024-01-01T00:00:00"
        assert data["agent"] == "claude-code"

    def test_from_dict(self):
        """Test StoryProgress deserialization."""
        data = {
            "story_id": "story-002",
            "status": "complete",
            "started_at": "2024-01-01T00:00:00",
            "completed_at": "2024-01-01T00:01:00",
        }
        progress = StoryProgress.from_dict(data)

        assert progress.story_id == "story-002"
        assert progress.status == StoryStatus.COMPLETE
        assert progress.completed_at == "2024-01-01T00:01:00"


class TestBatchProgress:
    """Tests for BatchProgress class."""

    def test_init(self):
        """Test BatchProgress initialization."""
        progress = BatchProgress(
            batch_num=1,
            total_stories=5,
        )

        assert progress.batch_num == 1
        assert progress.total_stories == 5
        assert progress.running == []
        assert progress.completed == []
        assert progress.failed == []

    def test_pending_count(self):
        """Test pending count calculation."""
        progress = BatchProgress(
            batch_num=1,
            total_stories=10,
            running=["s1", "s2"],
            completed=["s3", "s4", "s5"],
            failed=["s6"],
        )

        assert progress.pending_count == 4  # 10 - 2 - 3 - 1

    def test_progress_percent(self):
        """Test progress percentage calculation."""
        progress = BatchProgress(
            batch_num=1,
            total_stories=10,
            completed=["s1", "s2", "s3", "s4", "s5"],
            failed=["s6"],
        )

        assert progress.progress_percent == 60.0  # (5 + 1) / 10 * 100

    def test_is_complete(self):
        """Test completion check."""
        progress = BatchProgress(
            batch_num=1,
            total_stories=3,
            completed=["s1", "s2"],
            failed=["s3"],
        )

        assert progress.is_complete is True

    def test_to_dict(self):
        """Test BatchProgress serialization."""
        progress = BatchProgress(
            batch_num=2,
            total_stories=5,
            running=["s1"],
            completed=["s2"],
        )
        data = progress.to_dict()

        assert data["batch_num"] == 2
        assert data["total_stories"] == 5
        assert "s1" in data["running"]
        assert "s2" in data["completed"]


class TestParallelExecutor:
    """Tests for ParallelExecutor class."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories for testing."""
        return [
            {"id": "story-001", "title": "Setup project", "status": "pending"},
            {"id": "story-002", "title": "Add feature A", "status": "pending"},
            {"id": "story-003", "title": "Add feature B", "status": "pending"},
        ]

    @pytest.fixture
    def executor(self, tmp_path: Path):
        """Create a ParallelExecutor instance."""
        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,  # Disable persistence for tests
        )
        return ParallelExecutor(
            project_root=tmp_path,
            config=config,
        )

    @pytest.mark.asyncio
    async def test_execute_batch_mock(self, executor, sample_stories):
        """Test batch execution with mock orchestrator."""
        result = await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        assert isinstance(result, BatchResult)
        assert result.batch_num == 1
        assert result.stories_launched == 3
        # Without orchestrator, uses mock execution which succeeds
        assert result.stories_completed == 3
        assert result.success is True
        assert result.duration_seconds >= 0

    @pytest.mark.asyncio
    async def test_execute_batch_with_callbacks(self, executor, sample_stories):
        """Test batch execution with progress callbacks."""
        story_updates = []
        batch_updates = []

        def on_story_progress(progress: StoryProgress):
            story_updates.append(progress)

        def on_batch_progress(progress: BatchProgress):
            batch_updates.append(progress)

        result = await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
            on_progress=on_story_progress,
            on_batch_progress=on_batch_progress,
        )

        # Should have received story progress updates
        assert len(story_updates) > 0
        # Should have received batch progress updates
        assert len(batch_updates) > 0
        # All stories should be completed
        assert result.stories_completed == 3

    @pytest.mark.asyncio
    async def test_execute_batch_concurrency_limit(self, tmp_path: Path, sample_stories):
        """Test that concurrency limit is respected."""
        # Create executor with concurrency limit of 1
        config = ParallelExecutionConfig(
            max_concurrency=1,
            persist_progress=False,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
        )

        max_concurrent = 0
        current_running = 0
        lock = asyncio.Lock()

        original_execute = executor._execute_story

        async def mock_execute(story, on_progress=None):
            nonlocal max_concurrent, current_running
            async with lock:
                current_running += 1
                if current_running > max_concurrent:
                    max_concurrent = current_running

            # Simulate some work
            await asyncio.sleep(0.05)

            async with lock:
                current_running -= 1

            return await original_execute(story, on_progress)

        executor._execute_story = mock_execute

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # With concurrency limit of 1, max concurrent should be 1
        assert max_concurrent == 1

    @pytest.mark.asyncio
    async def test_execute_batch_timeout(self, tmp_path: Path):
        """Test batch timeout handling."""
        config = ParallelExecutionConfig(
            max_concurrency=2,
            timeout_seconds=1,  # Very short timeout
            persist_progress=False,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
        )

        # Create mock that delays forever
        async def slow_execute(story, on_progress=None):
            try:
                await asyncio.sleep(100)  # Will be cancelled
                return story["id"], True, "Done"
            except asyncio.CancelledError:
                # Re-raise to let the executor handle it
                raise

        executor._execute_story = slow_execute

        stories = [{"id": "story-001", "title": "Slow story", "status": "pending"}]
        result = await executor.execute_batch(
            stories=stories,
            batch_num=1,
        )

        # Should have timed out
        assert result.error is not None
        assert "timeout" in result.error.lower()
        assert result.success is False

    def test_get_current_progress(self, executor):
        """Test getting current progress."""
        # Before execution, should be None
        assert executor.get_current_progress() is None

    @pytest.mark.asyncio
    async def test_progress_persistence(self, tmp_path: Path, sample_stories):
        """Test that progress is persisted during execution."""
        # Create state manager mock
        state_manager = MagicMock()
        state_manager.read_iteration_state.return_value = {}
        state_manager.write_iteration_state = MagicMock()

        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            state_manager=state_manager,
        )

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # Should have called write_iteration_state multiple times
        assert state_manager.write_iteration_state.called

        # Check that parallel_execution data was written
        calls = state_manager.write_iteration_state.call_args_list
        assert any("parallel_execution" in str(call) for call in calls)

    def test_recover_progress(self, tmp_path: Path):
        """Test recovering progress from persisted state."""
        # Create state manager with saved progress
        state_manager = MagicMock()
        state_manager.read_iteration_state.return_value = {
            "parallel_execution": {
                "batch_num": 1,
                "total_stories": 3,
                "running": ["story-002"],
                "completed": ["story-001"],
                "failed": [],
                "story_progress": {
                    "story-001": {
                        "story_id": "story-001",
                        "status": "complete",
                    },
                    "story-002": {
                        "story_id": "story-002",
                        "status": "running",
                    },
                },
            },
        }

        config = ParallelExecutionConfig(persist_progress=True)
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            state_manager=state_manager,
        )

        progress = executor.recover_progress()

        assert progress is not None
        assert progress.batch_num == 1
        assert progress.total_stories == 3
        assert "story-001" in progress.completed
        assert "story-002" in progress.running


class TestParallelProgressDisplay:
    """Tests for ParallelProgressDisplay class."""

    def test_init(self):
        """Test display initialization."""
        display = ParallelProgressDisplay()
        # Should not crash even without Rich
        assert display._current_progress is None

    def test_update(self):
        """Test display update."""
        display = ParallelProgressDisplay()
        progress = BatchProgress(
            batch_num=1,
            total_stories=5,
            running=["s1"],
            completed=["s2", "s3"],
        )

        # Should not crash even without live display
        display.update(progress)
        assert display._current_progress == progress

    def test_context_manager(self):
        """Test context manager usage."""
        display = ParallelProgressDisplay()

        with display:
            progress = BatchProgress(batch_num=1, total_stories=3)
            display.update(progress)

        # Should complete without error


class TestRunParallelBatch:
    """Tests for run_parallel_batch convenience function."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories for testing."""
        return [
            {"id": "story-001", "title": "Task 1"},
            {"id": "story-002", "title": "Task 2"},
        ]

    @pytest.mark.asyncio
    async def test_run_parallel_batch(self, tmp_path: Path, sample_stories):
        """Test the convenience function."""
        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,
        )

        result = await run_parallel_batch(
            project_root=tmp_path,
            stories=sample_stories,
            batch_num=1,
            config=config,
            show_progress=False,
        )

        assert isinstance(result, BatchResult)
        assert result.batch_num == 1
        assert result.stories_launched == 2
        assert result.stories_completed == 2
        assert result.success is True


class TestStoryStatusEnum:
    """Tests for StoryStatus enum."""

    def test_status_values(self):
        """Test that all expected status values exist."""
        assert StoryStatus.PENDING.value == "pending"
        assert StoryStatus.RUNNING.value == "running"
        assert StoryStatus.COMPLETE.value == "complete"
        assert StoryStatus.FAILED.value == "failed"
        assert StoryStatus.RETRYING.value == "retrying"
