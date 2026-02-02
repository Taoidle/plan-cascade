"""Tests for the StrategyAnalyzer and related functionality."""

import pytest
from plan_cascade.core.strategy import (
    ExecutionFlow,
    FlowConfig,
    get_flow_config,
)
from plan_cascade.core.strategy_analyzer import (
    ExecutionStrategy,
    StrategyDecision,
    StrategyAnalyzer,
    AnalysisOutput,
    format_analysis_output,
    format_analysis_json,
    override_strategy,
    override_flow,
)


class TestExecutionFlow:
    """Tests for ExecutionFlow enum and FlowConfig."""

    def test_execution_flow_values(self):
        """Test that ExecutionFlow has the expected values."""
        assert ExecutionFlow.QUICK.value == "quick"
        assert ExecutionFlow.STANDARD.value == "standard"
        assert ExecutionFlow.FULL.value == "full"

    def test_get_flow_config_quick(self):
        """Test QUICK flow configuration."""
        config = get_flow_config(ExecutionFlow.QUICK)
        assert config.gate_mode == "soft"
        assert config.require_confirm is False
        assert config.enable_ai_verification is False
        assert config.require_code_review is False

    def test_get_flow_config_standard(self):
        """Test STANDARD flow configuration."""
        config = get_flow_config(ExecutionFlow.STANDARD)
        assert config.gate_mode == "soft"
        assert config.require_confirm is False
        assert config.enable_ai_verification is True
        assert config.require_code_review is False

    def test_get_flow_config_full(self):
        """Test FULL flow configuration."""
        config = get_flow_config(ExecutionFlow.FULL)
        assert config.gate_mode == "hard"
        assert config.require_confirm is True
        assert config.enable_ai_verification is True
        assert config.require_code_review is True

    def test_flow_config_to_dict(self):
        """Test FlowConfig serialization."""
        config = get_flow_config(ExecutionFlow.FULL)
        data = config.to_dict()
        assert data["gate_mode"] == "hard"
        assert data["require_confirm"] is True

    def test_flow_config_from_dict(self):
        """Test FlowConfig deserialization."""
        data = {"gate_mode": "hard", "require_confirm": True, "enable_ai_verification": True}
        config = FlowConfig.from_dict(data)
        assert config.gate_mode == "hard"
        assert config.require_confirm is True


