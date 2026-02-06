"""Tests for MCP Prompt Templates (mcp_server/server.py).

TDD tests for the MCP prompt workflows:
- start_feature_development (existing)
- start_mega_project (existing)
- start_worktree_development (new - story-006)
- start_design_review (new - story-006)
- start_spec_interview (new - story-006)
- resume_interrupted_task (new - story-006)
"""

import importlib
import importlib.util
import sys
import types
from pathlib import Path
from unittest.mock import MagicMock

import pytest


class _FakeMCP:
    """Minimal fake MCP server that captures prompt registrations."""

    def __init__(self):
        self._prompts = {}
        self._tools = {}

    def prompt(self):
        """Decorator that captures the prompt function."""
        def decorator(fn):
            self._prompts[fn.__name__] = fn
            return fn
        return decorator

    def tool(self):
        """Decorator that captures tool functions (unused but required)."""
        def decorator(fn):
            self._tools[fn.__name__] = fn
            return fn
        return decorator

    def call_prompt(self, name: str, **kwargs) -> str:
        """Call a registered prompt by name."""
        return self._prompts[name](**kwargs)


@pytest.fixture(scope="module")
def server_module():
    """Load server.py via importlib, mocking heavy dependencies.

    This avoids triggering the full import chain from mcp_server.tools
    and mcp.server.fastmcp, which require external packages and
    skills modules that may not be available in the test environment.
    """
    # Save original modules to restore later
    saved_modules = {}
    modules_to_mock = [
        "mcp",
        "mcp.server",
        "mcp.server.fastmcp",
        "mcp_server.tools",
        "mcp_server.resources",
    ]

    for mod_name in modules_to_mock:
        if mod_name in sys.modules:
            saved_modules[mod_name] = sys.modules[mod_name]

    try:
        # Create mock for FastMCP that returns our fake MCP
        fake_mcp = _FakeMCP()

        mock_fastmcp_module = types.ModuleType("mcp.server.fastmcp")
        mock_fastmcp_module.FastMCP = MagicMock(return_value=fake_mcp)

        mock_mcp = types.ModuleType("mcp")
        mock_mcp_server = types.ModuleType("mcp.server")
        mock_mcp_server.fastmcp = mock_fastmcp_module

        sys.modules["mcp"] = mock_mcp
        sys.modules["mcp.server"] = mock_mcp_server
        sys.modules["mcp.server.fastmcp"] = mock_fastmcp_module

        # Mock mcp_server.tools to avoid transitive import issues
        mock_tools = types.ModuleType("mcp_server.tools")
        mock_tools.register_prd_tools = MagicMock()
        mock_tools.register_mega_tools = MagicMock()
        mock_tools.register_execution_tools = MagicMock()
        mock_tools.register_worktree_tools = MagicMock()
        mock_tools.register_design_tools = MagicMock()
        mock_tools.register_spec_tools = MagicMock()
        mock_tools.register_dashboard_tools = MagicMock()
        sys.modules["mcp_server.tools"] = mock_tools

        # Mock mcp_server.resources
        mock_resources = types.ModuleType("mcp_server.resources")
        mock_resources.register_resources = MagicMock()
        sys.modules["mcp_server.resources"] = mock_resources

        # Ensure mcp_server package module exists
        if "mcp_server" not in sys.modules:
            sys.modules["mcp_server"] = types.ModuleType("mcp_server")

        # Load server.py via importlib
        server_path = Path(__file__).parent.parent / "mcp_server" / "server.py"

        # Remove from sys.modules if previously loaded (stale cache)
        if "mcp_server.server" in sys.modules:
            del sys.modules["mcp_server.server"]

        spec = importlib.util.spec_from_file_location(
            "mcp_server.server",
            str(server_path),
        )
        server_mod = importlib.util.module_from_spec(spec)
        sys.modules["mcp_server.server"] = server_mod
        spec.loader.exec_module(server_mod)

        # Attach the fake_mcp for test access
        server_mod._test_fake_mcp = fake_mcp

        yield server_mod

    finally:
        # Restore original modules
        for mod_name in modules_to_mock:
            if mod_name in saved_modules:
                sys.modules[mod_name] = saved_modules[mod_name]
            elif mod_name in sys.modules:
                del sys.modules[mod_name]
        if "mcp_server.server" in sys.modules:
            del sys.modules["mcp_server.server"]


@pytest.fixture
def fake_mcp(server_module):
    """Return the fake MCP instance with all prompts registered."""
    return server_module._test_fake_mcp


# =========================================================================
# Existing Prompts (sanity checks)
# =========================================================================


class TestExistingPrompts:
    """Sanity checks for the two pre-existing prompts."""

    def test_start_feature_development_registered(self, fake_mcp):
        assert "start_feature_development" in fake_mcp._prompts

    def test_start_mega_project_registered(self, fake_mcp):
        assert "start_mega_project" in fake_mcp._prompts

    def test_start_feature_development_returns_string(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_feature_development",
            description="Add user auth",
        )
        assert isinstance(result, str)
        assert len(result) > 0
        assert "Add user auth" in result

    def test_start_mega_project_returns_string(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_mega_project",
            description="E-commerce platform",
        )
        assert isinstance(result, str)
        assert len(result) > 0
        assert "E-commerce platform" in result


# =========================================================================
# start_worktree_development
# =========================================================================


