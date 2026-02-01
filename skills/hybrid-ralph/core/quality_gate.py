"""
Quality Gate System

Run verification after story completion to ensure code quality.
Supports typecheck, test, lint, and custom gates with auto-detection.
Includes automatic virtual environment detection for Python projects.
"""

import json
import os
import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


# Common virtual environment directory names
VENV_DIRS = [".venv", "venv", "env", ".env", "virtualenv", ".virtualenv"]


def detect_venv(project_root: Path) -> Optional[Path]:
    """
    Detect virtual environment in project directory.

    Checks for common venv directory names and validates they contain
    a Python interpreter.

    Args:
        project_root: Root directory of the project

    Returns:
        Path to venv directory if found, None otherwise
    """
    for venv_name in VENV_DIRS:
        venv_path = project_root / venv_name
        if venv_path.is_dir():
            # Verify it's a valid venv by checking for python executable
            if sys.platform == "win32":
                python_path = venv_path / "Scripts" / "python.exe"
            else:
                python_path = venv_path / "bin" / "python"

            if python_path.exists():
                return venv_path

    # Also check for conda environment indicator
    conda_meta = project_root / "conda-meta"
    if conda_meta.is_dir():
        return None  # Conda env, handled differently

    return None


def get_venv_bin_path(venv_path: Path) -> Path:
    """
    Get the bin/Scripts directory path for a virtual environment.

    Args:
        venv_path: Path to virtual environment root

    Returns:
        Path to bin (Unix) or Scripts (Windows) directory
    """
    if sys.platform == "win32":
        return venv_path / "Scripts"
    else:
        return venv_path / "bin"


def get_venv_python(venv_path: Path) -> Path:
    """
    Get the Python executable path for a virtual environment.

    Args:
        venv_path: Path to virtual environment root

    Returns:
        Path to python executable
    """
    bin_path = get_venv_bin_path(venv_path)
    if sys.platform == "win32":
        return bin_path / "python.exe"
    else:
        return bin_path / "python"


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
    error_summary: Optional[str] = None


@dataclass
class GateConfig:
    """Configuration for a single gate."""
    name: str
    type: GateType
    enabled: bool = True
    required: bool = True  # If True, failure blocks progression
    command: Optional[str] = None  # Custom command (for CUSTOM type)
    args: List[str] = field(default_factory=list)
    timeout_seconds: int = 300
    working_dir: Optional[str] = None
    env: Dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "name": self.name,
            "type": self.type.value,
            "enabled": self.enabled,
            "required": self.required,
            "command": self.command,
            "args": self.args,
            "timeout_seconds": self.timeout_seconds,
            "working_dir": self.working_dir,
            "env": self.env,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "GateConfig":
        """Create from dictionary."""
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
        )


