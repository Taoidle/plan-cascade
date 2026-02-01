"""
Error Parser Module for Plan Cascade

Provides structured error parsing for various quality gate tools.
Supports JSON and text output formats from mypy, pytest, ruff, eslint, tsc.
"""

import json
import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class ErrorSeverity(Enum):
    """Severity levels for errors."""
    ERROR = "error"
    WARNING = "warning"
    INFO = "info"
    NOTE = "note"


@dataclass
class ErrorInfo:
    """Structured error information from tool output."""
    file: str
    line: int | None
    column: int | None
    code: str | None
    message: str
    severity: str = "error"  # "error", "warning", "info", "note"

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "file": self.file,
            "line": self.line,
            "column": self.column,
            "code": self.code,
            "message": self.message,
            "severity": self.severity,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ErrorInfo":
        """Create from dictionary."""
        return cls(
            file=data.get("file", ""),
            line=data.get("line"),
            column=data.get("column"),
            code=data.get("code"),
            message=data.get("message", ""),
            severity=data.get("severity", "error"),
        )

    def format_location(self) -> str:
        """Format file location as 'file:line:column'."""
        parts = [self.file]
        if self.line is not None:
            parts.append(str(self.line))
            if self.column is not None:
                parts.append(str(self.column))
        return ":".join(parts)

    def format_short(self) -> str:
        """Format as short single-line summary."""
        location = self.format_location()
        code_part = f"[{self.code}] " if self.code else ""
        return f"{location}: {code_part}{self.message}"


class MypyParser:
    """Parser for mypy type checker output."""

    # Pattern for mypy text output: file:line:column: severity: message
    TEXT_PATTERN = re.compile(
        r"^(?P<file>[^:]+):(?P<line>\d+):(?:(?P<column>\d+):)?\s*"
        r"(?P<severity>error|warning|note):\s*(?P<message>.+?)(?:\s*\[(?P<code>[^\]]+)\])?$"
    )

    @classmethod
    def parse_json(cls, output: str) -> list[ErrorInfo]:
        """Parse mypy JSON output format (--output-format=json)."""
        errors = []

        for line in output.strip().split("\n"):
            if not line.strip():
                continue

            try:
                data = json.loads(line)
                errors.append(ErrorInfo(
                    file=data.get("file", ""),
                    line=data.get("line"),
                    column=data.get("column"),
                    code=data.get("code"),
                    message=data.get("message", ""),
                    severity=data.get("severity", "error"),
                ))
            except json.JSONDecodeError:
                # Not JSON, skip
                continue

        return errors

    @classmethod
    def parse_text(cls, output: str) -> list[ErrorInfo]:
        """Parse mypy default text output format."""
        errors = []

        for line in output.strip().split("\n"):
            match = cls.TEXT_PATTERN.match(line.strip())
            if match:
                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")) if match.group("column") else None,
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity=match.group("severity"),
                ))

        return errors

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Auto-detect format and parse mypy output."""
        # Try JSON first
        errors = cls.parse_json(output)
        if errors:
            return errors

        # Fall back to text parsing
        return cls.parse_text(output)


class RuffParser:
    """Parser for ruff linter output."""

    # Pattern for ruff text output: file:line:column: code message
    TEXT_PATTERN = re.compile(
        r"^(?P<file>[^:]+):(?P<line>\d+):(?P<column>\d+):\s*"
        r"(?P<code>[A-Z]+\d+)\s+(?P<message>.+)$"
    )

    @classmethod
    def parse_json(cls, output: str) -> list[ErrorInfo]:
        """Parse ruff JSON output format (--output-format=json)."""
        errors = []

        try:
            data = json.loads(output)
            if isinstance(data, list):
                for item in data:
                    errors.append(ErrorInfo(
                        file=item.get("filename", ""),
                        line=item.get("location", {}).get("row"),
                        column=item.get("location", {}).get("column"),
                        code=item.get("code"),
                        message=item.get("message", ""),
                        severity="error",
                    ))
        except json.JSONDecodeError:
            pass

        return errors

    @classmethod
    def parse_text(cls, output: str) -> list[ErrorInfo]:
        """Parse ruff default text output format."""
        errors = []

        for line in output.strip().split("\n"):
            match = cls.TEXT_PATTERN.match(line.strip())
            if match:
                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity="error",
                ))

        return errors

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Auto-detect format and parse ruff output."""
        # Try JSON first
        errors = cls.parse_json(output)
        if errors:
            return errors

        # Fall back to text parsing
        return cls.parse_text(output)


