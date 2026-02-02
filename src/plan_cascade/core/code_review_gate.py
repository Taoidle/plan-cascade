"""
Code Review Gate for Plan Cascade

Provides AI-powered code review of story implementations to ensure
code quality, naming clarity, complexity, pattern adherence, and security.
"""

import asyncio
import json
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .quality_gate import Gate, GateConfig, GateOutput, GateType

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


class ReviewSeverity(Enum):
    """Severity levels for review findings."""
    CRITICAL = "critical"  # Must be fixed before merge
    HIGH = "high"          # Should be fixed
    MEDIUM = "medium"      # Consider fixing
    LOW = "low"            # Minor suggestion
    INFO = "info"          # Informational note


class ReviewCategory(Enum):
    """Categories for review findings."""
    CODE_QUALITY = "code_quality"
    NAMING_CLARITY = "naming_clarity"
    COMPLEXITY = "complexity"
    PATTERN_ADHERENCE = "pattern_adherence"
    SECURITY = "security"


@dataclass
class ReviewFinding:
    """A single finding from the code review."""
    category: ReviewCategory
    severity: ReviewSeverity
    file: str
    line: int | None
    title: str
    description: str
    suggestion: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "category": self.category.value,
            "severity": self.severity.value,
            "file": self.file,
            "line": self.line,
            "title": self.title,
            "description": self.description,
            "suggestion": self.suggestion,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ReviewFinding":
        """Create from dictionary."""
        return cls(
            category=ReviewCategory(data.get("category", "code_quality")),
            severity=ReviewSeverity(data.get("severity", "medium")),
            file=data.get("file", ""),
            line=data.get("line"),
            title=data.get("title", ""),
            description=data.get("description", ""),
            suggestion=data.get("suggestion"),
        )


@dataclass
class DimensionScore:
    """Score for a single review dimension."""
    dimension: str
    score: float  # 0.0 - 1.0
    max_points: int
    earned_points: float
    notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "dimension": self.dimension,
            "score": self.score,
            "max_points": self.max_points,
            "earned_points": self.earned_points,
            "notes": self.notes,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "DimensionScore":
        """Create from dictionary."""
        return cls(
            dimension=data.get("dimension", ""),
            score=data.get("score", 0.0),
            max_points=data.get("max_points", 0),
            earned_points=data.get("earned_points", 0.0),
            notes=data.get("notes", ""),
        )


@dataclass
class CodeReviewResult:
    """Result of a code review."""
    story_id: str
    overall_score: float  # 0.0 - 1.0
    passed: bool
    confidence: float  # 0.0 - 1.0
    dimension_scores: list[DimensionScore] = field(default_factory=list)
    findings: list[ReviewFinding] = field(default_factory=list)
    summary: str = ""
    has_critical: bool = False

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "story_id": self.story_id,
            "overall_score": self.overall_score,
            "passed": self.passed,
            "confidence": self.confidence,
            "dimension_scores": [d.to_dict() for d in self.dimension_scores],
            "findings": [f.to_dict() for f in self.findings],
            "summary": self.summary,
            "has_critical": self.has_critical,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "CodeReviewResult":
        """Create from dictionary."""
        return cls(
            story_id=data.get("story_id", ""),
            overall_score=data.get("overall_score", 0.0),
            passed=data.get("passed", False),
            confidence=data.get("confidence", 0.0),
            dimension_scores=[
                DimensionScore.from_dict(d)
                for d in data.get("dimension_scores", [])
            ],
            findings=[
                ReviewFinding.from_dict(f)
                for f in data.get("findings", [])
            ],
            summary=data.get("summary", ""),
            has_critical=data.get("has_critical", False),
        )


