"""Tests for error parser module."""

import json
import pytest

from plan_cascade.core.error_parser import (
    ErrorInfo,
    MypyParser,
    RuffParser,
    PytestParser,
    TscParser,
    EslintParser,
    FlakeParser,
    PyrightParser,
    generate_error_summary,
)


class TestErrorInfo:
    """Tests for ErrorInfo dataclass."""

    def test_init(self):
        """Test ErrorInfo initialization."""
        error = ErrorInfo(
            file="src/main.py",
            line=10,
            column=5,
            code="E501",
            message="Line too long",
            severity="error",
        )

        assert error.file == "src/main.py"
        assert error.line == 10
        assert error.column == 5
        assert error.code == "E501"
        assert error.message == "Line too long"
        assert error.severity == "error"

    def test_to_dict(self):
        """Test converting to dictionary."""
        error = ErrorInfo(
            file="test.py",
            line=1,
            column=None,
            code="W001",
            message="Warning",
            severity="warning",
        )

        result = error.to_dict()

        assert result["file"] == "test.py"
        assert result["line"] == 1
        assert result["column"] is None
        assert result["code"] == "W001"
        assert result["severity"] == "warning"

    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "file": "app.py",
            "line": 42,
            "column": 10,
            "code": "E001",
            "message": "Error message",
            "severity": "error",
        }

        error = ErrorInfo.from_dict(data)

        assert error.file == "app.py"
        assert error.line == 42
        assert error.column == 10

    def test_format_location(self):
        """Test location formatting."""
        error = ErrorInfo(
            file="test.py",
            line=10,
            column=5,
            code=None,
            message="test",
        )

        assert error.format_location() == "test.py:10:5"

        error_no_column = ErrorInfo(
            file="test.py",
            line=10,
            column=None,
            code=None,
            message="test",
        )

        assert error_no_column.format_location() == "test.py:10"

        error_no_line = ErrorInfo(
            file="test.py",
            line=None,
            column=None,
            code=None,
            message="test",
        )

        assert error_no_line.format_location() == "test.py"

    def test_format_short(self):
        """Test short format."""
        error = ErrorInfo(
            file="test.py",
            line=10,
            column=5,
            code="E001",
            message="Error message",
        )

        result = error.format_short()
        assert "test.py:10:5" in result
        assert "[E001]" in result
        assert "Error message" in result


class TestMypyParser:
    """Tests for MypyParser."""

    def test_parse_json_format(self):
        """Test parsing mypy JSON output."""
        output = """{"file": "src/main.py", "line": 10, "column": 5, "code": "arg-type", "message": "Argument 1 has incompatible type", "severity": "error"}
{"file": "src/utils.py", "line": 20, "column": 10, "code": "return-value", "message": "Missing return statement", "severity": "error"}"""

        errors = MypyParser.parse_json(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].code == "arg-type"
        assert errors[1].file == "src/utils.py"
        assert errors[1].line == 20

    def test_parse_text_format(self):
        """Test parsing mypy text output."""
        output = """src/main.py:10:5: error: Argument 1 has incompatible type [arg-type]
src/utils.py:20: error: Missing return statement
src/app.py:30:15: note: See documentation"""

        errors = MypyParser.parse_text(output)

        assert len(errors) == 3
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].severity == "error"
        assert errors[0].code == "arg-type"
        assert errors[1].file == "src/utils.py"
        assert errors[1].line == 20
        assert errors[1].column is None
        assert errors[2].severity == "note"

    def test_parse_auto_detects_format(self):
        """Test that parse() auto-detects JSON vs text."""
        json_output = '{"file": "test.py", "line": 1, "message": "error", "severity": "error"}'
        text_output = "test.py:1: error: Some error"

        json_errors = MypyParser.parse(json_output)
        text_errors = MypyParser.parse(text_output)

        assert len(json_errors) == 1
        assert len(text_errors) == 1