class TestStartWorktreeDevelopment:
    """Tests for the start_worktree_development prompt."""

    def test_prompt_registered(self, fake_mcp):
        assert "start_worktree_development" in fake_mcp._prompts

    def test_returns_non_empty_string(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="user-authentication",
            target_branch="feature/user-auth",
            description="Implement JWT-based authentication",
        )
        assert isinstance(result, str)
        assert len(result) > 0

    def test_includes_parameters_in_output(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="payment-gateway",
            target_branch="feature/payments",
            description="Integrate Stripe payment processing",
        )
        assert "payment-gateway" in result
        assert "feature/payments" in result
        assert "Integrate Stripe payment processing" in result

    def test_includes_worktree_tool_references(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="my-feature",
            target_branch="feature/my-feature",
            description="A feature",
        )
        assert "worktree_create" in result
        assert "worktree_list" in result
        assert "worktree_complete" in result

    def test_includes_planning_tool_references(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="my-feature",
            target_branch="feature/my-feature",
            description="A feature",
        )
        assert "prd_generate" in result
        assert "design_generate" in result

    def test_includes_section_headers(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="my-feature",
            target_branch="feature/my-feature",
            description="A feature",
        )
        # Should have markdown section headers
        assert "##" in result
        # Should have a workflow section
        assert "Workflow" in result or "Steps" in result or "workflow" in result

    def test_includes_markdown_formatting(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="my-feature",
            target_branch="feature/my-feature",
            description="A feature",
        )
        # Should use numbered or bulleted steps
        assert "1." in result or "- " in result


# =========================================================================
# start_design_review
# =========================================================================


class TestStartDesignReview:
    """Tests for the start_design_review prompt."""

    def test_prompt_registered(self, fake_mcp):
        assert "start_design_review" in fake_mcp._prompts

    def test_returns_non_empty_string(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review",
            scope="feature",
        )
        assert isinstance(result, str)
        assert len(result) > 0

    def test_includes_scope_parameter(self, fake_mcp):
        result_feature = fake_mcp.call_prompt(
            "start_design_review",
            scope="feature",
        )
        assert "feature" in result_feature.lower()

        result_project = fake_mcp.call_prompt(
            "start_design_review",
            scope="project",
        )
        assert "project" in result_project.lower()

    def test_includes_design_tool_references(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review",
            scope="feature",
        )
        assert "design_get" in result
        assert "design_review" in result
        assert "design_generate" in result

    def test_includes_section_headers(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review",
            scope="feature",
        )
        assert "##" in result

    def test_includes_checklist_or_steps(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review",
            scope="feature",
        )
        # Should have numbered steps or checklist items
        assert "1." in result or "- [" in result or "- " in result

    def test_includes_review_guidance(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review",
            scope="project",
        )
        # Should mention reviewing or checklist concepts
        assert "review" in result.lower() or "checklist" in result.lower()


# =========================================================================
# start_spec_interview
# =========================================================================


class TestStartSpecInterview:
    """Tests for the start_spec_interview prompt."""

    def test_prompt_registered(self, fake_mcp):
        assert "start_spec_interview" in fake_mcp._prompts

    def test_returns_non_empty_string(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="Build a REST API for user management",
        )
        assert isinstance(result, str)
        assert len(result) > 0

    def test_includes_description_parameter(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="Real-time chat application with WebSockets",
        )
        assert "Real-time chat application with WebSockets" in result

    def test_includes_optional_flow_parameter(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="Some feature",
            flow="quick",
        )
        assert "quick" in result.lower()

    def test_default_flow_when_omitted(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="Some feature",
        )
        # Should still return valid content with default flow
        assert isinstance(result, str)
        assert len(result) > 0
        # Should mention standard flow or just work without flow
        assert "standard" in result.lower() or "spec" in result.lower()

    def test_includes_spec_tool_references(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="A feature",
        )
        assert "spec_start" in result
        assert "spec_submit_answers" in result
        assert "spec_get_status" in result
        assert "spec_resume" in result
        assert "spec_cleanup" in result

    def test_includes_section_headers(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="A feature",
        )
        assert "##" in result

    def test_includes_workflow_steps(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview",
            description="A feature",
        )
        assert "1." in result


# =========================================================================
# resume_interrupted_task
# =========================================================================


class TestResumeInterruptedTask:
    """Tests for the resume_interrupted_task prompt."""

    def test_prompt_registered(self, fake_mcp):
        assert "resume_interrupted_task" in fake_mcp._prompts

    def test_returns_non_empty_string(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert isinstance(result, str)
        assert len(result) > 0

    def test_no_required_parameters(self, fake_mcp):
        """Should work with no arguments."""
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert isinstance(result, str)

    def test_includes_recovery_tool_references(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "dashboard" in result
        assert "session_recover" in result
        assert "get_execution_status" in result

    def test_includes_section_headers(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "##" in result

    def test_includes_recovery_steps(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "1." in result

    def test_includes_resume_guidance(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        # Should mention recovery or resume concepts
        assert "recover" in result.lower() or "resume" in result.lower() or "interrupt" in result.lower()


# =========================================================================
# All prompts registered
# =========================================================================


class TestAllPromptsRegistered:
    """Verify all six prompts are registered."""

    def test_all_prompts_present(self, fake_mcp):
        expected_prompts = [
            "start_feature_development",
            "start_mega_project",
            "start_worktree_development",
            "start_design_review",
            "start_spec_interview",
            "resume_interrupted_task",
        ]
        for prompt_name in expected_prompts:
            assert prompt_name in fake_mcp._prompts, (
                f"Prompt '{prompt_name}' not registered"
            )