# Code review prompt template for the AI
CODE_REVIEW_PROMPT_TEMPLATE = '''You are an AI code reviewer. Your task is to review the code changes for story {story_id} and provide a structured quality assessment.

## Story: {story_id} - {title}
{description}

## Git Diff (Changes Made)
```diff
{git_diff}
```

## Changed Files Content
{file_contents}

{design_context}

## Review Dimensions

Score each dimension on a 0-100 scale. The total possible is 100 points:

1. **Code Quality (25 pts)**: Clean code, proper error handling, no code smells
   - Functions are focused and do one thing well
   - Proper error handling and edge cases
   - No dead code, unused variables, or commented-out code
   - Appropriate use of language features

2. **Naming & Clarity (20 pts)**: Clear naming, readability, self-documenting code
   - Variables, functions, and classes have descriptive names
   - Code is self-documenting without excessive comments
   - Consistent naming conventions
   - Clear intent visible in code structure

3. **Complexity (20 pts)**: Appropriate complexity, no over-engineering
   - Functions have reasonable cyclomatic complexity
   - No unnecessary abstractions or premature optimization
   - Logic is straightforward and followable
   - Nesting is minimal and code is flat where possible

4. **Pattern Adherence (20 pts)**: Follows project patterns and conventions
   - Consistent with existing codebase style
   - Uses established patterns from the project
   - Follows framework conventions
   - Maintains architectural boundaries

5. **Security (15 pts)**: No security vulnerabilities
   - No hardcoded secrets or credentials
   - Input validation where needed
   - No SQL injection, XSS, or command injection risks
   - Proper authentication/authorization checks if applicable

## Severity Levels for Findings

- **critical**: Must be fixed before merge (security vulnerabilities, data loss risks)
- **high**: Should be fixed (significant bugs, poor patterns)
- **medium**: Consider fixing (code smells, minor issues)
- **low**: Minor suggestions (style, optimization)
- **info**: Informational notes (observations, tips)

## Return ONLY a JSON object with this exact structure:

{{
  "overall_score": 0.0-1.0,
  "confidence": 0.0-1.0,
  "dimension_scores": [
    {{
      "dimension": "code_quality",
      "score": 0.0-1.0,
      "max_points": 25,
      "earned_points": 0-25,
      "notes": "brief assessment"
    }},
    {{
      "dimension": "naming_clarity",
      "score": 0.0-1.0,
      "max_points": 20,
      "earned_points": 0-20,
      "notes": "brief assessment"
    }},
    {{
      "dimension": "complexity",
      "score": 0.0-1.0,
      "max_points": 20,
      "earned_points": 0-20,
      "notes": "brief assessment"
    }},
    {{
      "dimension": "pattern_adherence",
      "score": 0.0-1.0,
      "max_points": 20,
      "earned_points": 0-20,
      "notes": "brief assessment"
    }},
    {{
      "dimension": "security",
      "score": 0.0-1.0,
      "max_points": 15,
      "earned_points": 0-15,
      "notes": "brief assessment"
    }}
  ],
  "findings": [
    {{
      "category": "code_quality|naming_clarity|complexity|pattern_adherence|security",
      "severity": "critical|high|medium|low|info",
      "file": "path/to/file.py",
      "line": 42,
      "title": "Short issue title",
      "description": "Detailed description of the issue",
      "suggestion": "How to fix it (optional)"
    }}
  ],
  "summary": "Brief overall summary of the review"
}}

Important: Return ONLY the JSON object, no markdown formatting or explanation.
Only include findings for actual issues found. An empty findings array is valid for clean code.'''


