"""
Definition of Done (DoD) Gate Implementation for Plan Cascade.

Provides validation gates that run after story/feature execution to ensure
all completion conditions are met before leaving the Execute stage. Supports two levels:
- STANDARD: Basic completion (quality gates, AI verification, change summary)
- FULL: Stricter completion (code review, test changes, deployment notes)

ADR: DoD gates validate completion conditions, complementing DoR (readiness) gates.
"""

from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any


class DoDLevel(Enum):
    """DoD enforcement level."""
    STANDARD = "standard"  # Basic completion checks
    FULL = "full"         # Full completion checks (stricter)


@dataclass
class DoDCheckResult:
    """
    Result of a DoD (Definition of Done) check.

    Attributes:
        passed: Whether all checks passed
        warnings: Non-blocking issues found
        errors: Blocking issues found
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
    def from_dict(cls, data: dict[str, Any]) -> "DoDCheckResult":
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
    def combine(cls, results: list["DoDCheckResult"]) -> "DoDCheckResult":
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
# Standard Flow DoD Checks
# =============================================================================

def check_quality_gates_passed(
    gate_outputs: dict[str, Any],
    required_gates: list[str] | None = None,
) -> DoDCheckResult:
    """
    Check that all required quality gates passed.

    Args:
        gate_outputs: Dictionary of gate name -> GateOutput (or dict with 'passed' key)
        required_gates: List of gate names that must pass (all if None)

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="quality_gates_passed")

    if not gate_outputs:
        result.add_warning("No quality gate results provided")
        return result

    # Determine which gates to check
    gates_to_check = required_gates or list(gate_outputs.keys())
    result.details["gates_checked"] = gates_to_check

    passed_gates = []
    failed_gates = []

    for gate_name in gates_to_check:
        output = gate_outputs.get(gate_name)
        if output is None:
            result.add_error(f"Required gate '{gate_name}' was not executed")
            continue

        # Handle both GateOutput objects and dicts
        gate_passed = output.get("passed") if isinstance(output, dict) else getattr(output, "passed", False)

        if gate_passed:
            passed_gates.append(gate_name)
        else:
            failed_gates.append(gate_name)
            # Get error summary if available
            error_summary = (
                output.get("error_summary")
                if isinstance(output, dict)
                else getattr(output, "error_summary", None)
            )
            if error_summary:
                result.add_error(f"Gate '{gate_name}' failed: {error_summary}")
            else:
                result.add_error(f"Gate '{gate_name}' failed")

    result.details["passed_gates"] = passed_gates
    result.details["failed_gates"] = failed_gates

    if not failed_gates:
        result.details["message"] = f"All {len(passed_gates)} quality gate(s) passed"

    return result