class Gate(ABC):
    """Abstract base class for quality gates."""

    def __init__(self, config: GateConfig, project_root: Path):
        self.config = config
        self.project_root = project_root
        # Auto-detect virtual environment
        self._venv_path: Optional[Path] = detect_venv(project_root)
        self._venv_bin_path: Optional[Path] = (
            get_venv_bin_path(self._venv_path) if self._venv_path else None
        )
        self._venv_python: Optional[Path] = (
            get_venv_python(self._venv_path) if self._venv_path else None
        )

    @property
    def has_venv(self) -> bool:
        """Check if a virtual environment was detected."""
        return self._venv_path is not None

    @abstractmethod
    def execute(self, story_id: str, context: Dict[str, Any]) -> GateOutput:
        """Execute the gate and return results."""
        pass

    def _get_venv_command(self, command: List[str]) -> List[str]:
        """
        Adjust command to use virtual environment if available.

        For Python projects, replaces 'python' with venv python path.
        """
        if not self._venv_python or not command:
            return command

        # If command starts with 'python', use venv python
        if command[0] in ("python", "python3"):
            return [str(self._venv_python)] + command[1:]

        return command

    def _run_command(
        self,
        command: List[str],
        timeout: Optional[int] = None,
        env: Optional[Dict[str, str]] = None,
        cwd: Optional[Path] = None,
    ) -> Tuple[int, str, str, float]:
        """
        Run a command and return (exit_code, stdout, stderr, duration).

        Automatically detects and uses virtual environment if present.
        The venv bin/Scripts directory is prepended to PATH so tools
        installed in the venv are found first.
        """
        timeout = timeout or self.config.timeout_seconds
        cwd = cwd or (Path(self.config.working_dir) if self.config.working_dir else self.project_root)

        # Merge environment
        run_env = os.environ.copy()
        if self.config.env:
            run_env.update(self.config.env)
        if env:
            run_env.update(env)

        # Prepend venv bin directory to PATH if virtual environment detected
        if self._venv_bin_path and self._venv_bin_path.exists():
            current_path = run_env.get("PATH", "")
            venv_bin_str = str(self._venv_bin_path)
            # Only prepend if not already in PATH
            if venv_bin_str not in current_path:
                path_sep = ";" if sys.platform == "win32" else ":"
                run_env["PATH"] = f"{venv_bin_str}{path_sep}{current_path}"
            # Also set VIRTUAL_ENV for tools that check it
            run_env["VIRTUAL_ENV"] = str(self._venv_path)

        # Adjust command to use venv python if needed
        command = self._get_venv_command(command)

        start_time = datetime.now()

        try:
            kwargs: Dict[str, Any] = {
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

    # Commands by project type
    COMMANDS: Dict[ProjectType, List[str]] = {
        ProjectType.NODEJS: ["npx", "tsc", "--noEmit"],
        ProjectType.PYTHON: ["mypy", "."],
    }

    FALLBACK_COMMANDS: Dict[ProjectType, List[List[str]]] = {
        ProjectType.PYTHON: [
            ["pyright"],
            ["python", "-m", "mypy", "."],
        ],
    }

    def execute(self, story_id: str, context: Dict[str, Any]) -> GateOutput:
        """Execute typecheck."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)

        # Get command
        if self.config.command:
            command = [self.config.command] + self.config.args
        else:
            command = self.COMMANDS.get(project_type, ["echo", "No typecheck configured"])

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails with "not found"
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                exit_code, stdout, stderr, duration = self._run_command(fallback)
                if exit_code != -1:
                    command_str = " ".join(fallback)
                    break

        # Parse error summary
        error_summary = None
        if exit_code != 0:
            error_summary = self._parse_errors(stdout, stderr, project_type)

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
        )

    def _parse_errors(self, stdout: str, stderr: str, project_type: ProjectType) -> str:
        """Parse and summarize type errors."""
        output = stdout + "\n" + stderr
        lines = output.strip().split("\n")

        # Count errors
        error_count = 0
        error_lines = []

        for line in lines:
            if "error" in line.lower():
                error_count += 1
                if len(error_lines) < 5:  # Show first 5 errors
                    error_lines.append(line.strip())

        if error_lines:
            return f"{error_count} type error(s):\n" + "\n".join(error_lines)
        return f"Typecheck failed with exit code {self._get_last_exit_code()}"

    def _get_last_exit_code(self) -> int:
        return -1


class TestGate(Gate):
    """Test gate supporting pytest, jest, npm test."""

    COMMANDS: Dict[ProjectType, List[str]] = {
        ProjectType.NODEJS: ["npm", "test"],
        ProjectType.PYTHON: ["pytest", "-v"],
        ProjectType.RUST: ["cargo", "test"],
        ProjectType.GO: ["go", "test", "./..."],
    }

    FALLBACK_COMMANDS: Dict[ProjectType, List[List[str]]] = {
        ProjectType.NODEJS: [
            ["npx", "jest"],
            ["yarn", "test"],
        ],
        ProjectType.PYTHON: [
            ["python", "-m", "pytest", "-v"],
        ],
    }

    def execute(self, story_id: str, context: Dict[str, Any]) -> GateOutput:
        """Execute tests."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)

        # Get command
        if self.config.command:
            command = [self.config.command] + self.config.args
        else:
            command = self.COMMANDS.get(project_type, ["echo", "No tests configured"])

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                exit_code, stdout, stderr, duration = self._run_command(fallback)
                if exit_code != -1:
                    command_str = " ".join(fallback)
                    break

        # Parse error summary
        error_summary = None
        if exit_code != 0:
            error_summary = self._parse_test_failures(stdout, stderr, project_type)

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
        )

    def _parse_test_failures(self, stdout: str, stderr: str, project_type: ProjectType) -> str:
        """Parse and summarize test failures."""
        output = stdout + "\n" + stderr

        # Look for common test failure patterns
        patterns = {
            ProjectType.PYTHON: r"(\d+) failed",
            ProjectType.NODEJS: r"(\d+) failed|(\d+) failing",
        }

        import re
        pattern = patterns.get(project_type)
        if pattern:
            match = re.search(pattern, output, re.IGNORECASE)
            if match:
                failed = match.group(1) or match.group(2) if len(match.groups()) > 1 else match.group(1)
                return f"{failed} test(s) failed"

        # Generic summary
        lines = output.strip().split("\n")
        failure_lines = [l for l in lines if "fail" in l.lower() or "error" in l.lower()][:5]
        if failure_lines:
            return "Test failures:\n" + "\n".join(failure_lines)

        return "Tests failed"