class CodeReviewGate(Gate):
    """
    AI-powered gate to review code quality of story implementations.

    Uses an LLM to analyze git diffs and provide structured feedback on
    code quality, naming, complexity, patterns, and security.
    """

    # Gate group for ordering
    GATE_GROUP = "post_validation"

    def __init__(
        self,
        config: GateConfig,
        project_root: Path,
        llm_provider: "LLMProvider | None" = None,
    ):
        """
        Initialize the code review gate.

        Args:
            config: Gate configuration
            project_root: Root directory of the project
            llm_provider: LLM provider for review (uses default if None)
        """
        super().__init__(config, project_root)
        self._llm_provider = llm_provider

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
                return result.stdout[:15000]  # Larger limit for reviews

            # If no uncommitted changes, try last commit
            result = subprocess.run(
                ["git", "diff", "HEAD~1", "HEAD"],
                **kwargs,
            )
            if result.returncode == 0:
                return result.stdout[:15000]

            return "(no changes detected)"
        except Exception as e:
            return f"(error getting diff: {e})"

    def _read_changed_files(self, git_diff: str) -> str:
        """Read content of files mentioned in the diff."""
        file_contents = []
        files_seen = set()

        for line in git_diff.split("\n"):
            if line.startswith("+++ b/") or line.startswith("--- a/"):
                file_path = line[6:]
                if file_path in files_seen or file_path == "/dev/null":
                    continue
                files_seen.add(file_path)

                full_path = self.project_root / file_path
                if full_path.exists() and full_path.is_file():
                    try:
                        content = full_path.read_text(encoding="utf-8")
                        # Limit each file to 3000 chars for reviews
                        if len(content) > 3000:
                            content = content[:3000] + "\n... (truncated)"
                        file_contents.append(f"### {file_path}\n```\n{content}\n```\n")
                    except Exception:
                        pass

                if len(file_contents) >= 8:  # More files for reviews
                    break

        return "\n".join(file_contents) if file_contents else "(no file contents available)"

    def _load_design_context(self, story_id: str) -> str:
        """Load relevant design document context if available."""
        design_doc_path = self.project_root / "design_doc.json"
        if not design_doc_path.exists():
            return ""

        try:
            design_doc = json.loads(design_doc_path.read_text(encoding="utf-8"))

            # Get story mappings
            story_mappings = design_doc.get("story_mappings", {})
            story_mapping = story_mappings.get(story_id, {})

            if not story_mapping:
                return ""

            context_parts = ["## Architecture Context (from design_doc.json)\n"]

            # Add relevant components
            components = story_mapping.get("components", [])
            if components:
                context_parts.append("### Relevant Components")
                all_components = design_doc.get("components", [])
                for comp_name in components:
                    for comp in all_components:
                        if comp.get("name") == comp_name:
                            context_parts.append(f"- **{comp_name}**: {comp.get('purpose', '')}")
                            break
                context_parts.append("")

            # Add relevant patterns
            patterns = story_mapping.get("patterns", [])
            if patterns:
                context_parts.append("### Patterns to Follow")
                all_patterns = design_doc.get("architectural_patterns", [])
                for pattern_name in patterns:
                    for pattern in all_patterns:
                        if pattern.get("name") == pattern_name:
                            context_parts.append(f"- **{pattern_name}**: {pattern.get('rationale', '')}")
                            break
                context_parts.append("")

            # Add relevant ADRs
            adrs = story_mapping.get("adrs", [])
            if adrs:
                context_parts.append("### Relevant Decisions (ADRs)")
                all_adrs = design_doc.get("adrs", [])
                for adr_id in adrs:
                    for adr in all_adrs:
                        if adr.get("id") == adr_id:
                            context_parts.append(f"- **{adr_id}**: {adr.get('title', '')} - {adr.get('decision', '')}")
                            break

            return "\n".join(context_parts)
        except Exception:
            return ""

    async def _review_story_async(
        self,
        story: dict[str, Any],
        git_diff: str,
        file_contents: str,
        design_context: str,
    ) -> CodeReviewResult:
        """Review a story implementation using the LLM."""
        story_id = story.get("id", "unknown")
        title = story.get("title", "Untitled")
        description = story.get("description", "")

        # Build the prompt
        prompt = CODE_REVIEW_PROMPT_TEMPLATE.format(
            story_id=story_id,
            title=title,
            description=description,
            git_diff=git_diff,
            file_contents=file_contents,
            design_context=design_context,
        )

        try:
            llm = self._get_llm_provider()
            response = await llm.complete(
                messages=[{"role": "user", "content": prompt}],
                temperature=0.1,  # Low temperature for consistent reviews
                max_tokens=3000,
            )

            # Parse JSON response
            content = response.content.strip()
            # Handle potential markdown code blocks
            if content.startswith("```"):
                lines = content.split("\n")
                content = "\n".join(lines[1:-1] if lines[-1] == "```" else lines[1:])

            result_data = json.loads(content)

            # Parse dimension scores
            dimension_scores = [
                DimensionScore.from_dict(d)
                for d in result_data.get("dimension_scores", [])
            ]

            # Parse findings
            findings = [
                ReviewFinding.from_dict(f)
                for f in result_data.get("findings", [])
            ]

            # Check for critical findings
            has_critical = any(
                f.severity == ReviewSeverity.CRITICAL
                for f in findings
            )

            return CodeReviewResult(
                story_id=story_id,
                overall_score=result_data.get("overall_score", 0.0),
                passed=not has_critical,  # Will be adjusted based on config
                confidence=result_data.get("confidence", 0.0),
                dimension_scores=dimension_scores,
                findings=findings,
                summary=result_data.get("summary", ""),
                has_critical=has_critical,
            )

        except json.JSONDecodeError as e:
            return CodeReviewResult(
                story_id=story_id,
                overall_score=0.0,
                passed=False,
                confidence=0.0,
                summary=f"Failed to parse review response: {e}",
            )
        except Exception as e:
            return CodeReviewResult(
                story_id=story_id,
                overall_score=0.0,
                passed=False,
                confidence=0.0,
                summary=f"Review error: {e}",
            )

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute code review synchronously."""
        try:
            loop = asyncio.get_event_loop()
        except RuntimeError:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

        return loop.run_until_complete(self.execute_async(story_id, context))

    async def execute_async(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute code review asynchronously."""
        start_time = datetime.now()

        story = context.get("story", {})
        if not story:
            story = {"id": story_id}

        # Get configuration
        min_score = getattr(self.config, 'min_score', 0.7)
        block_on_critical = getattr(self.config, 'block_on_critical', True)
        confidence_threshold = self.config.confidence_threshold

        # Get git diff and file contents
        git_diff = self._get_git_diff(story_id)
        file_contents = self._read_changed_files(git_diff)
        design_context = self._load_design_context(story_id)

        # Review the implementation
        result = await self._review_story_async(
            story, git_diff, file_contents, design_context
        )

        duration = (datetime.now() - start_time).total_seconds()

        # Determine if passed based on configuration
        passed = True
        failure_reasons = []

        if result.overall_score < min_score:
            passed = False
            failure_reasons.append(f"Score {result.overall_score:.2f} < min {min_score}")

        if result.confidence < confidence_threshold:
            passed = False
            failure_reasons.append(f"Confidence {result.confidence:.2f} < threshold {confidence_threshold}")

        if block_on_critical and result.has_critical:
            passed = False
            critical_count = sum(
                1 for f in result.findings
                if f.severity == ReviewSeverity.CRITICAL
            )
            failure_reasons.append(f"{critical_count} critical finding(s)")

        # Update result passed status
        result.passed = passed

        # Build error summary if failed
        error_summary = None
        if not passed:
            error_summary = "; ".join(failure_reasons)
            if result.findings:
                top_issues = [
                    f"[{f.severity.value}] {f.title}"
                    for f in sorted(result.findings, key=lambda x: x.severity.value)[:3]
                ]
                error_summary += f"\nTop issues: {', '.join(top_issues)}"

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.CODE_REVIEW,
            passed=passed,
            exit_code=0 if passed else 1,
            stdout=json.dumps(result.to_dict(), indent=2),
            stderr="" if passed else (error_summary or "Code review failed"),
            duration_seconds=duration,
            command="ai-code-review",
            error_summary=error_summary,
        )
