"""
Definition of Ready (DoR) Gate Implementation for Plan Cascade.

Provides validation gates that run before story/feature execution to ensure
all prerequisites are met. Supports two modes:
- SOFT: Warnings only, execution continues
- HARD: Blocking, execution halts on failures

ADR-003: DoR gates start soft, become hard in Full Flow.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class GateMode(Enum):
    """Gate enforcement mode."""
    SOFT = "soft"   # Warnings only, execution continues
    HARD = "hard"   # Blocking, execution halts on failures


@dataclass
class ReadinessCheckResult:
    """
    Result of a readiness check.

    Attributes:
        passed: Whether all checks passed
        warnings: Non-blocking issues found
        errors: Blocking issues found (in HARD mode)
        suggestions: Improvement suggestions
        check_name: Name of the check that was run
        details: Additional details about the check
    """
    passed: bool = True
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    suggestions: list[str] = field(default_factory=list)
    check_name: str = ""
    details: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "passed": self.passed,
            "warnings": self.warnings,
            "errors": self.errors,
            "suggestions": self.suggestions,
            "check_name": self.check_name,
            "details": self.details,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ReadinessCheckResult":
        """Create from dictionary."""
        return cls(
            passed=data.get("passed", True),
            warnings=data.get("warnings", []),
            errors=data.get("errors", []),
            suggestions=data.get("suggestions", []),
            check_name=data.get("check_name", ""),
            details=data.get("details", {}),
        )

    @classmethod
    def combine(cls, results: list["ReadinessCheckResult"]) -> "ReadinessCheckResult":
        """Combine multiple check results into one."""
        if not results:
            return cls(passed=True, check_name="combined")

        combined = cls(
            passed=all(r.passed for r in results),
            warnings=[],
            errors=[],
            suggestions=[],
            check_name="combined",
            details={"checks": [r.check_name for r in results]},
        )

        for result in results:
            combined.warnings.extend(result.warnings)
            combined.errors.extend(result.errors)
            combined.suggestions.extend(result.suggestions)

        return combined

    def add_warning(self, warning: str) -> None:
        """Add a warning message."""
        self.warnings.append(warning)

    def add_error(self, error: str) -> None:
        """Add an error message and mark as failed."""
        self.errors.append(error)
        self.passed = False

    def add_suggestion(self, suggestion: str) -> None:
        """Add a suggestion."""
        self.suggestions.append(suggestion)

    def has_issues(self) -> bool:
        """Check if there are any warnings or errors."""
        return bool(self.warnings) or bool(self.errors)

    def get_summary(self) -> str:
        """Get a human-readable summary of the result."""
        status = "PASSED" if self.passed else "FAILED"
        parts = [f"[{status}] {self.check_name}"]

        if self.errors:
            parts.append(f"  Errors ({len(self.errors)}):")
            for error in self.errors:
                parts.append(f"    - {error}")

        if self.warnings:
            parts.append(f"  Warnings ({len(self.warnings)}):")
            for warning in self.warnings:
                parts.append(f"    - {warning}")

        if self.suggestions:
            parts.append(f"  Suggestions ({len(self.suggestions)}):")
            for suggestion in self.suggestions:
                parts.append(f"    - {suggestion}")

        return chr(10).join(parts)


# =============================================================================
# PRD DoR Checks
# =============================================================================

def check_acceptance_criteria(stories: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that all stories have testable acceptance criteria.

    Args:
        stories: List of story dictionaries from PRD

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="acceptance_criteria")

    for story in stories:
        story_id = story.get("id", "unknown")
        criteria = story.get("acceptance_criteria", [])

        if not criteria:
            result.add_error(f"Story {story_id} has no acceptance criteria")
            continue

        # Check for testable criteria (should have action verbs)
        testable_verbs = ["should", "must", "can", "will", "returns", "displays", "validates"]
        for i, criterion in enumerate(criteria, 1):
            criterion_lower = criterion.lower()
            if not any(verb in criterion_lower for verb in testable_verbs):
                result.add_warning(
                    f"Story {story_id} criterion {i} may not be testable: '{criterion[:50]}...'"
                )

    if not result.errors and not result.warnings:
        result.details["message"] = "All stories have testable acceptance criteria"

    return result


def check_dependencies(stories: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that story dependencies are valid and form a DAG (no cycles).

    Args:
        stories: List of story dictionaries from PRD

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="dependencies")

    story_ids = {s.get("id") for s in stories}

    # Build adjacency list
    graph: dict[str, list[str]] = {s.get("id", ""): [] for s in stories}

    for story in stories:
        story_id = story.get("id", "")
        deps = story.get("dependencies", [])

        for dep in deps:
            if dep not in story_ids:
                result.add_error(f"Story {story_id} depends on unknown story: {dep}")
            else:
                graph[story_id].append(dep)

    # Detect cycles using DFS
    visited: set[str] = set()
    rec_stack: set[str] = set()

    def has_cycle(node: str) -> bool:
        visited.add(node)
        rec_stack.add(node)

        for neighbor in graph.get(node, []):
            if neighbor not in visited:
                if has_cycle(neighbor):
                    return True
            elif neighbor in rec_stack:
                return True

        rec_stack.remove(node)
        return False

    for story_id in graph:
        if story_id not in visited:
            if has_cycle(story_id):
                result.add_error(f"Circular dependency detected involving story: {story_id}")
                break

    if not result.errors:
        result.details["message"] = "All dependencies are valid and form a DAG"

    return result


def check_verification_hints(stories: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that stories have verification hints for AI verification.

    Args:
        stories: List of story dictionaries from PRD

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="verification_hints")

    for story in stories:
        story_id = story.get("id", "unknown")
        verification = story.get("verification_hints", story.get("verification", {}))

        if not verification:
            result.add_warning(f"Story {story_id} has no verification hints")
            continue

        # Check for key verification elements
        if isinstance(verification, dict):
            if not verification.get("test_commands") and not verification.get("checks"):
                result.add_suggestion(
                    f"Story {story_id}: Consider adding test_commands or checks to verification"
                )

    return result


def check_risk_tags(stories: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that high-complexity stories have explicit risk tags.

    Args:
        stories: List of story dictionaries from PRD

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="risk_tags")

    high_risk_keywords = ["database", "migration", "auth", "security", "payment", "delete", "drop"]

    for story in stories:
        story_id = story.get("id", "unknown")
        title = story.get("title", "").lower()
        description = story.get("description", "").lower()
        risks = story.get("risks", [])

        # Check if story mentions high-risk keywords
        combined_text = f"{title} {description}"
        detected_risks = [kw for kw in high_risk_keywords if kw in combined_text]

        if detected_risks and not risks:
            result.add_warning(
                f"Story {story_id} mentions {detected_risks} but has no explicit risk tags"
            )
            result.add_suggestion(
                f"Story {story_id}: Consider adding risk assessment for: {', '.join(detected_risks)}"
            )

    return result



# =============================================================================
# Mega DoR Checks
# =============================================================================

def check_feature_count(features: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that feature count is within reasonable range (2-6).

    Args:
        features: List of feature dictionaries from mega-plan

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="feature_count")

    count = len(features)
    result.details["feature_count"] = count

    if count < 2:
        result.add_warning(
            f"Only {count} feature(s) found. Consider if mega-plan is needed or add more features."
        )
        result.add_suggestion("For single-feature projects, consider using hybrid-auto instead")
    elif count > 6:
        result.add_warning(
            f"Found {count} features. Consider grouping related features to reduce complexity."
        )
        result.add_suggestion("Large mega-plans may be harder to manage and have longer feedback loops")
    else:
        result.details["message"] = f"Feature count ({count}) is within optimal range (2-6)"

    return result


def check_dependency_dag(features: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that feature dependencies form a valid DAG and are batchable.

    Args:
        features: List of feature dictionaries from mega-plan

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="dependency_dag")

    feature_ids = {f.get("id") or f.get("name") for f in features}

    # Build adjacency list
    graph: dict[str, list[str]] = {}
    for f in features:
        fid = f.get("id") or f.get("name", "")
        graph[fid] = []

    for feature in features:
        fid = feature.get("id") or feature.get("name", "")
        deps = feature.get("dependencies", [])

        for dep in deps:
            if dep not in feature_ids:
                result.add_error(f"Feature {fid} depends on unknown feature: {dep}")
            else:
                graph[fid].append(dep)

    # Detect cycles
    visited: set[str] = set()
    rec_stack: set[str] = set()

    def has_cycle(node: str) -> bool:
        visited.add(node)
        rec_stack.add(node)

        for neighbor in graph.get(node, []):
            if neighbor not in visited:
                if has_cycle(neighbor):
                    return True
            elif neighbor in rec_stack:
                return True

        rec_stack.remove(node)
        return False

    for fid in graph:
        if fid not in visited:
            if has_cycle(fid):
                result.add_error(f"Circular dependency detected involving feature: {fid}")
                break

    # Check batchability - at least one feature with no dependencies
    features_no_deps = [f for f in features if not f.get("dependencies")]
    if not features_no_deps and features:
        result.add_error("No features without dependencies - cannot start execution")
    else:
        result.details["batch_1_count"] = len(features_no_deps)

    if not result.errors:
        result.details["message"] = "Dependencies form a valid batchable DAG"

    return result


def check_feature_boundaries(features: list[dict[str, Any]]) -> ReadinessCheckResult:
    """
    Check that features have clear boundaries and descriptions.

    Args:
        features: List of feature dictionaries from mega-plan

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="feature_boundaries")

    for feature in features:
        fid = feature.get("id") or feature.get("name", "unknown")
        description = feature.get("description", "")

        if not description:
            result.add_error(f"Feature {fid} has no description")
            continue

        if len(description) < 20:
            result.add_warning(
                f"Feature {fid} description is very short ({len(description)} chars)"
            )
            result.add_suggestion(
                f"Feature {fid}: Provide more detail for accurate PRD generation"
            )

        # Check for overlapping keywords that might indicate unclear boundaries
        if "and" in description.lower() and description.lower().count("and") > 2:
            result.add_warning(
                f"Feature {fid} description has many 'and' conjunctions - may need splitting"
            )

    return result



