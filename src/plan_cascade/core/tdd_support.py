"""
TDD (Test-Driven Development) Support for Plan Cascade.

Provides optional story-level TDD rhythm (red/green/refactor) with configurable
modes and quality gate integration. Supports three modes:
- OFF: No TDD enforcement
- ON: TDD prompts and compliance checking enabled
- AUTO: Automatically enable TDD based on risk assessment

ADR: TDD mode provides guidance through prompts + gate checking, not enforcement.
First iteration focuses on prompt-level guidance with gate-level compliance checks.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class TDDMode(Enum):
    """TDD enforcement mode."""
    OFF = "off"     # TDD disabled
    ON = "on"       # TDD enabled with prompts and compliance checks
    AUTO = "auto"   # Automatically decide based on risk assessment


@dataclass
class TDDTestRequirements:
    """
    Test requirements configuration for TDD compliance.

    Attributes:
        require_test_changes: Whether test file changes are required
        minimum_coverage_delta: Minimum coverage increase expected (0 = no requirement)
        test_patterns: File patterns to identify test files
    """
    require_test_changes: bool = True
    minimum_coverage_delta: float = 0.0
    test_patterns: list[str] = field(default_factory=lambda: [
        "test_", "_test.", ".test.", "tests/", "test/", "spec/",
    ])

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "require_test_changes": self.require_test_changes,
            "minimum_coverage_delta": self.minimum_coverage_delta,
            "test_patterns": self.test_patterns,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TDDTestRequirements":
        """Create from dictionary."""
        return cls(
            require_test_changes=data.get("require_test_changes", True),
            minimum_coverage_delta=data.get("minimum_coverage_delta", 0.0),
            test_patterns=data.get("test_patterns", [
                "test_", "_test.", ".test.", "tests/", "test/", "spec/",
            ]),
        )


@dataclass
class TDDConfig:
    """
    TDD mode configuration.

    Attributes:
        mode: TDD enforcement mode (OFF, ON, AUTO)
        enforce_for_high_risk: In AUTO mode, always enable TDD for high-risk stories
        test_requirements: Test file and coverage requirements
    """
    mode: TDDMode = TDDMode.OFF
    enforce_for_high_risk: bool = True
    test_requirements: TDDTestRequirements = field(default_factory=TDDTestRequirements)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "mode": self.mode.value,
            "enforce_for_high_risk": self.enforce_for_high_risk,
            "test_requirements": self.test_requirements.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TDDConfig":
        """Create from dictionary."""
        mode_str = data.get("mode", "off")
        try:
            mode = TDDMode(mode_str)
        except ValueError:
            mode = TDDMode.OFF

        test_req_data = data.get("test_requirements", {})
        test_requirements = TDDTestRequirements.from_dict(test_req_data)

        return cls(
            mode=mode,
            enforce_for_high_risk=data.get("enforce_for_high_risk", True),
            test_requirements=test_requirements,
        )


@dataclass
class TDDStepGuide:
    """
    TDD step-by-step guidance for red/green/refactor workflow.

    Attributes:
        step1_red: Guidance for writing failing tests first
        step2_green: Guidance for minimal implementation to pass tests
        step3_refactor: Guidance for refactoring while keeping tests green
        context_notes: Additional context-specific notes
    """
    step1_red: str = ""
    step2_green: str = ""
    step3_refactor: str = ""
    context_notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "step1_red": self.step1_red,
            "step2_green": self.step2_green,
            "step3_refactor": self.step3_refactor,
            "context_notes": self.context_notes,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TDDStepGuide":
        """Create from dictionary."""
        return cls(
            step1_red=data.get("step1_red", ""),
            step2_green=data.get("step2_green", ""),
            step3_refactor=data.get("step3_refactor", ""),
            context_notes=data.get("context_notes", ""),
        )

    def to_prompt(self) -> str:
        """Convert to formatted prompt text for AI agents."""
        return f"""## TDD Workflow Guide

### Step 1: RED - Write Failing Tests First
{self.step1_red}

### Step 2: GREEN - Minimal Implementation
{self.step2_green}

### Step 3: REFACTOR - Improve While Green
{self.step3_refactor}

{f"**Note:** {self.context_notes}" if self.context_notes else ""}"""


@dataclass
class TDDCheckResult:
    """
    Result of TDD compliance check.

    Attributes:
        passed: Whether the TDD compliance check passed
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
    check_name: str = "tdd_compliance"
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
    def from_dict(cls, data: dict[str, Any]) -> "TDDCheckResult":
        """Create from dictionary."""
        return cls(
            passed=data.get("passed", True),
            warnings=data.get("warnings", []),
            errors=data.get("errors", []),
            suggestions=data.get("suggestions", []),
            check_name=data.get("check_name", "tdd_compliance"),
            details=data.get("details", {}),
        )

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


