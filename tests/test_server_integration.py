#!/usr/bin/env python3
"""
Integration tests for Plan Cascade MCP Server (mcp_server/server.py).

Story-007: Server Registration and Integration Testing.

Verifies:
- Server version is 3.3.0
- Server name is "plan-cascade"
- initialize_server() calls all 7 register_*_tools functions + register_resources
- All 6 prompts are registered and return non-empty strings
- Cross-module integration: prompts reference correct tools from multiple stories
"""

import importlib
import importlib.util
import sys
import types
from pathlib import Path
from unittest.mock import MagicMock, call

import pytest


# ============================================================
# Fake MCP (captures both prompts and tool/resource decorators)
# ============================================================

class _FakeMCP:
    """Minimal fake MCP server that captures prompt registrations."""

    def __init__(self):
        self._prompts = {}
        self._tools = {}
        self._resources = {}

    def prompt(self):
        """Decorator that captures the prompt function."""
        def decorator(fn):
            self._prompts[fn.__name__] = fn
            return fn
        return decorator

    def tool(self):
        """Decorator that captures tool functions."""
        def decorator(fn):
            self._tools[fn.__name__] = fn
            return fn
        return decorator

    def resource(self, uri: str):
        """Decorator that captures resource functions."""
        def decorator(fn):
            self._resources[uri] = fn
            return fn
        return decorator

    def call_prompt(self, name: str, **kwargs) -> str:
        """Call a registered prompt by name."""
        return self._prompts[name](**kwargs)


# ============================================================
# Module-scoped fixture: load server.py with mocked deps
# ============================================================

@pytest.fixture(scope="module")
def server_env():
    """Load server.py via importlib, mocking heavy dependencies.

    Returns a dict with:
        - server_mod: the loaded server module
        - fake_mcp: the _FakeMCP instance the server used
        - mock_tools: the mocked mcp_server.tools module
        - mock_resources: the mocked mcp_server.resources module
        - fastmcp_cls: the MagicMock used as FastMCP class

    This avoids triggering the full import chain from mcp_server.tools
    and mcp.server.fastmcp, which depend on external packages and
    skills modules that may not be available in the test environment.
    """
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
        # --- Create mock for FastMCP that returns our fake MCP ---
        fake_mcp = _FakeMCP()

        mock_fastmcp_module = types.ModuleType("mcp.server.fastmcp")
        fastmcp_cls = MagicMock(return_value=fake_mcp)
        mock_fastmcp_module.FastMCP = fastmcp_cls

        mock_mcp = types.ModuleType("mcp")
        mock_mcp_server = types.ModuleType("mcp.server")
        mock_mcp_server.fastmcp = mock_fastmcp_module

        sys.modules["mcp"] = mock_mcp
        sys.modules["mcp.server"] = mock_mcp_server
        sys.modules["mcp.server.fastmcp"] = mock_fastmcp_module

        # --- Mock mcp_server.tools (avoids transitive import issues) ---
        mock_tools = types.ModuleType("mcp_server.tools")
        mock_tools.register_prd_tools = MagicMock()
        mock_tools.register_mega_tools = MagicMock()
        mock_tools.register_execution_tools = MagicMock()
        mock_tools.register_worktree_tools = MagicMock()
        mock_tools.register_design_tools = MagicMock()
        mock_tools.register_spec_tools = MagicMock()
        mock_tools.register_dashboard_tools = MagicMock()
        sys.modules["mcp_server.tools"] = mock_tools

        # --- Mock mcp_server.resources ---
        mock_resources = types.ModuleType("mcp_server.resources")
        mock_resources.register_resources = MagicMock()
        sys.modules["mcp_server.resources"] = mock_resources

        # --- Ensure mcp_server package module exists ---
        if "mcp_server" not in sys.modules:
            sys.modules["mcp_server"] = types.ModuleType("mcp_server")

        # --- Load server.py via importlib ---
        server_path = Path(__file__).parent.parent / "mcp_server" / "server.py"

        # Remove stale cached module
        if "mcp_server.server" in sys.modules:
            del sys.modules["mcp_server.server"]

        spec = importlib.util.spec_from_file_location(
            "mcp_server.server",
            str(server_path),
        )
        server_mod = importlib.util.module_from_spec(spec)
        sys.modules["mcp_server.server"] = server_mod
        spec.loader.exec_module(server_mod)

        yield {
            "server_mod": server_mod,
            "fake_mcp": fake_mcp,
            "mock_tools": mock_tools,
            "mock_resources": mock_resources,
            "fastmcp_cls": fastmcp_cls,
        }

    finally:
        for mod_name in modules_to_mock:
            if mod_name in saved_modules:
                sys.modules[mod_name] = saved_modules[mod_name]
            elif mod_name in sys.modules:
                del sys.modules[mod_name]
        if "mcp_server.server" in sys.modules:
            del sys.modules["mcp_server.server"]


