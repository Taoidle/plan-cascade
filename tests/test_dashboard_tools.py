#!/usr/bin/env python3
"""
Tests for Dashboard and Session Recovery MCP Tools (mcp_server/tools/dashboard_tools.py).

TDD tests for the four dashboard/recovery tools:
- dashboard: Unified status dashboard
- session_recover: Detect and prepare recovery for interrupted sessions
- get_configuration: Read current config from prd.json and agents.json
- update_configuration: Update configuration settings
"""

import json
import importlib
import importlib.util
import sys
import types
from pathlib import Path
from typing import Any, Dict

import pytest


# ============================================================
# Module Loading Helper
# ============================================================

def _load_dashboard_tools_module():
    """
    Load the dashboard_tools module directly to avoid triggering
    broken imports in sibling modules through __init__.py.
    """
    module_path = Path(__file__).parent.parent / "mcp_server" / "tools" / "dashboard_tools.py"
    spec = importlib.util.spec_from_file_location(
        "mcp_server.tools.dashboard_tools",
        str(module_path),
    )
    mod = importlib.util.module_from_spec(spec)

    # Ensure parent packages in sys.modules so import doesn't fail
    if "mcp_server" not in sys.modules:
        sys.modules["mcp_server"] = types.ModuleType("mcp_server")
    if "mcp_server.tools" not in sys.modules:
        sys.modules["mcp_server.tools"] = types.ModuleType("mcp_server.tools")

    sys.modules["mcp_server.tools.dashboard_tools"] = mod
    spec.loader.exec_module(mod)
    return mod


_dashboard_tools_mod = _load_dashboard_tools_module()
register_dashboard_tools = _dashboard_tools_mod.register_dashboard_tools


# ============================================================
# Fake MCP Server
# ============================================================

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


# ============================================================
# Fixtures
# ============================================================

@pytest.fixture
def fake_mcp():
    return _FakeMCP()


@pytest.fixture
def project_root(tmp_path):
    """Create a temporary project directory."""
    return tmp_path


@pytest.fixture
def project_with_tools(project_root, fake_mcp):
    """Register dashboard tools on a fake MCP and return (mcp, project_root)."""
    register_dashboard_tools(fake_mcp, project_root)
    return fake_mcp, project_root


@pytest.fixture
def sample_prd():
    """Create a sample PRD dictionary."""
    return {
        "metadata": {
            "description": "Test Feature",
            "created_at": "2026-01-01T00:00:00Z",
            "version": "1.0.0"
        },
        "goal": "Implement test feature",
        "objectives": ["Create unit tests", "Implement core logic"],
        "flow_config": {
            "flow": "full",
            "tdd": True,
            "confirm": True
        },
        "tdd_config": {
            "enabled": True,
            "framework": "pytest"
        },
        "execution_config": {
            "default_agent": "sonnet",
            "parallel": True
        },
        "stories": [
            {
                "id": "story-001",
                "title": "Set up infrastructure",
                "description": "Create the testing framework",
                "priority": "high",
                "status": "complete",
                "dependencies": [],
                "acceptance_criteria": ["Tests can run"]
            },
            {
                "id": "story-002",
                "title": "Implement core logic",
                "description": "Build the main logic",
                "priority": "high",
                "status": "in_progress",
                "dependencies": ["story-001"],
                "acceptance_criteria": ["Logic works"]
            },
            {
                "id": "story-003",
                "title": "Add API endpoints",
                "description": "Create REST API endpoints",
                "priority": "medium",
                "status": "pending",
                "dependencies": ["story-002"],
                "acceptance_criteria": ["Endpoints respond"]
            }
        ]
    }


@pytest.fixture
def sample_mega_plan():
    """Create a sample mega-plan dictionary."""
    return {
        "metadata": {
            "description": "Test Mega Project",
            "created_at": "2026-01-01T00:00:00Z",
            "version": "1.0.0"
        },
        "goal": "Build multi-feature project",
        "features": [
            {
                "id": "feature-001",
                "title": "Auth system",
                "status": "complete"
            },
            {
                "id": "feature-002",
                "title": "Dashboard",
                "status": "in_progress"
            }
        ]
    }