@dataclass
class StoryTestExpectations:
    """
    Test expectations for a story.

    Attributes:
        required: Whether tests are required for this story
        test_types: Types of tests expected (unit, integration, e2e)
        coverage_areas: Areas that should be covered by tests
        min_tests: Minimum number of test cases expected
    """
    required: bool = False
    test_types: list[str] = field(default_factory=list)
    coverage_areas: list[str] = field(default_factory=list)
    min_tests: int = 0

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "required": self.required,
            "test_types": self.test_types,
            "coverage_areas": self.coverage_areas,
            "min_tests": self.min_tests,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StoryTestExpectations":
        """Create from dictionary."""
        return cls(
            required=data.get("required", False),
            test_types=data.get("test_types", []),
            coverage_areas=data.get("coverage_areas", []),
            min_tests=data.get("min_tests", 0),
        )


# =============================================================================
# TDD Prompt Template Generation
# =============================================================================

def get_tdd_prompt_template(
    story: dict[str, Any] | None = None,
    config: TDDConfig | None = None,
) -> TDDStepGuide:
    """
    Generate TDD step-by-step guidance for AI agents.

    Creates a TDDStepGuide with complete instructions for the red/green/refactor
    workflow, optionally customized based on story context.

    Args:
        story: Optional story dictionary for context-specific guidance
        config: Optional TDD configuration

    Returns:
        TDDStepGuide with step-by-step TDD instructions
    """
    # Base guidance for each step
    step1_red = """1. **Understand the requirement**: Read the acceptance criteria carefully.
2. **Write a minimal failing test**: Create a test that verifies ONE specific behavior.
3. **Run the test**: Confirm it fails for the right reason (not syntax/import errors).
4. **Keep it simple**: Test only what's specified - avoid over-engineering tests.

Example approach:
- For a function: Test the expected output for a simple input
- For an API: Test the expected response for a valid request
- For a component: Test the expected rendering or behavior"""

    step2_green = """1. **Write minimal code**: Only enough to make the failing test pass.
2. **Avoid optimization**: Focus on correctness, not performance.
3. **Don't add features**: Resist adding functionality not covered by tests.
4. **Run tests frequently**: Verify the test passes after each change.

Key principles:
- The simplest solution that passes the test is correct
- Hardcoding values is acceptable if only one test case exists
- Add more tests to drive out the hardcoded values"""

    step3_refactor = """1. **Keep tests green**: Run tests after each change to ensure they still pass.
2. **Remove duplication**: Extract common patterns and shared code.
3. **Improve naming**: Use clear, descriptive names for functions and variables.
4. **Simplify logic**: Break complex functions into smaller, focused ones.

What to refactor:
- Code duplication (DRY principle)
- Long functions (single responsibility)
- Magic numbers/strings (named constants)
- Complex conditionals (extract to functions)"""

    context_notes = ""

    # Add context-specific notes if story is provided
    if story:
        tags = story.get("tags", [])
        priority = story.get("priority", "medium")
        test_expectations = story.get("test_expectations", {})

        notes = []

        # Add notes based on tags
        high_risk_tags = ["security", "auth", "database", "payment", "migration"]
        detected_risks = [tag for tag in tags if tag in high_risk_tags]
        if detected_risks:
            notes.append(f"High-risk areas detected ({', '.join(detected_risks)}): "
                        "Ensure comprehensive test coverage for edge cases and error handling.")

        # Add notes based on priority
        if priority == "high":
            notes.append("High priority story: Consider adding both positive and negative test cases.")

        # Add notes based on test expectations
        if test_expectations:
            test_types = test_expectations.get("test_types", [])
            if "integration" in test_types:
                notes.append("Integration tests expected: Test component interactions and data flow.")
            if "e2e" in test_types:
                notes.append("E2E tests expected: Test complete user flows through the system.")

        if notes:
            context_notes = " ".join(notes)

    return TDDStepGuide(
        step1_red=step1_red,
        step2_green=step2_green,
        step3_refactor=step3_refactor,
        context_notes=context_notes,
    )


# =============================================================================
# TDD Auto Mode Logic
# =============================================================================

