"""Tests for CodeReviewGate module."""

import asyncio
import json
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from plan_cascade.core.quality_gate import GateConfig, GateType, QualityGate
from plan_cascade.core.code_review_gate import (
    CodeReviewGate,
    CodeReviewResult,
    DimensionScore,
    ReviewCategory,
    ReviewFinding,
    ReviewSeverity,
)


class TestReviewSeverity:
    """Tests for ReviewSeverity enum."""

    def test_severity_values(self):
        """Test severity enum values."""
        assert ReviewSeverity.CRITICAL.value == "critical"
        assert ReviewSeverity.HIGH.value == "high"
        assert ReviewSeverity.MEDIUM.value == "medium"
        assert ReviewSeverity.LOW.value == "low"
        assert ReviewSeverity.INFO.value == "info"


class TestReviewCategory:
    """Tests for ReviewCategory enum."""

    def test_category_values(self):
        """Test category enum values."""
        assert ReviewCategory.CODE_QUALITY.value == "code_quality"
        assert ReviewCategory.NAMING_CLARITY.value == "naming_clarity"
        assert ReviewCategory.COMPLEXITY.value == "complexity"
        assert ReviewCategory.PATTERN_ADHERENCE.value == "pattern_adherence"
        assert ReviewCategory.SECURITY.value == "security"


class TestReviewFinding:
    """Tests for ReviewFinding dataclass."""

    def test_init(self):
        """Test ReviewFinding initialization."""
        finding = ReviewFinding(
            category=ReviewCategory.SECURITY,
            severity=ReviewSeverity.CRITICAL,
            file="auth.py",
            line=42,
            title="Hardcoded password",
            description="Password is hardcoded in source code",
            suggestion="Use environment variables or secrets manager",
        )

        assert finding.category == ReviewCategory.SECURITY
        assert finding.severity == ReviewSeverity.CRITICAL
        assert finding.file == "auth.py"
        assert finding.line == 42

    def test_to_dict(self):
        """Test converting to dictionary."""
        finding = ReviewFinding(
            category=ReviewCategory.CODE_QUALITY,
            severity=ReviewSeverity.MEDIUM,
            file="utils.py",
            line=10,
            title="Long function",
            description="Function is too long",
        )
        result = finding.to_dict()

        assert result["category"] == "code_quality"
        assert result["severity"] == "medium"
        assert result["file"] == "utils.py"
        assert result["line"] == 10
        assert result["title"] == "Long function"

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "category": "security",
            "severity": "high",
            "file": "api.py",
            "line": 100,
            "title": "SQL injection risk",
            "description": "User input not sanitized",
            "suggestion": "Use parameterized queries",
        }
        finding = ReviewFinding.from_dict(data)

        assert finding.category == ReviewCategory.SECURITY
        assert finding.severity == ReviewSeverity.HIGH
        assert finding.file == "api.py"
        assert finding.suggestion == "Use parameterized queries"


class TestDimensionScore:
    """Tests for DimensionScore dataclass."""

    def test_init(self):
        """Test DimensionScore initialization."""
        score = DimensionScore(
            dimension="code_quality",
            score=0.85,
            max_points=25,
            earned_points=21.25,
            notes="Good error handling",
        )

        assert score.dimension == "code_quality"
        assert score.score == 0.85
        assert score.max_points == 25
        assert score.earned_points == 21.25

    def test_to_dict(self):
        """Test converting to dictionary."""
        score = DimensionScore(
            dimension="security",
            score=1.0,
            max_points=15,
            earned_points=15,
            notes="No security issues found",
        )
        result = score.to_dict()

        assert result["dimension"] == "security"
        assert result["score"] == 1.0
        assert result["max_points"] == 15
        assert result["notes"] == "No security issues found"

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "dimension": "complexity",
            "score": 0.6,
            "max_points": 20,
            "earned_points": 12,
            "notes": "Some functions are complex",
        }
        score = DimensionScore.from_dict(data)

        assert score.dimension == "complexity"
        assert score.score == 0.6
        assert score.earned_points == 12


