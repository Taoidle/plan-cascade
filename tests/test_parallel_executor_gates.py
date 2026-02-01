"""Integration tests for ParallelExecutor with QualityGate improvements.

Tests the integration of:
- Async gate execution in batch
- Cache hit across batch stories
- fail_fast with retry interaction
- Incremental checking per story
"""

import asyncio
import sys
import time
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from plan_cascade.core.parallel_executor import (
    BatchProgress,
    BatchResult,
    GateProgress,
    ParallelExecutionConfig,
    ParallelExecutor,
    StoryProgress,
    StoryStatus,
    run_parallel_batch,
)
from plan_cascade.core.quality_gate import (
    GateConfig,
    GateOutput,
    GateType,
    ProjectType,
    QualityGate,
)
from plan_cascade.core.retry_manager import RetryConfig, RetryManager


class TestGateProgress:
    """Tests for GateProgress dataclass."""

    def test_gate_progress_init(self):
        """Test GateProgress initialization."""
        gp = GateProgress(
            gate_name="typecheck",
            status="running",
        )
        assert gp.gate_name == "typecheck"
        assert gp.status == "running"
        assert gp.duration_seconds == 0.0
        assert gp.from_cache is False
        assert gp.error_summary is None

    def test_gate_progress_to_dict(self):
        """Test GateProgress serialization."""
        gp = GateProgress(
            gate_name="test",
            status="passed",
            duration_seconds=1.5,
            from_cache=True,
        )
        data = gp.to_dict()

        assert data["gate_name"] == "test"
        assert data["status"] == "passed"
        assert data["duration_seconds"] == 1.5
        assert data["from_cache"] is True

    def test_gate_progress_from_dict(self):
        """Test GateProgress deserialization."""
        data = {
            "gate_name": "lint",
            "status": "failed",
            "duration_seconds": 0.8,
            "from_cache": False,
            "error_summary": "Lint errors found",
        }
        gp = GateProgress.from_dict(data)

        assert gp.gate_name == "lint"
        assert gp.status == "failed"
        assert gp.error_summary == "Lint errors found"


class TestStoryProgressWithGates:
    """Tests for StoryProgress with gate progress tracking."""

    def test_story_progress_gate_progress_field(self):
        """Test StoryProgress includes gate progress."""
        sp = StoryProgress(
            story_id="story-001",
            status=StoryStatus.RUNNING,
        )
        assert sp.gate_progress == {}
        assert sp.changed_files == []
        assert sp.cache_time_saved == 0.0

    def test_story_progress_with_gates(self):
        """Test StoryProgress with gate progress data."""
        sp = StoryProgress(
            story_id="story-001",
            status=StoryStatus.RUNNING,
            gate_progress={
                "typecheck": GateProgress("typecheck", "passed", 1.0),
                "test": GateProgress("test", "running", 0.0),
            },
            changed_files=["src/main.py", "tests/test_main.py"],
            cache_time_saved=2.5,
        )

        assert len(sp.gate_progress) == 2
        assert sp.gate_progress["typecheck"].status == "passed"
        assert sp.changed_files == ["src/main.py", "tests/test_main.py"]
        assert sp.cache_time_saved == 2.5

    def test_story_progress_to_dict_with_gates(self):
        """Test StoryProgress serialization includes gates."""
        sp = StoryProgress(
            story_id="story-001",
            status=StoryStatus.COMPLETE,
            gate_progress={
                "test": GateProgress("test", "cached", 0.5, from_cache=True),
            },
            cache_time_saved=1.0,
        )
        data = sp.to_dict()

        assert "gate_progress" in data
        assert "test" in data["gate_progress"]
        assert data["gate_progress"]["test"]["from_cache"] is True
        assert data["cache_time_saved"] == 1.0

    def test_story_progress_from_dict_with_gates(self):
        """Test StoryProgress deserialization includes gates."""
        data = {
            "story_id": "story-002",
            "status": "complete",
            "gate_progress": {
                "lint": {
                    "gate_name": "lint",
                    "status": "passed",
                    "duration_seconds": 0.3,
                    "from_cache": False,
                },
            },
            "changed_files": ["file.py"],
            "cache_time_saved": 0.5,
        }
        sp = StoryProgress.from_dict(data)

        assert len(sp.gate_progress) == 1
        assert sp.gate_progress["lint"].status == "passed"
        assert sp.changed_files == ["file.py"]
        assert sp.cache_time_saved == 0.5