@pytest.fixture(scope="module")
def server_mod(server_env):
    return server_env["server_mod"]


@pytest.fixture(scope="module")
def fake_mcp(server_env):
    return server_env["fake_mcp"]


@pytest.fixture(scope="module")
def mock_tools(server_env):
    return server_env["mock_tools"]


@pytest.fixture(scope="module")
def mock_resources(server_env):
    return server_env["mock_resources"]


@pytest.fixture(scope="module")
def fastmcp_cls(server_env):
    return server_env["fastmcp_cls"]


# ============================================================
# 1. Server Initialization Tests
# ============================================================


class TestServerInitialization:
    """Test that initialize_server() wires everything up correctly."""

    def test_initialize_calls_register_prd_tools(self, server_mod, mock_tools):
        server_mod.initialize_server()
        mock_tools.register_prd_tools.assert_called()

    def test_initialize_calls_register_mega_tools(self, server_mod, mock_tools):
        mock_tools.register_mega_tools.assert_called()

    def test_initialize_calls_register_execution_tools(self, server_mod, mock_tools):
        mock_tools.register_execution_tools.assert_called()

    def test_initialize_calls_register_worktree_tools(self, server_mod, mock_tools):
        mock_tools.register_worktree_tools.assert_called()

    def test_initialize_calls_register_design_tools(self, server_mod, mock_tools):
        mock_tools.register_design_tools.assert_called()

    def test_initialize_calls_register_spec_tools(self, server_mod, mock_tools):
        mock_tools.register_spec_tools.assert_called()

    def test_initialize_calls_register_dashboard_tools(self, server_mod, mock_tools):
        mock_tools.register_dashboard_tools.assert_called()

    def test_initialize_calls_register_resources(self, server_mod, mock_resources):
        mock_resources.register_resources.assert_called()

    def test_all_seven_register_functions_called(self, server_mod, mock_tools):
        """All 7 tool-register functions must have been called at least once."""
        register_fns = [
            mock_tools.register_prd_tools,
            mock_tools.register_mega_tools,
            mock_tools.register_execution_tools,
            mock_tools.register_worktree_tools,
            mock_tools.register_design_tools,
            mock_tools.register_spec_tools,
            mock_tools.register_dashboard_tools,
        ]
        for fn in register_fns:
            assert fn.call_count >= 1, f"{fn} was not called"


# ============================================================
# 2. All Tools Registered (import verification)
# ============================================================


class TestToolRegistration:
    """Verify that all 7 register functions are importable from __init__.py."""

    def test_register_prd_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_prd_tools")

    def test_register_mega_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_mega_tools")

    def test_register_execution_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_execution_tools")

    def test_register_worktree_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_worktree_tools")

    def test_register_design_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_design_tools")

    def test_register_spec_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_spec_tools")

    def test_register_dashboard_tools_exists(self, mock_tools):
        assert hasattr(mock_tools, "register_dashboard_tools")

    def test_all_seven_exports_present(self, mock_tools):
        expected = [
            "register_prd_tools",
            "register_mega_tools",
            "register_execution_tools",
            "register_worktree_tools",
            "register_design_tools",
            "register_spec_tools",
            "register_dashboard_tools",
        ]
        for name in expected:
            assert hasattr(mock_tools, name), f"Missing export: {name}"


# ============================================================
# 3. All Prompts Registered
# ============================================================


class TestAllPromptsRegistered:
    """Verify all 6 prompts are registered and return non-empty strings."""

    EXPECTED_PROMPTS = [
        "start_feature_development",
        "start_mega_project",
        "start_worktree_development",
        "start_design_review",
        "start_spec_interview",
        "resume_interrupted_task",
    ]

    def test_all_six_prompts_present(self, fake_mcp):
        for name in self.EXPECTED_PROMPTS:
            assert name in fake_mcp._prompts, f"Prompt '{name}' not registered"

    def test_prompt_count_is_six(self, fake_mcp):
        assert len(fake_mcp._prompts) == 6

    def test_start_feature_development_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_feature_development", description="Test feature"
        )
        assert isinstance(result, str) and len(result) > 0

    def test_start_mega_project_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_mega_project", description="Test project"
        )
        assert isinstance(result, str) and len(result) > 0

    def test_start_worktree_development_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="task",
            target_branch="branch",
            description="Test",
        )
        assert isinstance(result, str) and len(result) > 0

    def test_start_design_review_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_design_review", scope="feature"
        )
        assert isinstance(result, str) and len(result) > 0

    def test_start_spec_interview_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt(
            "start_spec_interview", description="Test spec"
        )
        assert isinstance(result, str) and len(result) > 0

    def test_resume_interrupted_task_returns_nonempty(self, fake_mcp):
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert isinstance(result, str) and len(result) > 0


