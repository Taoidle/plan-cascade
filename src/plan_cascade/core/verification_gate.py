"""
Implementation Verification Gate for Plan Cascade

Provides AI-powered verification of story implementations to ensure
completeness and detect skeleton code before marking stories as done.
"""

import asyncio
import json
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .quality_gate import Gate, GateConfig, GateOutput, GateType

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


@dataclass
class CriterionCheck:
    """Result of checking a single acceptance criterion."""
    criterion: str
    passed: bool
    evidence: str
    confidence: float

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "criterion": self.criterion,
            "passed": self.passed,
            "evidence": self.evidence,
            "confidence": self.confidence,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "CriterionCheck":
        """Create from dictionary."""
        return cls(
            criterion=data.get("criterion", ""),
            passed=data.get("passed", False),
            evidence=data.get("evidence", ""),
            confidence=data.get("confidence", 0.0),
        )


@dataclass
class VerificationResult:
    """Result of verifying a story implementation."""
    story_id: str
    overall_passed: bool
    confidence: float
    criteria_checks: list[CriterionCheck] = field(default_factory=list)
    skeleton_detected: bool = False
    skeleton_evidence: str | None = None
    missing_implementations: list[str] = field(default_factory=list)
    summary: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "story_id": self.story_id,
            "overall_passed": self.overall_passed,
            "confidence": self.confidence,
            "criteria_checks": [c.to_dict() for c in self.criteria_checks],
            "skeleton_detected": self.skeleton_detected,
            "skeleton_evidence": self.skeleton_evidence,
            "missing_implementations": self.missing_implementations,
            "summary": self.summary,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "VerificationResult":
        """Create from dictionary."""
        return cls(
            story_id=data.get("story_id", ""),
            overall_passed=data.get("overall_passed", False),
            confidence=data.get("confidence", 0.0),
            criteria_checks=[
                CriterionCheck.from_dict(c)
                for c in data.get("criteria_checks", [])
            ],
            skeleton_detected=data.get("skeleton_detected", False),
            skeleton_evidence=data.get("skeleton_evidence"),
            missing_implementations=data.get("missing_implementations", []),
            summary=data.get("summary", ""),
        )


@dataclass
class BatchVerificationResult:
    """Result of verifying all stories in a batch."""
    batch_num: int
    passed: bool
    results: dict[str, VerificationResult] = field(default_factory=dict)
    blocking_failures: list[str] = field(default_factory=list)
    async_fix_queue: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "batch_num": self.batch_num,
            "passed": self.passed,
            "results": {k: v.to_dict() for k, v in self.results.items()},
            "blocking_failures": self.blocking_failures,
            "async_fix_queue": self.async_fix_queue,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "BatchVerificationResult":
        """Create from dictionary."""
        results = {}
        for k, v in data.get("results", {}).items():
            results[k] = VerificationResult.from_dict(v)
        return cls(
            batch_num=data.get("batch_num", 0),
            passed=data.get("passed", False),
            results=results,
            blocking_failures=data.get("blocking_failures", []),
            async_fix_queue=data.get("async_fix_queue", []),
        )


# Verification prompt template for the AI
VERIFICATION_PROMPT_TEMPLATE = '''You are an implementation verification agent. Your task is to verify that the code changes actually implement the story requirements.

## Story: {story_id} - {title}
{description}

## Acceptance Criteria
{criteria}

## Git Diff (Changes Made)
```diff
{git_diff}
```

## Changed Files Content
{file_contents}

## Your Task
Analyze the code changes and verify:
1. Each acceptance criterion is implemented (not just stubbed)
2. No skeleton code (pass, ..., NotImplementedError, TODO, FIXME in new code)
3. The implementation is functional, not placeholder

## Skeleton Code Detection Rules
Mark skeleton_detected=true if you find ANY of these in NEW code (not existing code):
- Functions with only `pass`, `...`, or `raise NotImplementedError`
- TODO/FIXME comments in newly added code
- Placeholder return values like `return None`, `return ""`, `return []` without logic
- Empty function/method bodies
- Comments like "# implement later" or "# stub"

## Return ONLY a JSON object with this exact structure:
{{
  "overall_passed": true/false,
  "confidence": 0.0-1.0,
  "criteria_checks": [
    {{
      "criterion": "criterion text",
      "passed": true/false,
      "evidence": "specific code/line reference",
      "confidence": 0.0-1.0
    }}
  ],
  "skeleton_detected": true/false,
  "skeleton_evidence": "description of skeleton code found or null",
  "missing_implementations": ["list of missing features"],
  "summary": "brief summary of verification result"
}}

Important: Return ONLY the JSON object, no markdown formatting or explanation.'''


