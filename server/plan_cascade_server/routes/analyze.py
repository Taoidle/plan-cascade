"""
Strategy Analysis Routes

Provides endpoints for analyzing task descriptions and determining
the optimal execution strategy.
"""

from pathlib import Path
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, Field

router = APIRouter()


class AnalyzeRequest(BaseModel):
    """Request model for strategy analysis."""
    description: str = Field(..., description="Task description to analyze")
    context: Optional[str] = Field(default=None, description="Additional context")
    project_path: Optional[str] = Field(default=None, description="Project path for context")


class AnalyzeResponse(BaseModel):
    """Response model for strategy analysis."""
    strategy: str = Field(..., description="Recommended strategy: direct, hybrid_auto, or mega_plan")
    confidence: float = Field(..., description="Confidence score (0.0-1.0)")
    reasoning: str = Field(..., description="Explanation for the strategy choice")
    use_worktree: bool = Field(default=False, description="Whether to use git worktree")
    estimated_stories: int = Field(default=1, description="Estimated number of stories")
    estimated_features: int = Field(default=1, description="Estimated number of features (for mega_plan)")
    estimated_duration_hours: float = Field(default=1.0, description="Estimated duration in hours")
    complexity_indicators: List[str] = Field(default_factory=list, description="Detected complexity indicators")
    recommendations: List[str] = Field(default_factory=list, description="Additional recommendations")


@router.post("/analyze", response_model=AnalyzeResponse)
async def analyze_strategy(request: Request, body: AnalyzeRequest) -> AnalyzeResponse:
    """
    Analyze a task description and determine the optimal execution strategy.

    Uses AI-powered analysis to evaluate task complexity and recommend:
    - **direct**: Simple single-file changes, typos, minor fixes
    - **hybrid_auto**: Medium complexity features requiring PRD generation
    - **mega_plan**: Large projects requiring multi-feature orchestration

    The analysis considers:
    - Task complexity and scope
    - Number of likely stories/features
    - Whether isolation via worktree is recommended
    - Estimated duration
    """
    try:
        from plan_cascade.core.strategy_analyzer import StrategyAnalyzer
        from plan_cascade.backends.builtin import BuiltinAgent

        # Determine project path
        project_path = Path(body.project_path) if body.project_path else Path.cwd()

        # Create backend for LLM access
        backend = BuiltinAgent(project_path=project_path)

        # Try to get LLM for AI-powered analysis
        try:
            llm = backend.get_llm()
            use_llm = True
        except Exception:
            llm = None
            use_llm = False

        # Create analyzer
        analyzer = StrategyAnalyzer(
            llm=llm if use_llm else None,
            fallback_to_heuristic=True
        )

        # Analyze the task
        decision = await analyzer.analyze(
            description=body.description,
            context=body.context or "",
            project_path=project_path,
        )

        return AnalyzeResponse(
            strategy=decision.strategy.value,
            confidence=decision.confidence,
            reasoning=decision.reasoning,
            use_worktree=decision.use_worktree,
            estimated_stories=decision.estimated_stories,
            estimated_features=decision.estimated_features,
            estimated_duration_hours=decision.estimated_duration_hours,
            complexity_indicators=decision.complexity_indicators,
            recommendations=decision.recommendations,
        )

    except ImportError as e:
        # Fall back to heuristic-only analysis
        return _heuristic_analysis(body.description, body.context)

    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"Failed to analyze strategy: {str(e)}"
        )


