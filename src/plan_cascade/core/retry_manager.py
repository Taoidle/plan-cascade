"""
Retry Manager for Plan Cascade

Tracks failures, manages retries, and injects failure context into retry prompts.
Provides intelligent retry behavior with exponential backoff and failure analysis.
"""

import json
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any


class ErrorType(Enum):
    """Types of errors that can trigger retries."""
    TIMEOUT = "timeout"
    EXIT_CODE = "exit_code"
    QUALITY_GATE = "quality_gate"
    PROCESS_CRASH = "process_crash"
    UNKNOWN = "unknown"


@dataclass
class FailureRecord:
    """Record of a single failure attempt."""
    story_id: str
    attempt: int
    agent: str
    error_type: ErrorType
    error_message: str
    timestamp: str
    quality_gate_results: dict[str, Any] | None = None
    exit_code: int | None = None
    output_excerpt: str | None = None
    suggested_fixes: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "story_id": self.story_id,
            "attempt": self.attempt,
            "agent": self.agent,
            "error_type": self.error_type.value,
            "error_message": self.error_message,
            "timestamp": self.timestamp,
            "quality_gate_results": self.quality_gate_results,
            "exit_code": self.exit_code,
            "output_excerpt": self.output_excerpt,
            "suggested_fixes": self.suggested_fixes,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "FailureRecord":
        """Create from dictionary."""
        return cls(
            story_id=data["story_id"],
            attempt=data["attempt"],
            agent=data["agent"],
            error_type=ErrorType(data["error_type"]),
            error_message=data["error_message"],
            timestamp=data["timestamp"],
            quality_gate_results=data.get("quality_gate_results"),
            exit_code=data.get("exit_code"),
            output_excerpt=data.get("output_excerpt"),
            suggested_fixes=data.get("suggested_fixes", []),
        )


@dataclass
class RetryConfig:
    """Configuration for retry behavior."""
    max_retries: int = 3
    exponential_backoff: bool = True
    base_delay_seconds: float = 5.0
    max_delay_seconds: float = 60.0
    inject_failure_context: bool = True
    switch_agent_on_retry: bool = False
    retry_agent_chain: list[str] = field(default_factory=lambda: ["claude-code", "aider"])

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "max_retries": self.max_retries,
            "exponential_backoff": self.exponential_backoff,
            "base_delay_seconds": self.base_delay_seconds,
            "max_delay_seconds": self.max_delay_seconds,
            "inject_failure_context": self.inject_failure_context,
            "switch_agent_on_retry": self.switch_agent_on_retry,
            "retry_agent_chain": self.retry_agent_chain,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RetryConfig":
        """Create from dictionary."""
        return cls(
            max_retries=data.get("max_retries", 3),
            exponential_backoff=data.get("exponential_backoff", True),
            base_delay_seconds=data.get("base_delay_seconds", 5.0),
            max_delay_seconds=data.get("max_delay_seconds", 60.0),
            inject_failure_context=data.get("inject_failure_context", True),
            switch_agent_on_retry=data.get("switch_agent_on_retry", False),
            retry_agent_chain=data.get("retry_agent_chain", ["claude-code", "aider"]),
        )


@dataclass
class RetryState:
    """State of retries for a story."""
    story_id: str
    current_attempt: int = 0
    failures: list[FailureRecord] = field(default_factory=list)
    last_agent: str | None = None
    exhausted: bool = False

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "story_id": self.story_id,
            "current_attempt": self.current_attempt,
            "failures": [f.to_dict() for f in self.failures],
            "last_agent": self.last_agent,
            "exhausted": self.exhausted,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RetryState":
        """Create from dictionary."""
        return cls(
            story_id=data["story_id"],
            current_attempt=data.get("current_attempt", 0),
            failures=[FailureRecord.from_dict(f) for f in data.get("failures", [])],
            last_agent=data.get("last_agent"),
            exhausted=data.get("exhausted", False),
        )


