#!/usr/bin/env python3
"""
Tests for new MCP Resources (story-003).

Tests the six additional MCP resources registered by resources.py:
1. plan-cascade://design-doc - Full design_doc.json
2. plan-cascade://design-doc/{story_id} - Filtered design context for a story
3. plan-cascade://execution-context - Execution context markdown files
4. plan-cascade://worktree-config/{task_name} - Worktree planning config
5. plan-cascade://spec - spec.json content
6. plan-cascade://spec-interview-state - .state/spec-interview.json content
"""

import importlib
import importlib.util
import json
import pytest
from pathlib import Path
from typing import Any, Dict


# ---------------------------------------------------------------------------
# Direct module loading to avoid transitive __init__.py imports
# ---------------------------------------------------------------------------

_RESOURCES_PATH = (
    Path(__file__).parent.parent / "mcp_server" / "resources.py"
)


def _load_register_function():
    """Load register_resources without triggering mcp_server.__init__ imports."""
    spec = importlib.util.spec_from_file_location(
        "resources", str(_RESOURCES_PATH)
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.register_resources


# ---------------------------------------------------------------------------
# Helpers to register resources and invoke them by URI
# ---------------------------------------------------------------------------

def _register_resources(project_root: Path) -> Dict[str, Any]:
    """
    Register resources on a mock MCP server and return a dict of
    uri -> callable.
    """
    register_resources = _load_register_function()

    resources: Dict[str, Any] = {}

    class FakeMCP:
        """Minimal stand-in for FastMCP that captures registered resources."""

        def resource(self, uri: str):
            def decorator(fn):
                resources[uri] = fn
                return fn
            return decorator

        def tool(self):
            """No-op tool decorator in case resources module registers tools."""
            def decorator(fn):
                return fn
            return decorator

    fake_mcp = FakeMCP()
    register_resources(fake_mcp, project_root)
    return resources


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def project_dir(tmp_path: Path) -> Path:
    """Create a temporary project directory."""
    return tmp_path


@pytest.fixture
def resources(project_dir: Path) -> Dict[str, Any]:
    """Register all resources against the temp project directory."""
    return _register_resources(project_dir)


@pytest.fixture
def sample_design_doc():
    """Create a sample design_doc.json dictionary."""
    return {
        "metadata": {
            "level": "feature",
            "title": "Test Feature Design",
            "created_at": "2026-01-01T00:00:00Z"
        },
        "overview": {
            "title": "Test Feature",
            "summary": "A test feature for unit testing"
        },
        "architecture": {
            "patterns": ["MVC", "Repository"],
            "components": [
                {
                    "name": "AuthService",
                    "description": "Handles authentication",
                    "files": ["src/auth.py"]
                },
                {
                    "name": "UserRepo",
                    "description": "User data access layer",
                    "files": ["src/user_repo.py"]
                },
                {
                    "name": "APIGateway",
                    "description": "API routing",
                    "files": ["src/gateway.py"]
                }
            ]
        },
        "interfaces": {
            "apis": [
                {"id": "api-login", "method": "POST", "path": "/login"},
                {"id": "api-register", "method": "POST", "path": "/register"}
            ],
            "data_models": [
                {"name": "User", "fields": ["id", "email", "password"]},
                {"name": "Session", "fields": ["token", "user_id"]}
            ]
        },
        "decisions": [
            {
                "id": "ADR-001",
                "title": "Use JWT for auth",
                "rationale": "Stateless authentication"
            },
            {
                "id": "ADR-002",
                "title": "Use PostgreSQL",
                "rationale": "Relational data model"
            }
        ],
        "story_mappings": {
            "story-001": {
                "components": ["AuthService", "UserRepo"],
                "decisions": ["ADR-001"],
                "interfaces": ["api-login"]
            },
            "story-002": {
                "components": ["APIGateway"],
                "decisions": ["ADR-002"],
                "interfaces": ["api-register", "User"]
            }
        }
    }


@pytest.fixture
def sample_spec():
    """Create a sample spec.json dictionary."""
    return {
        "overview": {
            "title": "Test Spec",
            "goal": "Build a test feature",
            "problem": "Need testing"
        },
        "scope": {
            "in_scope": ["Feature A", "Feature B"],
            "out_of_scope": ["Feature C"]
        },
        "stories": [
            {"id": "story-001", "title": "First story"},
            {"id": "story-002", "title": "Second story"}
        ]
    }


@pytest.fixture
def sample_interview_state():
    """Create a sample spec-interview.json state dictionary."""
    return {
        "status": "in_progress",
        "flow_level": "standard",
        "first_principles": False,
        "max_questions": 18,
        "question_cursor": 3,
        "history": [
            {"ts": "2026-01-01T00:00:00Z", "question": "overview_goal", "answer": "Build test feature"},
            {"ts": "2026-01-01T00:01:00Z", "question": "overview_problem", "answer": "Need to solve X"},
            {"ts": "2026-01-01T00:02:00Z", "question": "scope_in_scope", "answer": "Module A, Module B"}
        ],
        "description": "Test interview"
    }


# ===================================================================
# Tests: plan-cascade://design-doc
# ===================================================================

class TestDesignDocResource:
    """Tests for the plan-cascade://design-doc resource."""

    def test_resource_registered(self, resources):
        """The design-doc resource should be registered."""
        assert "plan-cascade://design-doc" in resources

    def test_returns_design_doc_content(self, resources, project_dir, sample_design_doc):
        """Should return formatted JSON of design_doc.json."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc"]()
        parsed = json.loads(result)
        assert parsed["metadata"]["level"] == "feature"
        assert parsed["overview"]["title"] == "Test Feature"

    def test_returns_error_when_no_file(self, resources, project_dir):
        """Should return helpful error when design_doc.json doesn't exist."""
        result = resources["plan-cascade://design-doc"]()
        parsed = json.loads(result)
        assert "error" in parsed
        assert "design_generate" in parsed.get("hint", "").lower() or \
               "design_generate" in parsed.get("hint", "") or \
               "design" in result.lower()

    def test_handles_malformed_json(self, resources, project_dir):
        """Should handle malformed JSON gracefully."""
        (project_dir / "design_doc.json").write_text(
            "{ invalid json }", encoding="utf-8"
        )
        result = resources["plan-cascade://design-doc"]()
        parsed = json.loads(result)
        assert "error" in parsed


# ===================================================================
# Tests: plan-cascade://design-doc/{story_id}
# ===================================================================

class TestDesignDocStoryResource:
    """Tests for the plan-cascade://design-doc/{story_id} resource."""

    def test_resource_registered(self, resources):
        """The design-doc/{story_id} resource should be registered."""
        assert "plan-cascade://design-doc/{story_id}" in resources

    def test_returns_filtered_design_for_story(self, resources, project_dir, sample_design_doc):
        """Should return filtered design context based on story_mappings."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc/{story_id}"]("story-001")
        parsed = json.loads(result)

        # Should contain the story mapping
        assert "components" in parsed
        # story-001 maps to AuthService and UserRepo
        component_names = [c["name"] for c in parsed["components"]]
        assert "AuthService" in component_names
        assert "UserRepo" in component_names
        # APIGateway is NOT mapped to story-001
        assert "APIGateway" not in component_names

    def test_filters_decisions_by_story(self, resources, project_dir, sample_design_doc):
        """Should return only decisions mapped to the requested story."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc/{story_id}"]("story-001")
        parsed = json.loads(result)

        decision_ids = [d["id"] for d in parsed["decisions"]]
        assert "ADR-001" in decision_ids
        assert "ADR-002" not in decision_ids

    def test_filters_interfaces_by_story(self, resources, project_dir, sample_design_doc):
        """Should return only interfaces mapped to the requested story."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc/{story_id}"]("story-001")
        parsed = json.loads(result)

        api_ids = [a["id"] for a in parsed.get("apis", [])]
        assert "api-login" in api_ids
        assert "api-register" not in api_ids

    def test_returns_error_for_unmapped_story(self, resources, project_dir, sample_design_doc):
        """Should return error when story_id is not in story_mappings."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc/{story_id}"]("story-999")
        parsed = json.loads(result)
        assert "error" in parsed
        # Should list available stories
        assert "story-001" in str(parsed) or "available" in str(parsed).lower()

    def test_returns_error_when_no_design_doc(self, resources, project_dir):
        """Should return helpful error when design_doc.json doesn't exist."""
        result = resources["plan-cascade://design-doc/{story_id}"]("story-001")
        parsed = json.loads(result)
        assert "error" in parsed
        assert "design" in result.lower()

    def test_includes_overview_and_patterns(self, resources, project_dir, sample_design_doc):
        """Filtered result should still include overview and patterns."""
        (project_dir / "design_doc.json").write_text(
            json.dumps(sample_design_doc), encoding="utf-8"
        )

        result = resources["plan-cascade://design-doc/{story_id}"]("story-001")
        parsed = json.loads(result)

        assert "overview" in parsed
        assert "patterns" in parsed


