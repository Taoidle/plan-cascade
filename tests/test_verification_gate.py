"""Tests for ImplementationVerificationGate module."""

import asyncio
import json
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from plan_cascade.core.quality_gate import GateConfig, GateType
from plan_cascade.core.verification_gate import (
    BatchVerificationGate,
    BatchVerificationResult,
    CriterionCheck,
    ImplementationVerifyGate,
    VerificationResult,
)


class TestCriterionCheck:
    """Tests for CriterionCheck dataclass."""

    def test_init(self):
        """Test CriterionCheck initialization."""
        check = CriterionCheck(
            criterion="User can login",
            passed=True,
            evidence="login() function implemented at auth.py:42",
            confidence=0.95,
        )

        assert check.criterion == "User can login"
        assert check.passed is True
        assert check.confidence == 0.95

    def test_to_dict(self):
        """Test converting to dictionary."""
        check = CriterionCheck(
            criterion="Test criterion",
            passed=False,
            evidence="Not found",
            confidence=0.3,
        )
        result = check.to_dict()

        assert result["criterion"] == "Test criterion"
        assert result["passed"] is False
        assert result["confidence"] == 0.3

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "criterion": "API returns 200",
            "passed": True,
            "evidence": "route handler returns 200",
            "confidence": 0.85,
        }
        check = CriterionCheck.from_dict(data)

        assert check.criterion == "API returns 200"
        assert check.passed is True
        assert check.confidence == 0.85


class TestVerificationResult:
    """Tests for VerificationResult dataclass."""

    def test_init(self):
        """Test VerificationResult initialization."""
        result = VerificationResult(
            story_id="story-001",
            overall_passed=True,
            confidence=0.9,
            skeleton_detected=False,
            summary="All criteria met",
        )

        assert result.story_id == "story-001"
        assert result.overall_passed is True
        assert result.skeleton_detected is False

    def test_to_dict(self):
        """Test converting to dictionary."""
        result = VerificationResult(
            story_id="story-002",
            overall_passed=False,
            confidence=0.5,
            skeleton_detected=True,
            skeleton_evidence="Found 'raise NotImplementedError' in utils.py",
            missing_implementations=["error handling", "validation"],
            summary="Incomplete implementation",
        )
        data = result.to_dict()

        assert data["story_id"] == "story-002"
        assert data["skeleton_detected"] is True
        assert "NotImplementedError" in data["skeleton_evidence"]
        assert len(data["missing_implementations"]) == 2

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "story_id": "story-003",
            "overall_passed": True,
            "confidence": 0.95,
            "criteria_checks": [
                {
                    "criterion": "User can submit form",
                    "passed": True,
                    "evidence": "form handler at line 50",
                    "confidence": 0.9,
                }
            ],
            "skeleton_detected": False,
            "skeleton_evidence": None,
            "missing_implementations": [],
            "summary": "All criteria verified",
        }
        result = VerificationResult.from_dict(data)

        assert result.story_id == "story-003"
        assert result.overall_passed is True
        assert len(result.criteria_checks) == 1
        assert result.criteria_checks[0].criterion == "User can submit form"


class TestBatchVerificationResult:
    """Tests for BatchVerificationResult dataclass."""

    def test_init(self):
        """Test BatchVerificationResult initialization."""
        result = BatchVerificationResult(
            batch_num=1,
            passed=True,
        )

        assert result.batch_num == 1
        assert result.passed is True
        assert len(result.results) == 0
        assert len(result.blocking_failures) == 0

    def test_to_dict(self):
        """Test converting to dictionary."""
        verification = VerificationResult(
            story_id="story-001",
            overall_passed=True,
            confidence=0.9,
        )
        batch = BatchVerificationResult(
            batch_num=2,
            passed=True,
            results={"story-001": verification},
        )
        data = batch.to_dict()

        assert data["batch_num"] == 2
        assert "story-001" in data["results"]

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "batch_num": 3,
            "passed": False,
            "results": {
                "story-001": {
                    "story_id": "story-001",
                    "overall_passed": False,
                    "confidence": 0.4,
                    "skeleton_detected": True,
                    "skeleton_evidence": "pass in function",
                    "missing_implementations": [],
                    "summary": "Skeleton code detected",
                }
            },
            "blocking_failures": ["story-001"],
            "async_fix_queue": [],
        }
        result = BatchVerificationResult.from_dict(data)

        assert result.batch_num == 3
        assert result.passed is False
        assert "story-001" in result.blocking_failures


