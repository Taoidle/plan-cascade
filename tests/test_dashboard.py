"""Tests for Dashboard module."""

import json
import pytest
from pathlib import Path

from plan_cascade.core.dashboard import (
    ExecutionStatus,
    StoryStatus,
    ActionType,
    StoryInfo,
    BatchStatus,
    GateSummary,
    FailureInfo,
    RecommendedAction,
    DashboardState,
    DashboardAggregator,
    DashboardFormatter,
    read_progress_status,
    get_dashboard,
    format_dashboard,
    show_dashboard,
)


class TestExecutionStatus:
    def test_values(self):
        assert ExecutionStatus.NOT_STARTED.value == "not_started"
        assert ExecutionStatus.IN_PROGRESS.value == "in_progress"
        assert ExecutionStatus.COMPLETED.value == "completed"
        assert ExecutionStatus.FAILED.value == "failed"


class TestStoryStatus:
    def test_values(self):
        assert StoryStatus.PENDING.value == "pending"
        assert StoryStatus.IN_PROGRESS.value == "in_progress"
        assert StoryStatus.COMPLETE.value == "complete"
        assert StoryStatus.FAILED.value == "failed"


class TestStoryInfo:
    def test_to_dict(self):
        info = StoryInfo(
            story_id="story-001",
            title="Test Story",
            status=StoryStatus.COMPLETE,
        )
        d = info.to_dict()
        assert d["story_id"] == "story-001"
        assert d["status"] == "complete"

    def test_from_dict(self):
        d = {
            "story_id": "story-001",
            "title": "Test Story",
            "status": "complete",
        }
        info = StoryInfo.from_dict(d)
        assert info.story_id == "story-001"
        assert info.status == StoryStatus.COMPLETE


class TestGateSummary:
    def test_defaults(self):
        gs = GateSummary()
        assert gs.passed == 0
        assert gs.failed == 0
        assert gs.total == 0

    def test_to_dict(self):
        gs = GateSummary(passed=5, failed=2, total=7)
        d = gs.to_dict()
        assert d["passed"] == 5
        assert d["failed"] == 2


class TestDashboardState:
    def test_defaults(self):
        state = DashboardState()
        assert state.status == ExecutionStatus.NOT_STARTED
        assert state.progress_percent == 0
        assert state.is_complete is False

    def test_progress_percent(self):
        state = DashboardState(
            completed_stories=3,
            total_stories=10,
        )
        assert state.progress_percent == 30

    def test_to_dict_from_dict(self):
        state = DashboardState(
            status=ExecutionStatus.IN_PROGRESS,
            strategy="HYBRID",
            completed_stories=5,
            total_stories=10,
        )
        d = state.to_dict()
        restored = DashboardState.from_dict(d)
        assert restored.status == ExecutionStatus.IN_PROGRESS
        assert restored.strategy == "HYBRID"


class TestDashboardFormatter:
    def test_format_concise(self):
        state = DashboardState(
            status=ExecutionStatus.IN_PROGRESS,
            strategy="HYBRID",
            completed_stories=4,
            total_stories=10,
        )
        formatter = DashboardFormatter()
        output = formatter.format_concise(state)
        assert "HYBRID" in output
        assert "4/10" in output

    def test_format_verbose(self):
        state = DashboardState(
            status=ExecutionStatus.IN_PROGRESS,
            strategy="HYBRID",
        )
        formatter = DashboardFormatter()
        output = formatter.format_verbose(state)
        assert "DASHBOARD" in output
        assert "HYBRID" in output


class TestReadProgressStatus:
    def test_empty_file(self, tmp_path):
        progress = tmp_path / "progress.txt"
        progress.write_text("")
        result = read_progress_status(tmp_path)
        assert result == {}

    def test_complete_markers(self, tmp_path):
        progress = tmp_path / "progress.txt"
        text = "[STORY_COMPLETE] story-001" + chr(10) + "[COMPLETE] story-002" + chr(10)
        progress.write_text(text)
        result = read_progress_status(tmp_path)
        assert result["story-001"] == StoryStatus.COMPLETE
        assert result["story-002"] == StoryStatus.COMPLETE


class TestDashboardAggregator:
    def test_empty_project(self, tmp_path):
        aggregator = DashboardAggregator(tmp_path)
        state = aggregator.aggregate()
        assert state.status == ExecutionStatus.NOT_STARTED
        assert state.total_stories == 0

    def test_with_prd(self, tmp_path):
        prd = tmp_path / "prd.json"
        prd.write_text(json.dumps({
            "stories": [
                {"id": "story-001", "title": "Test", "dependencies": []},
            ]
        }))
        aggregator = DashboardAggregator(tmp_path)
        state = aggregator.aggregate()
        assert state.strategy == "HYBRID"
        assert state.total_stories == 1


class TestPublicAPI:
    def test_get_dashboard(self, tmp_path):
        state = get_dashboard(tmp_path)
        assert isinstance(state, DashboardState)

    def test_format_dashboard(self):
        state = DashboardState()
        output = format_dashboard(state)
        assert isinstance(output, str)

    def test_show_dashboard(self, tmp_path):
        output = show_dashboard(tmp_path)
        assert isinstance(output, str)