# ===================================================================
# Tests: plan-cascade://execution-context
# ===================================================================

class TestExecutionContextResource:
    """Tests for the plan-cascade://execution-context resource."""

    def test_resource_registered(self, resources):
        """The execution-context resource should be registered."""
        assert "plan-cascade://execution-context" in resources

    def test_returns_hybrid_context(self, resources, project_dir):
        """Should return .hybrid-execution-context.md content."""
        context_content = "# Hybrid Execution Context\n\nStory-001 complete.\n"
        (project_dir / ".hybrid-execution-context.md").write_text(
            context_content, encoding="utf-8"
        )

        result = resources["plan-cascade://execution-context"]()
        assert "Hybrid Execution Context" in result
        assert "Story-001 complete" in result

    def test_returns_mega_context(self, resources, project_dir):
        """Should return .mega-execution-context.md when hybrid doesn't exist."""
        context_content = "# Mega Execution Context\n\nFeature-001 in progress.\n"
        (project_dir / ".mega-execution-context.md").write_text(
            context_content, encoding="utf-8"
        )

        result = resources["plan-cascade://execution-context"]()
        assert "Mega Execution Context" in result
        assert "Feature-001 in progress" in result

    def test_prefers_hybrid_over_mega(self, resources, project_dir):
        """Should prefer .hybrid-execution-context.md when both exist."""
        (project_dir / ".hybrid-execution-context.md").write_text(
            "# Hybrid context", encoding="utf-8"
        )
        (project_dir / ".mega-execution-context.md").write_text(
            "# Mega context", encoding="utf-8"
        )

        result = resources["plan-cascade://execution-context"]()
        assert "Hybrid context" in result

    def test_returns_error_when_no_context(self, resources, project_dir):
        """Should return helpful message when no execution context exists."""
        result = resources["plan-cascade://execution-context"]()
        # Should mention that no context exists and suggest how to create one
        assert "no" in result.lower() or "not found" in result.lower() or "error" in result.lower()

    def test_both_contexts_returned(self, resources, project_dir):
        """When both exist, should indicate hybrid was returned (or return both)."""
        (project_dir / ".hybrid-execution-context.md").write_text(
            "# Hybrid\nHybrid data", encoding="utf-8"
        )
        (project_dir / ".mega-execution-context.md").write_text(
            "# Mega\nMega data", encoding="utf-8"
        )

        result = resources["plan-cascade://execution-context"]()
        # At minimum, the hybrid content should be present
        assert "Hybrid" in result