class TestParallelExecutionConfigExtended:
    """Tests for extended ParallelExecutionConfig options."""

    def test_config_includes_gate_options(self):
        """Test config includes new gate-related options."""
        config = ParallelExecutionConfig()

        assert hasattr(config, "gate_caching_enabled")
        assert hasattr(config, "gate_fail_fast")
        assert hasattr(config, "incremental_gates")

    def test_config_default_values(self):
        """Test default values for gate options."""
        config = ParallelExecutionConfig()

        assert config.gate_caching_enabled is True
        assert config.gate_fail_fast is False
        assert config.incremental_gates is True

    def test_config_custom_values(self):
        """Test custom values for gate options."""
        config = ParallelExecutionConfig(
            gate_caching_enabled=False,
            gate_fail_fast=True,
            incremental_gates=False,
        )

        assert config.gate_caching_enabled is False
        assert config.gate_fail_fast is True
        assert config.incremental_gates is False

    def test_config_to_dict_includes_gate_options(self):
        """Test config serialization includes gate options."""
        config = ParallelExecutionConfig(
            gate_caching_enabled=True,
            gate_fail_fast=True,
        )
        data = config.to_dict()

        assert data["gate_caching_enabled"] is True
        assert data["gate_fail_fast"] is True
        assert "incremental_gates" in data

    def test_config_from_dict_includes_gate_options(self):
        """Test config deserialization includes gate options."""
        data = {
            "gate_caching_enabled": False,
            "gate_fail_fast": True,
            "incremental_gates": False,
        }
        config = ParallelExecutionConfig.from_dict(data)

        assert config.gate_caching_enabled is False
        assert config.gate_fail_fast is True
        assert config.incremental_gates is False


class TestAsyncGateExecution:
    """Tests for async gate execution in ParallelExecutor."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories for testing."""
        return [
            {"id": "story-001", "title": "Setup project", "status": "pending"},
            {"id": "story-002", "title": "Add feature A", "status": "pending"},
        ]

    @pytest.fixture
    def mock_quality_gate(self, tmp_path: Path):
        """Create a mock QualityGate."""
        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["pass"]),
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        return QualityGate(tmp_path, gates=gates)

    @pytest.mark.asyncio
    async def test_executor_uses_async_gate_execution(self, tmp_path: Path, sample_stories):
        """Test that ParallelExecutor uses execute_all_async."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)

        # Track if execute_all_async was called
        original_execute_all_async = quality_gate.execute_all_async
        call_count = 0

        async def tracked_execute_all_async(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            return await original_execute_all_async(*args, **kwargs)

        quality_gate.execute_all_async = tracked_execute_all_async

        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,
            quality_gates_enabled=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
        )

        result = await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # Should have called execute_all_async for each story
        assert call_count == len(sample_stories)
        assert result.success is True

    @pytest.mark.asyncio
    async def test_gate_progress_updated_during_execution(self, tmp_path: Path, sample_stories):
        """Test that gate progress is updated during execution."""
        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["pass"]),
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)

        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,
            quality_gates_enabled=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
        )

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # Check that gate progress was tracked
        progress = executor.get_current_progress()
        assert progress is not None

        for story_id in ["story-001", "story-002"]:
            sp = progress.story_progress.get(story_id)
            assert sp is not None
            # Gates should have been tracked
            assert len(sp.gate_progress) == 2
            assert "typecheck" in sp.gate_progress
            assert "test" in sp.gate_progress


class TestGateCachingInBatch:
    """Tests for gate result caching across batch stories."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories for testing."""
        return [
            {"id": "story-001", "title": "Task 1"},
            {"id": "story-002", "title": "Task 2"},
            {"id": "story-003", "title": "Task 3"},
        ]

    @pytest.mark.asyncio
    async def test_gate_caching_enabled_by_default(self, tmp_path: Path, sample_stories):
        """Test that gate caching is enabled by default in config."""
        config = ParallelExecutionConfig()
        assert config.gate_caching_enabled is True

    @pytest.mark.asyncio
    async def test_cache_reused_across_stories(self, tmp_path: Path, sample_stories):
        """Test that cached gate results are reused across stories in batch."""
        # Create a gate that uses caching
        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates, use_cache=True)

        config = ParallelExecutionConfig(
            max_concurrency=1,  # Sequential to ensure consistent ordering
            persist_progress=False,
            quality_gates_enabled=True,
            gate_caching_enabled=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
        )

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        progress = executor.get_current_progress()
        assert progress is not None

        # Check cache usage - first story runs gate, subsequent may use cache
        # (depends on whether project state changed)
        first_story = progress.story_progress.get("story-001")
        assert first_story is not None
        assert "test" in first_story.gate_progress