class ImplementationVerifyGate(Gate):
    """
    AI-powered gate to verify story implementations are complete.

    Uses an LLM to analyze git diffs and acceptance criteria to ensure
    implementations are not skeleton code and meet requirements.
    """

    def __init__(
        self,
        config: GateConfig,
        project_root: Path,
        llm_provider: "LLMProvider | None" = None,
        confidence_threshold: float = 0.7,
    ):
        """
        Initialize the verification gate.

        Args:
            config: Gate configuration
            project_root: Root directory of the project
            llm_provider: LLM provider for verification (uses default if None)
            confidence_threshold: Minimum confidence for passing (0.0-1.0)
        """
        super().__init__(config, project_root)
        self._llm_provider = llm_provider
        self.confidence_threshold = confidence_threshold

    def _get_llm_provider(self) -> "LLMProvider":
        """Get or create the LLM provider."""
        if self._llm_provider is not None:
            return self._llm_provider

        # Lazy import and create default provider
        from ..llm.factory import LLMFactory
        self._llm_provider = LLMFactory.create("claude")
        return self._llm_provider

    def _get_git_diff(self, story_id: str) -> str:
        """Get git diff for recent changes."""
        try:
            kwargs: dict[str, Any] = {
                "capture_output": True,
                "text": True,
                "cwd": str(self.project_root),
            }
            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

            # Get diff of uncommitted changes
            result = subprocess.run(
                ["git", "diff", "HEAD"],
                **kwargs,
            )
            if result.returncode == 0 and result.stdout.strip():
                return result.stdout[:10000]  # Limit size

            # If no uncommitted changes, try last commit
            result = subprocess.run(
                ["git", "diff", "HEAD~1", "HEAD"],
                **kwargs,
            )
            if result.returncode == 0:
                return result.stdout[:10000]

            return "(no changes detected)"
        except Exception as e:
            return f"(error getting diff: {e})"

    def _read_changed_files(self, git_diff: str) -> str:
        """Read content of files mentioned in the diff."""
        # Extract file names from diff
        file_contents = []
        files_seen = set()

        for line in git_diff.split("\n"):
            if line.startswith("+++ b/") or line.startswith("--- a/"):
                file_path = line[6:]  # Remove prefix
                if file_path in files_seen or file_path == "/dev/null":
                    continue
                files_seen.add(file_path)

                full_path = self.project_root / file_path
                if full_path.exists() and full_path.is_file():
                    try:
                        content = full_path.read_text(encoding="utf-8")
                        # Limit each file to 2000 chars
                        if len(content) > 2000:
                            content = content[:2000] + "\n... (truncated)"
                        file_contents.append(f"### {file_path}\n```\n{content}\n```\n")
                    except Exception:
                        pass

                if len(file_contents) >= 5:  # Limit number of files
                    break

        return "\n".join(file_contents) if file_contents else "(no file contents available)"

    def _format_criteria(self, story: dict[str, Any]) -> str:
        """Format acceptance criteria for the prompt."""
        criteria = story.get("acceptance_criteria", [])
        if not criteria:
            return "(no acceptance criteria defined)"

        return "\n".join(f"- {c}" for c in criteria)

    async def _verify_story_async(
        self,
        story: dict[str, Any],
        git_diff: str,
        file_contents: str,
    ) -> VerificationResult:
        """Verify a story implementation using the LLM."""
        story_id = story.get("id", "unknown")
        title = story.get("title", "Untitled")
        description = story.get("description", "")
        criteria = self._format_criteria(story)

        # Build the prompt
        prompt = VERIFICATION_PROMPT_TEMPLATE.format(
            story_id=story_id,
            title=title,
            description=description,
            criteria=criteria,
            git_diff=git_diff,
            file_contents=file_contents,
        )

        try:
            llm = self._get_llm_provider()
            response = await llm.complete(
                messages=[{"role": "user", "content": prompt}],
                temperature=0.1,  # Low temperature for consistent verification
                max_tokens=2000,
            )

            # Parse JSON response
            content = response.content.strip()
            # Handle potential markdown code blocks
            if content.startswith("```"):
                lines = content.split("\n")
                content = "\n".join(lines[1:-1] if lines[-1] == "```" else lines[1:])

            result_data = json.loads(content)

            return VerificationResult(
                story_id=story_id,
                overall_passed=result_data.get("overall_passed", False),
                confidence=result_data.get("confidence", 0.0),
                criteria_checks=[
                    CriterionCheck.from_dict(c)
                    for c in result_data.get("criteria_checks", [])
                ],
                skeleton_detected=result_data.get("skeleton_detected", False),
                skeleton_evidence=result_data.get("skeleton_evidence"),
                missing_implementations=result_data.get("missing_implementations", []),
                summary=result_data.get("summary", ""),
            )

        except json.JSONDecodeError as e:
            return VerificationResult(
                story_id=story_id,
                overall_passed=False,
                confidence=0.0,
                summary=f"Failed to parse verification response: {e}",
            )
        except Exception as e:
            return VerificationResult(
                story_id=story_id,
                overall_passed=False,
                confidence=0.0,
                summary=f"Verification error: {e}",
            )

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute verification synchronously."""
        # Run the async method in a new event loop
        try:
            loop = asyncio.get_event_loop()
        except RuntimeError:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

        return loop.run_until_complete(self.execute_async(story_id, context))

    async def execute_async(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute verification asynchronously."""
        start_time = datetime.now()

        story = context.get("story", {})
        if not story:
            story = {"id": story_id}

        # Get git diff and file contents
        git_diff = self._get_git_diff(story_id)
        file_contents = self._read_changed_files(git_diff)

        # Verify the implementation
        result = await self._verify_story_async(story, git_diff, file_contents)

        duration = (datetime.now() - start_time).total_seconds()

        # Determine if passed based on confidence threshold and result
        passed = (
            result.overall_passed
            and result.confidence >= self.confidence_threshold
            and not result.skeleton_detected
        )

        # Build error summary if failed
        error_summary = None
        if not passed:
            errors = []
            if result.skeleton_detected:
                errors.append(f"SKELETON: {result.skeleton_evidence}")
            if not result.overall_passed:
                errors.append(f"Verification failed: {result.summary}")
            if result.confidence < self.confidence_threshold:
                errors.append(f"Low confidence: {result.confidence:.2f} < {self.confidence_threshold}")
            for missing in result.missing_implementations[:3]:
                errors.append(f"Missing: {missing}")
            error_summary = "; ".join(errors)

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.IMPLEMENTATION_VERIFY,
            passed=passed,
            exit_code=0 if passed else 1,
            stdout=json.dumps(result.to_dict(), indent=2),
            stderr="" if passed else (error_summary or "Verification failed"),
            duration_seconds=duration,
            command="ai-verification",
            error_summary=error_summary,
        )