def check_ai_verification(
    verification_result: dict[str, Any] | None,
    confidence_threshold: float = 0.7,
) -> DoDCheckResult:
    """
    Check that AI verification passed (story implementation verified).

    Args:
        verification_result: VerificationResult dict or object
        confidence_threshold: Minimum confidence required

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="ai_verification")

    if verification_result is None:
        result.add_warning("No AI verification result provided")
        result.add_suggestion("Run AI verification to validate implementation completeness")
        return result

    # Handle both objects and dicts
    if isinstance(verification_result, dict):
        overall_passed = verification_result.get("overall_passed", False)
        confidence = verification_result.get("confidence", 0.0)
        skeleton_detected = verification_result.get("skeleton_detected", False)
        skeleton_evidence = verification_result.get("skeleton_evidence")
        missing = verification_result.get("missing_implementations", [])
    else:
        overall_passed = getattr(verification_result, "overall_passed", False)
        confidence = getattr(verification_result, "confidence", 0.0)
        skeleton_detected = getattr(verification_result, "skeleton_detected", False)
        skeleton_evidence = getattr(verification_result, "skeleton_evidence", None)
        missing = getattr(verification_result, "missing_implementations", [])

    result.details["overall_passed"] = overall_passed
    result.details["confidence"] = confidence
    result.details["skeleton_detected"] = skeleton_detected

    if skeleton_detected:
        result.add_error(f"Skeleton code detected: {skeleton_evidence or 'incomplete implementation'}")

    if not overall_passed:
        result.add_error("AI verification did not pass")

    if confidence < confidence_threshold:
        result.add_warning(
            f"Verification confidence ({confidence:.2f}) below threshold ({confidence_threshold})"
        )

    if missing:
        for item in missing[:3]:  # Show first 3
            result.add_error(f"Missing implementation: {item}")
        if len(missing) > 3:
            result.details["additional_missing"] = len(missing) - 3

    if result.passed:
        result.details["message"] = f"AI verification passed with confidence {confidence:.2f}"

    return result


def check_skeleton_code_detection(
    verification_result: dict[str, Any] | None,
) -> DoDCheckResult:
    """
    Specifically check for skeleton code in the implementation.

    Args:
        verification_result: VerificationResult dict or object

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="skeleton_code_detection")

    if verification_result is None:
        result.add_warning("No verification result to check for skeleton code")
        return result

    # Handle both objects and dicts
    if isinstance(verification_result, dict):
        skeleton_detected = verification_result.get("skeleton_detected", False)
        skeleton_evidence = verification_result.get("skeleton_evidence")
    else:
        skeleton_detected = getattr(verification_result, "skeleton_detected", False)
        skeleton_evidence = getattr(verification_result, "skeleton_evidence", None)

    if skeleton_detected:
        result.add_error(f"Skeleton code detected: {skeleton_evidence or 'incomplete implementation'}")
        result.add_suggestion(
            "Replace placeholder code (pass, ..., NotImplementedError) with actual implementation"
        )
    else:
        result.details["message"] = "No skeleton code detected"

    return result


def check_change_summary(
    changed_files: list[str] | None = None,
    interfaces_changed: list[str] | None = None,
    summary_text: str | None = None,
) -> DoDCheckResult:
    """
    Check that minimum change documentation exists.

    Args:
        changed_files: List of files that were modified
        interfaces_changed: List of interfaces/APIs that were changed
        summary_text: Optional summary text of changes

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="change_summary")

    files_count = len(changed_files or [])
    interfaces_count = len(interfaces_changed or [])

    result.details["files_changed"] = files_count
    result.details["interfaces_changed"] = interfaces_count

    if files_count == 0:
        result.add_warning("No changed files recorded")
        result.add_suggestion("Document which files were modified")

    if interfaces_count > 0 and not summary_text:
        result.add_warning(
            f"{interfaces_count} interface(s) changed but no summary provided"
        )
        result.add_suggestion("Add a summary describing the interface changes")

    if summary_text:
        result.details["has_summary"] = True
        if len(summary_text) < 20:
            result.add_warning("Change summary is very brief")
    else:
        result.details["has_summary"] = False

    if files_count > 0:
        result.details["message"] = f"{files_count} file(s) changed"

    return result


# =============================================================================
# Full Flow DoD Checks (Additional)
# =============================================================================

def check_code_review(
    review_result: dict[str, Any] | None,
    min_score: float = 0.7,
    block_on_critical: bool = True,
) -> DoDCheckResult:
    """
    Check that code review was completed and passed.

    Args:
        review_result: CodeReviewResult dict or object
        min_score: Minimum score required to pass
        block_on_critical: Whether critical findings block completion

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="code_review")

    if review_result is None:
        result.add_error("Code review is required but was not completed")
        result.add_suggestion("Run code review with: /plan-cascade:approve --review")
        return result

    # Handle both objects and dicts
    if isinstance(review_result, dict):
        overall_score = review_result.get("overall_score", 0.0)
        passed = review_result.get("passed", False)
        has_critical = review_result.get("has_critical", False)
        findings = review_result.get("findings", [])
    else:
        overall_score = getattr(review_result, "overall_score", 0.0)
        passed = getattr(review_result, "passed", False)
        has_critical = getattr(review_result, "has_critical", False)
        findings = getattr(review_result, "findings", [])

    result.details["overall_score"] = overall_score
    result.details["finding_count"] = len(findings)

    if overall_score < min_score:
        result.add_error(
            f"Code review score ({overall_score:.2f}) below minimum ({min_score})"
        )

    if block_on_critical and has_critical:
        critical_count = sum(
            1 for f in findings
            if (f.get("severity") if isinstance(f, dict) else getattr(f, "severity", None)) == "critical"
        )
        result.add_error(f"{critical_count} critical finding(s) must be addressed")

    if not passed:
        result.add_error("Code review did not pass")

    # Add high-severity findings as warnings
    high_findings = [
        f for f in findings
        if (f.get("severity") if isinstance(f, dict) else getattr(f, "severity", None)) == "high"
    ]
    for finding in high_findings[:3]:
        title = finding.get("title") if isinstance(finding, dict) else getattr(finding, "title", "Unknown")
        result.add_warning(f"High severity: {title}")

    if result.passed:
        result.details["message"] = f"Code review passed with score {overall_score:.2f}"

    return result