# ============================================================
# 4. Version Check
# ============================================================


class TestVersionCheck:
    """Verify server version and name are correct."""

    def test_server_version_is_3_3_0(self, fastmcp_cls):
        """FastMCP was constructed with version='3.3.0'."""
        fastmcp_cls.assert_called_once()
        _, kwargs = fastmcp_cls.call_args
        assert kwargs["version"] == "3.3.0"

    def test_server_version_not_3_2_0(self, fastmcp_cls):
        _, kwargs = fastmcp_cls.call_args
        assert kwargs["version"] != "3.2.0"

    def test_server_name_is_plan_cascade(self, fastmcp_cls):
        _, kwargs = fastmcp_cls.call_args
        assert kwargs["name"] == "plan-cascade"

    def test_server_has_description(self, fastmcp_cls):
        _, kwargs = fastmcp_cls.call_args
        assert "description" in kwargs
        assert len(kwargs["description"]) > 0


# ============================================================
# 5. Cross-Module Integration
# ============================================================


class TestCrossModuleIntegration:
    """Verify prompts reference tools from the correct modules / multiple stories."""

    def test_worktree_prompt_references_worktree_create(self, fake_mcp):
        """start_worktree_development should reference worktree_create (story-002 tool)."""
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="task",
            target_branch="branch",
            description="desc",
        )
        assert "worktree_create" in result

    def test_worktree_prompt_references_prd_generate(self, fake_mcp):
        """start_worktree_development should also reference prd_generate (story-001 tool)."""
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="task",
            target_branch="branch",
            description="desc",
        )
        assert "prd_generate" in result

    def test_worktree_prompt_references_design_generate(self, fake_mcp):
        """start_worktree_development should reference design_generate (story-003 tool)."""
        result = fake_mcp.call_prompt(
            "start_worktree_development",
            task_name="task",
            target_branch="branch",
            description="desc",
        )
        assert "design_generate" in result

    def test_design_review_references_design_tools(self, fake_mcp):
        """start_design_review should reference design_get and design_review."""
        result = fake_mcp.call_prompt(
            "start_design_review", scope="feature"
        )
        assert "design_get" in result
        assert "design_review" in result
        assert "design_generate" in result

    def test_spec_interview_references_spec_tools(self, fake_mcp):
        """start_spec_interview should reference all spec tools (story-004)."""
        result = fake_mcp.call_prompt(
            "start_spec_interview", description="A feature"
        )
        assert "spec_start" in result
        assert "spec_submit_answers" in result
        assert "spec_get_status" in result
        assert "spec_resume" in result
        assert "spec_cleanup" in result

    def test_resume_prompt_references_dashboard(self, fake_mcp):
        """resume_interrupted_task should reference dashboard tool (story-005)."""
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "dashboard" in result

    def test_resume_prompt_references_session_recover(self, fake_mcp):
        """resume_interrupted_task should reference session_recover (story-005)."""
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "session_recover" in result

    def test_resume_prompt_references_execution_status(self, fake_mcp):
        """resume_interrupted_task should reference get_execution_status (story-001)."""
        result = fake_mcp.call_prompt("resume_interrupted_task")
        assert "get_execution_status" in result

    def test_mega_prompt_references_three_layers(self, fake_mcp):
        """start_mega_project should reference tools from all three layers."""
        result = fake_mcp.call_prompt(
            "start_mega_project", description="Large project"
        )
        # Layer 1 (mega)
        assert "mega_generate" in result
        # Layer 2 (PRD)
        assert "prd_generate" in result
        # Layer 3 (execution)
        assert "get_story_context" in result

    def test_feature_prompt_references_prd_and_execution_tools(self, fake_mcp):
        """start_feature_development should reference both PRD and execution tools."""
        result = fake_mcp.call_prompt(
            "start_feature_development", description="Feature"
        )
        assert "prd_generate" in result
        assert "mark_story_complete" in result
        assert "append_findings" in result