class RetryManager:
    """
    Manages retry logic for failed story executions.

    Features:
    - Track failure history per story
    - Exponential backoff between retries
    - Failure context injection into retry prompts
    - Agent switching on retry (optional)
    - Suggested fixes based on error analysis
    """

    # Patterns for analyzing errors and suggesting fixes
    ERROR_PATTERNS: dict[str, dict[str, Any]] = {
        "import_error": {
            "patterns": ["ImportError", "ModuleNotFoundError", "Cannot find module"],
            "fixes": [
                "Check that all required dependencies are installed",
                "Verify import paths are correct",
                "Run package manager install (npm install, pip install, etc.)",
            ],
        },
        "type_error": {
            "patterns": ["TypeError", "type error", "expected type"],
            "fixes": [
                "Review type annotations and ensure correct types are used",
                "Check for null/undefined values that need handling",
                "Verify function signatures match expected types",
            ],
        },
        "syntax_error": {
            "patterns": ["SyntaxError", "syntax error", "Unexpected token"],
            "fixes": [
                "Check for missing brackets, parentheses, or semicolons",
                "Verify proper string quoting",
                "Look for typos in keywords",
            ],
        },
        "test_failure": {
            "patterns": ["AssertionError", "test failed", "failing", "FAILED"],
            "fixes": [
                "Review failing test assertions",
                "Check if test expectations match implementation",
                "Look for edge cases not handled",
            ],
        },
        "timeout": {
            "patterns": ["timeout", "timed out", "deadline exceeded"],
            "fixes": [
                "Consider breaking the task into smaller parts",
                "Check for infinite loops or blocking operations",
                "Increase timeout if task legitimately needs more time",
            ],
        },
        "permission": {
            "patterns": ["Permission denied", "EACCES", "EPERM", "access denied"],
            "fixes": [
                "Check file permissions",
                "Verify user has necessary access rights",
                "Run with appropriate privileges if needed",
            ],
        },
        "verification_failure": {
            "patterns": [
                "Criterion not met",
                "skeleton detected",
                "SKELETON",
                "NotImplementedError",
                "pass  # TODO",
                "raise NotImplementedError",
                "missing implementation",
                "Low confidence",
            ],
            "fixes": [
                "Review acceptance criteria and ensure all are implemented",
                "Remove skeleton code (pass, NotImplementedError, TODO)",
                "Implement missing functionality from verification feedback",
                "Replace placeholder return values with actual logic",
            ],
        },
    }

    def __init__(
        self,
        project_root: Path,
        config: RetryConfig | None = None,
        state_file: Path | None = None,
    ):
        """
        Initialize retry manager.

        Args:
            project_root: Root directory of the project
            config: Retry configuration
            state_file: Path to state file (defaults to .retry-state.json)
        """
        self.project_root = Path(project_root)
        self.config = config or RetryConfig()
        self.state_file = state_file or (self.project_root / ".retry-state.json")
        self._states: dict[str, RetryState] = {}

        # Load existing state
        self._load_state()

    def record_failure(
        self,
        story_id: str,
        agent: str,
        error_type: ErrorType,
        error_message: str,
        quality_gate_results: dict[str, Any] | None = None,
        exit_code: int | None = None,
        output_excerpt: str | None = None,
    ) -> FailureRecord:
        """
        Record a failure for a story.

        Args:
            story_id: ID of the failed story
            agent: Agent that failed
            error_type: Type of error
            error_message: Error message
            quality_gate_results: Results from quality gates (if applicable)
            exit_code: Process exit code (if applicable)
            output_excerpt: Excerpt from output (if applicable)

        Returns:
            FailureRecord with suggested fixes
        """
        state = self._get_or_create_state(story_id)
        state.current_attempt += 1
        state.last_agent = agent

        # Analyze error and generate suggested fixes
        suggested_fixes = self._analyze_error(error_message, output_excerpt)

        # Add quality gate specific fixes
        if quality_gate_results:
            suggested_fixes.extend(self._analyze_quality_gate_failures(quality_gate_results))

        record = FailureRecord(
            story_id=story_id,
            attempt=state.current_attempt,
            agent=agent,
            error_type=error_type,
            error_message=error_message,
            timestamp=datetime.now().isoformat(),
            quality_gate_results=quality_gate_results,
            exit_code=exit_code,
            output_excerpt=output_excerpt[:1000] if output_excerpt else None,
            suggested_fixes=suggested_fixes,
        )

        state.failures.append(record)

        # Check if retries exhausted
        if state.current_attempt >= self.config.max_retries:
            state.exhausted = True

        self._save_state()
        return record

    def can_retry(self, story_id: str) -> bool:
        """Check if a story can be retried."""
        state = self._states.get(story_id)
        if not state:
            return True

        return not state.exhausted and state.current_attempt < self.config.max_retries

    def get_retry_count(self, story_id: str) -> int:
        """Get the current retry count for a story."""
        state = self._states.get(story_id)
        return state.current_attempt if state else 0

    def get_last_failure(self, story_id: str) -> FailureRecord | None:
        """Get the last failure record for a story."""
        state = self._states.get(story_id)
        if state and state.failures:
            return state.failures[-1]
        return None

    def build_retry_prompt(
        self,
        story: dict[str, Any],
        context: dict[str, Any],
        base_prompt: str,
    ) -> str:
        """
        Build a retry prompt with failure context injected.

        Args:
            story: Story dictionary from PRD
            context: Execution context
            base_prompt: Original prompt for the story

        Returns:
            Enhanced prompt with failure context
        """
        story_id = story.get("id", "unknown")
        state = self._states.get(story_id)

        if not state or not state.failures or not self.config.inject_failure_context:
            return base_prompt

        last_failure = state.failures[-1]

        # Build failure context section
        failure_context = self._build_failure_context(state, last_failure)

        # Prepend failure context to base prompt
        return f"{failure_context}\n\n{base_prompt}"

    def get_retry_agent(
        self,
        story_id: str,
        default_agent: str,
    ) -> str:
        """
        Get the agent to use for retry.

        May switch agents based on configuration and failure history.

        Args:
            story_id: ID of the story
            default_agent: Default agent if no switch needed

        Returns:
            Agent name to use for retry
        """
        if not self.config.switch_agent_on_retry:
            return default_agent

        state = self._states.get(story_id)
        if not state or not state.failures:
            return default_agent

        # Try to use a different agent from the chain
        used_agents = {f.agent for f in state.failures}

        for agent in self.config.retry_agent_chain:
            if agent not in used_agents:
                return agent

        # All agents tried, fall back to default
        return default_agent

    def get_retry_delay(self, story_id: str) -> float:
        """
        Get the delay before next retry (for exponential backoff).

        Args:
            story_id: ID of the story

        Returns:
            Delay in seconds
        """
        state = self._states.get(story_id)
        if not state:
            return 0

        if not self.config.exponential_backoff:
            return self.config.base_delay_seconds

        # Exponential backoff: base * 2^(attempt-1)
        delay = self.config.base_delay_seconds * (2 ** (state.current_attempt - 1))
        return min(delay, self.config.max_delay_seconds)

    def reset_story(self, story_id: str) -> None:
        """Reset retry state for a story (e.g., after successful completion)."""
        if story_id in self._states:
            del self._states[story_id]
            self._save_state()

    def get_failure_summary(self, story_id: str) -> str | None:
        """Get a summary of all failures for a story."""
        state = self._states.get(story_id)
        if not state or not state.failures:
            return None

        lines = [f"Story {story_id} - {len(state.failures)} failure(s):"]
        for failure in state.failures:
            lines.append(
                f"  Attempt {failure.attempt}: {failure.error_type.value} "
                f"via {failure.agent} - {failure.error_message[:100]}"
            )

        return "\n".join(lines)

    def _get_or_create_state(self, story_id: str) -> RetryState:
        """Get or create retry state for a story."""
        if story_id not in self._states:
            self._states[story_id] = RetryState(story_id=story_id)
        return self._states[story_id]

    def _analyze_error(
        self,
        error_message: str,
        output_excerpt: str | None = None,
    ) -> list[str]:
        """Analyze error and generate suggested fixes."""
        fixes = []
        text = f"{error_message} {output_excerpt or ''}"

        for pattern_name, pattern_info in self.ERROR_PATTERNS.items():
            for pattern in pattern_info["patterns"]:
                if pattern.lower() in text.lower():
                    fixes.extend(pattern_info["fixes"])
                    break

        # Deduplicate while preserving order
        seen = set()
        unique_fixes = []
        for fix in fixes:
            if fix not in seen:
                seen.add(fix)
                unique_fixes.append(fix)

        return unique_fixes[:5]  # Limit to 5 suggestions

    def _analyze_quality_gate_failures(
        self,
        results: dict[str, Any],
    ) -> list[str]:
        """Analyze quality gate failures and generate suggestions."""
        fixes = []

        for gate_name, result in results.items():
            if isinstance(result, dict) and not result.get("passed", True):
                gate_type = result.get("gate_type", "unknown")

                if gate_type == "typecheck":
                    fixes.append("Review and fix type errors in the changed files")
                elif gate_type == "test":
                    fixes.append("Review failing test assertions and update implementation")
                elif gate_type == "lint":
                    fixes.append("Fix lint issues - consider running the linter with --fix")
                elif gate_type == "implementation_verify":
                    fixes.append("Review verification feedback and complete implementation")
                    # Check for skeleton code in structured errors
                    for error in result.get("structured_errors", [])[:3]:
                        error_code = error.get("code", "")
                        if error_code == "SKELETON" or "skeleton" in error.get("message", "").lower():
                            fixes.append("Replace skeleton code with actual implementation")
                            break
                    # Check for missing implementations
                    error_summary = result.get("error_summary", "")
                    if "Missing:" in error_summary:
                        fixes.append("Implement all missing functionality listed in verification")

        return fixes

    def _build_failure_context(
        self,
        state: RetryState,
        last_failure: FailureRecord,
    ) -> str:
        """Build the failure context section for retry prompt."""
        lines = [
            "=" * 50,
            "PREVIOUS ATTEMPT FAILED",
            "=" * 50,
            f"Attempt: {last_failure.attempt} of {self.config.max_retries}",
            f"Error Type: {last_failure.error_type.value}",
            f"Error: {last_failure.error_message}",
        ]

        if last_failure.exit_code is not None:
            lines.append(f"Exit Code: {last_failure.exit_code}")

        if last_failure.output_excerpt:
            lines.extend([
                "",
                "Output Excerpt:",
                "-" * 40,
                last_failure.output_excerpt[:500],
                "-" * 40,
            ])

        if last_failure.quality_gate_results:
            lines.extend([
                "",
                "Quality Gate Results:",
            ])
            for gate, result in last_failure.quality_gate_results.items():
                if isinstance(result, dict):
                    passed = "PASS" if result.get("passed", False) else "FAIL"
                    lines.append(f"  - {gate}: {passed}")
                    if not result.get("passed", False) and result.get("error_summary"):
                        lines.append(f"    {result['error_summary'][:200]}")

        if last_failure.suggested_fixes:
            lines.extend([
                "",
                "SUGGESTED FIXES:",
                "-" * 40,
            ])
            for i, fix in enumerate(last_failure.suggested_fixes, 1):
                lines.append(f"  {i}. {fix}")

        lines.extend([
            "",
            "=" * 50,
            "Please address these issues in this retry attempt.",
            "=" * 50,
            "",
        ])

        return "\n".join(lines)

    def _load_state(self) -> None:
        """Load state from disk."""
        if not self.state_file.exists():
            return

        try:
            with open(self.state_file, encoding="utf-8") as f:
                data = json.load(f)

            for story_id, state_data in data.get("stories", {}).items():
                self._states[story_id] = RetryState.from_dict(state_data)
        except (json.JSONDecodeError, KeyError, TypeError):
            self._states = {}

    def _save_state(self) -> None:
        """Save state to disk."""
        data = {
            "version": "1.0.0",
            "updated_at": datetime.now().isoformat(),
            "config": self.config.to_dict(),
            "stories": {
                story_id: state.to_dict()
                for story_id, state in self._states.items()
            },
        }

        try:
            with open(self.state_file, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
        except OSError:
            pass  # State write failure is non-critical

    def get_all_states(self) -> dict[str, RetryState]:
        """Get all retry states."""
        return dict(self._states)

    def clear_all(self) -> None:
        """Clear all retry states."""
        self._states = {}
        if self.state_file.exists():
            try:
                self.state_file.unlink()
            except OSError:
                pass