def check_test_changes(
    changed_files: list[str] | None = None,
    test_files_changed: list[str] | None = None,
    code_files_changed: list[str] | None = None,
) -> DoDCheckResult:
    """
    Check that related test changes were made for code changes.

    Args:
        changed_files: All changed files
        test_files_changed: Test files that were modified
        code_files_changed: Code files that were modified

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="test_changes")

    # If specific lists not provided, try to categorize from changed_files
    if changed_files and (test_files_changed is None or code_files_changed is None):
        test_patterns = ["test_", "_test.", ".test.", "tests/", "test/", "spec/"]
        test_files = []
        code_files = []

        for f in changed_files:
            f_lower = f.lower()
            if any(p in f_lower for p in test_patterns):
                test_files.append(f)
            elif f.endswith((".py", ".ts", ".tsx", ".js", ".jsx", ".rs", ".go")):
                code_files.append(f)

        test_files_changed = test_files_changed or test_files
        code_files_changed = code_files_changed or code_files

    test_count = len(test_files_changed or [])
    code_count = len(code_files_changed or [])

    result.details["test_files_changed"] = test_count
    result.details["code_files_changed"] = code_count

    if code_count > 0 and test_count == 0:
        result.add_error(
            f"{code_count} code file(s) changed but no test files modified"
        )
        result.add_suggestion("Add or update tests for the changed code")
    elif code_count > 0 and test_count < code_count / 2:
        result.add_warning(
            f"Limited test coverage: {test_count} test file(s) for {code_count} code file(s)"
        )
        result.add_suggestion("Consider adding more tests")
    elif test_count > 0:
        result.details["message"] = f"{test_count} test file(s) updated alongside {code_count} code file(s)"

    return result


def check_deployment_notes(
    changed_files: list[str] | None = None,
    has_deployment_notes: bool = False,
    has_rollback_plan: bool = False,
) -> DoDCheckResult:
    """
    Check for deployment/rollback notes when config or migrations are changed.

    Args:
        changed_files: List of changed files
        has_deployment_notes: Whether deployment notes exist
        has_rollback_plan: Whether rollback plan exists

    Returns:
        DoDCheckResult with findings
    """
    result = DoDCheckResult(check_name="deployment_notes")

    # Patterns that indicate deployment-sensitive changes
    deployment_patterns = [
        "migration", "migrate", "config", "settings",
        ".env", "docker", "kubernetes", "k8s", "helm",
        "terraform", "ansible", "deploy", "infra",
        "schema", "database", "db_", "alembic",
    ]

    changed_files = changed_files or []
    deployment_files = []

    for f in changed_files:
        f_lower = f.lower()
        if any(p in f_lower for p in deployment_patterns):
            deployment_files.append(f)

    result.details["deployment_sensitive_files"] = len(deployment_files)

    if deployment_files:
        result.details["files"] = deployment_files[:5]  # Show first 5

        if not has_deployment_notes:
            result.add_error(
                f"{len(deployment_files)} deployment-sensitive file(s) changed but no deployment notes"
            )
            result.add_suggestion(
                "Add deployment notes describing required steps, environment changes, or migrations"
            )

        if not has_rollback_plan:
            result.add_warning("No rollback plan documented for deployment-sensitive changes")
            result.add_suggestion(
                "Document rollback steps in case deployment needs to be reverted"
            )
    else:
        result.details["message"] = "No deployment-sensitive files changed"

    return result


# =============================================================================
# WrapUp Summary
# =============================================================================

@dataclass
class GateSummaryEntry:
    """Summary entry for a single gate execution."""
    name: str
    passed: bool
    duration_seconds: float = 0.0
    error_summary: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "name": self.name,
            "passed": self.passed,
            "duration_seconds": self.duration_seconds,
            "error_summary": self.error_summary,
        }


@dataclass
class ChangeSummaryEntry:
    """Summary entry for changes made."""
    category: str  # e.g., "files", "interfaces", "tests"
    items: list[str]
    count: int

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "category": self.category,
            "items": self.items,
            "count": self.count,
        }


@dataclass
class WrapUpSummary:
    """
    Standardized wrap-up summary for completed execution.

    Attributes:
        story_id: ID of the completed story
        completed_at: Timestamp of completion
        dod_result: Combined DoD check result
        gate_summary: Summary of gates executed
        change_summary: Summary of changes made
        notes: Additional notes or context
    """
    story_id: str
    completed_at: str = ""
    dod_result: DoDCheckResult | None = None
    gate_summary: list[GateSummaryEntry] = field(default_factory=list)
    change_summary: list[ChangeSummaryEntry] = field(default_factory=list)
    notes: str = ""

    def __post_init__(self) -> None:
        """Set completed_at if not provided."""
        if not self.completed_at:
            self.completed_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "story_id": self.story_id,
            "completed_at": self.completed_at,
            "dod_result": self.dod_result.to_dict() if self.dod_result else None,
            "gate_summary": [g.to_dict() for g in self.gate_summary],
            "change_summary": [c.to_dict() for c in self.change_summary],
            "notes": self.notes,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "WrapUpSummary":
        """Create from dictionary."""
        dod_result = None
        if data.get("dod_result"):
            dod_result = DoDCheckResult.from_dict(data["dod_result"])

        gate_summary = []
        for g in data.get("gate_summary", []):
            gate_summary.append(GateSummaryEntry(
                name=g["name"],
                passed=g["passed"],
                duration_seconds=g.get("duration_seconds", 0.0),
                error_summary=g.get("error_summary"),
            ))

        change_summary = []
        for c in data.get("change_summary", []):
            change_summary.append(ChangeSummaryEntry(
                category=c["category"],
                items=c.get("items", []),
                count=c.get("count", 0),
            ))

        return cls(
            story_id=data.get("story_id", ""),
            completed_at=data.get("completed_at", ""),
            dod_result=dod_result,
            gate_summary=gate_summary,
            change_summary=change_summary,
            notes=data.get("notes", ""),
        )

    def to_markdown(self) -> str:
        """Generate human-readable markdown output."""
        lines = [
            f"# WrapUp Summary: {self.story_id}",
            "",
            f"**Completed at:** {self.completed_at}",
            "",
        ]

        # DoD Result
        if self.dod_result:
            status = "PASSED" if self.dod_result.passed else "FAILED"
            lines.extend([
                f"## Definition of Done: {status}",
                "",
            ])

            if self.dod_result.errors:
                lines.append("### Errors")
                for error in self.dod_result.errors:
                    lines.append(f"- {error}")
                lines.append("")

            if self.dod_result.warnings:
                lines.append("### Warnings")
                for warning in self.dod_result.warnings:
                    lines.append(f"- {warning}")
                lines.append("")

        # Gate Summary
        if self.gate_summary:
            lines.extend([
                "## Gate Summary",
                "",
                "| Gate | Status | Duration |",
                "|------|--------|----------|",
            ])
            for gate in self.gate_summary:
                status = "PASS" if gate.passed else "FAIL"
                duration = f"{gate.duration_seconds:.2f}s"
                lines.append(f"| {gate.name} | {status} | {duration} |")
            lines.append("")

        # Change Summary
        if self.change_summary:
            lines.extend([
                "## Change Summary",
                "",
            ])
            for change in self.change_summary:
                lines.append(f"### {change.category.title()} ({change.count})")
                for item in change.items[:10]:  # Show first 10
                    lines.append(f"- {item}")
                if change.count > 10:
                    lines.append(f"- ... and {change.count - 10} more")
                lines.append("")

        # Notes
        if self.notes:
            lines.extend([
                "## Notes",
                "",
                self.notes,
                "",
            ])

        return chr(10).join(lines)


def generate_wrapup_summary(
    story_id: str,
    dod_result: DoDCheckResult,
    gate_outputs: dict[str, Any] | None = None,
    changed_files: list[str] | None = None,
    interfaces_changed: list[str] | None = None,
    notes: str = "",
) -> WrapUpSummary:
    """
    Generate a standardized wrap-up summary.

    Args:
        story_id: ID of the completed story
        dod_result: Combined DoD check result
        gate_outputs: Dictionary of gate outputs
        changed_files: List of files changed
        interfaces_changed: List of interfaces changed
        notes: Additional notes

    Returns:
        WrapUpSummary with all information
    """
    # Build gate summary
    gate_summary = []
    if gate_outputs:
        for name, output in gate_outputs.items():
            if isinstance(output, dict):
                passed = output.get("passed", False)
                duration = output.get("duration_seconds", 0.0)
                error_summary = output.get("error_summary")
            else:
                passed = getattr(output, "passed", False)
                duration = getattr(output, "duration_seconds", 0.0)
                error_summary = getattr(output, "error_summary", None)

            gate_summary.append(GateSummaryEntry(
                name=name,
                passed=passed,
                duration_seconds=duration,
                error_summary=error_summary,
            ))

    # Build change summary
    change_summary = []

    if changed_files:
        # Categorize files
        test_files = []
        source_files = []
        config_files = []
        other_files = []

        test_patterns = ["test_", "_test.", ".test.", "tests/", "test/", "spec/"]
        config_patterns = ["config", ".env", ".json", ".yaml", ".yml", ".toml"]

        for f in changed_files:
            f_lower = f.lower()
            if any(p in f_lower for p in test_patterns):
                test_files.append(f)
            elif any(p in f_lower for p in config_patterns):
                config_files.append(f)
            elif f.endswith((".py", ".ts", ".tsx", ".js", ".jsx", ".rs", ".go")):
                source_files.append(f)
            else:
                other_files.append(f)

        if source_files:
            change_summary.append(ChangeSummaryEntry(
                category="source files",
                items=source_files,
                count=len(source_files),
            ))
        if test_files:
            change_summary.append(ChangeSummaryEntry(
                category="test files",
                items=test_files,
                count=len(test_files),
            ))
        if config_files:
            change_summary.append(ChangeSummaryEntry(
                category="config files",
                items=config_files,
                count=len(config_files),
            ))
        if other_files:
            change_summary.append(ChangeSummaryEntry(
                category="other files",
                items=other_files,
                count=len(other_files),
            ))

    if interfaces_changed:
        change_summary.append(ChangeSummaryEntry(
            category="interfaces",
            items=interfaces_changed,
            count=len(interfaces_changed),
        ))

    return WrapUpSummary(
        story_id=story_id,
        dod_result=dod_result,
        gate_summary=gate_summary,
        change_summary=change_summary,
        notes=notes,
    )


# =============================================================================
# DoneGate Class
# =============================================================================

class DoneGate:
    """
    Main DoD gate that orchestrates checks based on execution level.

    Supports STANDARD level (basic checks) and FULL level (all checks).
    Similar pattern to ReadinessGate for consistency.
    """

    def __init__(self, level: DoDLevel = DoDLevel.STANDARD):
        """
        Initialize the DoD gate.

        Args:
            level: DoD enforcement level (STANDARD or FULL)
        """
        self.level = level

    def check_standard(
        self,
        gate_outputs: dict[str, Any] | None = None,
        verification_result: dict[str, Any] | None = None,
        changed_files: list[str] | None = None,
        interfaces_changed: list[str] | None = None,
        summary_text: str | None = None,
        required_gates: list[str] | None = None,
        confidence_threshold: float = 0.7,
    ) -> DoDCheckResult:
        """
        Run Standard Flow DoD checks.

        Args:
            gate_outputs: Quality gate execution results
            verification_result: AI verification result
            changed_files: List of files changed
            interfaces_changed: List of interfaces changed
            summary_text: Summary text of changes
            required_gates: List of required gate names
            confidence_threshold: Minimum AI verification confidence

        Returns:
            Combined DoDCheckResult
        """
        results = [
            check_quality_gates_passed(gate_outputs or {}, required_gates),
            check_ai_verification(verification_result, confidence_threshold),
            check_skeleton_code_detection(verification_result),
            check_change_summary(changed_files, interfaces_changed, summary_text),
        ]

        combined = DoDCheckResult.combine(results)
        combined.check_name = "standard_dod"

        return combined

    def check_full(
        self,
        gate_outputs: dict[str, Any] | None = None,
        verification_result: dict[str, Any] | None = None,
        review_result: dict[str, Any] | None = None,
        changed_files: list[str] | None = None,
        interfaces_changed: list[str] | None = None,
        summary_text: str | None = None,
        required_gates: list[str] | None = None,
        confidence_threshold: float = 0.7,
        min_review_score: float = 0.7,
        has_deployment_notes: bool = False,
        has_rollback_plan: bool = False,
    ) -> DoDCheckResult:
        """
        Run Full Flow DoD checks (Standard + additional Full Flow checks).

        Args:
            gate_outputs: Quality gate execution results
            verification_result: AI verification result
            review_result: Code review result
            changed_files: List of files changed
            interfaces_changed: List of interfaces changed
            summary_text: Summary text of changes
            required_gates: List of required gate names
            confidence_threshold: Minimum AI verification confidence
            min_review_score: Minimum code review score
            has_deployment_notes: Whether deployment notes exist
            has_rollback_plan: Whether rollback plan exists

        Returns:
            Combined DoDCheckResult
        """
        # Start with standard checks
        results = [
            check_quality_gates_passed(gate_outputs or {}, required_gates),
            check_ai_verification(verification_result, confidence_threshold),
            check_skeleton_code_detection(verification_result),
            check_change_summary(changed_files, interfaces_changed, summary_text),
        ]

        # Add Full Flow checks
        results.extend([
            check_code_review(review_result, min_review_score),
            check_test_changes(changed_files),
            check_deployment_notes(changed_files, has_deployment_notes, has_rollback_plan),
        ])

        combined = DoDCheckResult.combine(results)
        combined.check_name = "full_dod"

        return combined

    def check(
        self,
        gate_outputs: dict[str, Any] | None = None,
        verification_result: dict[str, Any] | None = None,
        review_result: dict[str, Any] | None = None,
        changed_files: list[str] | None = None,
        interfaces_changed: list[str] | None = None,
        summary_text: str | None = None,
        required_gates: list[str] | None = None,
        confidence_threshold: float = 0.7,
        min_review_score: float = 0.7,
        has_deployment_notes: bool = False,
        has_rollback_plan: bool = False,
    ) -> DoDCheckResult:
        """
        Run DoD checks based on configured level.

        Args:
            (same as check_full)

        Returns:
            Combined DoDCheckResult for the configured level
        """
        if self.level == DoDLevel.FULL:
            return self.check_full(
                gate_outputs=gate_outputs,
                verification_result=verification_result,
                review_result=review_result,
                changed_files=changed_files,
                interfaces_changed=interfaces_changed,
                summary_text=summary_text,
                required_gates=required_gates,
                confidence_threshold=confidence_threshold,
                min_review_score=min_review_score,
                has_deployment_notes=has_deployment_notes,
                has_rollback_plan=has_rollback_plan,
            )
        else:
            return self.check_standard(
                gate_outputs=gate_outputs,
                verification_result=verification_result,
                changed_files=changed_files,
                interfaces_changed=interfaces_changed,
                summary_text=summary_text,
                required_gates=required_gates,
                confidence_threshold=confidence_threshold,
            )

    @classmethod
    def from_flow(cls, flow_name: str) -> "DoneGate":
        """
        Create a DoneGate with level based on ExecutionFlow.

        Args:
            flow_name: Name of the execution flow ("quick", "standard", "full")

        Returns:
            DoneGate with appropriate level
        """
        # Full flow uses FULL level, others use STANDARD
        level = DoDLevel.FULL if flow_name.lower() == "full" else DoDLevel.STANDARD
        return cls(level=level)
