"""
AI-Driven Strategy Analyzer for Plan Cascade

Analyzes task descriptions to determine the best execution strategy:
- Direct: Simple tasks that can be executed immediately
- Hybrid: Medium complexity tasks requiring PRD generation
- Mega: Large projects requiring multi-feature orchestration
"""

import json
import re
from collections.abc import Callable
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from ..llm.base import LLMProvider

# Type alias for streaming callback
OnTextCallback = Callable[[str], None]


class ExecutionStrategy(Enum):
    """Execution strategy types."""
    DIRECT = "direct"           # Simple task, execute directly
    HYBRID_AUTO = "hybrid_auto" # Medium task, auto-generate PRD
    MEGA_PLAN = "mega_plan"     # Large project, multi-PRD orchestration


@dataclass
class StrategyDecision:
    """Result of strategy analysis."""
    strategy: ExecutionStrategy
    use_worktree: bool
    estimated_stories: int
    confidence: float
    reasoning: str
    estimated_features: int = 1
    estimated_duration_hours: float = 1.0
    complexity_indicators: list[str] = None
    recommendations: list[str] = None

    def __post_init__(self):
        if self.complexity_indicators is None:
            self.complexity_indicators = []
        if self.recommendations is None:
            self.recommendations = []

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "strategy": self.strategy.value,
            "use_worktree": self.use_worktree,
            "estimated_stories": self.estimated_stories,
            "confidence": self.confidence,
            "reasoning": self.reasoning,
            "estimated_features": self.estimated_features,
            "estimated_duration_hours": self.estimated_duration_hours,
            "complexity_indicators": self.complexity_indicators,
            "recommendations": self.recommendations,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StrategyDecision":
        """Create from dictionary."""
        strategy_value = data.get("strategy", "hybrid_auto")
        if isinstance(strategy_value, str):
            strategy = ExecutionStrategy(strategy_value)
        else:
            strategy = strategy_value

        return cls(
            strategy=strategy,
            use_worktree=data.get("use_worktree", False),
            estimated_stories=data.get("estimated_stories", 1),
            confidence=data.get("confidence", 0.5),
            reasoning=data.get("reasoning", ""),
            estimated_features=data.get("estimated_features", 1),
            estimated_duration_hours=data.get("estimated_duration_hours", 1.0),
            complexity_indicators=data.get("complexity_indicators", []),
            recommendations=data.get("recommendations", []),
        )