# =============================================================================
# Direct DoR Checks
# =============================================================================

def check_blast_radius(task_description: str) -> ReadinessCheckResult:
    """
    Check blast radius indicators in task description.

    Args:
        task_description: Description of the direct task

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="blast_radius")

    high_blast_keywords = [
        "all files", "entire", "whole codebase", "every", "global",
        "refactor all", "rename all", "migrate all", "delete all"
    ]

    desc_lower = task_description.lower()
    detected = [kw for kw in high_blast_keywords if kw in desc_lower]

    if detected:
        result.add_warning(
            f"High blast radius detected: {detected}. Consider breaking into smaller tasks."
        )
        result.details["blast_indicators"] = detected
    else:
        result.details["message"] = "Blast radius appears contained"

    return result


def check_rollback_hint(task_description: str) -> ReadinessCheckResult:
    """
    Check if task has rollback considerations.

    Args:
        task_description: Description of the direct task

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="rollback_hint")

    # Destructive operations that need rollback planning
    destructive_keywords = ["delete", "remove", "drop", "truncate", "reset", "clear"]

    desc_lower = task_description.lower()
    detected = [kw for kw in destructive_keywords if kw in desc_lower]

    if detected:
        result.add_suggestion(
            f"Task involves destructive operations ({detected}). Consider adding rollback plan."
        )
        result.details["destructive_operations"] = detected

    return result