class TestFailFastWithRetry:
    """Tests for fail_fast and auto_retry interaction."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories for testing."""
        return [
            {"id": "story-001", "title": "Task 1"},
        ]

    @pytest.mark.asyncio
    async def test_retry_triggered_on_gate_failure(self, tmp_path: Path, sample_stories):
        """Test that retry is triggered when gates fail."""
        # Create a gate that fails on first call, succeeds on retry
        call_count = 0

        async def mock_execute_all_async(story_id, context=None):
            nonlocal call_count
            call_count += 1
            if call_count == 1:
                return {
                    "test": GateOutput(
                        gate_name="test",
                        gate_type=GateType.TEST,
                        passed=False,
                        exit_code=1,
                        stdout="",
                        stderr="Test failed",
                        duration_seconds=0.1,
                        command="pytest",
                        error_summary="1 test failed",
                    )
                }
            return {
                "test": GateOutput(
                    gate_name="test",
                    gate_type=GateType.TEST,
                    passed=True,
                    exit_code=0,
                    stdout="All tests passed",
                    stderr="",
                    duration_seconds=0.1,
                    command="pytest",
                )
            }

        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)
        quality_gate.execute_all_async = mock_execute_all_async
        quality_gate.should_allow_progression = lambda results: all(r.passed for r in results.values())
        quality_gate.get_failure_summary = lambda results: "Gate failed"
        quality_gate.invalidate_cache = lambda: None

        retry_config = RetryConfig(max_retries=2, base_delay_seconds=0.01)
        retry_manager = RetryManager(tmp_path, config=retry_config)

        config = ParallelExecutionConfig(
            max_concurrency=1,
            persist_progress=False,
            quality_gates_enabled=True,
            auto_retry_enabled=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
            retry_manager=retry_manager,
        )

        result = await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # Retry should have been triggered and succeeded
        assert call_count >= 2  # At least initial + retry
        assert result.success is True

    @pytest.mark.asyncio
    async def test_structured_errors_passed_to_retry(self, tmp_path: Path, sample_stories):
        """Test that structured errors from gates are passed to retry manager."""
        from plan_cascade.core.error_parser import ErrorInfo

        structured_errors = [
            ErrorInfo(
                file="src/main.py",
                line=10,
                column=5,
                code="E001",
                message="Undefined variable 'x'",
                severity="error",
            ),
        ]

        async def mock_execute_all_async(story_id, context=None):
            return {
                "typecheck": GateOutput(
                    gate_name="typecheck",
                    gate_type=GateType.TYPECHECK,
                    passed=False,
                    exit_code=1,
                    stdout="",
                    stderr="Type error",
                    duration_seconds=0.1,
                    command="mypy .",
                    error_summary="1 error found",
                    structured_errors=structured_errors,
                )
            }

        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["fail"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)
        quality_gate.execute_all_async = mock_execute_all_async
        quality_gate.should_allow_progression = lambda results: all(r.passed for r in results.values())
        quality_gate.get_failure_summary = lambda results: "Gate failed"
        quality_gate.invalidate_cache = lambda: None

        retry_config = RetryConfig(max_retries=1, base_delay_seconds=0.01)
        retry_manager = RetryManager(tmp_path, config=retry_config)

        config = ParallelExecutionConfig(
            max_concurrency=1,
            persist_progress=False,
            quality_gates_enabled=True,
            auto_retry_enabled=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
            retry_manager=retry_manager,
        )

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        # Check that retry manager received the failure with structured errors
        failure = retry_manager.get_last_failure("story-001")
        assert failure is not None
        assert failure.quality_gate_results is not None
        assert "typecheck" in failure.quality_gate_results
        assert failure.quality_gate_results["typecheck"]["structured_errors"]