class TestCodeReviewResult:
    """Tests for CodeReviewResult dataclass."""

    def test_init(self):
        """Test CodeReviewResult initialization."""
        result = CodeReviewResult(
            story_id="story-001",
            overall_score=0.85,
            passed=True,
            confidence=0.9,
            summary="Good implementation",
        )

        assert result.story_id == "story-001"
        assert result.overall_score == 0.85
        assert result.passed is True
        assert result.has_critical is False

    def test_init_with_findings(self):
        """Test initialization with findings."""
        finding = ReviewFinding(
            category=ReviewCategory.CODE_QUALITY,
            severity=ReviewSeverity.MEDIUM,
            file="main.py",
            line=1,
            title="Test finding",
            description="Test description",
        )
        result = CodeReviewResult(
            story_id="story-002",
            overall_score=0.7,
            passed=True,
            confidence=0.8,
            findings=[finding],
        )

        assert len(result.findings) == 1
        assert result.findings[0].title == "Test finding"

    def test_to_dict(self):
        """Test converting to dictionary."""
        dimension_score = DimensionScore(
            dimension="code_quality",
            score=0.9,
            max_points=25,
            earned_points=22.5,
        )
        result = CodeReviewResult(
            story_id="story-003",
            overall_score=0.8,
            passed=True,
            confidence=0.85,
            dimension_scores=[dimension_score],
            summary="Good code",
            has_critical=False,
        )
        data = result.to_dict()

        assert data["story_id"] == "story-003"
        assert data["overall_score"] == 0.8
        assert len(data["dimension_scores"]) == 1
        assert data["has_critical"] is False

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "story_id": "story-004",
            "overall_score": 0.75,
            "passed": False,
            "confidence": 0.9,
            "dimension_scores": [
                {
                    "dimension": "security",
                    "score": 0.5,
                    "max_points": 15,
                    "earned_points": 7.5,
                    "notes": "Issues found",
                }
            ],
            "findings": [
                {
                    "category": "security",
                    "severity": "critical",
                    "file": "auth.py",
                    "line": 50,
                    "title": "SQL Injection",
                    "description": "Vulnerable query",
                }
            ],
            "summary": "Security issues",
            "has_critical": True,
        }
        result = CodeReviewResult.from_dict(data)

        assert result.story_id == "story-004"
        assert result.passed is False
        assert len(result.findings) == 1
        assert result.has_critical is True