# Keywords that indicate high-risk areas requiring TDD
HIGH_RISK_KEYWORDS = [
    "security", "auth", "authentication", "authorization",
    "database", "migration", "schema", "payment", "billing",
    "delete", "remove", "drop", "sensitive", "credential",
    "encrypt", "decrypt", "hash", "token", "session",
    "api", "endpoint", "integration", "critical",
]

# Tags that indicate high-risk stories
HIGH_RISK_TAGS = [
    "security", "auth", "database", "payment", "migration",
    "critical", "breaking-change", "data-model",
]


def should_enable_tdd(
    story: dict[str, Any],
    config: TDDConfig | None = None,
) -> bool:
    """
    Determine whether TDD should be enabled for a story in AUTO mode.

    Analyzes story context including title, description, tags, and risk level
    to automatically decide whether TDD should be enabled.

    Args:
        story: Story dictionary from PRD
        config: Optional TDD configuration

    Returns:
        True if TDD should be enabled for this story
    """
    if config is None:
        config = TDDConfig(mode=TDDMode.AUTO)

    # If TDD mode is OFF, never enable
    if config.mode == TDDMode.OFF:
        return False

    # If TDD mode is ON, always enable
    if config.mode == TDDMode.ON:
        return True

    # AUTO mode - analyze story context
    if config.mode != TDDMode.AUTO:
        return False

    # Check test_expectations field
    test_expectations = story.get("test_expectations", {})
    if isinstance(test_expectations, dict) and test_expectations.get("required"):
        return True

    # Check for high-risk tags
    tags = story.get("tags", [])
    if isinstance(tags, list):
        for tag in tags:
            if isinstance(tag, str) and tag.lower() in HIGH_RISK_TAGS:
                if config.enforce_for_high_risk:
                    return True

    # Check title and description for high-risk keywords
    title = story.get("title", "")
    description = story.get("description", "")
    combined_text = f"{title} {description}".lower()

    for keyword in HIGH_RISK_KEYWORDS:
        if keyword in combined_text:
            if config.enforce_for_high_risk:
                return True

    # Check priority - high priority stories may benefit from TDD
    priority = story.get("priority", "medium")
    if priority == "high":
        # High priority alone doesn't trigger TDD, but combined with risk does
        pass

    # Check context_estimate - large stories benefit from TDD
    context_estimate = story.get("context_estimate", "medium")
    if context_estimate in ["large", "xlarge"]:
        return True

    # Default: don't enable TDD for low-risk stories
    return False


def get_tdd_recommendation(
    story: dict[str, Any],
    config: TDDConfig | None = None,
) -> str:
    """
    Get a TDD recommendation string for strategy analysis output.

    Args:
        story: Story dictionary from PRD
        config: Optional TDD configuration

    Returns:
        Recommendation string: "off", "on", or "auto" with reasoning
    """
    if config is None:
        config = TDDConfig(mode=TDDMode.AUTO)

    if config.mode == TDDMode.OFF:
        return "off (TDD disabled in configuration)"

    if config.mode == TDDMode.ON:
        return "on (TDD enabled in configuration)"

    # AUTO mode
    should_tdd = should_enable_tdd(story, config)
    if should_tdd:
        # Determine why
        reasons = []

        test_expectations = story.get("test_expectations", {})
        if isinstance(test_expectations, dict) and test_expectations.get("required"):
            reasons.append("test_expectations.required=true")

        tags = story.get("tags", [])
        high_risk_tags = [t for t in tags if isinstance(t, str) and t.lower() in HIGH_RISK_TAGS]
        if high_risk_tags:
            reasons.append(f"high-risk tags: {', '.join(high_risk_tags)}")

        title = story.get("title", "")
        description = story.get("description", "")
        combined_text = f"{title} {description}".lower()
        keywords_found = [kw for kw in HIGH_RISK_KEYWORDS if kw in combined_text]
        if keywords_found and not high_risk_tags:
            reasons.append(f"high-risk keywords: {', '.join(keywords_found[:3])}")

        context_estimate = story.get("context_estimate", "medium")
        if context_estimate in ["large", "xlarge"]:
            reasons.append(f"context_estimate={context_estimate}")

        reason_str = "; ".join(reasons) if reasons else "risk factors detected"
        return f"on (auto-enabled: {reason_str})"

    return "auto (no risk factors detected, TDD optional)"


# =============================================================================
# TDD Compliance Checking
# =============================================================================

def is_test_file(filepath: str, patterns: list[str] | None = None) -> bool:
    """
    Check if a file path represents a test file.

    Args:
        filepath: Path to check
        patterns: Optional custom patterns (uses defaults if None)

    Returns:
        True if the file appears to be a test file
    """
    if patterns is None:
        patterns = ["test_", "_test.", ".test.", "tests/", "test/", "spec/"]

    filepath_lower = filepath.lower()
    return any(pattern in filepath_lower for pattern in patterns)