class TestIncrementalChecking:
    """Tests for incremental checking based on changed files."""

    @pytest.fixture
    def sample_stories(self):
        """Create sample stories with file information."""
        return [
            {
                "id": "story-001",
                "title": "Update main module",
                "files": ["src/main.py", "tests/test_main.py"],
            },
            {
                "id": "story-002",
                "title": "Update utils",
                "affected_files": ["src/utils.py"],
            },
        ]

    @pytest.mark.asyncio
    async def test_changed_files_tracked_in_progress(self, tmp_path: Path, sample_stories):
        """Test that changed files are tracked in story progress."""
        gates = [
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)

        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,
            quality_gates_enabled=True,
            incremental_gates=True,
        )
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=config,
            quality_gate=quality_gate,
        )

        await executor.execute_batch(
            stories=sample_stories,
            batch_num=1,
        )

        progress = executor.get_current_progress()
        assert progress is not None

        # Check story-001 has its files tracked
        sp1 = progress.story_progress.get("story-001")
        assert sp1 is not None
        assert sp1.changed_files == ["src/main.py", "tests/test_main.py"]

        # Check story-002 has its affected_files tracked
        sp2 = progress.story_progress.get("story-002")
        assert sp2 is not None
        assert sp2.changed_files == ["src/utils.py"]

    def test_get_changed_files_from_story_files(self, tmp_path: Path):
        """Test _get_changed_files_for_story with files field."""
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=ParallelExecutionConfig(persist_progress=False),
        )

        story = {"id": "story-001", "files": ["a.py", "b.py"]}
        result = executor._get_changed_files_for_story(story)

        assert result == ["a.py", "b.py"]

    def test_get_changed_files_from_affected_files(self, tmp_path: Path):
        """Test _get_changed_files_for_story with affected_files field."""
        executor = ParallelExecutor(
            project_root=tmp_path,
            config=ParallelExecutionConfig(persist_progress=False),
        )

        story = {"id": "story-001", "affected_files": ["c.py"]}
        result = executor._get_changed_files_for_story(story)

        assert result == ["c.py"]


class TestProgressDisplayWithGates:
    """Tests for progress display with gate status."""

    def test_format_gate_status_empty(self, tmp_path: Path):
        """Test formatting gate status with no gates."""
        from plan_cascade.core.parallel_executor import ParallelProgressDisplay

        display = ParallelProgressDisplay()
        sp = StoryProgress(story_id="story-001", status=StoryStatus.RUNNING)

        result = display._format_gate_status(sp)
        assert result == "-"

    def test_format_gate_status_with_gates(self, tmp_path: Path):
        """Test formatting gate status with multiple gates."""
        from plan_cascade.core.parallel_executor import ParallelProgressDisplay

        display = ParallelProgressDisplay()
        sp = StoryProgress(
            story_id="story-001",
            status=StoryStatus.RUNNING,
            gate_progress={
                "typecheck": GateProgress("typecheck", "passed"),
                "test": GateProgress("test", "running"),
                "lint": GateProgress("lint", "cached", from_cache=True),
            },
        )

        result = display._format_gate_status(sp)

        # Should contain status indicators for each gate
        assert "T:" in result  # typecheck
        assert "L:" in result  # lint