class TestImplementationVerifyGate:
    """Tests for ImplementationVerifyGate class."""

    @pytest.fixture
    def gate_config(self):
        """Create a test gate configuration."""
        return GateConfig(
            name="impl-verify",
            type=GateType.IMPLEMENTATION_VERIFY,
            enabled=True,
            required=True,
        )

    @pytest.fixture
    def mock_llm_provider(self):
        """Create a mock LLM provider."""
        provider = MagicMock()
        provider.complete = AsyncMock()
        return provider

    def test_init(self, tmp_path: Path, gate_config: GateConfig):
        """Test ImplementationVerifyGate initialization."""
        gate = ImplementationVerifyGate(gate_config, tmp_path)

        assert gate.config == gate_config
        assert gate.project_root == tmp_path
        assert gate.confidence_threshold == 0.7

    def test_init_custom_threshold(self, tmp_path: Path, gate_config: GateConfig):
        """Test initialization with custom confidence threshold."""
        gate = ImplementationVerifyGate(
            gate_config, tmp_path, confidence_threshold=0.9
        )

        assert gate.confidence_threshold == 0.9

    def test_format_criteria_with_criteria(self, tmp_path: Path, gate_config: GateConfig):
        """Test formatting acceptance criteria."""
        gate = ImplementationVerifyGate(gate_config, tmp_path)
        story = {
            "acceptance_criteria": [
                "User can login with email",
                "User can reset password",
                "Session expires after 24 hours",
            ]
        }

        result = gate._format_criteria(story)

        assert "User can login with email" in result
        assert "User can reset password" in result
        assert "Session expires" in result

    def test_format_criteria_empty(self, tmp_path: Path, gate_config: GateConfig):
        """Test formatting when no criteria defined."""
        gate = ImplementationVerifyGate(gate_config, tmp_path)
        story = {}

        result = gate._format_criteria(story)

        assert "no acceptance criteria" in result.lower()

    @pytest.mark.asyncio
    async def test_execute_async_passes(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test verification passes with good implementation."""
        # Mock LLM response
        response = MagicMock()
        response.content = json.dumps({
            "overall_passed": True,
            "confidence": 0.95,
            "criteria_checks": [],
            "skeleton_detected": False,
            "skeleton_evidence": None,
            "missing_implementations": [],
            "summary": "All criteria met",
        })
        mock_llm_provider.complete.return_value = response

        gate = ImplementationVerifyGate(
            gate_config, tmp_path, llm_provider=mock_llm_provider
        )

        context = {
            "story": {
                "id": "story-001",
                "title": "Test Story",
                "description": "A test story",
                "acceptance_criteria": ["Criterion 1"],
            }
        }

        output = await gate.execute_async("story-001", context)

        assert output.passed is True
        assert output.gate_type == GateType.IMPLEMENTATION_VERIFY

    @pytest.mark.asyncio
    async def test_execute_async_detects_skeleton(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test verification fails when skeleton code detected."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_passed": False,
            "confidence": 0.8,
            "criteria_checks": [],
            "skeleton_detected": True,
            "skeleton_evidence": "Found 'pass' in login() function",
            "missing_implementations": ["authentication logic"],
            "summary": "Skeleton code detected",
        })
        mock_llm_provider.complete.return_value = response

        gate = ImplementationVerifyGate(
            gate_config, tmp_path, llm_provider=mock_llm_provider
        )

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        assert "SKELETON" in (output.error_summary or "")

    @pytest.mark.asyncio
    async def test_execute_async_low_confidence(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test verification fails with low confidence."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_passed": True,
            "confidence": 0.5,  # Below threshold
            "criteria_checks": [],
            "skeleton_detected": False,
            "skeleton_evidence": None,
            "missing_implementations": [],
            "summary": "Uncertain verification",
        })
        mock_llm_provider.complete.return_value = response

        gate = ImplementationVerifyGate(
            gate_config, tmp_path, llm_provider=mock_llm_provider
        )

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        assert "Low confidence" in (output.error_summary or "")


class TestBatchVerificationGate:
    """Tests for BatchVerificationGate class."""

    @pytest.fixture
    def mock_llm_provider(self):
        """Create a mock LLM provider."""
        provider = MagicMock()
        provider.complete = AsyncMock()
        return provider

    def test_init(self, tmp_path: Path):
        """Test BatchVerificationGate initialization."""
        gate = BatchVerificationGate(tmp_path)

        assert gate.project_root == tmp_path
        assert gate.confidence_threshold == 0.7

    def test_story_blocks_others_true(self, tmp_path: Path):
        """Test detecting blocking story."""
        gate = BatchVerificationGate(tmp_path)

        batch_stories = [
            {"id": "story-001"},
            {"id": "story-002"},
        ]
        prd = {
            "stories": [
                {"id": "story-001"},
                {"id": "story-002"},
                {"id": "story-003", "dependencies": ["story-001"]},  # Future story
            ]
        }

        result = gate._story_blocks_others("story-001", batch_stories, prd)

        assert result is True  # story-003 depends on story-001

    def test_story_blocks_others_false(self, tmp_path: Path):
        """Test story that doesn't block others."""
        gate = BatchVerificationGate(tmp_path)

        batch_stories = [
            {"id": "story-001"},
            {"id": "story-002"},
        ]
        prd = {
            "stories": [
                {"id": "story-001"},
                {"id": "story-002"},
                {"id": "story-003", "dependencies": []},  # No dependencies
            ]
        }

        result = gate._story_blocks_others("story-001", batch_stories, prd)

        assert result is False

    @pytest.mark.asyncio
    async def test_verify_batch_all_pass(self, tmp_path: Path, mock_llm_provider: MagicMock):
        """Test batch verification when all stories pass."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_passed": True,
            "confidence": 0.9,
            "criteria_checks": [],
            "skeleton_detected": False,
            "skeleton_evidence": None,
            "missing_implementations": [],
            "summary": "All criteria met",
        })
        mock_llm_provider.complete.return_value = response

        gate = BatchVerificationGate(tmp_path, llm_provider=mock_llm_provider)

        stories = [
            {"id": "story-001", "title": "Story 1"},
            {"id": "story-002", "title": "Story 2"},
        ]
        prd = {"stories": stories}

        result = await gate.verify_batch(stories, batch_num=1, prd=prd)

        assert result.passed is True
        assert len(result.blocking_failures) == 0
        assert "story-001" in result.results
        assert "story-002" in result.results

    @pytest.mark.asyncio
    async def test_verify_batch_blocking_failure(
        self, tmp_path: Path, mock_llm_provider: MagicMock
    ):
        """Test batch verification with a blocking failure."""
        # First story fails, second passes
        responses = [
            MagicMock(content=json.dumps({
                "overall_passed": False,
                "confidence": 0.8,
                "criteria_checks": [],
                "skeleton_detected": True,
                "skeleton_evidence": "pass in function",
                "missing_implementations": ["logic"],
                "summary": "Skeleton detected",
            })),
            MagicMock(content=json.dumps({
                "overall_passed": True,
                "confidence": 0.9,
                "criteria_checks": [],
                "skeleton_detected": False,
                "skeleton_evidence": None,
                "missing_implementations": [],
                "summary": "Complete",
            })),
        ]
        mock_llm_provider.complete.side_effect = responses

        gate = BatchVerificationGate(tmp_path, llm_provider=mock_llm_provider)

        stories = [
            {"id": "story-001", "title": "Story 1"},
            {"id": "story-002", "title": "Story 2"},
        ]
        prd = {
            "stories": stories + [
                {"id": "story-003", "dependencies": ["story-001"]},  # Depends on story-001
            ]
        }

        result = await gate.verify_batch(stories, batch_num=1, prd=prd)

        assert result.passed is False
        assert "story-001" in result.blocking_failures