class PytestParser:
    """Parser for pytest test output."""

    # Pattern for pytest failure: FAILED path/to/test.py::test_name - message
    FAILED_PATTERN = re.compile(
        r"^FAILED\s+(?P<file>[^:]+)::(?P<test_name>[^\s]+)(?:\s+-\s+(?P<message>.+))?$"
    )

    # Pattern for assertion error with file/line info
    ASSERTION_PATTERN = re.compile(
        r"^(?P<file>[^:]+):(?P<line>\d+):\s*(?P<message>.+)$"
    )

    # Pattern for short test summary info
    SHORT_SUMMARY_PATTERN = re.compile(
        r"^(?P<status>FAILED|ERROR)\s+(?P<file>[^:]+)::(?P<test_name>[^\s]+)"
    )

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Parse pytest output for test failures."""
        errors = []
        seen = set()  # Track (file, test_name) to avoid duplicates

        lines = output.strip().split("\n")

        for i, line in enumerate(lines):
            line = line.strip()

            # Check for FAILED pattern
            match = cls.FAILED_PATTERN.match(line)
            if match:
                file_path = match.group("file")
                test_name = match.group("test_name")
                message = match.group("message") or f"Test {test_name} failed"

                key = (file_path, test_name)
                if key not in seen:
                    seen.add(key)
                    errors.append(ErrorInfo(
                        file=file_path,
                        line=None,
                        column=None,
                        code=None,
                        message=f"{test_name}: {message}",
                        severity="error",
                    ))
                continue

            # Check for short summary pattern
            match = cls.SHORT_SUMMARY_PATTERN.match(line)
            if match:
                file_path = match.group("file")
                test_name = match.group("test_name")
                status = match.group("status")

                key = (file_path, test_name)
                if key not in seen:
                    seen.add(key)
                    errors.append(ErrorInfo(
                        file=file_path,
                        line=None,
                        column=None,
                        code=None,
                        message=f"{test_name} {status.lower()}",
                        severity="error",
                    ))

        return errors


class TscParser:
    """Parser for TypeScript compiler (tsc) output."""

    # Pattern for tsc text output: file(line,column): error TS1234: message
    TEXT_PATTERN = re.compile(
        r"^(?P<file>[^(]+)\((?P<line>\d+),(?P<column>\d+)\):\s*"
        r"(?P<severity>error|warning)\s+(?P<code>TS\d+):\s*(?P<message>.+)$"
    )

    # Alternative pattern: file:line:column - error TS1234: message
    ALT_PATTERN = re.compile(
        r"^(?P<file>[^:]+):(?P<line>\d+):(?P<column>\d+)\s*-\s*"
        r"(?P<severity>error|warning)\s+(?P<code>TS\d+):\s*(?P<message>.+)$"
    )

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Parse tsc output for type errors."""
        errors = []

        for line in output.strip().split("\n"):
            line = line.strip()

            # Try primary pattern
            match = cls.TEXT_PATTERN.match(line)
            if match:
                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity=match.group("severity"),
                ))
                continue

            # Try alternative pattern
            match = cls.ALT_PATTERN.match(line)
            if match:
                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity=match.group("severity"),
                ))

        return errors


class EslintParser:
    """Parser for ESLint linter output."""

    # Pattern for eslint text output: file:line:column: message [rule]
    TEXT_PATTERN = re.compile(
        r"^\s*(?P<line>\d+):(?P<column>\d+)\s+(?P<severity>error|warning)\s+"
        r"(?P<message>.+?)\s+(?P<code>\S+)$"
    )

    @classmethod
    def parse_json(cls, output: str) -> list[ErrorInfo]:
        """Parse eslint JSON output format (--format=json)."""
        errors = []

        try:
            data = json.loads(output)
            if isinstance(data, list):
                for file_result in data:
                    file_path = file_result.get("filePath", "")
                    for msg in file_result.get("messages", []):
                        severity_num = msg.get("severity", 2)
                        severity = "warning" if severity_num == 1 else "error"

                        errors.append(ErrorInfo(
                            file=file_path,
                            line=msg.get("line"),
                            column=msg.get("column"),
                            code=msg.get("ruleId"),
                            message=msg.get("message", ""),
                            severity=severity,
                        ))
        except json.JSONDecodeError:
            pass

        return errors

    @classmethod
    def parse_text(cls, output: str) -> list[ErrorInfo]:
        """Parse eslint default text output format."""
        errors = []
        current_file = ""

        for line in output.strip().split("\n"):
            line_stripped = line.strip()

            # Check if this is a file path line
            if line_stripped and not line_stripped[0].isdigit() and not line_stripped.startswith("✖"):
                # This might be a file path
                if "/" in line_stripped or "\\" in line_stripped:
                    current_file = line_stripped
                continue

            # Try to match error line
            match = cls.TEXT_PATTERN.match(line)
            if match and current_file:
                errors.append(ErrorInfo(
                    file=current_file,
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity=match.group("severity"),
                ))

        return errors

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Auto-detect format and parse eslint output."""
        # Try JSON first
        errors = cls.parse_json(output)
        if errors:
            return errors

        # Fall back to text parsing
        return cls.parse_text(output)


class FlakeParser:
    """Parser for flake8 linter output."""

    # Pattern for flake8 output: file:line:column: code message
    TEXT_PATTERN = re.compile(
        r"^(?P<file>[^:]+):(?P<line>\d+):(?P<column>\d+):\s*"
        r"(?P<code>[A-Z]\d+)\s+(?P<message>.+)$"
    )

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Parse flake8 output."""
        errors = []

        for line in output.strip().split("\n"):
            match = cls.TEXT_PATTERN.match(line.strip())
            if match:
                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=match.group("code"),
                    message=match.group("message").strip(),
                    severity="error",
                ))

        return errors