# ===================================================================
# Tests: plan-cascade://worktree-config/{task_name}
# ===================================================================

class TestWorktreeConfigResource:
    """Tests for the plan-cascade://worktree-config/{task_name} resource."""

    def test_resource_registered(self, resources):
        """The worktree-config resource should be registered."""
        assert "plan-cascade://worktree-config/{task_name}" in resources

    def test_returns_planning_config(self, resources, project_dir):
        """Should return .planning-config.json for a given worktree task."""
        # Create a worktree directory with planning config
        wt_dir = project_dir / ".worktree" / "my-task"
        wt_dir.mkdir(parents=True)

        config = {
            "version": "1.0.0",
            "task_name": "my-task",
            "target_branch": "main",
            "branch_name": "task/my-task",
            "status": "active",
            "description": "Test task"
        }
        (wt_dir / ".planning-config.json").write_text(
            json.dumps(config), encoding="utf-8"
        )

        result = resources["plan-cascade://worktree-config/{task_name}"]("my-task")
        parsed = json.loads(result)
        assert parsed["task_name"] == "my-task"
        assert parsed["target_branch"] == "main"
        assert parsed["status"] == "active"

    def test_returns_error_for_nonexistent_worktree(self, resources, project_dir):
        """Should return helpful error when the worktree doesn't exist."""
        result = resources["plan-cascade://worktree-config/{task_name}"]("nonexistent")
        parsed = json.loads(result)
        assert "error" in parsed
        assert "worktree" in result.lower() or "not found" in result.lower()

    def test_returns_error_when_no_config_in_worktree(self, resources, project_dir):
        """Should return error when worktree exists but has no .planning-config.json."""
        wt_dir = project_dir / ".worktree" / "empty-task"
        wt_dir.mkdir(parents=True)

        result = resources["plan-cascade://worktree-config/{task_name}"]("empty-task")
        parsed = json.loads(result)
        assert "error" in parsed

    def test_handles_malformed_config(self, resources, project_dir):
        """Should handle malformed JSON in .planning-config.json."""
        wt_dir = project_dir / ".worktree" / "bad-config"
        wt_dir.mkdir(parents=True)
        (wt_dir / ".planning-config.json").write_text(
            "{ bad json }", encoding="utf-8"
        )

        result = resources["plan-cascade://worktree-config/{task_name}"]("bad-config")
        parsed = json.loads(result)
        assert "error" in parsed


