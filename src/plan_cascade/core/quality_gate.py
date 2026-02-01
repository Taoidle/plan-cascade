"""
Quality Gate System for Plan Cascade

Run verification after story completion to ensure code quality.
Supports typecheck, test, lint, and custom gates with auto-detection.
Supports both synchronous and asynchronous (parallel) execution.
"""

import asyncio
import os
import re
import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .changed_files import ChangedFilesDetector

if TYPE_CHECKING:
    from .gate_cache import GateCache
from .error_parser import (
    ErrorInfo,
    EslintParser,
    FlakeParser,
    MypyParser,
    PyrightParser,
    PytestParser,
    RuffParser,
    TscParser,
    generate_error_summary,
)


class GateType(Enum):
    """Types of quality gates."""
    TYPECHECK = "typecheck"
    TEST = "test"
    LINT = "lint"
    CUSTOM = "custom"


class ProjectType(Enum):
    """Types of projects for auto-detection."""
    NODEJS = "nodejs"
    PYTHON = "python"
    RUST = "rust"
    GO = "go"
    UNKNOWN = "unknown"


@dataclass
class GateOutput:
    """Output from a quality gate execution."""
    gate_name: str
    gate_type: GateType
    passed: bool
    exit_code: int
    stdout: str
    stderr: str
    duration_seconds: float
    command: str
    error_summary: str | None = None
    structured_errors: list[ErrorInfo] = field(default_factory=list)
    skipped: bool = False  # True if gate was skipped (fail_fast or no changed files)
    checked_files: list[str] | None = None  # Files checked in incremental mode
    from_cache: bool = False  # True if result was retrieved from cache


@dataclass
class GateConfig:
    """Configuration for a single gate."""
    name: str
    type: GateType
    enabled: bool = True
    required: bool = True  # If True, failure blocks progression
    command: str | None = None  # Custom command (for CUSTOM type)
    args: list[str] = field(default_factory=list)
    timeout_seconds: int = 300
    working_dir: str | None = None
    env: dict[str, str] = field(default_factory=dict)
    project_type: ProjectType | None = None  # Applicable project type for this gate
    incremental: bool = False  # If True, only check changed files

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        result = {
            "name": self.name,
            "type": self.type.value,
            "enabled": self.enabled,
            "required": self.required,
            "command": self.command,
            "args": self.args,
            "timeout_seconds": self.timeout_seconds,
            "working_dir": self.working_dir,
            "env": self.env,
            "incremental": self.incremental,
        }
        if self.project_type is not None:
            result["project_type"] = self.project_type.value
        return result

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "GateConfig":
        """Create from dictionary."""
        project_type = None
        if "project_type" in data and data["project_type"] is not None:
            project_type = ProjectType(data["project_type"])
        return cls(
            name=data["name"],
            type=GateType(data["type"]),
            enabled=data.get("enabled", True),
            required=data.get("required", True),
            command=data.get("command"),
            args=data.get("args", []),
            timeout_seconds=data.get("timeout_seconds", 300),
            working_dir=data.get("working_dir"),
            env=data.get("env", {}),
            project_type=project_type,
            incremental=data.get("incremental", False),
        )