class TestCodeReviewGate:
    """Tests for CodeReviewGate class."""

    @pytest.fixture
    def gate_config(self):
        """Create a test gate configuration."""
        return GateConfig(
            name="code-review",
            type=GateType.CODE_REVIEW,
            enabled=True,
            required=False,
            min_score=0.7,
            block_on_critical=True,
        )

    @pytest.fixture
    def mock_llm_provider(self):
        """Create a mock LLM provider."""
        provider = MagicMock()
        provider.complete = AsyncMock()
        return provider

    def test_init(self, tmp_path: Path, gate_config: GateConfig):
        """Test CodeReviewGate initialization."""
        gate = CodeReviewGate(gate_config, tmp_path)

        assert gate.config == gate_config
        assert gate.project_root == tmp_path

    def test_init_with_llm_provider(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test initialization with custom LLM provider."""
        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        assert gate._llm_provider == mock_llm_provider

    @pytest.mark.asyncio
    async def test_execute_async_passes(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test code review passes with good implementation."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_score": 0.85,
            "confidence": 0.9,
            "dimension_scores": [
                {"dimension": "code_quality", "score": 0.9, "max_points": 25, "earned_points": 22.5, "notes": "Good"},
                {"dimension": "naming_clarity", "score": 0.85, "max_points": 20, "earned_points": 17, "notes": "Clear"},
                {"dimension": "complexity", "score": 0.8, "max_points": 20, "earned_points": 16, "notes": "Simple"},
                {"dimension": "pattern_adherence", "score": 0.85, "max_points": 20, "earned_points": 17, "notes": "Follows patterns"},
                {"dimension": "security", "score": 0.9, "max_points": 15, "earned_points": 13.5, "notes": "Secure"},
            ],
            "findings": [],
            "summary": "Well-implemented feature",
        })
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        context = {
            "story": {
                "id": "story-001",
                "title": "Test Story",
                "description": "A test story",
            }
        }

        output = await gate.execute_async("story-001", context)

        assert output.passed is True
        assert output.gate_type == GateType.CODE_REVIEW

    @pytest.mark.asyncio
    async def test_execute_async_fails_low_score(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test code review fails with low score."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_score": 0.5,  # Below min_score of 0.7
            "confidence": 0.9,
            "dimension_scores": [],
            "findings": [
                {
                    "category": "code_quality",
                    "severity": "high",
                    "file": "main.py",
                    "line": 10,
                    "title": "Poor error handling",
                    "description": "No try-catch blocks",
                }
            ],
            "summary": "Needs improvement",
        })
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        assert "Score" in (output.error_summary or "")

    @pytest.mark.asyncio
    async def test_execute_async_fails_critical_finding(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test code review fails with critical finding."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_score": 0.8,  # Good score
            "confidence": 0.95,
            "dimension_scores": [],
            "findings": [
                {
                    "category": "security",
                    "severity": "critical",
                    "file": "auth.py",
                    "line": 50,
                    "title": "Hardcoded credentials",
                    "description": "API key is hardcoded",
                }
            ],
            "summary": "Critical security issue",
        })
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        assert "critical" in (output.error_summary or "").lower()

    @pytest.mark.asyncio
    async def test_execute_async_critical_allowed_when_disabled(
        self, tmp_path: Path, mock_llm_provider: MagicMock
    ):
        """Test critical findings don't block when block_on_critical is False."""
        config = GateConfig(
            name="code-review",
            type=GateType.CODE_REVIEW,
            enabled=True,
            min_score=0.7,
            block_on_critical=False,  # Disabled
        )

        response = MagicMock()
        response.content = json.dumps({
            "overall_score": 0.8,
            "confidence": 0.9,
            "dimension_scores": [],
            "findings": [
                {
                    "category": "security",
                    "severity": "critical",
                    "file": "auth.py",
                    "line": 50,
                    "title": "Issue",
                    "description": "Description",
                }
            ],
            "summary": "Has critical but allowed",
        })
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(config, tmp_path, llm_provider=mock_llm_provider)

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is True  # Critical doesn't block

    @pytest.mark.asyncio
    async def test_execute_async_low_confidence(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test code review fails with low confidence."""
        response = MagicMock()
        response.content = json.dumps({
            "overall_score": 0.9,
            "confidence": 0.5,  # Below threshold
            "dimension_scores": [],
            "findings": [],
            "summary": "Uncertain review",
        })
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        assert "Confidence" in (output.error_summary or "")

    @pytest.mark.asyncio
    async def test_execute_async_handles_json_error(
        self, tmp_path: Path, gate_config: GateConfig, mock_llm_provider: MagicMock
    ):
        """Test handling of JSON parse error."""
        response = MagicMock()
        response.content = "This is not valid JSON"
        mock_llm_provider.complete.return_value = response

        gate = CodeReviewGate(gate_config, tmp_path, llm_provider=mock_llm_provider)

        context = {"story": {"id": "story-001"}}
        output = await gate.execute_async("story-001", context)

        assert output.passed is False
        # Result should still be parseable
        result_data = json.loads(output.stdout)
        assert "parse" in result_data.get("summary", "").lower()

    def test_load_design_context_no_file(self, tmp_path: Path, gate_config: GateConfig):
        """Test loading design context when no file exists."""
        gate = CodeReviewGate(gate_config, tmp_path)

        context = gate._load_design_context("story-001")

        assert context == ""

    def test_load_design_context_with_file(self, tmp_path: Path, gate_config: GateConfig):
        """Test loading design context from design_doc.json."""
        design_doc = {
            "components": [
                {"name": "AuthService", "purpose": "Handle authentication"}
            ],
            "architectural_patterns": [
                {"name": "Repository", "rationale": "Data access abstraction"}
            ],
            "adrs": [
                {"id": "ADR-001", "title": "Use JWT", "decision": "Use JWT for tokens"}
            ],
            "story_mappings": {
                "story-001": {
                    "components": ["AuthService"],
                    "patterns": ["Repository"],
                    "adrs": ["ADR-001"],
                }
            }
        }
        (tmp_path / "design_doc.json").write_text(json.dumps(design_doc))

        gate = CodeReviewGate(gate_config, tmp_path)

        context = gate._load_design_context("story-001")

        assert "AuthService" in context
        assert "Repository" in context
        assert "ADR-001" in context


class TestCodeReviewGateIntegration:
    """Integration tests for CodeReviewGate with QualityGate."""

    def test_code_review_gate_lazy_loaded(self, tmp_path: Path):
        """Test that CodeReviewGate is lazy loaded."""
        # Initially not in GATE_CLASSES
        assert GateType.CODE_REVIEW not in QualityGate.GATE_CLASSES

        # After calling _get_gate_class, it should be loaded
        gate_class = QualityGate._get_gate_class(GateType.CODE_REVIEW)

        assert gate_class == CodeReviewGate
        assert GateType.CODE_REVIEW in QualityGate.GATE_CLASSES

    def test_code_review_in_post_validation_group(self, tmp_path: Path):
        """Test that CODE_REVIEW is in POST_VALIDATION group."""
        from plan_cascade.core.quality_gate import GateGroup

        qg = QualityGate(tmp_path)
        group = qg._get_gate_group(GateType.CODE_REVIEW)

        assert group == GateGroup.POST_VALIDATION