class StrategyAnalyzer:
    """
    AI-driven strategy analyzer.

    Uses LLM to analyze task descriptions and determine the best
    execution strategy based on complexity, scope, and requirements.
    """

    ANALYSIS_PROMPT = """Analyze the following development task and determine the best execution strategy.

## Task Description
{description}

## Project Context
{context}

## Strategy Options
1. **direct**: Simple single-file changes like fixing typos, adding a button, small bug fixes
2. **hybrid_auto**: Medium complexity features like implementing login, adding API endpoints, creating components
3. **mega_plan**: Large projects like building complete systems, major refactoring, multi-feature development

## Analysis Required
Analyze the task complexity and return a JSON response:

```json
{{
    "strategy": "direct" | "hybrid_auto" | "mega_plan",
    "use_worktree": true | false,
    "estimated_stories": <number of stories needed>,
    "confidence": <0.0-1.0>,
    "reasoning": "<brief explanation of why this strategy was chosen>",
    "estimated_features": <number of features for mega_plan, 1 for others>,
    "estimated_duration_hours": <estimated time in hours>,
    "complexity_indicators": ["<indicator1>", "<indicator2>", ...],
    "recommendations": ["<recommendation1>", "<recommendation2>", ...]
}}
```

## Guidelines for strategy selection
- **direct**: Task mentions single file, quick fix, small change, typo, minor update
- **hybrid_auto**: Task involves implementing a feature, multiple files, integration, API
- **mega_plan**: Task involves complete system, platform, multiple features, architecture

## Guidelines for use_worktree
- Set `true` if: experimental feature, major changes, need isolation, parallel development
- Set `false` if: simple changes, normal feature development

Return ONLY the JSON object, no additional text."""

    def __init__(
        self,
        llm: Optional["LLMProvider"] = None,
        fallback_to_heuristic: bool = True
    ):
        """
        Initialize the strategy analyzer.

        Args:
            llm: LLM provider for AI analysis (optional)
            fallback_to_heuristic: Use heuristic analysis if LLM fails
        """
        self.llm = llm
        self.fallback_to_heuristic = fallback_to_heuristic

    async def analyze(
        self,
        description: str,
        context: str = "",
        project_path: Path | None = None,
        on_text: OnTextCallback | None = None
    ) -> StrategyDecision:
        """
        Analyze a task description and determine the best strategy.

        Args:
            description: Task description
            context: Additional context about the project
            project_path: Path to the project (for gathering context)
            on_text: Optional callback for streaming LLM output during analysis

        Returns:
            StrategyDecision with recommended strategy
        """
        # Gather additional context if project path provided
        if project_path and not context:
            context = await self._gather_context(project_path)

        # Try LLM analysis first
        if self.llm:
            try:
                return await self._analyze_with_llm(description, context, on_text)
            except Exception:
                if not self.fallback_to_heuristic:
                    raise
                # Fall through to heuristic analysis

        # Use heuristic analysis as fallback
        return self._analyze_heuristic(description, context)

    async def _analyze_with_llm(
        self,
        description: str,
        context: str,
        on_text: OnTextCallback | None = None
    ) -> StrategyDecision:
        """
        Analyze using LLM with optional streaming output.

        Args:
            description: Task description
            context: Project context
            on_text: Optional callback for streaming LLM output

        Returns:
            StrategyDecision from LLM analysis
        """
        prompt = self.ANALYSIS_PROMPT.format(
            description=description,
            context=context or "No additional context provided."
        )

        # Pass on_text callback to LLM for streaming support
        response = await self.llm.complete(
            [{"role": "user", "content": prompt}],
            on_text=on_text
        )

        return self._parse_llm_response(response.content)

    def _parse_llm_response(self, content: str) -> StrategyDecision:
        """
        Parse LLM response into StrategyDecision.

        Args:
            content: LLM response content

        Returns:
            StrategyDecision from parsed response
        """
        # Extract JSON from response
        json_match = re.search(r'\{[\s\S]*\}', content)
        if not json_match:
            raise ValueError("No JSON found in LLM response")

        try:
            data = json.loads(json_match.group())
        except json.JSONDecodeError as e:
            raise ValueError(f"Invalid JSON in LLM response: {e}")

        # Map strategy string to enum
        strategy_str = data.get("strategy", "hybrid_auto").lower()
        strategy_map = {
            "direct": ExecutionStrategy.DIRECT,
            "hybrid_auto": ExecutionStrategy.HYBRID_AUTO,
            "hybrid": ExecutionStrategy.HYBRID_AUTO,
            "mega_plan": ExecutionStrategy.MEGA_PLAN,
            "mega": ExecutionStrategy.MEGA_PLAN,
        }
        strategy = strategy_map.get(strategy_str, ExecutionStrategy.HYBRID_AUTO)

        return StrategyDecision(
            strategy=strategy,
            use_worktree=data.get("use_worktree", False),
            estimated_stories=data.get("estimated_stories", 1),
            confidence=data.get("confidence", 0.8),
            reasoning=data.get("reasoning", "AI analysis"),
            estimated_features=data.get("estimated_features", 1),
            estimated_duration_hours=data.get("estimated_duration_hours", 1.0),
            complexity_indicators=data.get("complexity_indicators", []),
            recommendations=data.get("recommendations", []),
        )

    def _analyze_heuristic(
        self,
        description: str,
        context: str = ""
    ) -> StrategyDecision:
        """
        Analyze using rule-based heuristics.

        Args:
            description: Task description
            context: Project context

        Returns:
            StrategyDecision from heuristic analysis
        """
        description_lower = description.lower()
        word_count = len(description.split())
        indicators = []
        recommendations = []

        # Define keyword patterns for each strategy
        mega_keywords = [
            "platform", "system", "architecture", "multiple features",
            "microservices", "complete solution", "full stack",
            "end to end", "e2e", "entire", "comprehensive", "rewrite",
            "migrate", "overhaul"
        ]

        hybrid_keywords = [
            "implement", "create", "build", "develop", "add feature",
            "integration", "api", "authentication", "database",
            "workflow", "process", "multi-step", "component"
        ]

        direct_keywords = [
            "fix bug", "fix typo", "update", "modify", "change", "tweak",
            "simple", "minor", "small", "quick", "single file", "rename"
        ]

        worktree_keywords = [
            "experimental", "prototype", "isolation", "parallel",
            "don't affect", "without breaking", "test separately"
        ]

        # Score each strategy
        mega_score = sum(1 for kw in mega_keywords if kw in description_lower)
        hybrid_score = sum(1 for kw in hybrid_keywords if kw in description_lower)
        direct_score = sum(1 for kw in direct_keywords if kw in description_lower)
        worktree_score = sum(1 for kw in worktree_keywords if kw in description_lower)

        # Adjust based on description length
        if word_count > 200:
            mega_score += 2
            indicators.append("Long description suggests complex project")
        elif word_count > 100:
            hybrid_score += 1
            indicators.append("Medium description suggests multi-story task")
        elif word_count < 30:
            direct_score += 1
            indicators.append("Short description suggests simple task")

        # Check for lists (multiple items)
        bullet_count = description.count("-") + description.count("*")
        number_count = sum(1 for i in range(10) if f"{i}." in description)
        list_count = bullet_count + number_count

        if list_count >= 5:
            mega_score += 2
            indicators.append(f"Found {list_count} list items suggesting multiple features")
        elif list_count >= 3:
            hybrid_score += 1
            indicators.append(f"Found {list_count} list items suggesting multiple stories")

        # Determine strategy
        if mega_score >= 3 and mega_score > hybrid_score:
            strategy = ExecutionStrategy.MEGA_PLAN
            confidence = min(0.9, 0.5 + mega_score * 0.1)
            estimated_features = max(2, list_count // 2 if list_count > 0 else mega_score)
            estimated_stories = estimated_features * 3
            reasoning = "Task complexity and scope suggest multi-feature architecture"
            recommendations.extend([
                "Consider breaking into independent features",
                "Use worktrees for parallel development",
                "Define feature dependencies carefully",
            ])
        elif hybrid_score >= 2 or (word_count > 50 and direct_score < 2):
            strategy = ExecutionStrategy.HYBRID_AUTO
            confidence = min(0.9, 0.5 + hybrid_score * 0.1)
            estimated_features = 1
            estimated_stories = max(2, hybrid_score + 1)
            reasoning = "Task complexity suggests structured multi-story approach"
            recommendations.extend([
                "Generate PRD with clear story dependencies",
                "Consider quality gates between stories",
            ])
        else:
            strategy = ExecutionStrategy.DIRECT
            confidence = min(0.9, 0.5 + direct_score * 0.1)
            estimated_features = 1
            estimated_stories = 1
            reasoning = "Task appears simple enough for direct execution"
            recommendations.extend([
                "Execute task directly without PRD",
                "Consider adding acceptance criteria",
            ])

        # Determine worktree usage
        use_worktree = worktree_score > 0 or strategy == ExecutionStrategy.MEGA_PLAN

        # Estimate duration
        if strategy == ExecutionStrategy.MEGA_PLAN:
            duration = estimated_features * 4.0
        elif strategy == ExecutionStrategy.HYBRID_AUTO:
            duration = estimated_stories * 1.0
        else:
            duration = 0.5

        return StrategyDecision(
            strategy=strategy,
            use_worktree=use_worktree,
            estimated_stories=estimated_stories,
            confidence=confidence,
            reasoning=reasoning,
            estimated_features=estimated_features,
            estimated_duration_hours=duration,
            complexity_indicators=indicators,
            recommendations=recommendations,
        )

    async def _gather_context(self, project_path: Path) -> str:
        """
        Gather context from the project.

        Args:
            project_path: Path to the project

        Returns:
            Context string with project information
        """
        context_parts = []

        # Check for common project files
        project_files = [
            ("package.json", "Node.js project"),
            ("pyproject.toml", "Python project"),
            ("Cargo.toml", "Rust project"),
            ("go.mod", "Go project"),
            ("pom.xml", "Java/Maven project"),
        ]

        for filename, description in project_files:
            filepath = project_path / filename
            if filepath.exists():
                context_parts.append(f"- {description} detected ({filename})")

        # Check for existing PRD
        prd_path = project_path / "prd.json"
        if prd_path.exists():
            context_parts.append("- Existing PRD found in project")

        # Check for README
        readme_path = project_path / "README.md"
        if readme_path.exists():
            try:
                content = readme_path.read_text(encoding="utf-8")[:500]
                context_parts.append(f"- README excerpt: {content[:200]}...")
            except Exception:
                pass

        # Count source files
        try:
            py_count = len(list(project_path.glob("**/*.py")))
            js_count = len(list(project_path.glob("**/*.js")))
            ts_count = len(list(project_path.glob("**/*.ts")))

            if py_count > 0:
                context_parts.append(f"- Python files: {py_count}")
            if js_count > 0:
                context_parts.append(f"- JavaScript files: {js_count}")
            if ts_count > 0:
                context_parts.append(f"- TypeScript files: {ts_count}")
        except Exception:
            pass

        if context_parts:
            return "Project context:\n" + "\n".join(context_parts)
        return "No additional project context available."


def override_strategy(
    decision: StrategyDecision,
    new_strategy: ExecutionStrategy,
    reason: str
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
        use_worktree=decision.use_worktree,
        estimated_stories=decision.estimated_stories,
        confidence=1.0,  # User override has full confidence
        reasoning=f"User override: {reason}",
        estimated_features=decision.estimated_features,
        estimated_duration_hours=decision.estimated_duration_hours,
        complexity_indicators=decision.complexity_indicators + ["User override applied"],
        recommendations=decision.recommendations,
    )