class TestStrategyDecision:
    """Tests for StrategyDecision dataclass."""

    def test_strategy_decision_defaults(self):
        """Test StrategyDecision default values."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test reasoning",
        )
        assert decision.flow == ExecutionFlow.STANDARD
        assert decision.confirm_points == []
        assert decision.tdd_recommendation == "auto"
        assert decision.risk_level == "medium"
        assert decision.requires_architecture_decisions is False

    def test_strategy_decision_to_dict(self):
        """Test StrategyDecision serialization with new fields."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test",
            flow=ExecutionFlow.FULL,
            confirm_points=["Point 1"],
            tdd_recommendation="on",
            risk_level="high",
            requires_architecture_decisions=True,
        )
        data = decision.to_dict()
        assert data["strategy"] == "hybrid_auto"
        assert data["flow"] == "full"
        assert data["confirm_points"] == ["Point 1"]
        assert data["tdd_recommendation"] == "on"
        assert data["risk_level"] == "high"
        assert data["requires_architecture_decisions"] is True

    def test_strategy_decision_from_dict_with_new_fields(self):
        """Test StrategyDecision deserialization with new fields."""
        data = {
            "strategy": "hybrid_auto",
            "use_worktree": True,
            "estimated_stories": 5,
            "confidence": 0.6,
            "reasoning": "Complex task",
            "flow": "full",
            "confirm_points": ["Confirm strategy?"],
            "tdd_recommendation": "on",
            "risk_level": "high",
            "requires_architecture_decisions": True,
        }
        decision = StrategyDecision.from_dict(data)
        assert decision.strategy == ExecutionStrategy.HYBRID_AUTO
        assert decision.flow == ExecutionFlow.FULL
        assert decision.confirm_points == ["Confirm strategy?"]
        assert decision.tdd_recommendation == "on"
        assert decision.risk_level == "high"

    def test_strategy_decision_from_dict_backward_compatible(self):
        """Test StrategyDecision deserialization handles missing new fields."""
        # Old format without new fields
        data = {
            "strategy": "hybrid_auto",
            "use_worktree": False,
            "estimated_stories": 3,
            "confidence": 0.8,
            "reasoning": "Test",
        }
        decision = StrategyDecision.from_dict(data)
        # Should use defaults for missing fields
        assert decision.flow == ExecutionFlow.STANDARD
        assert decision.confirm_points == []
        assert decision.tdd_recommendation == "auto"
        assert decision.risk_level == "medium"

    def test_get_flow_config_method(self):
        """Test StrategyDecision.get_flow_config() convenience method."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test",
            flow=ExecutionFlow.FULL,
        )
        config = decision.get_flow_config()
        assert config.gate_mode == "hard"
        assert config.require_confirm is True


class TestStrategyAnalyzerFlowSelection:
    """Tests for flow selection logic in StrategyAnalyzer."""

    def test_select_flow_quick(self):
        """Test QUICK flow selection conditions."""
        analyzer = StrategyAnalyzer()
        # QUICK: risk_level='low' AND confidence>0.85 AND estimated_stories<=2
        flow = analyzer._select_flow(
            confidence=0.9,
            risk_level="low",
            requires_architecture_decisions=False,
            estimated_stories=2,
        )
        assert flow == ExecutionFlow.QUICK

    def test_select_flow_full_high_risk(self):
        """Test FULL flow selection for high risk."""
        analyzer = StrategyAnalyzer()
        flow = analyzer._select_flow(
            confidence=0.9,
            risk_level="high",
            requires_architecture_decisions=False,
            estimated_stories=2,
        )
        assert flow == ExecutionFlow.FULL

    def test_select_flow_full_low_confidence(self):
        """Test FULL flow selection for low confidence."""
        analyzer = StrategyAnalyzer()
        flow = analyzer._select_flow(
            confidence=0.6,
            risk_level="low",
            requires_architecture_decisions=False,
            estimated_stories=2,
        )
        assert flow == ExecutionFlow.FULL

    def test_select_flow_full_architecture_decisions(self):
        """Test FULL flow selection when architecture decisions needed."""
        analyzer = StrategyAnalyzer()
        # Architecture decisions trigger FULL flow even with high confidence
        # Note: The condition is checked AFTER the QUICK condition, so with
        # low risk, high confidence, and few stories, QUICK wins.
        # For FULL to win, we need the architecture flag to be dominant.
        flow = analyzer._select_flow(
            confidence=0.8,  # Below 0.85 threshold for QUICK
            risk_level="low",
            requires_architecture_decisions=True,
            estimated_stories=2,
        )
        assert flow == ExecutionFlow.FULL

    def test_select_flow_standard_default(self):
        """Test STANDARD flow as default."""
        analyzer = StrategyAnalyzer()
        flow = analyzer._select_flow(
            confidence=0.8,
            risk_level="medium",
            requires_architecture_decisions=False,
            estimated_stories=4,
        )
        assert flow == ExecutionFlow.STANDARD


class TestConfirmPointsGeneration:
    """Tests for confirmation points generation."""

    def test_generate_confirm_points_low_confidence(self):
        """Test confirm points generated for low confidence."""
        analyzer = StrategyAnalyzer()
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.6,  # Low confidence
            reasoning="Uncertain analysis",
            risk_level="medium",
        )
        points = analyzer._generate_confirm_points(decision)
        assert len(points) >= 1
        assert any("confidence" in p.lower() for p in points)

    def test_generate_confirm_points_high_risk(self):
        """Test confirm points generated for high risk."""
        analyzer = StrategyAnalyzer()
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test",
            risk_level="high",  # High risk
        )
        points = analyzer._generate_confirm_points(decision)
        assert len(points) >= 1
        assert any("risk" in p.lower() for p in points)

    def test_generate_confirm_points_architecture_decisions(self):
        """Test confirm points for architecture decisions."""
        analyzer = StrategyAnalyzer()
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test",
            risk_level="medium",
            requires_architecture_decisions=True,
        )
        points = analyzer._generate_confirm_points(decision)
        assert len(points) >= 1
        assert any("architecture" in p.lower() or "design" in p.lower() for p in points)

    def test_generate_confirm_points_max_three(self):
        """Test that confirm points are limited to 3."""
        analyzer = StrategyAnalyzer()
        decision = StrategyDecision(
            strategy=ExecutionStrategy.MEGA_PLAN,
            use_worktree=True,
            estimated_stories=10,  # Many stories
            confidence=0.5,  # Very low confidence
            reasoning="Complex task",
            risk_level="high",  # High risk
            requires_architecture_decisions=True,  # Needs architecture
            flow=ExecutionFlow.FULL,
            tdd_recommendation="on",
        )
        points = analyzer._generate_confirm_points(decision)
        assert len(points) <= 3


class TestAnalysisOutput:
    """Tests for AnalysisOutput formatting."""

    def test_analysis_output_from_decision(self):
        """Test AnalysisOutput.from_decision()."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=4,
            confidence=0.8,
            reasoning="Feature development task",
            flow=ExecutionFlow.STANDARD,
            risk_level="medium",
            requires_architecture_decisions=False,
        )
        output = AnalysisOutput.from_decision(decision)
        assert output.strategy == ExecutionStrategy.HYBRID_AUTO
        assert output.flow == ExecutionFlow.STANDARD
        assert output.scope == "multiple_modules"
        assert output.complexity == "moderate"

    def test_analysis_output_to_dict(self):
        """Test AnalysisOutput serialization."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.DIRECT,
            use_worktree=False,
            estimated_stories=1,
            confidence=0.9,
            reasoning="Simple fix",
            flow=ExecutionFlow.QUICK,
            risk_level="low",
        )
        output = AnalysisOutput.from_decision(decision)
        data = output.to_dict()
        assert "key_factors" in data
        assert "strategy_decision" in data
        assert "confirm_points" in data
        assert "tdd_recommendation" in data
        assert data["key_factors"]["scope"] == "single_file"
        assert data["strategy_decision"]["strategy"] == "direct"

    def test_format_analysis_output(self):
        """Test human-readable formatting."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test task",
            flow=ExecutionFlow.STANDARD,
            risk_level="medium",
            recommendations=["Use quality gates"],
        )
        output = format_analysis_output(decision, use_color=False)
        assert "AUTO STRATEGY ANALYSIS" in output
        assert "HYBRID AUTO" in output  # Formatted with space for readability
        assert "STANDARD" in output
        assert "80%" in output

    def test_format_analysis_json(self):
        """Test JSON formatting."""
        decision = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Test",
        )
        json_output = format_analysis_json(decision)
        import json
        data = json.loads(json_output)
        assert data["strategy_decision"]["strategy"] == "hybrid_auto"