class LintGate(Gate):
    """Lint gate supporting eslint, ruff, clippy."""

    COMMANDS: Dict[ProjectType, List[str]] = {
        ProjectType.NODEJS: ["npx", "eslint", "."],
        ProjectType.PYTHON: ["ruff", "check", "."],
        ProjectType.RUST: ["cargo", "clippy"],
        ProjectType.GO: ["golangci-lint", "run"],
    }

    FALLBACK_COMMANDS: Dict[ProjectType, List[List[str]]] = {
        ProjectType.PYTHON: [
            ["python", "-m", "ruff", "check", "."],
            ["flake8", "."],
        ],
        ProjectType.NODEJS: [
            ["./node_modules/.bin/eslint", "."],
        ],
    }

    def execute(self, story_id: str, context: Dict[str, Any]) -> GateOutput:
        """Execute linting."""
        project_type = context.get("project_type", ProjectType.UNKNOWN)

        # Get command
        if self.config.command:
            command = [self.config.command] + self.config.args
        else:
            command = self.COMMANDS.get(project_type, ["echo", "No linter configured"])

        command_str = " ".join(command)
        exit_code, stdout, stderr, duration = self._run_command(command)

        # Try fallbacks if primary fails
        if exit_code == -1 and "not found" in stderr.lower():
            fallbacks = self.FALLBACK_COMMANDS.get(project_type, [])
            for fallback in fallbacks:
                exit_code, stdout, stderr, duration = self._run_command(fallback)
                if exit_code != -1:
                    command_str = " ".join(fallback)
                    break

        # Parse error summary
        error_summary = None
        if exit_code != 0:
            error_summary = self._parse_lint_errors(stdout, stderr)

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
        )

    def _parse_lint_errors(self, stdout: str, stderr: str) -> str:
        """Parse and summarize lint errors."""
        output = stdout + "\n" + stderr
        lines = output.strip().split("\n")

        # Count issues
        error_lines = [l for l in lines if l.strip() and not l.startswith(" ")][:5]

        if error_lines:
            return f"Lint issues found:\n" + "\n".join(error_lines)
        return "Lint check failed"