class TestRuffParser:
    """Tests for RuffParser."""

    def test_parse_json_format(self):
        """Test parsing ruff JSON output."""
        output = json.dumps([
            {
                "filename": "src/main.py",
                "location": {"row": 10, "column": 5},
                "code": "E501",
                "message": "Line too long",
            },
            {
                "filename": "src/utils.py",
                "location": {"row": 20, "column": 1},
                "code": "F401",
                "message": "Unused import",
            },
        ])

        errors = RuffParser.parse_json(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].code == "E501"
        assert errors[1].code == "F401"

    def test_parse_text_format(self):
        """Test parsing ruff text output."""
        output = """src/main.py:10:5: E501 Line too long (120 > 100)
src/utils.py:20:1: F401 Unused import"""

        errors = RuffParser.parse_text(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].code == "E501"

    def test_parse_auto_detects_format(self):
        """Test that parse() auto-detects JSON vs text."""
        json_output = '[{"filename": "test.py", "location": {"row": 1, "column": 1}, "code": "E501", "message": "error"}]'
        text_output = "test.py:1:1: E501 Some error"

        json_errors = RuffParser.parse(json_output)
        text_errors = RuffParser.parse(text_output)

        assert len(json_errors) == 1
        assert len(text_errors) == 1


class TestPytestParser:
    """Tests for PytestParser."""

    def test_parse_failed_tests(self):
        """Test parsing pytest FAILED output."""
        output = """FAILED tests/test_main.py::test_function - AssertionError: assert 1 == 2
FAILED tests/test_utils.py::test_helper - ValueError: invalid input"""

        errors = PytestParser.parse(output)

        assert len(errors) == 2
        assert errors[0].file == "tests/test_main.py"
        assert "test_function" in errors[0].message
        assert errors[1].file == "tests/test_utils.py"
        assert "test_helper" in errors[1].message

    def test_parse_short_summary(self):
        """Test parsing pytest short summary format."""
        output = """= short test summary info =
FAILED tests/test_main.py::test_one
ERROR tests/test_utils.py::test_two"""

        errors = PytestParser.parse(output)

        assert len(errors) == 2
        assert errors[0].file == "tests/test_main.py"
        assert errors[1].file == "tests/test_utils.py"

    def test_no_duplicates(self):
        """Test that parser doesn't create duplicate errors."""
        output = """FAILED tests/test_main.py::test_func - error
FAILED tests/test_main.py::test_func - error again"""

        errors = PytestParser.parse(output)

        # Should deduplicate based on (file, test_name)
        assert len(errors) == 1


class TestTscParser:
    """Tests for TscParser."""

    def test_parse_parentheses_format(self):
        """Test parsing tsc output with parentheses format."""
        output = """src/main.ts(10,5): error TS2345: Argument of type 'string' is not assignable
src/utils.ts(20,10): warning TS6133: Variable is declared but not used"""

        errors = TscParser.parse(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.ts"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].code == "TS2345"
        assert errors[0].severity == "error"
        assert errors[1].severity == "warning"

    def test_parse_colon_format(self):
        """Test parsing tsc output with colon format."""
        output = """src/main.ts:10:5 - error TS2345: Type error message"""

        errors = TscParser.parse(output)

        assert len(errors) == 1
        assert errors[0].file == "src/main.ts"
        assert errors[0].line == 10
        assert errors[0].column == 5


class TestEslintParser:
    """Tests for EslintParser."""

    def test_parse_json_format(self):
        """Test parsing eslint JSON output."""
        output = json.dumps([
            {
                "filePath": "/src/main.js",
                "messages": [
                    {
                        "line": 10,
                        "column": 5,
                        "severity": 2,
                        "ruleId": "no-unused-vars",
                        "message": "Unused variable",
                    },
                    {
                        "line": 20,
                        "column": 1,
                        "severity": 1,
                        "ruleId": "prefer-const",
                        "message": "Use const",
                    },
                ],
            },
        ])

        errors = EslintParser.parse_json(output)

        assert len(errors) == 2
        assert errors[0].file == "/src/main.js"
        assert errors[0].line == 10
        assert errors[0].code == "no-unused-vars"
        assert errors[0].severity == "error"  # severity 2 = error
        assert errors[1].severity == "warning"  # severity 1 = warning

    def test_parse_text_format(self):
        """Test parsing eslint text output."""
        output = """/src/main.js
   10:5  error  Unused variable  no-unused-vars
   20:1  warning  Use const  prefer-const"""

        errors = EslintParser.parse_text(output)

        assert len(errors) == 2
        assert errors[0].file == "/src/main.js"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].severity == "error"