class TestOverrideFunctions:
    """Tests for override functions."""

    def test_override_strategy_preserves_new_fields(self):
        """Test that override_strategy preserves new fields."""
        original = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Original",
            flow=ExecutionFlow.FULL,
            confirm_points=["Point 1"],
            tdd_recommendation="on",
            risk_level="high",
        )
        overridden = override_strategy(original, ExecutionStrategy.MEGA_PLAN, "User choice")
        assert overridden.strategy == ExecutionStrategy.MEGA_PLAN
        assert overridden.flow == ExecutionFlow.FULL  # Preserved
        assert overridden.confirm_points == ["Point 1"]  # Preserved
        assert overridden.tdd_recommendation == "on"  # Preserved
        assert overridden.confidence == 1.0  # Override has full confidence

    def test_override_flow(self):
        """Test flow override function."""
        original = StrategyDecision(
            strategy=ExecutionStrategy.HYBRID_AUTO,
            use_worktree=False,
            estimated_stories=3,
            confidence=0.8,
            reasoning="Original",
            flow=ExecutionFlow.STANDARD,
        )
        overridden = override_flow(original, ExecutionFlow.FULL, "User wants strict mode")
        assert overridden.flow == ExecutionFlow.FULL
        assert overridden.strategy == ExecutionStrategy.HYBRID_AUTO  # Unchanged
        assert "Flow override" in overridden.complexity_indicators[-1]


class TestHeuristicAnalysis:
    """Tests for heuristic analysis including new fields."""

    def test_heuristic_simple_task(self):
        """Test heuristic analysis for a simple task."""
        analyzer = StrategyAnalyzer()
        decision = analyzer._analyze_heuristic("Fix typo in README")
        assert decision.strategy == ExecutionStrategy.DIRECT
        # Note: QUICK requires confidence > 0.85, but short descriptions get ~0.7
        # So simple tasks typically get STANDARD flow unless confidence is very high
        assert decision.flow in [ExecutionFlow.STANDARD, ExecutionFlow.QUICK]
        assert decision.risk_level == "low"
        assert decision.tdd_recommendation == "off"  # DIRECT doesn't need TDD

    def test_heuristic_feature_task(self):
        """Test heuristic analysis for a feature task."""
        analyzer = StrategyAnalyzer()
        decision = analyzer._analyze_heuristic(
            "Implement user authentication with login and registration API"
        )
        assert decision.strategy == ExecutionStrategy.HYBRID_AUTO
        assert decision.flow in [ExecutionFlow.STANDARD, ExecutionFlow.QUICK]

    def test_heuristic_risky_task(self):
        """Test heuristic analysis for a risky task."""
        analyzer = StrategyAnalyzer()
        decision = analyzer._analyze_heuristic(
            "Critical security migration with breaking changes to the authentication infrastructure"
        )
        assert decision.risk_level == "high"
        assert decision.flow == ExecutionFlow.FULL

    def test_heuristic_architecture_task(self):
        """Test heuristic analysis for architecture-heavy task."""
        analyzer = StrategyAnalyzer()
        decision = analyzer._analyze_heuristic(
            "Design the new microservice architecture with API design and data model schema"
        )
        assert decision.requires_architecture_decisions is True
        # Architecture tasks should have confirm points
        assert len(decision.confirm_points) > 0