def _heuristic_analysis(description: str, context: Optional[str] = None) -> AnalyzeResponse:
    """
    Perform heuristic-based strategy analysis when LLM is not available.

    Uses keyword detection and pattern matching to estimate complexity.
    """
    description_lower = description.lower()

    # Strategy detection keywords (same as /plan-cascade:auto command)
    mega_keywords = [
        "platform", "system", "architecture", "infrastructure", "framework",
        "multiple features", "several modules", "various components",
        "complete", "comprehensive", "full", "entire", "whole", "end-to-end", "e2e",
        "microservices", "monorepo", "multi-tenant", "distributed"
    ]

    hybrid_keywords = [
        "implement", "create", "build", "develop", "design", "integrate",
        "feature", "function", "module", "component", "api", "endpoint", "service", "handler",
        "authentication", "authorization", "login", "registration", "crud", "database", "cache"
    ]

    isolation_keywords = [
        "experimental", "experiment", "prototype", "poc", "proof of concept",
        "parallel", "isolation", "isolated", "separate", "independently",
        "refactor", "refactoring", "rewrite", "restructure", "reorganize",
        "risky", "breaking", "major change", "don't affect", "without affecting"
    ]

    direct_keywords = [
        "fix", "typo", "update", "modify", "change", "rename", "remove", "delete", "add",
        "minor", "simple", "quick", "small", "single", "one", "only", "just", "trivial", "tiny",
        "file", "line", "button", "text", "string", "config", "setting", "style", "css"
    ]

    # Check for mega plan indicators
    mega_matches = [kw for kw in mega_keywords if kw in description_lower]
    if mega_matches:
        return AnalyzeResponse(
            strategy="mega_plan",
            confidence=0.75,
            reasoning=f"Detected large project indicators: {', '.join(mega_matches[:3])}",
            use_worktree=True,
            estimated_stories=10,
            estimated_features=3,
            estimated_duration_hours=20.0,
            complexity_indicators=mega_matches,
            recommendations=["Consider breaking into smaller features", "Use worktree for isolation"],
        )

    # Check for isolation indicators + hybrid
    isolation_matches = [kw for kw in isolation_keywords if kw in description_lower]
    hybrid_matches = [kw for kw in hybrid_keywords if kw in description_lower]

    if isolation_matches and hybrid_matches:
        return AnalyzeResponse(
            strategy="hybrid_auto",
            confidence=0.70,
            reasoning=f"Feature development with isolation recommended: {', '.join(isolation_matches[:2])}",
            use_worktree=True,
            estimated_stories=4,
            estimated_features=1,
            estimated_duration_hours=4.0,
            complexity_indicators=isolation_matches + hybrid_matches[:2],
            recommendations=["Use worktree for safe development", "Test thoroughly before merging"],
        )

    # Check for hybrid indicators
    if hybrid_matches:
        return AnalyzeResponse(
            strategy="hybrid_auto",
            confidence=0.65,
            reasoning=f"Medium complexity feature development: {', '.join(hybrid_matches[:3])}",
            use_worktree=False,
            estimated_stories=3,
            estimated_features=1,
            estimated_duration_hours=2.0,
            complexity_indicators=hybrid_matches,
            recommendations=["Review generated PRD before execution"],
        )

    # Check for direct indicators
    direct_matches = [kw for kw in direct_keywords if kw in description_lower]
    if direct_matches:
        return AnalyzeResponse(
            strategy="direct",
            confidence=0.80,
            reasoning=f"Simple task detected: {', '.join(direct_matches[:3])}",
            use_worktree=False,
            estimated_stories=1,
            estimated_features=1,
            estimated_duration_hours=0.25,
            complexity_indicators=direct_matches,
            recommendations=["Execute directly without PRD generation"],
        )

    # Default to hybrid_auto
    return AnalyzeResponse(
        strategy="hybrid_auto",
        confidence=0.50,
        reasoning="Unable to determine complexity from description, defaulting to hybrid_auto",
        use_worktree=False,
        estimated_stories=3,
        estimated_features=1,
        estimated_duration_hours=2.0,
        complexity_indicators=[],
        recommendations=["Provide more details for better strategy selection"],
    )


@router.get("/strategies")
async def get_strategies() -> Dict[str, Any]:
    """
    Get information about available execution strategies.

    Returns details about each strategy to help users understand their options.
    """
    return {
        "strategies": [
            {
                "id": "direct",
                "name": "Direct Execution",
                "description": "Execute the task immediately without generating a PRD. Best for simple, single-file changes like typos, minor bug fixes, or small updates.",
                "use_cases": [
                    "Fix typo in README",
                    "Update a button's text",
                    "Add a simple config value",
                    "Remove unused import"
                ],
                "estimated_time": "5-15 minutes"
            },
            {
                "id": "hybrid_auto",
                "name": "Hybrid Auto",
                "description": "Automatically generate a PRD with user stories and execute them in order. Best for medium-complexity features that need structured development.",
                "use_cases": [
                    "Implement user login",
                    "Add API endpoint",
                    "Create new component",
                    "Integrate third-party service"
                ],
                "estimated_time": "30 minutes - 2 hours"
            },
            {
                "id": "mega_plan",
                "name": "Mega Plan",
                "description": "Generate a project-level plan with multiple features, each with their own PRD. Best for large projects or major refactoring efforts.",
                "use_cases": [
                    "Build complete authentication system",
                    "Create e-commerce platform",
                    "Major codebase refactoring",
                    "Implement multiple related features"
                ],
                "estimated_time": "2+ hours"
            }
        ],
        "options": {
            "use_worktree": {
                "description": "Develop in an isolated Git worktree to avoid affecting the main branch. Recommended for risky changes or parallel development.",
                "default": False
            }
        }
    }