class TestFlakeParser:
    """Tests for FlakeParser."""

    def test_parse_output(self):
        """Test parsing flake8 output."""
        output = """src/main.py:10:5: E501 line too long (120 > 100)
src/utils.py:20:1: F401 'os' imported but unused"""

        errors = FlakeParser.parse(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].code == "E501"
        assert errors[1].code == "F401"


class TestPyrightParser:
    """Tests for PyrightParser."""

    def test_parse_json_format(self):
        """Test parsing pyright JSON output."""
        output = json.dumps({
            "generalDiagnostics": [
                {
                    "file": "src/main.py",
                    "severity": "error",
                    "message": "Type error",
                    "rule": "reportGeneralTypeIssues",
                    "range": {
                        "start": {"line": 10, "character": 5},
                    },
                },
            ],
        })

        errors = PyrightParser.parse_json(output)

        assert len(errors) == 1
        assert errors[0].file == "src/main.py"
        assert errors[0].line == 10
        assert errors[0].column == 5
        assert errors[0].code == "reportGeneralTypeIssues"

    def test_parse_text_format(self):
        """Test parsing pyright text output."""
        output = """  src/main.py:10:5 - error: Type mismatch
  src/utils.py:20:1 - warning: Unused variable"""

        errors = PyrightParser.parse_text(output)

        assert len(errors) == 2
        assert errors[0].file == "src/main.py"
        assert errors[0].severity == "error"
        assert errors[1].severity == "warning"


class TestGenerateErrorSummary:
    """Tests for generate_error_summary function."""

    def test_empty_errors(self):
        """Test with no errors."""
        result = generate_error_summary([])
        assert result == "No errors found"

    def test_single_error(self):
        """Test with single error."""
        errors = [
            ErrorInfo(
                file="test.py",
                line=10,
                column=5,
                code="E001",
                message="Error message",
                severity="error",
            ),
        ]

        result = generate_error_summary(errors)

        assert "1 error(s)" in result
        assert "test.py:10:5" in result
        assert "[E001]" in result

    def test_multiple_errors(self):
        """Test with multiple errors."""
        errors = [
            ErrorInfo(file="a.py", line=1, column=1, code="E001", message="Error 1", severity="error"),
            ErrorInfo(file="b.py", line=2, column=2, code="E002", message="Error 2", severity="error"),
            ErrorInfo(file="c.py", line=3, column=3, code="W001", message="Warning 1", severity="warning"),
        ]

        result = generate_error_summary(errors)

        assert "2 error(s)" in result
        assert "1 warning(s)" in result

    def test_max_errors_limit(self):
        """Test that max_errors limits output."""
        errors = [
            ErrorInfo(file=f"test{i}.py", line=i, column=1, code="E001", message=f"Error {i}", severity="error")
            for i in range(10)
        ]

        result = generate_error_summary(errors, max_errors=3)

        # Should show only 3 detailed errors
        assert "... and 7 more" in result

    def test_errors_sorted_by_severity(self):
        """Test that errors are sorted with errors before warnings."""
        errors = [
            ErrorInfo(file="w.py", line=1, column=1, code="W001", message="Warning", severity="warning"),
            ErrorInfo(file="e.py", line=1, column=1, code="E001", message="Error", severity="error"),
        ]

        result = generate_error_summary(errors)

        # Find the detailed error lines (after the header)
        lines = result.split("\n")
        detail_lines = [line for line in lines if line.strip().startswith("-")]

        # Error should appear before warning in the detail lines
        assert len(detail_lines) == 2
        assert "e.py" in detail_lines[0]
        assert "w.py" in detail_lines[1]