class Gate(ABC):
    """Abstract base class for quality gates."""

    # File extensions handled by this gate type (override in subclasses)
    FILE_EXTENSIONS: dict[ProjectType, list[str]] = {}

    def __init__(self, config: GateConfig, project_root: Path):
        """
        Initialize a quality gate.

        Args:
            config: Gate configuration
            project_root: Root directory of the project
        """
        self.config = config
        self.project_root = project_root
        self._changed_files_detector: ChangedFilesDetector | None = None

    def _get_changed_files_detector(self) -> ChangedFilesDetector:
        """Get or create the changed files detector."""
        if self._changed_files_detector is None:
            self._changed_files_detector = ChangedFilesDetector(self.project_root)
        return self._changed_files_detector

    def _get_changed_files_for_gate(
        self,
        project_type: ProjectType,
        context: dict[str, Any],
    ) -> list[str] | None:
        """
        Get changed files relevant to this gate type.

        Returns None if incremental mode is disabled or not in a git repo.
        Returns empty list if no relevant files changed.
        Returns list of file paths if there are changes.
        """
        if not self.config.incremental:
            return None

        detector = self._get_changed_files_detector()

        if not detector.is_git_repository():
            return None

        # Get extensions for this gate type and project type
        extensions = self.FILE_EXTENSIONS.get(project_type, [])
        if not extensions:
            # No specific extensions, return all changed files
            return detector.get_changed_files()

        return detector.get_changed_files_by_extension(extensions)

    def _create_skipped_output(
        self,
        reason: str,
        checked_files: list[str] | None = None,
    ) -> GateOutput:
        """Create a skipped gate output."""
        return GateOutput(
            gate_name=self.config.name,
            gate_type=self.config.type,
            passed=True,  # Skipped gates are considered passed
            exit_code=0,
            stdout=reason,
            stderr="",
            duration_seconds=0.0,
            command="",
            skipped=True,
            checked_files=checked_files,
        )

    @abstractmethod
    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute the gate synchronously and return results."""
        pass

    async def execute_async(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """
        Execute the gate asynchronously and return results.

        Default implementation wraps the synchronous execute() method
        using asyncio.to_thread() for parallel execution.

        Args:
            story_id: ID of the story being verified
            context: Additional context for the gate

        Returns:
            GateOutput with execution results
        """
        return await asyncio.to_thread(self.execute, story_id, context)

    def _run_command(
        self,
        command: list[str],
        timeout: int | None = None,
        env: dict[str, str] | None = None,
        cwd: Path | None = None,
    ) -> tuple[int, str, str, float]:
        """
        Run a command and return (exit_code, stdout, stderr, duration).
        """
        timeout = timeout or self.config.timeout_seconds
        cwd = cwd or (Path(self.config.working_dir) if self.config.working_dir else self.project_root)

        # Merge environment
        run_env = os.environ.copy()
        if self.config.env:
            run_env.update(self.config.env)
        if env:
            run_env.update(env)

        start_time = datetime.now()

        try:
            kwargs: dict[str, Any] = {
                "capture_output": True,
                "text": True,
                "timeout": timeout,
                "cwd": str(cwd),
                "env": run_env,
            }

            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

            result = subprocess.run(command, **kwargs)
            duration = (datetime.now() - start_time).total_seconds()

            return result.returncode, result.stdout, result.stderr, duration

        except subprocess.TimeoutExpired:
            duration = (datetime.now() - start_time).total_seconds()
            return -1, "", f"Command timed out after {timeout} seconds", duration
        except FileNotFoundError as e:
            duration = (datetime.now() - start_time).total_seconds()
            return -1, "", f"Command not found: {e}", duration
        except Exception as e:
            duration = (datetime.now() - start_time).total_seconds()
            return -1, "", f"Error running command: {e}", duration


class TypeCheckGate(Gate):
    """Typecheck gate supporting tsc, mypy, pyright."""

    # File extensions handled by this gate type
    FILE_EXTENSIONS: dict[ProjectType, list[str]] = {
        ProjectType.PYTHON: [".py", ".pyi"],
        ProjectType.NODEJS: [".ts", ".tsx", ".js", ".jsx"],
    }

    # Commands by project type - prefer JSON output format
    COMMANDS: dict[ProjectType, list[str]] = {
        ProjectType.NODEJS: ["npx", "tsc", "--noEmit"],
        ProjectType.PYTHON: ["mypy", "--output-format=json", "."],
    }

    # Fallback commands without JSON format
    FALLBACK_COMMANDS: dict[ProjectType, list[list[str]]] = {
        ProjectType.PYTHON: [
            ["mypy", "."],  # fallback to text format
            ["pyright", "--outputjson"],
            ["pyright"],
            ["python", "-m", "mypy", "--output-format=json", "."],
            ["python", "-m", "mypy", "."],
        ],
    }

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute typecheck."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)
        use_json = True  # Track if we're using JSON output format
        checked_files: list[str] | None = None

        # Check for incremental mode
        changed_files = self._get_changed_files_for_gate(project_type, context)
        if changed_files is not None:
            if not changed_files:
                # No relevant files changed, skip the gate
                return self._create_skipped_output(
                    "No Python/TypeScript files changed, skipping typecheck",
                    checked_files=[],
                )
            checked_files = changed_files

        # Get command
        if self.config.command:
            command = [self.config.command] + self.config.args
            use_json = "--output-format=json" in self.config.args or "--outputjson" in self.config.args
        else:
            command = list(self.COMMANDS.get(project_type, ["echo", "No typecheck configured"]))

        # In incremental mode, replace "." with specific files for mypy
        if checked_files and project_type == ProjectType.PYTHON:
            # Remove "." from command and add specific files
            if "." in command:
                command.remove(".")
            command.extend(checked_files)

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails with "not found"
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                fallback_cmd = list(fallback)
                # Apply incremental mode to fallback command
                if checked_files and project_type == ProjectType.PYTHON:
                    if "." in fallback_cmd:
                        fallback_cmd.remove(".")
                    fallback_cmd.extend(checked_files)

                exit_code, stdout, stderr, duration = self._run_command(fallback_cmd)
                if exit_code != -1:
                    command_str = " ".join(fallback_cmd)
                    use_json = "--output-format=json" in fallback or "--outputjson" in fallback
                    break

        # Parse structured errors and generate summary
        structured_errors: list[ErrorInfo] = []
        error_summary = None
        if exit_code != 0:
            structured_errors = self._parse_errors(stdout, stderr, project_type, use_json)
            error_summary = generate_error_summary(structured_errors)

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.TYPECHECK,
            passed=exit_code == 0,
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration_seconds=duration,
            command=command_str,
            error_summary=error_summary,
            structured_errors=structured_errors,
            checked_files=checked_files,
        )

    def _parse_errors(
        self, stdout: str, stderr: str, project_type: ProjectType, use_json: bool = True
    ) -> list[ErrorInfo]:
        """Parse type errors into structured ErrorInfo objects."""
        output = stdout + "\n" + stderr

        if project_type == ProjectType.PYTHON:
            # Try mypy parser first (handles both JSON and text)
            errors = MypyParser.parse(output)
            if errors:
                return errors
            # Try pyright parser
            errors = PyrightParser.parse(output)
            if errors:
                return errors

        elif project_type == ProjectType.NODEJS:
            # Parse TypeScript compiler output
            errors = TscParser.parse(output)
            if errors:
                return errors

        # Fallback: create a single error from the output
        lines = output.strip().split("\n")
        error_lines = [line.strip() for line in lines if "error" in line.lower()][:5]
        if error_lines:
            return [ErrorInfo(
                file="",
                line=None,
                column=None,
                code=None,
                message="\n".join(error_lines),
                severity="error",
            )]

        return []


class TestGate(Gate):
    """Test gate supporting pytest, jest, npm test."""

    # File extensions that trigger test runs
    FILE_EXTENSIONS: dict[ProjectType, list[str]] = {
        ProjectType.PYTHON: [".py"],
        ProjectType.NODEJS: [".ts", ".tsx", ".js", ".jsx"],
        ProjectType.RUST: [".rs"],
        ProjectType.GO: [".go"],
    }

    COMMANDS: dict[ProjectType, list[str]] = {
        ProjectType.NODEJS: ["npm", "test"],
        ProjectType.PYTHON: ["pytest", "-v"],
        ProjectType.RUST: ["cargo", "test"],
        ProjectType.GO: ["go", "test", "./..."],
    }

    FALLBACK_COMMANDS: dict[ProjectType, list[list[str]]] = {
        ProjectType.NODEJS: [
            ["npx", "jest"],
            ["yarn", "test"],
        ],
        ProjectType.PYTHON: [
            ["python", "-m", "pytest", "-v"],
        ],
    }

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute tests."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)
        checked_files: list[str] | None = None
        test_files: list[str] | None = None

        # Check for incremental mode
        changed_files = self._get_changed_files_for_gate(project_type, context)
        if changed_files is not None:
            if not changed_files:
                # No relevant files changed, skip the gate
                return self._create_skipped_output(
                    "No source files changed, skipping tests",
                    checked_files=[],
                )
            checked_files = changed_files

            # For Python, try to infer related test files
            if project_type == ProjectType.PYTHON:
                detector = self._get_changed_files_detector()
                test_files = detector.get_test_files_for_changes(changed_files)

        # Get command
        if self.config.command:
            command = list([self.config.command] + self.config.args)
        else:
            command = list(self.COMMANDS.get(project_type, ["echo", "No tests configured"]))

        # In incremental mode for Python, use pytest --lf (last failed) or specific test files
        if checked_files and project_type == ProjectType.PYTHON:
            if test_files:
                # Run specific test files that correspond to changed code
                command.extend(test_files)
            else:
                # No specific test files found, use --lf to run last failed tests
                # or run tests related to the changed modules
                if "--lf" not in command:
                    command.append("--lf")

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                fallback_cmd = list(fallback)
                # Apply incremental mode to fallback
                if checked_files and project_type == ProjectType.PYTHON:
                    if test_files:
                        fallback_cmd.extend(test_files)
                    elif "--lf" not in fallback_cmd:
                        fallback_cmd.append("--lf")

                exit_code, stdout, stderr, duration = self._run_command(fallback_cmd)
                if exit_code != -1:
                    command_str = " ".join(fallback_cmd)
                    break

        # Parse structured errors and generate summary
        structured_errors: list[ErrorInfo] = []
        error_summary = None
        if exit_code != 0:
            structured_errors = self._parse_test_failures(stdout, stderr, project_type)
            error_summary = generate_error_summary(structured_errors) if structured_errors else "Tests failed"

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.TEST,
            passed=exit_code == 0,
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration_seconds=duration,
            command=command_str,
            error_summary=error_summary,
            structured_errors=structured_errors,
            checked_files=checked_files,
        )

    def _parse_test_failures(
        self, stdout: str, stderr: str, project_type: ProjectType
    ) -> list[ErrorInfo]:
        """Parse test failures into structured ErrorInfo objects."""
        output = stdout + "\n" + stderr

        if project_type == ProjectType.PYTHON:
            # Use pytest parser
            errors = PytestParser.parse(output)
            if errors:
                return errors

        # Fallback: extract failure count and create a summary error
        patterns = {
            ProjectType.PYTHON: r"(\d+) failed",
            ProjectType.NODEJS: r"(\d+) failed|(\d+) failing",
        }

        pattern = patterns.get(project_type)
        if pattern:
            match = re.search(pattern, output, re.IGNORECASE)
            if match:
                failed = match.group(1) or (match.group(2) if len(match.groups()) > 1 else None)
                if failed:
                    return [ErrorInfo(
                        file="",
                        line=None,
                        column=None,
                        code=None,
                        message=f"{failed} test(s) failed",
                        severity="error",
                    )]

        # Generic fallback
        lines = output.strip().split("\n")
        failure_lines = [line.strip() for line in lines if "fail" in line.lower() or "error" in line.lower()][:5]
        if failure_lines:
            return [ErrorInfo(
                file="",
                line=None,
                column=None,
                code=None,
                message=line,
                severity="error",
            ) for line in failure_lines]

        return []


class LintGate(Gate):
    """Lint gate supporting eslint, ruff, clippy."""

    # File extensions handled by this gate type
    FILE_EXTENSIONS: dict[ProjectType, list[str]] = {
        ProjectType.PYTHON: [".py"],
        ProjectType.NODEJS: [".ts", ".tsx", ".js", ".jsx"],
        ProjectType.RUST: [".rs"],
        ProjectType.GO: [".go"],
    }

    # Commands by project type - prefer JSON output format
    COMMANDS: dict[ProjectType, list[str]] = {
        ProjectType.NODEJS: ["npx", "eslint", "--format=json", "."],
        ProjectType.PYTHON: ["ruff", "check", "--output-format=json", "."],
        ProjectType.RUST: ["cargo", "clippy"],
        ProjectType.GO: ["golangci-lint", "run"],
    }

    # Fallback commands (some with JSON, some without)
    FALLBACK_COMMANDS: dict[ProjectType, list[list[str]]] = {
        ProjectType.PYTHON: [
            ["ruff", "check", "."],  # fallback to text format
            ["python", "-m", "ruff", "check", "--output-format=json", "."],
            ["python", "-m", "ruff", "check", "."],
            ["flake8", "."],
        ],
        ProjectType.NODEJS: [
            ["npx", "eslint", "."],  # fallback to text format
            ["./node_modules/.bin/eslint", "--format=json", "."],
            ["./node_modules/.bin/eslint", "."],
        ],
    }

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute linting."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)
        use_json = True  # Track if we're using JSON output format
        checked_files: list[str] | None = None

        # Check for incremental mode
        changed_files = self._get_changed_files_for_gate(project_type, context)
        if changed_files is not None:
            if not changed_files:
                # No relevant files changed, skip the gate
                return self._create_skipped_output(
                    "No lintable files changed, skipping lint",
                    checked_files=[],
                )
            checked_files = changed_files

        # Get command
        if self.config.command:
            command = list([self.config.command] + self.config.args)
            use_json = "--output-format=json" in self.config.args or "--format=json" in self.config.args
        else:
            command = list(self.COMMANDS.get(project_type, ["echo", "No linter configured"]))

        # In incremental mode, replace "." with specific files
        if checked_files and project_type in [ProjectType.PYTHON, ProjectType.NODEJS]:
            if "." in command:
                command.remove(".")
            command.extend(checked_files)

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                fallback_cmd = list(fallback)
                # Apply incremental mode to fallback
                if checked_files and project_type in [ProjectType.PYTHON, ProjectType.NODEJS]:
                    if "." in fallback_cmd:
                        fallback_cmd.remove(".")
                    fallback_cmd.extend(checked_files)

                exit_code, stdout, stderr, duration = self._run_command(fallback_cmd)
                if exit_code != -1:
                    command_str = " ".join(fallback_cmd)
                    use_json = "--output-format=json" in fallback or "--format=json" in fallback
                    break

        # Parse structured errors and generate summary
        structured_errors: list[ErrorInfo] = []
        error_summary = None
        if exit_code != 0:
            structured_errors = self._parse_lint_errors(stdout, stderr, project_type, use_json)
            error_summary = generate_error_summary(structured_errors)

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.LINT,
            passed=exit_code == 0,
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration_seconds=duration,
            command=command_str,
            error_summary=error_summary,
            structured_errors=structured_errors,
            checked_files=checked_files,
        )

    def _parse_lint_errors(
        self, stdout: str, stderr: str, project_type: ProjectType, use_json: bool = True
    ) -> list[ErrorInfo]:
        """Parse lint errors into structured ErrorInfo objects."""
        output = stdout + "\n" + stderr

        if project_type == ProjectType.PYTHON:
            # Try ruff parser first
            errors = RuffParser.parse(output)
            if errors:
                return errors
            # Try flake8 parser
            errors = FlakeParser.parse(output)
            if errors:
                return errors

        elif project_type == ProjectType.NODEJS:
            # Parse ESLint output
            errors = EslintParser.parse(output)
            if errors:
                return errors

        # Fallback: create errors from non-empty lines
        lines = output.strip().split("\n")
        error_lines = [line.strip() for line in lines if line.strip() and not line.startswith(" ")][:5]
        if error_lines:
            return [ErrorInfo(
                file="",
                line=None,
                column=None,
                code=None,
                message=line,
                severity="error",
            ) for line in error_lines]

        return []


class CustomGate(Gate):
    """Custom gate for user-defined scripts."""

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """Execute custom command."""
        if not self.config.command:
            return GateOutput(
                gate_name=self.config.name,
                gate_type=GateType.CUSTOM,
                passed=False,
                exit_code=-1,
                stdout="",
                stderr="No command specified for custom gate",
                duration_seconds=0,
                command="",
                error_summary="No command specified",
            )

        command = [self.config.command] + self.config.args
        command_str = " ".join(command)

        # Substitute variables
        command = [
            arg.replace("{story_id}", story_id)
            .replace("{project_root}", str(self.project_root))
            for arg in command
        ]

        exit_code, stdout, stderr, duration = self._run_command(command)

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.CUSTOM,
            passed=exit_code == 0,
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration_seconds=duration,
            command=command_str,
            error_summary=stderr[:500] if exit_code != 0 and stderr else None,
        )


class QualityGate:
    """
    Main quality gate manager.

    Executes quality gates after story completion to verify code quality.
    Supports auto-detection of project type and appropriate tools.
    """

    GATE_CLASSES: dict[GateType, type] = {
        GateType.TYPECHECK: TypeCheckGate,
        GateType.TEST: TestGate,
        GateType.LINT: LintGate,
        GateType.CUSTOM: CustomGate,
    }

    # Common subdirectory names for mixed projects
    SUBDIR_NAMES = ["frontend", "backend", "web", "api", "server", "client", "app", "src"]

    # Project file to type mapping
    PROJECT_FILE_CHECKS = [
        ("package.json", ProjectType.NODEJS),
        ("pyproject.toml", ProjectType.PYTHON),
        ("setup.py", ProjectType.PYTHON),
        ("requirements.txt", ProjectType.PYTHON),
        ("Cargo.toml", ProjectType.RUST),
        ("go.mod", ProjectType.GO),
    ]

    def __init__(
        self,
        project_root: Path,
        gates: list[GateConfig] | None = None,
        fail_fast: bool = False,
        use_cache: bool = False,
        cache_ttl: int | None = None,
    ):
        """
        Initialize quality gate manager.

        Args:
            project_root: Root directory of the project
            gates: List of gate configurations
            fail_fast: If True, stop execution when a required gate fails
            use_cache: If True, cache gate results and reuse when project state unchanged
            cache_ttl: Time-to-live for cache entries in seconds (None for no expiration)
        """
        self.project_root = Path(project_root)
        self.gates = gates or []
        self.fail_fast = fail_fast
        self.use_cache = use_cache
        self.cache_ttl = cache_ttl
        self._project_type: ProjectType | None = None
        self._project_types: list[ProjectType] | None = None
        self._cache: GateCache | None = None

    @property
    def cache(self) -> "GateCache":
        """
        Get the gate cache (lazily initialized).

        Returns:
            GateCache instance for caching gate results
        """
        if self._cache is None:
            from .gate_cache import GateCache

            self._cache = GateCache(self.project_root)
        return self._cache

    def invalidate_cache(self, gate_name: str | None = None) -> None:
        """
        Manually invalidate the cache.

        Args:
            gate_name: Specific gate to invalidate, or None to clear all
        """
        if self._cache is not None:
            self._cache.invalidate(gate_name)

    def detect_project_types(self) -> list[ProjectType]:
        """
        Auto-detect all project types from configuration files.

        Scans both the project root and common subdirectories (frontend/, backend/, etc.)
        to detect mixed projects like Python backend + Node.js frontend.

        Returns:
            List of detected project types (may be empty or contain multiple types)
        """
        if self._project_types is not None:
            return self._project_types

        detected: set[ProjectType] = set()

        # Check root directory
        for filename, project_type in self.PROJECT_FILE_CHECKS:
            if (self.project_root / filename).exists():
                detected.add(project_type)

        # Check common subdirectories
        for subdir_name in self.SUBDIR_NAMES:
            subdir = self.project_root / subdir_name
            if subdir.is_dir():
                for filename, project_type in self.PROJECT_FILE_CHECKS:
                    if (subdir / filename).exists():
                        detected.add(project_type)

        # Convert to sorted list for consistent ordering
        # Order: NODEJS, PYTHON, RUST, GO, UNKNOWN
        type_order = [ProjectType.NODEJS, ProjectType.PYTHON, ProjectType.RUST, ProjectType.GO]
        self._project_types = [t for t in type_order if t in detected]

        # If nothing detected, return UNKNOWN
        if not self._project_types:
            self._project_types = [ProjectType.UNKNOWN]

        return self._project_types

    def detect_project_type(self) -> ProjectType:
        """
        Auto-detect primary project type from configuration files.

        This is a backward-compatible method that returns the first (primary) detected type.
        For mixed project detection, use detect_project_types() instead.

        Returns:
            The primary detected project type
        """
        if self._project_type is not None:
            return self._project_type

        types = self.detect_project_types()
        self._project_type = types[0] if types else ProjectType.UNKNOWN
        return self._project_type

    def execute_all(
        self,
        story_id: str,
        context: dict[str, Any] | None = None,
    ) -> dict[str, GateOutput]:
        """
        Execute all enabled gates.

        If fail_fast is enabled, stops execution when a required gate fails
        and marks remaining gates as skipped.

        If use_cache is enabled, checks cache before executing each gate
        and returns cached results when project state hasn't changed.

        Args:
            story_id: ID of the story being verified
            context: Additional context for gates

        Returns:
            Dictionary mapping gate names to outputs
        """
        context = context or {}
        context["project_type"] = self.detect_project_type()

        results: dict[str, GateOutput] = {}
        should_skip_remaining = False

        # Collect enabled gates for potential skipping
        enabled_gates = [g for g in self.gates if g.enabled and self.GATE_CLASSES.get(g.type)]

        for gate_config in enabled_gates:
            gate_class = self.GATE_CLASSES.get(gate_config.type)
            if not gate_class:
                continue

            if should_skip_remaining:
                # Create skipped output for this gate
                results[gate_config.name] = GateOutput(
                    gate_name=gate_config.name,
                    gate_type=gate_config.type,
                    passed=False,
                    exit_code=-1,
                    stdout="",
                    stderr="Skipped due to fail_fast: previous required gate failed",
                    duration_seconds=0.0,
                    command="",
                    error_summary="Skipped due to fail_fast",
                    skipped=True,
                )
                continue

            # Check cache if enabled
            if self.use_cache:
                cached_result = self.cache.get(gate_config.name)
                if cached_result is not None:
                    results[gate_config.name] = cached_result
                    # Check if cached failure should trigger fail_fast
                    if self.fail_fast and gate_config.required and not cached_result.passed:
                        should_skip_remaining = True
                    continue

            gate = gate_class(gate_config, self.project_root)
            result = gate.execute(story_id, context)
            results[gate_config.name] = result

            # Cache the result if caching is enabled
            if self.use_cache:
                self.cache.set(gate_config.name, result, self.cache_ttl)

            # Check if we should stop on this failure
            if self.fail_fast and gate_config.required and not result.passed:
                should_skip_remaining = True

        return results

    async def execute_all_async(
        self,
        story_id: str,
        context: dict[str, Any] | None = None,
    ) -> dict[str, GateOutput]:
        """
        Execute all enabled gates in parallel.

        Uses asyncio.gather() to run all gates concurrently. Each gate's
        execute_async() method wraps the synchronous subprocess call with
        asyncio.to_thread() for true parallel execution.

        The parallel execution time should be approximately equal to the
        slowest gate rather than the sum of all gate execution times.

        If fail_fast is enabled, when a required gate fails, all other
        running gates are cancelled and marked as skipped.

        If use_cache is enabled, checks cache before executing each gate
        and returns cached results when project state hasn't changed.

        Args:
            story_id: ID of the story being verified
            context: Additional context for gates

        Returns:
            Dictionary mapping gate names to outputs
        """
        context = context or {}
        context["project_type"] = self.detect_project_type()

        results: dict[str, GateOutput] = {}

        # Collect gates to execute, checking cache first
        gates_to_run: list[tuple[str, Gate, GateConfig]] = []
        for gate_config in self.gates:
            if not gate_config.enabled:
                continue

            gate_class = self.GATE_CLASSES.get(gate_config.type)
            if not gate_class:
                continue

            # Check cache if enabled
            if self.use_cache:
                cached_result = self.cache.get(gate_config.name)
                if cached_result is not None:
                    results[gate_config.name] = cached_result
                    continue

            gate = gate_class(gate_config, self.project_root)
            gates_to_run.append((gate_config.name, gate, gate_config))

        # If all results were cached, return immediately
        if not gates_to_run:
            return results

        # Check if any cached result is a required failure (for fail_fast)
        if self.fail_fast:
            for gate_config in self.gates:
                if not gate_config.enabled or not gate_config.required:
                    continue
                cached = results.get(gate_config.name)
                if cached and not cached.passed:
                    # Required gate failed in cache, skip all remaining
                    for name, _, gc in gates_to_run:
                        results[name] = GateOutput(
                            gate_name=name,
                            gate_type=gc.type,
                            passed=False,
                            exit_code=-1,
                            stdout="",
                            stderr=f"Skipped due to fail_fast: '{gate_config.name}' failed (cached)",
                            duration_seconds=0.0,
                            command="",
                            error_summary=f"Skipped due to fail_fast: '{gate_config.name}' failed (cached)",
                            skipped=True,
                        )
                    return results

        if not self.fail_fast:
            # Original parallel execution without fail_fast
            async def run_gate(name: str, gate: Gate, config: GateConfig) -> tuple[str, GateOutput, GateConfig]:
                output = await gate.execute_async(story_id, context)
                return (name, output, config)

            tasks = [run_gate(name, gate, config) for name, gate, config in gates_to_run]
            results_list = await asyncio.gather(*tasks)

            for name, output, config in results_list:
                results[name] = output
                # Cache the result if caching is enabled
                if self.use_cache:
                    self.cache.set(name, output, self.cache_ttl)

            return results

        # Fail-fast parallel execution with task cancellation
        pending_tasks: dict[asyncio.Task[GateOutput], tuple[str, GateConfig]] = {}
        failure_occurred = False
        failing_gate_name: str | None = None

        # Create tasks for all gates
        for name, gate, gate_config in gates_to_run:
            task = asyncio.create_task(gate.execute_async(story_id, context))
            pending_tasks[task] = (name, gate_config)

        # Process tasks as they complete
        while pending_tasks:
            # Wait for the first task to complete
            done, _ = await asyncio.wait(
                pending_tasks.keys(),
                return_when=asyncio.FIRST_COMPLETED
            )

            for task in done:
                name, gate_config = pending_tasks.pop(task)

                try:
                    output = task.result()
                    results[name] = output

                    # Cache the result if caching is enabled
                    if self.use_cache:
                        self.cache.set(name, output, self.cache_ttl)

                    # Check if this is a required gate failure
                    if gate_config.required and not output.passed and not failure_occurred:
                        failure_occurred = True
                        failing_gate_name = name

                        # Cancel all remaining tasks
                        for remaining_task in list(pending_tasks.keys()):
                            remaining_task.cancel()

                except asyncio.CancelledError:
                    # Task was cancelled due to fail_fast
                    results[name] = GateOutput(
                        gate_name=name,
                        gate_type=gate_config.type,
                        passed=False,
                        exit_code=-1,
                        stdout="",
                        stderr=f"Skipped due to fail_fast: '{failing_gate_name}' failed",
                        duration_seconds=0.0,
                        command="",
                        error_summary=f"Skipped due to fail_fast: '{failing_gate_name}' failed",
                        skipped=True,
                    )

        return results

    def should_allow_progression(self, outputs: dict[str, GateOutput]) -> bool:
        """
        Check if all required gates passed.

        Args:
            outputs: Results from execute_all

        Returns:
            True if all required gates passed
        """
        for gate_config in self.gates:
            if not gate_config.enabled:
                continue

            if not gate_config.required:
                continue

            output = outputs.get(gate_config.name)
            if output and not output.passed:
                return False

        return True

    def get_failure_summary(self, outputs: dict[str, GateOutput]) -> str | None:
        """
        Get a summary of all gate failures.

        Uses structured errors when available for more detailed summaries,
        falling back to error_summary string if no structured errors.
        Skipped gates are reported separately from actual failures.
        """
        failures = []
        skipped = []

        for gate_config in self.gates:
            output = outputs.get(gate_config.name)
            if output and not output.passed:
                required = " (required)" if gate_config.required else " (optional)"

                if output.skipped:
                    # Track skipped gates separately
                    skipped.append(f"- {gate_config.name}{required}: Skipped")
                else:
                    # Use structured errors if available for better summary
                    if output.structured_errors:
                        summary = generate_error_summary(output.structured_errors, max_errors=3)
                    else:
                        summary = output.error_summary or f"{gate_config.name} failed"

                    failures.append(f"- {gate_config.name}{required}:\n  {summary.replace(chr(10), chr(10) + '  ')}")

        result_parts = []
        if failures:
            result_parts.append("Quality gate failures:\n" + "\n".join(failures))
        if skipped:
            result_parts.append("Skipped gates (due to fail_fast):\n" + "\n".join(skipped))

        if result_parts:
            return "\n\n".join(result_parts)
        return None

    @classmethod
    def from_prd(cls, project_root: Path, prd: dict[str, Any]) -> "QualityGate":
        """
        Create QualityGate from PRD configuration.

        Args:
            project_root: Root directory of the project
            prd: PRD dictionary with quality_gates section

        Returns:
            Configured QualityGate instance
        """
        quality_gates_config = prd.get("quality_gates", {})

        if not quality_gates_config.get("enabled", True):
            return cls(project_root, gates=[])

        gates_data = quality_gates_config.get("gates", [])
        gates = [GateConfig.from_dict(g) for g in gates_data]
        fail_fast = quality_gates_config.get("fail_fast", False)
        use_cache = quality_gates_config.get("use_cache", False)
        cache_ttl = quality_gates_config.get("cache_ttl")

        return cls(
            project_root,
            gates=gates,
            fail_fast=fail_fast,
            use_cache=use_cache,
            cache_ttl=cache_ttl,
        )

    @classmethod
    def create_default(
        cls,
        project_root: Path,
        fail_fast: bool = False,
        use_cache: bool = False,
        cache_ttl: int | None = None,
    ) -> "QualityGate":
        """
        Create QualityGate with default gates based on all detected project types.

        For mixed projects (e.g., Python backend + Node.js frontend), this creates
        gates for each detected project type, with appropriate naming to distinguish
        them (e.g., "typecheck-python", "typecheck-nodejs").

        Args:
            project_root: Root directory of the project
            fail_fast: If True, stop execution when a required gate fails
            use_cache: If True, cache gate results and reuse when project state unchanged
            cache_ttl: Time-to-live for cache entries in seconds (None for no expiration)

        Returns:
            QualityGate with auto-detected default gates for all project types
        """
        instance = cls(project_root, fail_fast=fail_fast, use_cache=use_cache, cache_ttl=cache_ttl)
        project_types = instance.detect_project_types()

        default_gates: list[GateConfig] = []
        is_mixed = len(project_types) > 1 and ProjectType.UNKNOWN not in project_types

        for project_type in project_types:
            if project_type == ProjectType.UNKNOWN:
                continue

            # Generate suffix for mixed projects
            suffix = f"-{project_type.value}" if is_mixed else ""

            # Add typecheck gate for types that support it
            if project_type in [ProjectType.NODEJS, ProjectType.PYTHON]:
                default_gates.append(GateConfig(
                    name=f"typecheck{suffix}",
                    type=GateType.TYPECHECK,
                    enabled=True,
                    required=True,
                    project_type=project_type,
                ))

            # Add test gate
            default_gates.append(GateConfig(
                name=f"tests{suffix}",
                type=GateType.TEST,
                enabled=True,
                required=True,
                project_type=project_type,
            ))

            # Add lint gate
            default_gates.append(GateConfig(
                name=f"lint{suffix}",
                type=GateType.LINT,
                enabled=True,
                required=False,  # Lint is optional by default
                project_type=project_type,
            ))

        # If no types detected (only UNKNOWN), add generic gates
        if not default_gates:
            default_gates.append(GateConfig(
                name="tests",
                type=GateType.TEST,
                enabled=True,
                required=True,
            ))
            default_gates.append(GateConfig(
                name="lint",
                type=GateType.LINT,
                enabled=True,
                required=False,
            ))

        instance.gates = default_gates
        return instance

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for PRD serialization."""
        result = {
            "enabled": len(self.gates) > 0,
            "fail_fast": self.fail_fast,
            "gates": [g.to_dict() for g in self.gates],
        }
        if self.use_cache:
            result["use_cache"] = self.use_cache
        if self.cache_ttl is not None:
            result["cache_ttl"] = self.cache_ttl
        return result
