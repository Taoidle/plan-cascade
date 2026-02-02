"""
AI-Driven Strategy Analyzer for Plan Cascade

Analyzes task descriptions to determine the best execution strategy:
- Direct: Simple tasks that can be executed immediately
- Hybrid: Medium complexity tasks requiring PRD generation
- Mega: Large projects requiring multi-feature orchestration

Also determines execution flow (Quick/Standard/Full) based on risk and confidence.
"""

import json
import re
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

from .strategy import ExecutionFlow, FlowConfig, get_flow_config

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
    """
    Result of strategy analysis.

    Extended with flow, confirm_points, and tdd_recommendation fields
    for workflow depth control and improved explainability.
    """
    strategy: ExecutionStrategy
    use_worktree: bool
    estimated_stories: int
    confidence: float
    reasoning: str
    estimated_features: int = 1
    estimated_duration_hours: float = 1.0
    complexity_indicators: list[str] = None
    recommendations: list[str] = None
    # New fields for Flow support (Story-002)
    flow: ExecutionFlow = ExecutionFlow.STANDARD
    confirm_points: list[str] = None
    tdd_recommendation: str = "auto"  # "off", "on", or "auto"
    # Analysis factors for standardized output (Story-005)
    risk_level: str = "medium"  # "low", "medium", "high"
    requires_architecture_decisions: bool = False

    def __post_init__(self):
        if self.complexity_indicators is None:
            self.complexity_indicators = []
        if self.recommendations is None:
            self.recommendations = []
        if self.confirm_points is None:
            self.confirm_points = []

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
            # New fields
            "flow": self.flow.value,
            "confirm_points": self.confirm_points,
            "tdd_recommendation": self.tdd_recommendation,
            "risk_level": self.risk_level,
            "requires_architecture_decisions": self.requires_architecture_decisions,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StrategyDecision":
        """
        Create from dictionary.

        Backward compatible: handles missing new fields gracefully.
        """
        strategy_value = data.get("strategy", "hybrid_auto")
        if isinstance(strategy_value, str):
            strategy = ExecutionStrategy(strategy_value)
        else:
            strategy = strategy_value

        # Parse flow with backward compatibility
        flow_value = data.get("flow", "standard")
        if isinstance(flow_value, str):
            try:
                flow = ExecutionFlow(flow_value)
            except ValueError:
                flow = ExecutionFlow.STANDARD
        else:
            flow = flow_value if isinstance(flow_value, ExecutionFlow) else ExecutionFlow.STANDARD

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
            # New fields with defaults for backward compatibility
            flow=flow,
            confirm_points=data.get("confirm_points", []),
            tdd_recommendation=data.get("tdd_recommendation", "auto"),
            risk_level=data.get("risk_level", "medium"),
            requires_architecture_decisions=data.get("requires_architecture_decisions", False),
        )

    def get_flow_config(self) -> FlowConfig:
        """Get the FlowConfig for this decision's flow level."""
        return get_flow_config(self.flow)