def check_test_requirements(task_description: str, has_tests: bool = False) -> ReadinessCheckResult:
    """
    Check if task should have test requirements.

    Args:
        task_description: Description of the direct task
        has_tests: Whether tests are included in the task

    Returns:
        ReadinessCheckResult with findings
    """
    result = ReadinessCheckResult(check_name="test_requirements")

    # Keywords suggesting tests are important
    test_worthy_keywords = ["implement", "create", "add", "new feature", "api", "endpoint"]

    desc_lower = task_description.lower()
    is_test_worthy = any(kw in desc_lower for kw in test_worthy_keywords)

    if is_test_worthy and not has_tests:
        result.add_suggestion(
            "Task appears to add new functionality. Consider including tests."
        )

    return result



# =============================================================================
# ReadinessGate Class
# =============================================================================

class ReadinessGate:
    """
    Main DoR gate that orchestrates checks based on execution type.

    Supports SOFT mode (warnings only) and HARD mode (blocking).
    ADR-003: Gates start soft, become hard in Full Flow.
    """

    def __init__(self, mode: GateMode = GateMode.SOFT):
        """
        Initialize the readiness gate.

        Args:
            mode: Gate enforcement mode (SOFT or HARD)
        """
        self.mode = mode

    def check_prd(self, prd: dict[str, Any]) -> ReadinessCheckResult:
        """
        Run all PRD DoR checks.

        Args:
            prd: PRD dictionary with stories

        Returns:
            Combined ReadinessCheckResult
        """
        stories = prd.get("stories", [])

        results = [
            check_acceptance_criteria(stories),
            check_dependencies(stories),
            check_verification_hints(stories),
            check_risk_tags(stories),
        ]

        combined = ReadinessCheckResult.combine(results)
        combined.check_name = "prd_readiness"

        # In SOFT mode, convert errors to warnings
        if self.mode == GateMode.SOFT and combined.errors:
            combined.warnings.extend([f"[Soft] {e}" for e in combined.errors])
            combined.errors = []
            combined.passed = True

        return combined

    def check_mega(self, mega_plan: dict[str, Any]) -> ReadinessCheckResult:
        """
        Run all Mega DoR checks.

        Args:
            mega_plan: Mega plan dictionary with features

        Returns:
            Combined ReadinessCheckResult
        """
        features = mega_plan.get("features", [])

        results = [
            check_feature_count(features),
            check_dependency_dag(features),
            check_feature_boundaries(features),
        ]

        combined = ReadinessCheckResult.combine(results)
        combined.check_name = "mega_readiness"

        # In SOFT mode, convert errors to warnings
        if self.mode == GateMode.SOFT and combined.errors:
            combined.warnings.extend([f"[Soft] {e}" for e in combined.errors])
            combined.errors = []
            combined.passed = True

        return combined

    def check_direct(
        self,
        task_description: str,
        has_tests: bool = False
    ) -> ReadinessCheckResult:
        """
        Run all Direct DoR checks.

        Args:
            task_description: Description of the direct task
            has_tests: Whether tests are included

        Returns:
            Combined ReadinessCheckResult
        """
        results = [
            check_blast_radius(task_description),
            check_rollback_hint(task_description),
            check_test_requirements(task_description, has_tests),
        ]

        combined = ReadinessCheckResult.combine(results)
        combined.check_name = "direct_readiness"

        # In SOFT mode, convert errors to warnings
        if self.mode == GateMode.SOFT and combined.errors:
            combined.warnings.extend([f"[Soft] {e}" for e in combined.errors])
            combined.errors = []
            combined.passed = True

        return combined

    @classmethod
    def from_flow(cls, flow_name: str) -> "ReadinessGate":
        """
        Create a ReadinessGate with mode based on ExecutionFlow.

        Args:
            flow_name: Name of the execution flow ("quick", "standard", "full")

        Returns:
            ReadinessGate with appropriate mode
        """
        # Full flow uses hard mode, others use soft
        mode = GateMode.HARD if flow_name.lower() == "full" else GateMode.SOFT
        return cls(mode=mode)