class BatchVerificationGate:
    """
    Verifies all stories in a batch after execution.

    Determines which failures are blocking (story has dependents) vs
    non-blocking (can be fixed asynchronously).
    """

    def __init__(
        self,
        project_root: Path,
        llm_provider: "LLMProvider | None" = None,
        confidence_threshold: float = 0.7,
    ):
        """
        Initialize batch verification gate.

        Args:
            project_root: Root directory of the project
            llm_provider: LLM provider for verification
            confidence_threshold: Minimum confidence for passing
        """
        self.project_root = Path(project_root)
        self._llm_provider = llm_provider
        self.confidence_threshold = confidence_threshold

    async def verify_batch(
        self,
        stories: list[dict[str, Any]],
        batch_num: int,
        prd: dict[str, Any],
    ) -> BatchVerificationResult:
        """
        Verify all stories in a batch.

        Args:
            stories: List of story dictionaries that were executed
            batch_num: The batch number
            prd: Full PRD for dependency analysis

        Returns:
            BatchVerificationResult with per-story results and blocking failures
        """
        # Create a gate config for verification
        config = GateConfig(
            name="batch-verification",
            type=GateType.IMPLEMENTATION_VERIFY,
            enabled=True,
            required=True,
        )

        gate = ImplementationVerifyGate(
            config=config,
            project_root=self.project_root,
            llm_provider=self._llm_provider,
            confidence_threshold=self.confidence_threshold,
        )

        # Verify each story in parallel
        tasks = []
        for story in stories:
            story_id = story.get("id", "unknown")
            context = {"story": story}
            tasks.append((story_id, gate.execute_async(story_id, context)))

        results: dict[str, VerificationResult] = {}
        blocking_failures: list[str] = []
        async_fix_queue: list[str] = []

        # Execute all verifications
        for story_id, task in tasks:
            output = await task

            # Parse the verification result from stdout
            try:
                result_data = json.loads(output.stdout)
                result = VerificationResult.from_dict(result_data)
            except (json.JSONDecodeError, Exception):
                result = VerificationResult(
                    story_id=story_id,
                    overall_passed=output.passed,
                    confidence=1.0 if output.passed else 0.0,
                    summary=output.error_summary or "",
                )

            results[story_id] = result

            # Determine if failure is blocking
            if not output.passed:
                if self._story_blocks_others(story_id, stories, prd):
                    blocking_failures.append(story_id)
                else:
                    async_fix_queue.append(story_id)

        # Batch passes if no blocking failures
        passed = len(blocking_failures) == 0

        return BatchVerificationResult(
            batch_num=batch_num,
            passed=passed,
            results=results,
            blocking_failures=blocking_failures,
            async_fix_queue=async_fix_queue,
        )

    def _story_blocks_others(
        self,
        story_id: str,
        batch_stories: list[dict[str, Any]],
        prd: dict[str, Any],
    ) -> bool:
        """
        Check if a story has dependents in future batches.

        Args:
            story_id: ID of the story to check
            batch_stories: Stories in the current batch
            prd: Full PRD with all stories

        Returns:
            True if other stories depend on this one
        """
        batch_story_ids = {s.get("id") for s in batch_stories}
        all_stories = prd.get("stories", [])

        for story in all_stories:
            if story.get("id") in batch_story_ids:
                continue  # Skip stories in current batch

            dependencies = story.get("dependencies", [])
            if story_id in dependencies:
                return True

        return False
