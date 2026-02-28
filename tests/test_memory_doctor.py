#!/usr/bin/env python3
"""
Tests for Memory Doctor — Decision Conflict Detection.

Tests the MemoryDoctor class for detecting conflicts, duplicates,
superseded decisions, and for collecting/applying actions.
"""

import json
import tempfile
from dataclasses import dataclass
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from src.plan_cascade.core.memory_doctor import (
    Diagnosis,
    DiagnosisType,
    MemoryDoctor,
)


@pytest.fixture
def temp_project_dir():
    """Create a temporary project directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


def make_design_doc(decisions: list[dict], level: str = "feature") -> dict:
    """Build a minimal design_doc.json structure."""
    return {
        "metadata": {
            "created_at": "2024-01-01T00:00:00Z",
            "version": "1.0.0",
            "source": "ai-generated",
            "level": level,
        },
        "overview": {"title": "Test", "summary": "Test doc", "goals": [], "non_goals": []},
        "architecture": {"components": [], "data_flow": "", "patterns": []},
        "interfaces": {"apis": [], "data_models": []},
        "decisions": decisions,
        "story_mappings": {} if level == "feature" else None,
        "feature_mappings": {} if level == "project" else None,
    }


def make_decision(
    id: str, title: str, decision: str, status: str = "accepted", **kwargs
) -> dict:
    """Build a minimal ADR dict."""
    d = {
        "id": id,
        "title": title,
        "context": f"Context for {title}",
        "decision": decision,
        "rationale": f"Rationale for {title}",
        "alternatives_considered": [],
        "status": status,
    }
    d.update(kwargs)
    return d


def write_design_doc(filepath: Path, decisions: list[dict], level: str = "feature"):
    """Write a design_doc.json to disk."""
    doc = make_design_doc(decisions, level)
    filepath.parent.mkdir(parents=True, exist_ok=True)
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=2, ensure_ascii=False)


def make_mock_llm(response_json: list[dict]) -> MagicMock:
    """Create a mock LLM provider returning a specific JSON response."""
    mock = MagicMock()
    resp = MagicMock()
    resp.content = json.dumps(response_json)
    mock.complete = AsyncMock(return_value=resp)
    return mock


# =============================================================================
# Tests for collect_all_decisions
# =============================================================================


class TestCollectAllDecisions:
    """Tests for collecting decisions from design documents."""

    def test_collect_from_root(self, temp_project_dir):
        """Collects decisions from project root design_doc.json."""
        decisions = [
            make_decision("ADR-001", "Use REST", "API uses REST"),
            make_decision("ADR-002", "Use PostgreSQL", "DB is PostgreSQL"),
        ]
        write_design_doc(temp_project_dir / "design_doc.json", decisions, level="project")

        doctor = MemoryDoctor(temp_project_dir)
        result = doctor.collect_all_decisions()

        assert len(result) == 2
        assert result[0]["id"] == "ADR-001"
        assert result[1]["id"] == "ADR-002"
        assert all("_source" in d for d in result)

    def test_collect_from_subdirectories(self, temp_project_dir):
        """Collects decisions from feature subdirectories."""
        write_design_doc(
            temp_project_dir / "design_doc.json",
            [make_decision("ADR-001", "Root", "Root decision")],
            level="project",
        )
        write_design_doc(
            temp_project_dir / "feature-auth" / "design_doc.json",
            [make_decision("ADR-F001", "Auth JWT", "Use JWT")],
        )
        write_design_doc(
            temp_project_dir / "feature-order" / "design_doc.json",
            [make_decision("ADR-F001", "Order REST", "Use REST for orders")],
        )

        doctor = MemoryDoctor(temp_project_dir)
        result = doctor.collect_all_decisions()

        assert len(result) == 3

    def test_collect_no_files(self, temp_project_dir):
        """Returns empty list when no design docs exist."""
        doctor = MemoryDoctor(temp_project_dir)
        result = doctor.collect_all_decisions()
        assert result == []

    def test_collect_ignores_malformed_json(self, temp_project_dir):
        """Skips files with invalid JSON."""
        filepath = temp_project_dir / "design_doc.json"
        with open(filepath, "w") as f:
            f.write("{invalid json")

        doctor = MemoryDoctor(temp_project_dir)
        result = doctor.collect_all_decisions()
        assert result == []

    def test_collect_ignores_hidden_dirs(self, temp_project_dir):
        """Skips hidden directories (starting with .)."""
        write_design_doc(
            temp_project_dir / ".hidden" / "design_doc.json",
            [make_decision("ADR-X001", "Hidden", "Should not appear")],
        )
        doctor = MemoryDoctor(temp_project_dir)
        result = doctor.collect_all_decisions()
        assert result == []


# =============================================================================
# Tests for diagnose_new_decisions (passive trigger)
# =============================================================================


class TestDiagnoseNewDecisions:
    """Tests for passive trigger diagnosis."""

    @pytest.mark.asyncio
    async def test_conflict_detected(self, temp_project_dir):
        """Detects conflict between contradictory decisions."""
        existing = [
            {**make_decision("ADR-F003", "Custom JSON", "API uses custom JSON"), "_source": "auth/design_doc.json"},
        ]
        new_decisions = [
            {**make_decision("ADR-F012", "JSON:API", "API uses JSON:API spec"), "_source": "order/design_doc.json"},
        ]

        llm_response = [
            {
                "type": "conflict",
                "decision_a_id": "ADR-F003",
                "decision_b_id": "ADR-F012",
                "explanation": "Contradictory API response format specifications",
                "suggestion": "Deprecate ADR-F003, keep ADR-F012",
            }
        ]
        mock_llm = make_mock_llm(llm_response)

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.diagnose_new_decisions(new_decisions, existing)

        assert len(result) == 1
        assert result[0].type == DiagnosisType.CONFLICT
        assert result[0].decision_a["id"] == "ADR-F003"
        assert result[0].decision_b["id"] == "ADR-F012"

    @pytest.mark.asyncio
    async def test_no_issues(self, temp_project_dir):
        """Returns empty when decisions are compatible."""
        existing = [
            {**make_decision("ADR-001", "Use REST", "API uses REST"), "_source": "root/design_doc.json"},
        ]
        new_decisions = [
            {**make_decision("ADR-F001", "Use JWT", "Auth uses JWT"), "_source": "auth/design_doc.json"},
        ]

        mock_llm = make_mock_llm([])
        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.diagnose_new_decisions(new_decisions, existing)

        assert result == []

    @pytest.mark.asyncio
    async def test_empty_inputs(self, temp_project_dir):
        """Returns empty for empty decision lists."""
        mock_llm = make_mock_llm([])
        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)

        assert await doctor.diagnose_new_decisions([], []) == []
        assert await doctor.diagnose_new_decisions([], [{"id": "x"}]) == []
        assert await doctor.diagnose_new_decisions([{"id": "x"}], []) == []

    @pytest.mark.asyncio
    async def test_no_llm_provider(self, temp_project_dir):
        """Returns empty when no LLM provider is available."""
        doctor = MemoryDoctor(temp_project_dir, llm_provider=None)
        result = await doctor.diagnose_new_decisions(
            [{"id": "x", "_source": "a"}],
            [{"id": "y", "_source": "b"}],
        )
        assert result == []


# =============================================================================
# Tests for full_diagnosis (active trigger)
# =============================================================================


class TestFullDiagnosis:
    """Tests for active trigger full diagnosis."""

    @pytest.mark.asyncio
    async def test_duplicate_detected(self, temp_project_dir):
        """Detects semantic duplicates across decisions."""
        all_decisions = [
            {**make_decision("ADR-F001", "JWT Auth", "Use JWT for authentication"), "_source": "a.json"},
            {**make_decision("ADR-F008", "JWT Token Auth", "Authentication via JWT tokens"), "_source": "b.json"},
        ]

        llm_response = [
            {
                "type": "duplicate",
                "decision_a_id": "ADR-F001",
                "decision_b_id": "ADR-F008",
                "explanation": "Both decisions specify JWT for authentication",
                "suggestion": "Merge into ADR-F001",
            }
        ]
        mock_llm = make_mock_llm(llm_response)

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis(all_decisions)

        assert len(result) == 1
        assert result[0].type == DiagnosisType.DUPLICATE

    @pytest.mark.asyncio
    async def test_superseded_detected(self, temp_project_dir):
        """Detects when a newer decision supersedes an older one."""
        all_decisions = [
            {**make_decision("ADR-001", "MySQL", "Use MySQL"), "_source": "a.json"},
            {**make_decision("ADR-005", "PostgreSQL", "Migrate to PostgreSQL"), "_source": "a.json"},
        ]

        llm_response = [
            {
                "type": "superseded",
                "decision_a_id": "ADR-001",
                "decision_b_id": "ADR-005",
                "explanation": "ADR-005 replaces MySQL with PostgreSQL",
                "suggestion": "Deprecate ADR-001",
            }
        ]
        mock_llm = make_mock_llm(llm_response)

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis(all_decisions)

        assert len(result) == 1
        assert result[0].type == DiagnosisType.SUPERSEDED

    @pytest.mark.asyncio
    async def test_too_few_decisions(self, temp_project_dir):
        """Returns empty when fewer than 2 decisions."""
        mock_llm = make_mock_llm([])
        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)

        assert await doctor.full_diagnosis([]) == []
        assert await doctor.full_diagnosis([{"id": "x"}]) == []

    @pytest.mark.asyncio
    async def test_collects_decisions_when_none_provided(self, temp_project_dir):
        """Uses collect_all_decisions when all_decisions is None."""
        write_design_doc(
            temp_project_dir / "design_doc.json",
            [
                make_decision("ADR-001", "A", "Decision A"),
                make_decision("ADR-002", "B", "Decision B"),
            ],
            level="project",
        )

        mock_llm = make_mock_llm([])
        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis(None)

        assert result == []
        # Verify LLM was called (since there are 2+ decisions)
        mock_llm.complete.assert_called_once()

    @pytest.mark.asyncio
    async def test_handles_llm_error(self, temp_project_dir):
        """Returns empty on LLM error."""
        mock_llm = MagicMock()
        mock_llm.complete = AsyncMock(side_effect=Exception("API error"))

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis([
            {**make_decision("ADR-001", "A", "A"), "_source": "a.json"},
            {**make_decision("ADR-002", "B", "B"), "_source": "b.json"},
        ])

        assert result == []

    @pytest.mark.asyncio
    async def test_handles_invalid_llm_json(self, temp_project_dir):
        """Returns empty when LLM returns invalid JSON."""
        mock_llm = MagicMock()
        resp = MagicMock()
        resp.content = "This is not JSON"
        mock_llm.complete = AsyncMock(return_value=resp)

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis([
            {**make_decision("ADR-001", "A", "A"), "_source": "a.json"},
            {**make_decision("ADR-002", "B", "B"), "_source": "b.json"},
        ])

        assert result == []

    @pytest.mark.asyncio
    async def test_handles_markdown_fenced_json(self, temp_project_dir):
        """Parses JSON even when wrapped in markdown code fencing."""
        mock_llm = MagicMock()
        resp = MagicMock()
        resp.content = '```json\n[{"type": "conflict", "decision_a_id": "ADR-001", "decision_b_id": "ADR-002", "explanation": "test", "suggestion": "test"}]\n```'
        mock_llm.complete = AsyncMock(return_value=resp)

        doctor = MemoryDoctor(temp_project_dir, llm_provider=mock_llm)
        result = await doctor.full_diagnosis([
            {**make_decision("ADR-001", "A", "A"), "_source": "a.json"},
            {**make_decision("ADR-002", "B", "B"), "_source": "b.json"},
        ])

        assert len(result) == 1
        assert result[0].type == DiagnosisType.CONFLICT


# =============================================================================
# Tests for format_report
# =============================================================================


class TestFormatReport:
    """Tests for diagnosis report formatting."""

    def test_empty_report(self, temp_project_dir):
        """Formats report with no issues."""
        doctor = MemoryDoctor(temp_project_dir)
        report = doctor.format_report([], total_scanned=5, source_count=2)

        assert "MEMORY DOCTOR" in report
        assert "No issues found" in report
        assert "5 decisions" in report
        assert "2 sources" in report

    def test_report_with_findings(self, temp_project_dir):
        """Formats report with mixed finding types."""
        diagnoses = [
            Diagnosis(
                type=DiagnosisType.CONFLICT,
                decision_a=make_decision("ADR-001", "REST", "Use REST"),
                decision_b=make_decision("ADR-005", "GraphQL", "Use GraphQL"),
                explanation="Contradictory API styles",
                suggestion="Deprecate ADR-001",
                source_a="root/design_doc.json",
                source_b="feature/design_doc.json",
            ),
            Diagnosis(
                type=DiagnosisType.DUPLICATE,
                decision_a=make_decision("ADR-F001", "JWT", "Use JWT"),
                decision_b=make_decision("ADR-F008", "JWT Token", "Use JWT tokens"),
                explanation="Same decision, different words",
                suggestion="Merge into ADR-F001",
                source_a="auth/design_doc.json",
                source_b="order/design_doc.json",
            ),
        ]

        doctor = MemoryDoctor(temp_project_dir)
        report = doctor.format_report(diagnoses, total_scanned=10, source_count=3)

        assert "CONFLICT (1)" in report
        assert "DUPLICATE (1)" in report
        assert "ADR-001" in report
        assert "ADR-005" in report
        assert "Found: 2 issues" in report


# =============================================================================
# Tests for apply_action
# =============================================================================


class TestApplyAction:
    """Tests for applying resolution actions."""

    def test_deprecate_action(self, temp_project_dir):
        """Deprecate action sets status and metadata."""
        decisions = [make_decision("ADR-001", "Old", "Old decision")]
        write_design_doc(temp_project_dir / "design_doc.json", decisions, level="project")

        diagnosis = Diagnosis(
            type=DiagnosisType.CONFLICT,
            decision_a=decisions[0],
            decision_b=make_decision("ADR-005", "New", "New decision"),
            explanation="Test",
            suggestion="Deprecate ADR-001",
            source_a=str(temp_project_dir / "design_doc.json"),
            source_b="other.json",
        )

        doctor = MemoryDoctor(temp_project_dir)
        doctor.apply_action(diagnosis, "deprecate")

        with open(temp_project_dir / "design_doc.json") as f:
            data = json.load(f)

        updated = data["decisions"][0]
        assert updated["status"] == "deprecated"
        assert updated["deprecated_by"] == "ADR-005"
        assert "deprecated_at" in updated

    def test_merge_action(self, temp_project_dir):
        """Merge action updates rationale and removes duplicate."""
        decisions_a = [make_decision("ADR-001", "Keep", "Keep this")]
        decisions_b = [make_decision("ADR-002", "Remove", "Remove this")]

        write_design_doc(temp_project_dir / "a" / "design_doc.json", decisions_a)
        write_design_doc(temp_project_dir / "b" / "design_doc.json", decisions_b)

        diagnosis = Diagnosis(
            type=DiagnosisType.DUPLICATE,
            decision_a=decisions_a[0],
            decision_b=decisions_b[0],
            explanation="Same thing",
            suggestion="Merge",
            source_a=str(temp_project_dir / "a" / "design_doc.json"),
            source_b=str(temp_project_dir / "b" / "design_doc.json"),
        )

        doctor = MemoryDoctor(temp_project_dir)
        doctor.apply_action(diagnosis, "merge")

        # A should have merge note
        with open(temp_project_dir / "a" / "design_doc.json") as f:
            data_a = json.load(f)
        assert "Merged from ADR-002" in data_a["decisions"][0]["rationale"]

        # B should have no decisions
        with open(temp_project_dir / "b" / "design_doc.json") as f:
            data_b = json.load(f)
        assert len(data_b["decisions"]) == 0

    def test_skip_action(self, temp_project_dir):
        """Skip action does nothing."""
        decisions = [make_decision("ADR-001", "Test", "Test")]
        write_design_doc(temp_project_dir / "design_doc.json", decisions, level="project")

        diagnosis = Diagnosis(
            type=DiagnosisType.CONFLICT,
            decision_a=decisions[0],
            decision_b=make_decision("ADR-002", "Other", "Other"),
            explanation="Test",
            suggestion="Skip",
            source_a=str(temp_project_dir / "design_doc.json"),
            source_b="other.json",
        )

        doctor = MemoryDoctor(temp_project_dir)
        doctor.apply_action(diagnosis, "skip")

        with open(temp_project_dir / "design_doc.json") as f:
            data = json.load(f)
        assert data["decisions"][0]["status"] == "accepted"  # Unchanged

    def test_deprecate_missing_file(self, temp_project_dir):
        """Deprecate handles missing source file gracefully."""
        diagnosis = Diagnosis(
            type=DiagnosisType.CONFLICT,
            decision_a=make_decision("ADR-001", "Test", "Test"),
            decision_b=make_decision("ADR-002", "Test2", "Test2"),
            explanation="Test",
            suggestion="Test",
            source_a=str(temp_project_dir / "nonexistent.json"),
            source_b="other.json",
        )

        doctor = MemoryDoctor(temp_project_dir)
        # Should not raise
        doctor.apply_action(diagnosis, "deprecate")


# =============================================================================
# Tests for Diagnosis.to_dict
# =============================================================================


class TestDiagnosisToDict:
    """Tests for Diagnosis serialization."""

    def test_to_dict(self):
        d = Diagnosis(
            type=DiagnosisType.CONFLICT,
            decision_a={"id": "ADR-001", "title": "A"},
            decision_b={"id": "ADR-002", "title": "B"},
            explanation="Conflict between A and B",
            suggestion="Deprecate A",
            source_a="a.json",
            source_b="b.json",
        )
        result = d.to_dict()
        assert result["type"] == "conflict"
        assert result["decision_a_id"] == "ADR-001"
        assert result["decision_b_id"] == "ADR-002"
        assert result["explanation"] == "Conflict between A and B"