class CustomGate(Gate):
    """Custom gate for user-defined scripts."""

    def execute(self, story_id: str, context: Dict[str, Any]) -> GateOutput:
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

    GATE_CLASSES: Dict[GateType, type] = {
        GateType.TYPECHECK: TypeCheckGate,
        GateType.TEST: TestGate,
        GateType.LINT: LintGate,
        GateType.CUSTOM: CustomGate,
    }

    def __init__(
        self,
        project_root: Path,
        gates: Optional[List[GateConfig]] = None,
    ):
        """
        Initialize quality gate manager.

        Args:
            project_root: Root directory of the project
            gates: List of gate configurations
        """
        self.project_root = Path(project_root)
        self.gates = gates or []
        self._project_type: Optional[ProjectType] = None
        # Auto-detect virtual environment
        self._venv_path: Optional[Path] = detect_venv(project_root)

    def detect_venv(self) -> Optional[Path]:
        """
        Detect virtual environment in project.

        Returns:
            Path to venv directory if found, None otherwise
        """
        return self._venv_path

    def get_venv_info(self) -> Dict[str, Any]:
        """
        Get information about detected virtual environment.

        Returns:
            Dictionary with venv information:
            - detected: bool - whether venv was found
            - path: str | None - path to venv directory
            - bin_path: str | None - path to bin/Scripts directory
            - python_path: str | None - path to python executable
        """
        if not self._venv_path:
            return {
                "detected": False,
                "path": None,
                "bin_path": None,
                "python_path": None,
            }

        bin_path = get_venv_bin_path(self._venv_path)
        python_path = get_venv_python(self._venv_path)

        return {
            "detected": True,
            "path": str(self._venv_path),
            "bin_path": str(bin_path) if bin_path.exists() else None,
            "python_path": str(python_path) if python_path.exists() else None,
        }

    def detect_project_type(self) -> ProjectType:
        """Auto-detect project type from configuration files."""
        if self._project_type is not None:
            return self._project_type

        # Check for various project files
        checks = [
            ("package.json", ProjectType.NODEJS),
            ("pyproject.toml", ProjectType.PYTHON),
            ("setup.py", ProjectType.PYTHON),
            ("requirements.txt", ProjectType.PYTHON),
            ("Cargo.toml", ProjectType.RUST),
            ("go.mod", ProjectType.GO),
        ]

        for filename, project_type in checks:
            if (self.project_root / filename).exists():
                self._project_type = project_type
                return project_type

        self._project_type = ProjectType.UNKNOWN
        return self._project_type

    def execute_all(
        self,
        story_id: str,
        context: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, GateOutput]:
        """
        Execute all enabled gates.

        Args:
            story_id: ID of the story being verified
            context: Additional context for gates

        Returns:
            Dictionary mapping gate names to outputs
        """
        context = context or {}
        context["project_type"] = self.detect_project_type()

        results: Dict[str, GateOutput] = {}

        for gate_config in self.gates:
            if not gate_config.enabled:
                continue

            gate_class = self.GATE_CLASSES.get(gate_config.type)
            if not gate_class:
                continue

            gate = gate_class(gate_config, self.project_root)
            results[gate_config.name] = gate.execute(story_id, context)

        return results

    def should_allow_progression(self, outputs: Dict[str, GateOutput]) -> bool:
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

    def get_failure_summary(self, outputs: Dict[str, GateOutput]) -> Optional[str]:
        """Get a summary of all gate failures."""
        failures = []

        for gate_config in self.gates:
            output = outputs.get(gate_config.name)
            if output and not output.passed:
                summary = output.error_summary or f"{gate_config.name} failed"
                required = " (required)" if gate_config.required else " (optional)"
                failures.append(f"- {gate_config.name}{required}: {summary}")

        if failures:
            return "Quality gate failures:\n" + "\n".join(failures)
        return None

    @classmethod
    def from_prd(cls, project_root: Path, prd: Dict[str, Any]) -> "QualityGate":
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

        return cls(project_root, gates=gates)

    @classmethod
    def create_default(cls, project_root: Path) -> "QualityGate":
        """
        Create QualityGate with default gates based on project type.

        Args:
            project_root: Root directory of the project

        Returns:
            QualityGate with auto-detected default gates
        """
        instance = cls(project_root)
        project_type = instance.detect_project_type()

        default_gates = []

        # Add typecheck gate
        if project_type in [ProjectType.NODEJS, ProjectType.PYTHON]:
            default_gates.append(GateConfig(
                name="typecheck",
                type=GateType.TYPECHECK,
                enabled=True,
                required=True,
            ))

        # Add test gate
        default_gates.append(GateConfig(
            name="tests",
            type=GateType.TEST,
            enabled=True,
            required=True,
        ))

        # Add lint gate
        default_gates.append(GateConfig(
            name="lint",
            type=GateType.LINT,
            enabled=True,
            required=False,  # Lint is optional by default
        ))

        instance.gates = default_gates
        return instance

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for PRD serialization."""
        return {
            "enabled": len(self.gates) > 0,
            "gates": [g.to_dict() for g in self.gates],
        }