def check_tdd_compliance(
    story: dict[str, Any],
    changed_files: list[str] | None = None,
    config: TDDConfig | None = None,
    gate_outputs: dict[str, Any] | None = None,
) -> TDDCheckResult:
    """
    Check TDD compliance for a story's implementation.

    Verifies that stories have appropriate test changes when TDD mode is enabled.
    Integrates with the quality gate system pattern.

    Args:
        story: Story dictionary from PRD
        changed_files: List of files changed during story implementation
        config: Optional TDD configuration
        gate_outputs: Optional existing gate outputs for context

    Returns:
        TDDCheckResult with compliance findings
    """
    if config is None:
        config = TDDConfig(mode=TDDMode.AUTO)

    result = TDDCheckResult(check_name="tdd_compliance")
    changed_files = changed_files or []

    # Get story info for result details
    story_id = story.get("id", "unknown")
    result.details["story_id"] = story_id
    result.details["tdd_mode"] = config.mode.value

    # Check if TDD should be enabled for this story
    tdd_enabled = should_enable_tdd(story, config)
    result.details["tdd_enabled"] = tdd_enabled

    if not tdd_enabled:
        result.details["message"] = "TDD not required for this story"
        return result

    # TDD is enabled - check for test changes
    test_patterns = config.test_requirements.test_patterns
    test_files = [f for f in changed_files if is_test_file(f, test_patterns)]
    code_files = [f for f in changed_files if not is_test_file(f, test_patterns)]

    result.details["test_files_changed"] = len(test_files)
    result.details["code_files_changed"] = len(code_files)
    result.details["test_files"] = test_files
    result.details["code_files"] = code_files

    # Check test requirements
    if config.test_requirements.require_test_changes:
        if code_files and not test_files:
            # Check if this is a high-risk story
            test_expectations = story.get("test_expectations", {})
            is_required = isinstance(test_expectations, dict) and test_expectations.get("required", False)

            tags = story.get("tags", [])
            is_high_risk = any(
                isinstance(t, str) and t.lower() in HIGH_RISK_TAGS
                for t in tags
            )

            if is_required or (is_high_risk and config.enforce_for_high_risk):
                result.add_error(
                    f"Story {story_id}: Code changes detected but no test files modified. "
                    "TDD requires writing tests before or alongside implementation."
                )
            else:
                result.add_warning(
                    f"Story {story_id}: Code changes without corresponding test changes. "
                    "Consider adding tests for better coverage."
                )
            result.add_suggestion(
                "Follow TDD workflow: 1) Write failing test, 2) Implement to pass, 3) Refactor"
            )

    # Check test expectations if specified
    test_expectations = story.get("test_expectations", {})
    if isinstance(test_expectations, dict):
        min_tests = test_expectations.get("min_tests", 0)
        if min_tests > 0 and len(test_files) < min_tests:
            result.add_warning(
                f"Story {story_id}: Expected at least {min_tests} test file(s), "
                f"but only {len(test_files)} changed."
            )

        coverage_areas = test_expectations.get("coverage_areas", [])
        if coverage_areas:
            result.add_suggestion(
                f"Ensure tests cover: {', '.join(coverage_areas)}"
            )

    # Set success message if passed
    if result.passed:
        if test_files:
            result.details["message"] = (
                f"TDD compliance check passed: {len(test_files)} test file(s) "
                f"changed alongside {len(code_files)} code file(s)"
            )
        else:
            result.details["message"] = "TDD compliance check passed (no code changes)"

    return result


# =============================================================================
# Helper Functions
# =============================================================================

def get_tdd_config_from_prd(prd: dict[str, Any]) -> TDDConfig:
    """
    Extract TDD configuration from PRD.

    Args:
        prd: PRD dictionary that may contain tdd_config section

    Returns:
        TDDConfig extracted from PRD or default configuration
    """
    tdd_config_data = prd.get("tdd_config", {})
    if tdd_config_data:
        return TDDConfig.from_dict(tdd_config_data)
    return TDDConfig()


def add_tdd_to_prd(prd: dict[str, Any], config: TDDConfig) -> dict[str, Any]:
    """
    Add TDD configuration to PRD.

    Args:
        prd: PRD dictionary to update
        config: TDD configuration to add

    Returns:
        Updated PRD dictionary
    """
    prd["tdd_config"] = config.to_dict()
    return prd
