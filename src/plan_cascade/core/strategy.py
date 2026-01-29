"""
Execution Strategy Selection for Plan Cascade

Determines the appropriate execution strategy based on task complexity:
- Direct: Simple tasks, single-story execution
- Hybrid: Medium tasks, multi-story PRD with dependencies
- Mega: Complex projects, multi-feature orchestration
"""

from dataclasses import dataclass
from enum import Enum
from typing import Any


class ExecutionStrategy(Enum):
    """Execution strategy types."""
    DIRECT = "direct"    # Simple task, execute directly
    HYBRID = "hybrid"    # Medium task, generate PRD with stories
    MEGA = "mega"        # Complex project, multi-feature with worktrees


@dataclass
class StrategyDecision:
    """Result of strategy analysis."""
    strategy: ExecutionStrategy
    confidence: float  # 0.0 to 1.0
    reasoning: str
    estimated_stories: int
    estimated_features: int
    estimated_duration_hours: float
    complexity_indicators: list[str]
    recommendations: list[str]

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "strategy": self.strategy.value,
            "confidence": self.confidence,
            "reasoning": self.reasoning,
            "estimated_stories": self.estimated_stories,
            "estimated_features": self.estimated_features,
            "estimated_duration_hours": self.estimated_duration_hours,
            "complexity_indicators": self.complexity_indicators,
            "recommendations": self.recommendations,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StrategyDecision":
        """Create from dictionary."""
        strategy = data.get("strategy", "hybrid")
        if isinstance(strategy, str):
            strategy = ExecutionStrategy(strategy)

        return cls(
            strategy=strategy,
            confidence=data.get("confidence", 0.5),
            reasoning=data.get("reasoning", ""),
            estimated_stories=data.get("estimated_stories", 1),
            estimated_features=data.get("estimated_features", 1),
            estimated_duration_hours=data.get("estimated_duration_hours", 1.0),
            complexity_indicators=data.get("complexity_indicators", []),
            recommendations=data.get("recommendations", []),
        )


def analyze_task_complexity(
    description: str,
    context: dict[str, Any] | None = None,
) -> StrategyDecision:
    """
    Analyze task description to determine appropriate execution strategy.

    This is a heuristic-based analysis. For production use, this would
    be enhanced with LLM analysis for more accurate complexity estimation.

    Args:
        description: Task description
        context: Optional context (codebase info, constraints, etc.)

    Returns:
        StrategyDecision with recommended strategy
    """
    description_lower = description.lower()
    word_count = len(description.split())

    indicators = []
    recommendations = []

    # Complexity indicators for MEGA (multi-feature)
    mega_keywords = [
        "platform", "system", "architecture", "multiple features",
        "microservices", "complete solution", "full stack",
        "end to end", "e2e", "entire", "comprehensive"
    ]

    # Complexity indicators for HYBRID (multi-story)
    hybrid_keywords = [
        "implement", "create", "build", "develop", "add feature",
        "integration", "api", "authentication", "database",
        "workflow", "process", "multi-step"
    ]

    # Simplicity indicators for DIRECT
    direct_keywords = [
        "fix bug", "update", "modify", "change", "tweak",
        "simple", "minor", "small", "quick", "single file"
    ]

    # Count keyword matches
    mega_score = sum(1 for kw in mega_keywords if kw in description_lower)
    hybrid_score = sum(1 for kw in hybrid_keywords if kw in description_lower)
    direct_score = sum(1 for kw in direct_keywords if kw in description_lower)

    # Description length factor
    if word_count > 200:
        mega_score += 2
        indicators.append("Long description suggests complex project")
    elif word_count > 100:
        hybrid_score += 1
        indicators.append("Medium description suggests multi-story task")
    elif word_count < 30:
        direct_score += 1
        indicators.append("Short description suggests simple task")

    # Bullet points / numbered lists suggest multiple components
    bullet_count = description.count("-") + description.count("*") + sum(
        1 for i in range(10) if f"{i}." in description
    )
    if bullet_count >= 5:
        mega_score += 2
        indicators.append(f"Found {bullet_count} list items suggesting multiple features")
    elif bullet_count >= 3:
        hybrid_score += 1
        indicators.append(f"Found {bullet_count} list items suggesting multiple stories")

    # Context-based adjustments
    if context:
        if context.get("is_greenfield", False):
            mega_score += 1
            indicators.append("Greenfield project suggests comprehensive approach")
        if context.get("existing_codebase_size", 0) > 10000:
            hybrid_score += 1
            indicators.append("Large codebase suggests careful multi-story approach")

    # Determine strategy
    if mega_score >= 3 and mega_score > hybrid_score:
        strategy = ExecutionStrategy.MEGA
        confidence = min(0.9, 0.5 + mega_score * 0.1)
        estimated_features = max(2, mega_score)
        estimated_stories = estimated_features * 3
        reasoning = "Task complexity and scope suggest multi-feature architecture"
        recommendations.extend([
            "Consider breaking into independent features with clear interfaces",
            "Use worktrees for parallel feature development",
            "Define feature dependencies carefully",
        ])
    elif hybrid_score >= 2 or (word_count > 50 and direct_score < 2):
        strategy = ExecutionStrategy.HYBRID
        confidence = min(0.9, 0.5 + hybrid_score * 0.1)
        estimated_features = 1
        estimated_stories = max(2, hybrid_score + 1)
        reasoning = "Task complexity suggests structured multi-story approach"
        recommendations.extend([
            "Generate PRD with clear story dependencies",
            "Consider quality gates between stories",
            "Use iteration loop for automatic progression",
        ])
    else:
        strategy = ExecutionStrategy.DIRECT
        confidence = min(0.9, 0.5 + direct_score * 0.1)
        estimated_features = 1
        estimated_stories = 1
        reasoning = "Task appears simple enough for direct execution"
        recommendations.extend([
            "Execute task directly without PRD generation",
            "Consider adding acceptance criteria for verification",
        ])

    # Estimate duration (rough heuristic)
    if strategy == ExecutionStrategy.MEGA:
        estimated_duration = estimated_features * 4.0  # 4 hours per feature
    elif strategy == ExecutionStrategy.HYBRID:
        estimated_duration = estimated_stories * 1.0   # 1 hour per story
    else:
        estimated_duration = 0.5  # 30 minutes for simple task

    return StrategyDecision(
        strategy=strategy,
        confidence=confidence,
        reasoning=reasoning,
        estimated_stories=estimated_stories,
        estimated_features=estimated_features,
        estimated_duration_hours=estimated_duration,
        complexity_indicators=indicators,
        recommendations=recommendations,
    )


def override_strategy(
    decision: StrategyDecision,
    new_strategy: ExecutionStrategy,
    reason: str,
) -> StrategyDecision:
    """
    Override a strategy decision (for expert mode).

    Args:
        decision: Original decision
        new_strategy: New strategy to use
        reason: Reason for override

    Returns:
        Updated StrategyDecision
    """
    return StrategyDecision(
        strategy=new_strategy,
        confidence=1.0,  # User override has full confidence
        reasoning=f"User override: {reason}",
        estimated_stories=decision.estimated_stories,
        estimated_features=decision.estimated_features,
        estimated_duration_hours=decision.estimated_duration_hours,
        complexity_indicators=decision.complexity_indicators + ["User override applied"],
        recommendations=decision.recommendations,
    )