class TestIntegrationEndToEnd:
    """End-to-end integration tests."""

    @pytest.mark.asyncio
    async def test_full_batch_execution_with_gates(self, tmp_path: Path):
        """Test complete batch execution with all gate features."""
        stories = [
            {"id": "story-001", "title": "Task 1", "files": ["a.py"]},
            {"id": "story-002", "title": "Task 2", "files": ["b.py"]},
        ]

        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["pass"]),
            GateConfig(name="test", type=GateType.TEST, command="echo", args=["pass"]),
            GateConfig(name="lint", type=GateType.LINT, command="echo", args=["pass"], required=False),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates, use_cache=True)

        config = ParallelExecutionConfig(
            max_concurrency=2,
            persist_progress=False,
            quality_gates_enabled=True,
            gate_caching_enabled=True,
            incremental_gates=True,
        )

        result = await run_parallel_batch(
            project_root=tmp_path,
            stories=stories,
            batch_num=1,
            config=config,
            quality_gate=quality_gate,
            show_progress=False,
        )

        assert result.success is True
        assert result.stories_completed == 2
        assert result.stories_failed == 0

    @pytest.mark.asyncio
    async def test_batch_with_gate_failure_and_retry(self, tmp_path: Path):
        """Test batch execution with gate failure and retry."""
        stories = [
            {"id": "story-001", "title": "Task 1"},
        ]

        # Create gates that pass on second attempt
        execution_count = {"typecheck": 0}

        async def mock_gate_async(story_id, context=None):
            execution_count["typecheck"] += 1
            if execution_count["typecheck"] == 1:
                return {
                    "typecheck": GateOutput(
                        gate_name="typecheck",
                        gate_type=GateType.TYPECHECK,
                        passed=False,
                        exit_code=1,
                        stdout="",
                        stderr="Error",
                        duration_seconds=0.1,
                        command="mypy",
                        error_summary="Type errors",
                    )
                }
            return {
                "typecheck": GateOutput(
                    gate_name="typecheck",
                    gate_type=GateType.TYPECHECK,
                    passed=True,
                    exit_code=0,
                    stdout="OK",
                    stderr="",
                    duration_seconds=0.1,
                    command="mypy",
                )
            }

        gates = [
            GateConfig(name="typecheck", type=GateType.TYPECHECK, command="echo", args=["pass"]),
        ]
        quality_gate = QualityGate(tmp_path, gates=gates)
        quality_gate.execute_all_async = mock_gate_async
        quality_gate.should_allow_progression = lambda results: all(r.passed for r in results.values())
        quality_gate.get_failure_summary = lambda results: "Gate failed"
        quality_gate.invalidate_cache = lambda: None

        retry_config = RetryConfig(max_retries=2, base_delay_seconds=0.01)
        retry_manager = RetryManager(tmp_path, config=retry_config)

        config = ParallelExecutionConfig(
            max_concurrency=1,
            persist_progress=False,
            quality_gates_enabled=True,
            auto_retry_enabled=True,
        )

        result = await run_parallel_batch(
            project_root=tmp_path,
            stories=stories,
            batch_num=1,
            config=config,
            quality_gate=quality_gate,
            retry_manager=retry_manager,
            show_progress=False,
        )

        # Should succeed after retry
        assert result.success is True
        assert execution_count["typecheck"] >= 2