class StrategyAnalyzer:
    """
    AI-driven strategy analyzer.

    Uses LLM to analyze task descriptions and determine the best
    execution strategy based on complexity, scope, and requirements.
    Also determines the appropriate execution flow (Quick/Standard/Full).
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

## Flow Options
1. **quick**: Fastest path, minimal gating - for low-risk, high-confidence tasks
2. **standard**: Balanced speed and quality (default)
3. **full**: Strict methodology + strict gating - for high-risk or architecture decisions

## Analysis Required
Analyze the task complexity and return a JSON response:

```json
{{
    "strategy": "direct" | "hybrid_auto" | "mega_plan",
    "flow": "quick" | "standard" | "full",
    "use_worktree": true | false,
    "estimated_stories": <number of stories needed>,
    "confidence": <0.0-1.0>,
    "reasoning": "<brief explanation of why this strategy was chosen>",
    "estimated_features": <number of features for mega_plan, 1 for others>,
    "estimated_duration_hours": <estimated time in hours>,
    "complexity_indicators": ["<indicator1>", "<indicator2>", ...],
    "recommendations": ["<recommendation1>", "<recommendation2>", ...],
    "risk_level": "low" | "medium" | "high",
    "requires_architecture_decisions": true | false
}}
```

## Guidelines for strategy selection
- **direct**: Task mentions single file, quick fix, small change, typo, minor update
- **hybrid_auto**: Task involves implementing a feature, multiple files, integration, API
- **mega_plan**: Task involves complete system, platform, multiple features, architecture

## Guidelines for flow selection
- **quick**: Low risk, high confidence (>0.85), <= 2 stories
- **standard**: Medium complexity, moderate risk (default)
- **full**: High risk, low confidence (<0.7), or requires architecture decisions

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
        on_text: OnTextCallback | None = None,
        flow_override: Optional[ExecutionFlow] = None
    ) -> StrategyDecision:
        """
        Analyze a task description and determine the best strategy and flow.

        Args:
            description: Task description
            context: Additional context about the project
            project_path: Path to the project (for gathering context)
            on_text: Optional callback for streaming LLM output during analysis
            flow_override: Optional flow to force instead of auto-selection

        Returns:
            StrategyDecision with recommended strategy and flow
        """
        # Gather additional context if project path provided
        if project_path and not context:
            context = await self._gather_context(project_path)

        # Try LLM analysis first
        decision: StrategyDecision
        if self.llm:
            try:
                decision = await self._analyze_with_llm(description, context, on_text)
            except Exception:
                if not self.fallback_to_heuristic:
                    raise
                # Fall through to heuristic analysis
                decision = self._analyze_heuristic(description, context)
        else:
            # Use heuristic analysis as fallback
            decision = self._analyze_heuristic(description, context)

        # Apply flow override if provided
        if flow_override is not None:
            decision = StrategyDecision(
                strategy=decision.strategy,
                use_worktree=decision.use_worktree,
                estimated_stories=decision.estimated_stories,
                confidence=decision.confidence,
                reasoning=decision.reasoning,
                estimated_features=decision.estimated_features,
                estimated_duration_hours=decision.estimated_duration_hours,
                complexity_indicators=decision.complexity_indicators + ["Flow override applied"],
                recommendations=decision.recommendations,
                flow=flow_override,
                confirm_points=decision.confirm_points,
                tdd_recommendation=decision.tdd_recommendation,
                risk_level=decision.risk_level,
                requires_architecture_decisions=decision.requires_architecture_decisions,
            )

        return decision

    def _select_flow(
        self,
        confidence: float,
        risk_level: str,
        requires_architecture_decisions: bool,
        estimated_stories: int
    ) -> ExecutionFlow:
        """
        Select the appropriate execution flow based on analysis results.

        Flow selection logic (Story-003):
        - QUICK: risk_level='low' AND confidence>0.85 AND estimated_stories<=2
        - FULL: risk_level='high' OR confidence<0.7 OR requires_architecture_decisions=true
        - STANDARD: All other cases (default)

        Args:
            confidence: Confidence level (0.0-1.0)
            risk_level: Risk assessment ("low", "medium", "high")
            requires_architecture_decisions: Whether architecture decisions are needed
            estimated_stories: Number of estimated stories

        Returns:
            ExecutionFlow appropriate for the task
        """
        # QUICK conditions: low risk AND high confidence AND small scope
        if (risk_level == "low" and
            confidence > 0.85 and
            estimated_stories <= 2):
            return ExecutionFlow.QUICK

        # FULL conditions: high risk OR low confidence OR architecture decisions
        if (risk_level == "high" or
            confidence < 0.7 or
            requires_architecture_decisions):
            return ExecutionFlow.FULL

        # Default to STANDARD
        return ExecutionFlow.STANDARD

    def _determine_tdd_recommendation(
        self,
        flow: ExecutionFlow,
        risk_level: str,
        strategy: ExecutionStrategy
    ) -> str:
        """
        Determine TDD recommendation based on flow and risk.

        Args:
            flow: The selected execution flow
            risk_level: Risk assessment
            strategy: The selected strategy

        Returns:
            TDD recommendation: "off", "on", or "auto"
        """
        # Direct strategy rarely needs TDD
        if strategy == ExecutionStrategy.DIRECT:
            return "off"

        # Full flow or high risk suggests TDD
        if flow == ExecutionFlow.FULL or risk_level == "high":
            return "on"

        # Default to auto (let the agent decide)
        return "auto"

    def _generate_confirm_points(self, decision: StrategyDecision) -> list[str]:
        """
        Generate confirmation points based on analysis results.

        Generates 1-3 confirmation points when:
        - Low confidence (<0.7): Suggest confirming strategy selection
        - High risk: Suggest confirming risk mitigation measures
        - Architecture decisions needed: Suggest confirming design direction

        Args:
            decision: The StrategyDecision to analyze

        Returns:
            List of 1-3 confirmation point questions
        """
        confirm_points: list[str] = []

        # Low confidence - suggest confirming strategy
        if decision.confidence < 0.7:
            strategy_name = decision.strategy.value.replace("_", " ").title()
            confirm_points.append(
                f"The analysis confidence is {decision.confidence:.0%}. "
                f"Do you want to proceed with {strategy_name} strategy, or would you "
                f"prefer a different approach?"
            )

        # High risk - suggest confirming risk mitigation
        if decision.risk_level == "high":
            if decision.use_worktree:
                confirm_points.append(
                    "This task is identified as high-risk. A worktree will be used "
                    "for isolation. Have you considered rollback procedures if needed?"
                )
            else:
                confirm_points.append(
                    "This task is identified as high-risk. Do you want to enable "
                    "worktree isolation for safer development?"
                )

        # Architecture decisions needed - suggest confirming design direction
        if decision.requires_architecture_decisions:
            if decision.strategy == ExecutionStrategy.MEGA_PLAN:
                confirm_points.append(
                    "Architecture decisions are required. Have you reviewed the "
                    "design document for feature boundaries and interfaces?"
                )
            else:
                confirm_points.append(
                    "This task involves architecture decisions. Would you like to "
                    "create a design document before proceeding?"
                )

        # Additional check: Many stories might need confirmation
        if decision.estimated_stories >= 5 and len(confirm_points) < 3:
            confirm_points.append(
                f"This task is estimated to require {decision.estimated_stories} stories. "
                f"Have you verified the scope is correctly understood?"
            )

        # TDD recommendation for high-risk
        if decision.tdd_recommendation == "on" and len(confirm_points) < 3:
            if decision.flow == ExecutionFlow.FULL:
                confirm_points.append(
                    "TDD is recommended for this task. Will you write tests before "
                    "implementing each story?"
                )

        # Limit to 3 confirmation points to avoid information overload
        return confirm_points[:3]

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

        decision = self._parse_llm_response(response.content)

        # Generate confirm points based on analysis
        confirm_points = self._generate_confirm_points(decision)
        decision.confirm_points = confirm_points

        return decision

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

        # Extract risk level and architecture decisions
        risk_level = data.get("risk_level", "medium")
        requires_arch = data.get("requires_architecture_decisions", False)
        confidence = data.get("confidence", 0.8)
        estimated_stories = data.get("estimated_stories", 1)

        # Parse flow from LLM response or auto-select
        flow_str = data.get("flow", "").lower()
        flow_map = {
            "quick": ExecutionFlow.QUICK,
            "standard": ExecutionFlow.STANDARD,
            "full": ExecutionFlow.FULL,
        }
        if flow_str in flow_map:
            flow = flow_map[flow_str]
        else:
            # Auto-select flow based on analysis
            flow = self._select_flow(confidence, risk_level, requires_arch, estimated_stories)

        # Determine TDD recommendation
        tdd_recommendation = self._determine_tdd_recommendation(flow, risk_level, strategy)

        return StrategyDecision(
            strategy=strategy,
            use_worktree=data.get("use_worktree", False),
            estimated_stories=estimated_stories,
            confidence=confidence,
            reasoning=data.get("reasoning", "AI analysis"),
            estimated_features=data.get("estimated_features", 1),
            estimated_duration_hours=data.get("estimated_duration_hours", 1.0),
            complexity_indicators=data.get("complexity_indicators", []),
            recommendations=data.get("recommendations", []),
            flow=flow,
            confirm_points=[],  # Will be populated later
            tdd_recommendation=tdd_recommendation,
            risk_level=risk_level,
            requires_architecture_decisions=requires_arch,
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

        # Risk indicators
        high_risk_keywords = [
            "breaking change", "migration", "refactor", "rewrite",
            "critical", "security", "performance", "infrastructure"
        ]

        architecture_keywords = [
            "architecture", "design", "pattern", "interface", "api design",
            "schema", "data model", "microservice", "modular"
        ]

        # Score each strategy
        mega_score = sum(1 for kw in mega_keywords if kw in description_lower)
        hybrid_score = sum(1 for kw in hybrid_keywords if kw in description_lower)
        direct_score = sum(1 for kw in direct_keywords if kw in description_lower)
        worktree_score = sum(1 for kw in worktree_keywords if kw in description_lower)
        risk_score = sum(1 for kw in high_risk_keywords if kw in description_lower)
        arch_score = sum(1 for kw in architecture_keywords if kw in description_lower)

        # Determine risk level
        if risk_score >= 2 or worktree_score >= 2:
            risk_level = "high"
        elif risk_score == 1 or mega_score >= 2:
            risk_level = "medium"
        else:
            risk_level = "low"

        # Determine if architecture decisions are needed
        requires_architecture_decisions = arch_score >= 2 or mega_score >= 3

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

        # Select flow based on analysis (Story-003)
        flow = self._select_flow(confidence, risk_level, requires_architecture_decisions, estimated_stories)

        # Determine TDD recommendation
        tdd_recommendation = self._determine_tdd_recommendation(flow, risk_level, strategy)

        # Create decision
        decision = StrategyDecision(
            strategy=strategy,
            use_worktree=use_worktree,
            estimated_stories=estimated_stories,
            confidence=confidence,
            reasoning=reasoning,
            estimated_features=estimated_features,
            estimated_duration_hours=duration,
            complexity_indicators=indicators,
            recommendations=recommendations,
            flow=flow,
            confirm_points=[],  # Will be populated by _generate_confirm_points
            tdd_recommendation=tdd_recommendation,
            risk_level=risk_level,
            requires_architecture_decisions=requires_architecture_decisions,
        )

        # Generate confirm points
        decision.confirm_points = self._generate_confirm_points(decision)

        return decision

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
        flow=decision.flow,
        confirm_points=decision.confirm_points,
        tdd_recommendation=decision.tdd_recommendation,
        risk_level=decision.risk_level,
        requires_architecture_decisions=decision.requires_architecture_decisions,
    )


def override_flow(
    decision: StrategyDecision,
    new_flow: ExecutionFlow,
    reason: str = ""
) -> StrategyDecision:
    """
    Override a flow decision.

    Args:
        decision: Original decision
        new_flow: New flow to use
        reason: Reason for override (optional)

    Returns:
        Updated StrategyDecision with new flow
    """
    indicators = decision.complexity_indicators.copy()
    if reason:
        indicators.append(f"Flow override: {reason}")
    else:
        indicators.append("Flow override applied")

    return StrategyDecision(
        strategy=decision.strategy,
        use_worktree=decision.use_worktree,
        estimated_stories=decision.estimated_stories,
        confidence=decision.confidence,
        reasoning=decision.reasoning,
        estimated_features=decision.estimated_features,
        estimated_duration_hours=decision.estimated_duration_hours,
        complexity_indicators=indicators,
        recommendations=decision.recommendations,
        flow=new_flow,
        confirm_points=decision.confirm_points,
        tdd_recommendation=decision.tdd_recommendation,
        risk_level=decision.risk_level,
        requires_architecture_decisions=decision.requires_architecture_decisions,
    )


# =============================================================================
# Analysis Output Formatting (Story-005)
# =============================================================================

@dataclass
class AnalysisOutput:
    """
    Standardized analysis output structure.

    Contains four main sections:
    1. Key Factors: scope, complexity, risk, parallelism assessment
    2. Strategy Decision: strategy, flow, confidence
    3. Confirmation Points: questions for user confirmation (if any)
    4. TDD Recommendation: test-driven development guidance
    """
    # Key Factors
    scope: str  # "single_file", "single_module", "multiple_modules", "cross_cutting"
    complexity: str  # "simple", "moderate", "complex", "architectural"
    risk: str  # "low", "medium", "high"
    parallelism: str  # "none", "some", "significant"

    # Strategy Decision
    strategy: ExecutionStrategy
    flow: ExecutionFlow
    confidence: float
    reasoning: str

    # Confirmation Points
    confirm_points: list[str]

    # TDD Recommendation
    tdd_recommendation: str  # "off", "on", "auto"

    # Additional context
    estimated_stories: int
    estimated_features: int
    estimated_duration_hours: float
    use_worktree: bool
    complexity_indicators: list[str]
    recommendations: list[str]

    @classmethod
    def from_decision(cls, decision: StrategyDecision) -> "AnalysisOutput":
        """
        Create AnalysisOutput from a StrategyDecision.

        Args:
            decision: The strategy decision to convert

        Returns:
            AnalysisOutput with structured analysis data
        """
        # Determine scope based on strategy and stories
        if decision.strategy == ExecutionStrategy.DIRECT:
            scope = "single_file"
        elif decision.strategy == ExecutionStrategy.MEGA_PLAN:
            scope = "cross_cutting"
        elif decision.estimated_stories <= 2:
            scope = "single_module"
        else:
            scope = "multiple_modules"

        # Determine complexity based on stories and architecture needs
        if decision.strategy == ExecutionStrategy.DIRECT:
            complexity = "simple"
        elif decision.requires_architecture_decisions:
            complexity = "architectural"
        elif decision.estimated_stories >= 5:
            complexity = "complex"
        else:
            complexity = "moderate"

        # Determine parallelism benefit
        if decision.strategy == ExecutionStrategy.DIRECT:
            parallelism = "none"
        elif decision.strategy == ExecutionStrategy.MEGA_PLAN:
            parallelism = "significant"
        elif decision.estimated_stories >= 3:
            parallelism = "some"
        else:
            parallelism = "none"

        return cls(
            scope=scope,
            complexity=complexity,
            risk=decision.risk_level,
            parallelism=parallelism,
            strategy=decision.strategy,
            flow=decision.flow,
            confidence=decision.confidence,
            reasoning=decision.reasoning,
            confirm_points=decision.confirm_points,
            tdd_recommendation=decision.tdd_recommendation,
            estimated_stories=decision.estimated_stories,
            estimated_features=decision.estimated_features,
            estimated_duration_hours=decision.estimated_duration_hours,
            use_worktree=decision.use_worktree,
            complexity_indicators=decision.complexity_indicators,
            recommendations=decision.recommendations,
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "key_factors": {
                "scope": self.scope,
                "complexity": self.complexity,
                "risk": self.risk,
                "parallelism": self.parallelism,
            },
            "strategy_decision": {
                "strategy": self.strategy.value,
                "flow": self.flow.value,
                "confidence": self.confidence,
                "reasoning": self.reasoning,
            },
            "confirm_points": self.confirm_points,
            "tdd_recommendation": self.tdd_recommendation,
            "estimates": {
                "stories": self.estimated_stories,
                "features": self.estimated_features,
                "duration_hours": self.estimated_duration_hours,
                "use_worktree": self.use_worktree,
            },
            "analysis_details": {
                "complexity_indicators": self.complexity_indicators,
                "recommendations": self.recommendations,
            },
        }


def format_analysis_output(decision: StrategyDecision, use_color: bool = True) -> str:
    """
    Format a StrategyDecision into a human-readable analysis output.

    The output includes four clearly separated sections:
    1. Key Factors: scope, complexity, risk, parallelism
    2. Strategy Decision: strategy, flow, confidence
    3. Confirmation Points: user confirmation questions (if any)
    4. TDD Recommendation: test-driven development guidance

    Args:
        decision: The strategy decision to format
        use_color: Whether to use ANSI color codes (default True)

    Returns:
        Formatted string suitable for terminal display
    """
    output = AnalysisOutput.from_decision(decision)

    # Define formatting helpers
    def header(text: str) -> str:
        if use_color:
            return f"\033[1;36m{text}\033[0m"  # Bold cyan
        return text

    def label(text: str) -> str:
        if use_color:
            return f"\033[1m{text}\033[0m"  # Bold
        return text

    def value_color(text: str, level: str = "normal") -> str:
        if not use_color:
            return text
        colors = {
            "low": "\033[32m",      # Green
            "medium": "\033[33m",   # Yellow
            "high": "\033[31m",     # Red
            "good": "\033[32m",     # Green
            "normal": "\033[0m",    # Default
        }
        return f"{colors.get(level, colors['normal'])}{text}\033[0m"

    # Build output sections
    lines = []
    separator = "=" * 60

    lines.append(separator)
    lines.append(header("AUTO STRATEGY ANALYSIS"))
    lines.append(separator)
    lines.append("")

    # Section 1: Key Factors
    lines.append(header("Key Factors:"))
    lines.append(f"  {label('Scope:')}        {output.scope.replace('_', ' ').title()}")
    lines.append(f"  {label('Complexity:')}   {output.complexity.title()}")

    risk_level = "good" if output.risk == "low" else output.risk
    lines.append(f"  {label('Risk:')}         {value_color(output.risk.title(), risk_level)}")
    lines.append(f"  {label('Parallelism:')}  {output.parallelism.title()}")
    lines.append("")

    # Section 2: Strategy Decision
    lines.append(header("Strategy Decision:"))
    strategy_name = output.strategy.value.replace("_", " ").upper()
    flow_name = output.flow.value.upper()
    confidence_pct = f"{output.confidence:.0%}"

    confidence_level = "good" if output.confidence >= 0.8 else ("medium" if output.confidence >= 0.7 else "high")
    lines.append(f"  {label('Strategy:')}     {strategy_name}")
    lines.append(f"  {label('Flow:')}         {flow_name}")
    lines.append(f"  {label('Confidence:')}   {value_color(confidence_pct, confidence_level)}")
    lines.append(f"  {label('Reasoning:')}    {output.reasoning}")
    lines.append("")

    # Section 3: Confirmation Points (if any)
    if output.confirm_points:
        lines.append(header("Confirmation Points:"))
        for i, point in enumerate(output.confirm_points, 1):
            lines.append(f"  {i}. {point}")
        lines.append("")

    # Section 4: TDD Recommendation
    lines.append(header("TDD Recommendation:"))
    tdd_display = {
        "off": "Not recommended for this task",
        "on": "Recommended - write tests before implementation",
        "auto": "Optional - agent will decide based on context",
    }
    lines.append(f"  {tdd_display.get(output.tdd_recommendation, output.tdd_recommendation)}")
    lines.append("")

    # Additional info
    lines.append(header("Estimates:"))
    lines.append(f"  {label('Stories:')}      {output.estimated_stories}")
    if output.estimated_features > 1:
        lines.append(f"  {label('Features:')}     {output.estimated_features}")
    lines.append(f"  {label('Duration:')}     ~{output.estimated_duration_hours:.1f} hours")
    lines.append(f"  {label('Worktree:')}     {'Yes' if output.use_worktree else 'No'}")
    lines.append("")

    # Recommendations
    if output.recommendations:
        lines.append(header("Recommendations:"))
        for rec in output.recommendations:
            lines.append(f"  - {rec}")
        lines.append("")

    lines.append(separator)

    return "\n".join(lines)


def format_analysis_json(decision: StrategyDecision) -> str:
    """
    Format a StrategyDecision as JSON for machine-readable output.

    Useful for --explain flag when piping to other tools.

    Args:
        decision: The strategy decision to format

    Returns:
        JSON string with indentation
    """
    output = AnalysisOutput.from_decision(decision)
    return json.dumps(output.to_dict(), indent=2)