class PyrightParser:
    """Parser for Pyright type checker output."""

    # Pattern for pyright text output: file:line:column - severity: message
    TEXT_PATTERN = re.compile(
        r"^\s*(?P<file>[^:]+):(?P<line>\d+):(?P<column>\d+)\s*-\s*"
        r"(?P<severity>error|warning|information):\s*(?P<message>.+)$"
    )

    @classmethod
    def parse_json(cls, output: str) -> list[ErrorInfo]:
        """Parse pyright JSON output format (--outputjson)."""
        errors = []

        try:
            data = json.loads(output)
            diagnostics = data.get("generalDiagnostics", [])

            for diag in diagnostics:
                severity_map = {
                    "error": "error",
                    "warning": "warning",
                    "information": "info",
                }
                severity = severity_map.get(diag.get("severity", "error"), "error")

                errors.append(ErrorInfo(
                    file=diag.get("file", ""),
                    line=diag.get("range", {}).get("start", {}).get("line"),
                    column=diag.get("range", {}).get("start", {}).get("character"),
                    code=diag.get("rule"),
                    message=diag.get("message", ""),
                    severity=severity,
                ))
        except json.JSONDecodeError:
            pass

        return errors

    @classmethod
    def parse_text(cls, output: str) -> list[ErrorInfo]:
        """Parse pyright text output."""
        errors = []

        for line in output.strip().split("\n"):
            match = cls.TEXT_PATTERN.match(line)
            if match:
                severity = match.group("severity")
                if severity == "information":
                    severity = "info"

                errors.append(ErrorInfo(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    column=int(match.group("column")),
                    code=None,
                    message=match.group("message").strip(),
                    severity=severity,
                ))

        return errors

    @classmethod
    def parse(cls, output: str) -> list[ErrorInfo]:
        """Auto-detect format and parse pyright output."""
        # Try JSON first
        errors = cls.parse_json(output)
        if errors:
            return errors

        # Fall back to text parsing
        return cls.parse_text(output)


def generate_error_summary(errors: list[ErrorInfo], max_errors: int = 5) -> str:
    """
    Generate a human-readable summary of errors.

    Args:
        errors: List of ErrorInfo objects
        max_errors: Maximum number of errors to show in detail

    Returns:
        Formatted error summary string
    """
    if not errors:
        return "No errors found"

    # Count by severity
    error_count = sum(1 for e in errors if e.severity == "error")
    warning_count = sum(1 for e in errors if e.severity == "warning")

    # Build summary header
    parts = []
    if error_count > 0:
        parts.append(f"{error_count} error(s)")
    if warning_count > 0:
        parts.append(f"{warning_count} warning(s)")

    header = ", ".join(parts) if parts else "Issues found"

    # Get unique files
    files = list(set(e.file for e in errors))
    if len(files) > 3:
        files_str = f" in {len(files)} files"
    elif files:
        files_str = f" in {', '.join(files[:3])}"
    else:
        files_str = ""

    # Build detailed error list
    lines = [f"{header}{files_str}:"]

    # Show errors first, then warnings
    sorted_errors = sorted(errors, key=lambda e: (0 if e.severity == "error" else 1, e.file, e.line or 0))

    for error in sorted_errors[:max_errors]:
        lines.append(f"  - {error.format_short()}")

    if len(errors) > max_errors:
        remaining = len(errors) - max_errors
        lines.append(f"  ... and {remaining} more")

    return "\n".join(lines)
