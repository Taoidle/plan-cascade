"""Tests for Spec Interview MCP Tools (mcp_server/tools/spec_tools.py).

TDD tests for the five spec interview lifecycle tools:
- spec_start
- spec_resume
- spec_submit_answers
- spec_get_status
- spec_cleanup
"""

import json
from pathlib import Path
from typing import Any, Dict
from unittest.mock import MagicMock

import pytest

# We register tools on a mock MCP server and then invoke them by name.
# This mirrors how prd_tools and execution_tools are tested in the project.


class _FakeMCP:
    """Minimal fake MCP server that captures tool registrations."""

    def __init__(self):
        self._tools: Dict[str, Any] = {}

    def tool(self):
        """Decorator that captures the function."""
        def decorator(fn):
            self._tools[fn.__name__] = fn
            return fn
        return decorator

    def call(self, name: str, **kwargs) -> Dict[str, Any]:
        """Call a registered tool by name."""
        return self._tools[name](**kwargs)


@pytest.fixture
def fake_mcp():
    return _FakeMCP()


@pytest.fixture
def project_with_spec_tools(tmp_path, fake_mcp):
    """Register spec tools on a fake MCP and return (mcp, project_root).

    We import spec_tools directly using importlib.util to avoid triggering
    mcp_server/tools/__init__.py which imports execution_tools that depends
    on skills/hybrid-ralph/core modules (not always available in test envs).
    """
    import importlib.util
    import sys

    spec_tools_path = Path(__file__).parent.parent / "mcp_server" / "tools" / "spec_tools.py"

    # Ensure the parent package is in sys.modules if not already
    if "mcp_server" not in sys.modules:
        import types
        sys.modules["mcp_server"] = types.ModuleType("mcp_server")
    if "mcp_server.tools" not in sys.modules:
        import types
        sys.modules["mcp_server.tools"] = types.ModuleType("mcp_server.tools")

    spec = importlib.util.spec_from_file_location(
        "mcp_server.tools.spec_tools",
        str(spec_tools_path),
    )
    spec_tools = importlib.util.module_from_spec(spec)
    sys.modules["mcp_server.tools.spec_tools"] = spec_tools
    spec.loader.exec_module(spec_tools)

    spec_tools.register_spec_tools(fake_mcp, tmp_path)
    return fake_mcp, tmp_path


# =========================================================================
# spec_start
# =========================================================================