@pytest.fixture
def sample_agents():
    """Create a sample agents.json dictionary."""
    return {
        "default_agent": "sonnet",
        "agents": {
            "sonnet": {
                "name": "Claude Sonnet",
                "model": "claude-sonnet-4-20250514"
            },
            "opus": {
                "name": "Claude Opus",
                "model": "claude-opus-4-20250514"
            }
        }
    }


# ============================================================
# Tool Registration
# ============================================================

class TestToolRegistration:
    """Verify all four tools are registered."""

    def test_all_tools_registered(self, project_with_tools):
        mcp, root = project_with_tools
        expected_tools = [
            "dashboard",
            "session_recover",
            "get_configuration",
            "update_configuration",
        ]
        for tool_name in expected_tools:
            assert tool_name in mcp._tools, f"Tool '{tool_name}' not registered"


# ============================================================
# dashboard tool
# ============================================================

class TestDashboard:
    """Tests for the dashboard tool."""

    def test_dashboard_no_state_files(self, project_with_tools):
        """Dashboard returns success with no active mode when no state files exist."""
        mcp, root = project_with_tools
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] is None
        assert "recommended_action" in result

    def test_dashboard_detects_mega_mode(self, project_with_tools, sample_mega_plan):
        """Dashboard detects mega mode from mega-plan.json."""
        mcp, root = project_with_tools
        (root / "mega-plan.json").write_text(
            json.dumps(sample_mega_plan), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] == "mega"

    def test_dashboard_detects_worktree_mode(self, project_with_tools):
        """Dashboard detects hybrid-worktree mode from .planning-config.json."""
        mcp, root = project_with_tools
        (root / ".planning-config.json").write_text(
            json.dumps({"mode": "worktree", "worktrees": []}), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] == "hybrid-worktree"

    def test_dashboard_detects_hybrid_auto_mode(self, project_with_tools, sample_prd):
        """Dashboard detects hybrid-auto mode from prd.json."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] == "hybrid-auto"

    def test_dashboard_mega_takes_priority(self, project_with_tools, sample_prd, sample_mega_plan):
        """When multiple state files exist, mega-plan.json takes priority."""
        mcp, root = project_with_tools
        (root / "mega-plan.json").write_text(
            json.dumps(sample_mega_plan), encoding="utf-8"
        )
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] == "mega"

    def test_dashboard_worktree_over_hybrid_auto(self, project_with_tools, sample_prd):
        """When .planning-config.json and prd.json exist, worktree takes priority."""
        mcp, root = project_with_tools
        (root / ".planning-config.json").write_text(
            json.dumps({"mode": "worktree"}), encoding="utf-8"
        )
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] == "hybrid-worktree"

    def test_dashboard_prd_progress(self, project_with_tools, sample_prd):
        """Dashboard returns PRD progress information."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert "prd_progress" in result
        prd_prog = result["prd_progress"]
        assert prd_prog["total_stories"] == 3
        assert prd_prog["complete"] == 1
        assert prd_prog["in_progress"] == 1
        assert prd_prog["pending"] == 1

    def test_dashboard_mega_progress(self, project_with_tools, sample_mega_plan):
        """Dashboard returns mega-plan progress information."""
        mcp, root = project_with_tools
        (root / "mega-plan.json").write_text(
            json.dumps(sample_mega_plan), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert "mega_progress" in result
        mega_prog = result["mega_progress"]
        assert mega_prog["total_features"] == 2
        assert mega_prog["complete"] == 1
        assert mega_prog["in_progress"] == 1

    def test_dashboard_agent_status(self, project_with_tools, sample_prd):
        """Dashboard returns agent status when .agent-status.json exists."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        agent_status = {
            "running": [
                {"story_id": "story-002", "agent": "sonnet", "started_at": "2026-01-01T00:00:00Z"}
            ],
            "completed": [
                {"story_id": "story-001", "agent": "sonnet"}
            ],
            "failed": []
        }
        (root / ".agent-status.json").write_text(
            json.dumps(agent_status), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert "agent_status" in result
        assert result["agent_status"]["running"] == 1
        assert result["agent_status"]["completed"] == 1

    def test_dashboard_design_doc_status(self, project_with_tools, sample_prd):
        """Dashboard includes design_doc status when design_doc.json exists."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        design_doc = {
            "metadata": {"level": "feature"},
            "overview": {"title": "Test"},
            "architecture": {"components": [{"name": "A"}, {"name": "B"}]},
            "story_mappings": {"story-001": {}, "story-002": {}}
        }
        (root / "design_doc.json").write_text(
            json.dumps(design_doc), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert "design_doc_status" in result
        dds = result["design_doc_status"]
        assert dds["exists"] is True
        assert dds["level"] == "feature"
        assert dds["components_count"] == 2
        assert dds["mapped_stories"] == 2

    def test_dashboard_no_design_doc(self, project_with_tools, sample_prd):
        """Dashboard shows design_doc does not exist when file is missing."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["design_doc_status"]["exists"] is False

    def test_dashboard_recommended_action_no_files(self, project_with_tools):
        """Recommended action when no state files exist."""
        mcp, root = project_with_tools
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["recommended_action"] is not None
        assert isinstance(result["recommended_action"], str)

    def test_dashboard_recommended_action_with_prd(self, project_with_tools, sample_prd):
        """Recommended action when PRD exists with pending stories."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["recommended_action"] is not None

    def test_dashboard_active_worktrees(self, project_with_tools):
        """Dashboard returns active worktree info from .planning-config.json."""
        mcp, root = project_with_tools
        planning_config = {
            "mode": "worktree",
            "worktrees": [
                {"branch": "feature-1", "path": "/tmp/wt1", "status": "active"},
                {"branch": "feature-2", "path": "/tmp/wt2", "status": "active"}
            ]
        }
        (root / ".planning-config.json").write_text(
            json.dumps(planning_config), encoding="utf-8"
        )
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert "active_worktrees" in result
        assert result["active_worktrees"] == 2

    def test_dashboard_handles_corrupt_json(self, project_with_tools):
        """Dashboard handles corrupt JSON files gracefully."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text("{ corrupt json !!!", encoding="utf-8")
        result = mcp.call("dashboard")

        assert result["success"] is True
        assert result["active_mode"] is None


# ============================================================
# session_recover tool
# ============================================================

class TestSessionRecover:
    """Tests for the session_recover tool."""

    def test_no_interrupted_session(self, project_with_tools):
        """No recovery needed when no state files exist."""
        mcp, root = project_with_tools
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is False

    def test_detect_hybrid_interrupted(self, project_with_tools, sample_prd):
        """Detect interrupted hybrid-auto session from prd.json with in-progress stories."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is True
        assert result["detected_mode"] == "hybrid-auto"
        assert len(result["incomplete_items"]) > 0

    def test_detect_mega_interrupted(self, project_with_tools, sample_mega_plan):
        """Detect interrupted mega session from mega-plan.json with in-progress features."""
        mcp, root = project_with_tools
        (root / "mega-plan.json").write_text(
            json.dumps(sample_mega_plan), encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is True
        assert result["detected_mode"] == "mega"

    def test_detect_worktree_interrupted(self, project_with_tools):
        """Detect interrupted worktree session from .planning-config.json."""
        mcp, root = project_with_tools
        planning_config = {
            "mode": "worktree",
            "worktrees": [
                {"branch": "feature-1", "path": "/tmp/wt1", "status": "active"}
            ]
        }
        (root / ".planning-config.json").write_text(
            json.dumps(planning_config), encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is True
        assert result["detected_mode"] == "hybrid-worktree"

    def test_detect_from_hybrid_context_file(self, project_with_tools):
        """Detect interrupted session from .hybrid-execution-context.md."""
        mcp, root = project_with_tools
        (root / ".hybrid-execution-context.md").write_text(
            "# Hybrid Execution Context\nBatch 1 in progress",
            encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is True

    def test_detect_from_mega_context_file(self, project_with_tools):
        """Detect interrupted session from .mega-execution-context.md."""
        mcp, root = project_with_tools
        (root / ".mega-execution-context.md").write_text(
            "# Mega Execution Context\nFeature 1 in progress",
            encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is True

    def test_resume_action_included(self, project_with_tools, sample_prd):
        """session_recover includes a recommended resume action."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert "resume_action" in result
        assert isinstance(result["resume_action"], str)

    def test_current_state_included(self, project_with_tools, sample_prd):
        """session_recover includes current state description."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert "current_state" in result

    def test_no_in_progress_means_no_recovery(self, project_with_tools):
        """Completed PRD does not need recovery."""
        mcp, root = project_with_tools
        prd = {
            "metadata": {"description": "Done"},
            "stories": [
                {"id": "story-001", "title": "Done", "status": "complete", "dependencies": []}
            ]
        }
        (root / "prd.json").write_text(json.dumps(prd), encoding="utf-8")
        result = mcp.call("session_recover")

        assert result["success"] is True
        assert result["recovery_needed"] is False

    def test_handles_corrupt_json(self, project_with_tools):
        """session_recover handles corrupt JSON files gracefully."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text("{ bad json", encoding="utf-8")
        result = mcp.call("session_recover")

        assert result["success"] is True


# ============================================================
# get_configuration tool
# ============================================================

class TestGetConfiguration:
    """Tests for the get_configuration tool."""

    def test_no_config_files(self, project_with_tools):
        """Returns empty config when no files exist."""
        mcp, root = project_with_tools
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["prd_config"] is None
        assert result["agents_config"] is None

    def test_prd_config_returned(self, project_with_tools, sample_prd):
        """Returns config from prd.json when it exists."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["prd_config"] is not None
        prd_cfg = result["prd_config"]
        assert "flow_config" in prd_cfg
        assert "tdd_config" in prd_cfg
        assert "execution_config" in prd_cfg

    def test_agents_config_returned(self, project_with_tools, sample_agents):
        """Returns config from agents.json when it exists."""
        mcp, root = project_with_tools
        (root / "agents.json").write_text(
            json.dumps(sample_agents), encoding="utf-8"
        )
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["agents_config"] is not None
        assert result["agents_config"]["default_agent"] == "sonnet"

    def test_both_configs_returned(self, project_with_tools, sample_prd, sample_agents):
        """Returns both prd and agents configs when both exist."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        (root / "agents.json").write_text(
            json.dumps(sample_agents), encoding="utf-8"
        )
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["prd_config"] is not None
        assert result["agents_config"] is not None

    def test_handles_corrupt_prd(self, project_with_tools):
        """Returns None for prd_config when prd.json is corrupt."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text("not json!", encoding="utf-8")
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["prd_config"] is None

    def test_handles_corrupt_agents(self, project_with_tools):
        """Returns None for agents_config when agents.json is corrupt."""
        mcp, root = project_with_tools
        (root / "agents.json").write_text("not json!", encoding="utf-8")
        result = mcp.call("get_configuration")

        assert result["success"] is True
        assert result["agents_config"] is None


# ============================================================
# update_configuration tool
# ============================================================

class TestUpdateConfiguration:
    """Tests for the update_configuration tool."""

    def test_update_flow_in_prd(self, project_with_tools, sample_prd):
        """Update flow config in prd.json."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            flow="quick",
        )

        assert result["success"] is True
        # Verify prd.json was updated
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["flow_config"]["flow"] == "quick"

    def test_update_tdd_in_prd(self, project_with_tools, sample_prd):
        """Update TDD config in prd.json."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            tdd=False,
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["tdd_config"]["enabled"] is False

    def test_update_confirm_in_prd(self, project_with_tools, sample_prd):
        """Update confirm config in prd.json."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            confirm=False,
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["flow_config"]["confirm"] is False

    def test_update_default_agent_in_prd(self, project_with_tools, sample_prd):
        """Update default_agent in prd.json execution_config."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            default_agent="opus",
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["execution_config"]["default_agent"] == "opus"

    def test_update_default_agent_in_agents_json(self, project_with_tools, sample_agents):
        """Update default_agent in agents.json."""
        mcp, root = project_with_tools
        (root / "agents.json").write_text(
            json.dumps(sample_agents), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            default_agent="opus",
        )

        assert result["success"] is True
        agents = json.loads((root / "agents.json").read_text(encoding="utf-8"))
        assert agents["default_agent"] == "opus"

    def test_update_both_prd_and_agents(self, project_with_tools, sample_prd, sample_agents):
        """Update default_agent in both files when both exist."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        (root / "agents.json").write_text(
            json.dumps(sample_agents), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            default_agent="opus",
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        agents = json.loads((root / "agents.json").read_text(encoding="utf-8"))
        assert prd["execution_config"]["default_agent"] == "opus"
        assert agents["default_agent"] == "opus"

    def test_update_no_files_exist(self, project_with_tools):
        """Fails gracefully when no config files exist."""
        mcp, root = project_with_tools
        result = mcp.call(
            "update_configuration",
            flow="quick",
        )

        assert result["success"] is False
        assert "error" in result

    def test_update_multiple_settings(self, project_with_tools, sample_prd):
        """Update multiple settings at once."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            flow="quick",
            tdd=False,
            confirm=False,
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["flow_config"]["flow"] == "quick"
        assert prd["tdd_config"]["enabled"] is False
        assert prd["flow_config"]["confirm"] is False

    def test_update_creates_missing_sections(self, project_with_tools):
        """Update creates config sections in prd.json if they don't exist."""
        mcp, root = project_with_tools
        minimal_prd = {
            "metadata": {"description": "Test"},
            "stories": []
        }
        (root / "prd.json").write_text(
            json.dumps(minimal_prd), encoding="utf-8"
        )
        result = mcp.call(
            "update_configuration",
            flow="full",
            tdd=True,
        )

        assert result["success"] is True
        prd = json.loads((root / "prd.json").read_text(encoding="utf-8"))
        assert prd["flow_config"]["flow"] == "full"
        assert prd["tdd_config"]["enabled"] is True

    def test_update_no_changes_specified(self, project_with_tools, sample_prd):
        """When no changes specified, returns success with no changes message."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        result = mcp.call("update_configuration")

        assert result["success"] is True
        assert "no changes" in result.get("message", "").lower() or result.get("changes") == []

    def test_update_handles_corrupt_prd(self, project_with_tools):
        """Fails gracefully when prd.json is corrupt."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text("{ bad", encoding="utf-8")
        result = mcp.call(
            "update_configuration",
            flow="quick",
        )

        assert result["success"] is False
        assert "error" in result


# ============================================================
# Integration Tests
# ============================================================

class TestDashboardIntegration:
    """Integration tests combining dashboard tools."""

    def test_dashboard_then_config(self, project_with_tools, sample_prd, sample_agents):
        """Use dashboard then check and update config."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        (root / "agents.json").write_text(
            json.dumps(sample_agents), encoding="utf-8"
        )

        # 1. Check dashboard
        dash = mcp.call("dashboard")
        assert dash["success"] is True
        assert dash["active_mode"] == "hybrid-auto"

        # 2. Get config
        cfg = mcp.call("get_configuration")
        assert cfg["success"] is True
        assert cfg["prd_config"]["flow_config"]["flow"] == "full"

        # 3. Update config
        update = mcp.call("update_configuration", flow="quick")
        assert update["success"] is True

        # 4. Verify config changed
        cfg2 = mcp.call("get_configuration")
        assert cfg2["prd_config"]["flow_config"]["flow"] == "quick"

    def test_session_recover_full_flow(self, project_with_tools, sample_prd):
        """Full recovery flow: detect session, check state, get recommended action."""
        mcp, root = project_with_tools
        (root / "prd.json").write_text(
            json.dumps(sample_prd), encoding="utf-8"
        )
        (root / ".hybrid-execution-context.md").write_text(
            "# Recovery Context\nBatch 2 in progress", encoding="utf-8"
        )

        # 1. Detect recovery
        recover = mcp.call("session_recover")
        assert recover["success"] is True
        assert recover["recovery_needed"] is True

        # 2. Check dashboard for details
        dash = mcp.call("dashboard")
        assert dash["success"] is True
        assert dash["active_mode"] == "hybrid-auto"