# ===================================================================
# Tests: plan-cascade://spec
# ===================================================================

class TestSpecResource:
    """Tests for the plan-cascade://spec resource."""

    def test_resource_registered(self, resources):
        """The spec resource should be registered."""
        assert "plan-cascade://spec" in resources

    def test_returns_spec_content(self, resources, project_dir, sample_spec):
        """Should return spec.json content as formatted JSON."""
        (project_dir / "spec.json").write_text(
            json.dumps(sample_spec), encoding="utf-8"
        )

        result = resources["plan-cascade://spec"]()
        parsed = json.loads(result)
        assert parsed["overview"]["title"] == "Test Spec"
        assert parsed["overview"]["goal"] == "Build a test feature"

    def test_returns_error_when_no_spec(self, resources, project_dir):
        """Should return helpful error when spec.json doesn't exist."""
        result = resources["plan-cascade://spec"]()
        parsed = json.loads(result)
        assert "error" in parsed
        assert "spec" in result.lower()

    def test_handles_malformed_json(self, resources, project_dir):
        """Should handle malformed JSON in spec.json gracefully."""
        (project_dir / "spec.json").write_text(
            "not json", encoding="utf-8"
        )
        result = resources["plan-cascade://spec"]()
        parsed = json.loads(result)
        assert "error" in parsed


# ===================================================================
# Tests: plan-cascade://spec-interview-state
# ===================================================================

class TestSpecInterviewStateResource:
    """Tests for the plan-cascade://spec-interview-state resource."""

    def test_resource_registered(self, resources):
        """The spec-interview-state resource should be registered."""
        assert "plan-cascade://spec-interview-state" in resources

    def test_returns_interview_state(self, resources, project_dir, sample_interview_state):
        """Should return .state/spec-interview.json content."""
        state_dir = project_dir / ".state"
        state_dir.mkdir(parents=True)
        (state_dir / "spec-interview.json").write_text(
            json.dumps(sample_interview_state), encoding="utf-8"
        )

        result = resources["plan-cascade://spec-interview-state"]()
        parsed = json.loads(result)
        assert parsed["status"] == "in_progress"
        assert parsed["flow_level"] == "standard"
        assert parsed["question_cursor"] == 3

    def test_returns_error_when_no_state(self, resources, project_dir):
        """Should return helpful error when .state/spec-interview.json doesn't exist."""
        result = resources["plan-cascade://spec-interview-state"]()
        parsed = json.loads(result)
        assert "error" in parsed
        assert "spec" in result.lower() or "interview" in result.lower()

    def test_handles_malformed_json(self, resources, project_dir):
        """Should handle malformed JSON gracefully."""
        state_dir = project_dir / ".state"
        state_dir.mkdir(parents=True)
        (state_dir / "spec-interview.json").write_text(
            "broken", encoding="utf-8"
        )
        result = resources["plan-cascade://spec-interview-state"]()
        parsed = json.loads(result)
        assert "error" in parsed

    def test_returns_error_when_state_dir_missing(self, resources, project_dir):
        """Should return error when .state/ directory doesn't even exist."""
        result = resources["plan-cascade://spec-interview-state"]()
        parsed = json.loads(result)
        assert "error" in parsed


# ===================================================================
# Tests: Registration
# ===================================================================

class TestRegistration:
    """Tests that all six new resources are properly registered."""

    def test_all_six_new_resources_registered(self, resources):
        """All six new resources should be registered."""
        expected_uris = [
            "plan-cascade://design-doc",
            "plan-cascade://design-doc/{story_id}",
            "plan-cascade://execution-context",
            "plan-cascade://worktree-config/{task_name}",
            "plan-cascade://spec",
            "plan-cascade://spec-interview-state",
        ]
        for uri in expected_uris:
            assert uri in resources, f"Resource {uri} not registered"

    def test_original_resources_still_registered(self, resources):
        """Original resources should still be registered alongside new ones."""
        original_uris = [
            "plan-cascade://prd",
            "plan-cascade://mega-plan",
            "plan-cascade://findings",
            "plan-cascade://progress",
            "plan-cascade://mega-status",
            "plan-cascade://mega-findings",
            "plan-cascade://story/{story_id}",
            "plan-cascade://feature/{feature_id}",
        ]
        for uri in original_uris:
            assert uri in resources, f"Original resource {uri} missing after adding new resources"

    def test_all_resources_are_callable(self, resources):
        """All registered resources should be callable functions."""
        for uri, fn in resources.items():
            assert callable(fn), f"Resource {uri} is not callable"