class TestSpecStart:
    """Tests for the spec_start tool."""

    def test_start_creates_state_file(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_start",
            description="Build a REST API for user management",
        )

        assert result["success"] is True
        assert "questions" in result
        assert isinstance(result["questions"], list)
        assert len(result["questions"]) > 0

        # State file should exist
        state_path = root / ".state" / "spec-interview.json"
        assert state_path.exists()

        state = json.loads(state_path.read_text(encoding="utf-8"))
        assert state["status"] == "in_progress"
        assert state["description"] == "Build a REST API for user management"

    def test_start_with_flow_parameter(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_start",
            description="Add payment integration",
            flow="quick",
        )

        assert result["success"] is True
        state_path = root / ".state" / "spec-interview.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        assert state["flow_level"] == "quick"

    def test_start_with_full_flow(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_start",
            description="Enterprise auth system",
            flow="full",
            first_principles=True,
            max_questions=25,
        )

        assert result["success"] is True
        state_path = root / ".state" / "spec-interview.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        assert state["flow_level"] == "full"
        assert state["first_principles"] is True
        assert state["max_questions"] == 25

    def test_start_with_output_dir(self, project_with_spec_tools, tmp_path):
        mcp, root = project_with_spec_tools
        output_dir = tmp_path / "custom_output"
        output_dir.mkdir()

        result = mcp.call(
            "spec_start",
            description="Some feature",
            output_dir=str(output_dir),
        )

        assert result["success"] is True
        # State should be in the custom output dir
        state_path = output_dir / ".state" / "spec-interview.json"
        assert state_path.exists()

    def test_start_defaults(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_start",
            description="Simple task",
        )

        assert result["success"] is True
        state_path = root / ".state" / "spec-interview.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        assert state["flow_level"] == "standard"
        assert state["first_principles"] is False
        assert state["max_questions"] == 18

    def test_start_returns_question_ids(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_start",
            description="Build a chat app",
        )

        assert result["success"] is True
        for q in result["questions"]:
            assert "id" in q
            assert "text" in q

    def test_start_fails_when_interview_already_active(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        # Start first interview
        mcp.call("spec_start", description="First interview")

        # Starting second should fail
        result = mcp.call("spec_start", description="Second interview")
        assert result["success"] is False
        assert "error" in result
        assert "already" in result["error"].lower() or "active" in result["error"].lower()


# =========================================================================
# spec_resume
# =========================================================================


class TestSpecResume:
    """Tests for the spec_resume tool."""

    def test_resume_returns_current_questions(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        # Start an interview first
        start_result = mcp.call(
            "spec_start",
            description="Build an API",
        )
        assert start_result["success"] is True

        # Resume should return the same state
        resume_result = mcp.call("spec_resume")
        assert resume_result["success"] is True
        assert "questions" in resume_result
        assert "progress" in resume_result

    def test_resume_fails_when_no_active_interview(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call("spec_resume")
        assert result["success"] is False
        assert "error" in result

    def test_resume_shows_progress(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        mcp.call("spec_start", description="Build an API")

        resume_result = mcp.call("spec_resume")
        assert resume_result["success"] is True
        progress = resume_result["progress"]
        assert "questions_asked" in progress
        assert "total_questions" in progress
        assert "completion_percentage" in progress


# =========================================================================
# spec_submit_answers
# =========================================================================


class TestSpecSubmitAnswers:
    """Tests for the spec_submit_answers tool."""

    def test_submit_answers_processes_and_advances(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        start_result = mcp.call(
            "spec_start",
            description="Build an API",
        )
        questions = start_result["questions"]
        question_ids = [q["id"] for q in questions]

        # Submit answers for all returned questions
        answers = {qid: f"Answer for {qid}" for qid in question_ids}
        result = mcp.call("spec_submit_answers", answers=answers)

        assert result["success"] is True
        assert "message" in result

    def test_submit_answers_updates_state_file(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        start_result = mcp.call(
            "spec_start",
            description="Build an API",
        )
        questions = start_result["questions"]
        question_ids = [q["id"] for q in questions]

        answers = {qid: f"Answer for {qid}" for qid in question_ids}
        mcp.call("spec_submit_answers", answers=answers)

        state_path = root / ".state" / "spec-interview.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        assert state["question_cursor"] > 0
        assert len(state["history"]) > 0

    def test_submit_answers_fails_when_no_active_interview(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call(
            "spec_submit_answers",
            answers={"q1": "answer1"},
        )
        assert result["success"] is False
        assert "error" in result

    def test_submit_answers_with_compile(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        start_result = mcp.call(
            "spec_start",
            description="Build a simple feature",
            flow="quick",
            max_questions=3,
        )
        questions = start_result["questions"]
        question_ids = [q["id"] for q in questions]

        answers = {qid: f"Detailed answer for {qid}" for qid in question_ids}
        result = mcp.call(
            "spec_submit_answers",
            answers=answers,
            compile=True,
        )

        assert result["success"] is True
        # When compile=True and interview is complete or answers submitted,
        # the spec outputs should be created
        if result.get("status") == "finalized" or result.get("compiled"):
            spec_path = root / "spec.json"
            assert spec_path.exists() or "spec_json_path" in result

    def test_submit_empty_answers_fails(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        mcp.call("spec_start", description="Build an API")

        result = mcp.call("spec_submit_answers", answers={})
        assert result["success"] is False
        assert "error" in result


# =========================================================================
# spec_get_status
# =========================================================================


class TestSpecGetStatus:
    """Tests for the spec_get_status tool."""

    def test_status_when_no_interview(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call("spec_get_status")

        assert result["success"] is True
        assert result["active"] is False

    def test_status_when_interview_active(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        mcp.call("spec_start", description="Build an API")

        result = mcp.call("spec_get_status")
        assert result["success"] is True
        assert result["active"] is True
        assert "questions_asked" in result
        assert "questions_remaining" in result
        assert "completion_percentage" in result

    def test_status_shows_progress_after_answers(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        start_result = mcp.call(
            "spec_start",
            description="Build an API",
        )
        questions = start_result["questions"]
        question_ids = [q["id"] for q in questions]

        answers = {qid: f"Answer for {qid}" for qid in question_ids}
        mcp.call("spec_submit_answers", answers=answers)

        result = mcp.call("spec_get_status")
        assert result["success"] is True
        assert result["questions_asked"] > 0
        assert result["completion_percentage"] >= 0


# =========================================================================
# spec_cleanup
# =========================================================================


class TestSpecCleanup:
    """Tests for the spec_cleanup tool."""

    def test_cleanup_removes_state_file(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        mcp.call("spec_start", description="Build an API")

        state_path = root / ".state" / "spec-interview.json"
        assert state_path.exists()

        result = mcp.call("spec_cleanup")
        assert result["success"] is True
        assert not state_path.exists()

    def test_cleanup_when_no_state(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        result = mcp.call("spec_cleanup")
        assert result["success"] is True
        assert "message" in result

    def test_cleanup_with_remove_outputs(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        # Create some output files to test cleanup
        (root / "spec.json").write_text('{"test": true}', encoding="utf-8")
        (root / "spec.md").write_text("# Test", encoding="utf-8")

        result = mcp.call("spec_cleanup", remove_outputs=True)
        assert result["success"] is True
        assert not (root / "spec.json").exists()
        assert not (root / "spec.md").exists()

    def test_cleanup_without_remove_outputs_preserves_files(self, project_with_spec_tools):
        mcp, root = project_with_spec_tools
        (root / "spec.json").write_text('{"test": true}', encoding="utf-8")
        (root / "spec.md").write_text("# Test", encoding="utf-8")

        result = mcp.call("spec_cleanup", remove_outputs=False)
        assert result["success"] is True
        # Output files should still exist
        assert (root / "spec.json").exists()
        assert (root / "spec.md").exists()


# =========================================================================
# Integration: Full lifecycle
# =========================================================================


class TestSpecToolsIntegration:
    """Integration tests for the full spec interview lifecycle."""

    def test_full_lifecycle(self, project_with_spec_tools):
        """Test start -> submit -> status -> cleanup."""
        mcp, root = project_with_spec_tools

        # 1. Start
        start_result = mcp.call(
            "spec_start",
            description="Build a REST API with CRUD endpoints",
            flow="quick",
            max_questions=3,
        )
        assert start_result["success"] is True
        questions = start_result["questions"]
        assert len(questions) > 0

        # 2. Check status
        status = mcp.call("spec_get_status")
        assert status["success"] is True
        assert status["active"] is True

        # 3. Submit answers
        answers = {q["id"]: f"Answer: {q['text']}" for q in questions}
        submit_result = mcp.call("spec_submit_answers", answers=answers)
        assert submit_result["success"] is True

        # 4. Check status again (should show progress)
        status2 = mcp.call("spec_get_status")
        assert status2["success"] is True
        assert status2["questions_asked"] > 0

        # 5. Cleanup
        cleanup_result = mcp.call("spec_cleanup")
        assert cleanup_result["success"] is True

        # 6. Verify cleanup
        final_status = mcp.call("spec_get_status")
        assert final_status["active"] is False

    def test_resume_lifecycle(self, project_with_spec_tools):
        """Test start -> submit partial -> resume -> submit more."""
        mcp, root = project_with_spec_tools

        # 1. Start
        start_result = mcp.call(
            "spec_start",
            description="Build a chat application",
            flow="standard",
        )
        assert start_result["success"] is True
        questions = start_result["questions"]

        # 2. Submit partial answers (just first question)
        if len(questions) > 0:
            partial_answers = {questions[0]["id"]: "Use WebSocket for real-time"}
            mcp.call("spec_submit_answers", answers=partial_answers)

        # 3. Resume (simulating new session)
        resume_result = mcp.call("spec_resume")
        assert resume_result["success"] is True
        assert "questions" in resume_result
        assert resume_result["progress"]["questions_asked"] > 0

    def test_all_tools_registered(self, project_with_spec_tools):
        """Verify all five tools are registered."""
        mcp, root = project_with_spec_tools
        expected_tools = [
            "spec_start",
            "spec_resume",
            "spec_submit_answers",
            "spec_get_status",
            "spec_cleanup",
        ]
        for tool_name in expected_tools:
            assert tool_name in mcp._tools, f"Tool '{tool_name}' not registered"
