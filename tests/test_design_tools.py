#!/usr/bin/env python3
"""
Tests for Design Document MCP Tools.

Tests the four MCP tools registered by design_tools.py:
- design_generate: Generate design_doc.json from PRD or mega-plan
- design_import: Import external documents into design_doc.json schema
- design_review: Review design_doc.json for completeness/consistency
- design_get: Read design_doc.json contents with optional story filtering
"""

import json
import importlib
import importlib.util
import pytest
import sys
import tempfile
from pathlib import Path
from unittest.mock import MagicMock


# ============================================================
# Module Loading Helper
# ============================================================

def _load_design_tools_module():
    """
    Load the design_tools module directly to avoid triggering
    broken imports in sibling modules through __init__.py.
    """
    module_path = Path(__file__).parent.parent / "mcp_server" / "tools" / "design_tools.py"
    spec = importlib.util.spec_from_file_location("design_tools", str(module_path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_design_tools_mod = _load_design_tools_module()
register_design_tools = _design_tools_mod.register_design_tools


# ============================================================
# Fixtures
# ============================================================

@pytest.fixture
def temp_project_dir():
    """Create a temporary project directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def mock_mcp():
    """Create a mock MCP server that captures tool registrations."""
    mcp = MagicMock()
    tools = {}

    def tool_decorator():
        def decorator(func):
            tools[func.__name__] = func
            return func
        return decorator

    mcp.tool = tool_decorator
    mcp._registered_tools = tools
    return mcp


@pytest.fixture
def sample_prd():
    """Create a sample PRD dictionary."""
    return {
        "metadata": {
            "description": "Test Feature Development",
            "created_at": "2026-01-01T00:00:00Z",
            "version": "1.0.0"
        },
        "goal": "Implement test feature with full coverage",
        "objectives": [
            "Create unit tests",
            "Implement core logic",
            "Add integration tests"
        ],
        "stories": [
            {
                "id": "story-001",
                "title": "Set up test infrastructure",
                "description": "Create the testing framework and fixtures",
                "priority": "high",
                "status": "pending",
                "dependencies": [],
                "acceptance_criteria": ["Tests can run"],
                "tags": ["setup"]
            },
            {
                "id": "story-002",
                "title": "Implement core logic",
                "description": "Build the main business logic",
                "priority": "high",
                "status": "pending",
                "dependencies": ["story-001"],
                "acceptance_criteria": ["Logic works correctly"],
                "tags": ["core"]
            },
            {
                "id": "story-003",
                "title": "Add API endpoints",
                "description": "Create REST API endpoints",
                "priority": "medium",
                "status": "pending",
                "dependencies": ["story-002"],
                "acceptance_criteria": ["Endpoints respond correctly"],
                "tags": ["api"]
            }
        ]
    }


@pytest.fixture
def sample_mega_plan():
    """Create a sample mega-plan dictionary."""
    return {
        "goal": "Build Complete Platform",
        "features": [
            {
                "id": "feature-001",
                "name": "Authentication",
                "description": "User authentication system"
            },
            {
                "id": "feature-002",
                "name": "Dashboard",
                "description": "Main dashboard with analytics"
            }
        ]
    }


@pytest.fixture
def sample_design_doc():
    """Create a sample design_doc.json dictionary."""
    return {
        "metadata": {
            "created_at": "2026-01-01T00:00:00Z",
            "version": "1.0.0",
            "source": "ai-generated",
            "level": "feature",
            "prd_reference": "prd.json"
        },
        "overview": {
            "title": "Test Feature Development",
            "summary": "Implement test feature with full coverage",
            "goals": ["Create unit tests", "Implement core logic"],
            "non_goals": []
        },
        "architecture": {
            "components": [
                {
                    "name": "TestRunner",
                    "description": "Runs test suites",
                    "responsibilities": ["Execute tests"],
                    "dependencies": [],
                    "files": ["src/test_runner.py"]
                }
            ],
            "data_flow": "Tests flow through TestRunner",
            "patterns": [
                {
                    "name": "Repository Pattern",
                    "description": "Data access abstraction",
                    "rationale": "Testability"
                }
            ]
        },
        "interfaces": {
            "apis": [
                {
                    "id": "API-001",
                    "method": "GET",
                    "path": "/api/tests",
                    "description": "Get test results",
                    "request_body": {},
                    "response": {}
                }
            ],
            "data_models": [
                {
                    "name": "TestResult",
                    "description": "Test execution result",
                    "fields": {"id": "string", "status": "string"}
                }
            ]
        },
        "decisions": [
            {
                "id": "ADR-F001",
                "title": "Use pytest for testing",
                "context": "Need a testing framework",
                "decision": "Use pytest",
                "rationale": "Industry standard",
                "alternatives_considered": ["unittest"],
                "status": "accepted"
            }
        ],
        "story_mappings": {
            "story-001": {
                "components": ["TestRunner"],
                "decisions": [],
                "interfaces": ["TestResult"]
            },
            "story-002": {
                "components": ["TestRunner"],
                "decisions": ["ADR-F001"],
                "interfaces": []
            },
            "story-003": {
                "components": [],
                "decisions": [],
                "interfaces": ["API-001"]
            }
        }
    }


@pytest.fixture
def register_tools(mock_mcp, temp_project_dir):
    """Register design tools and return the registered tool functions."""
    register_design_tools(mock_mcp, temp_project_dir)
    return mock_mcp._registered_tools


# ============================================================
# Test: Tool Registration
# ============================================================

class TestToolRegistration:
    """Test that all four tools are properly registered."""

    def test_all_four_tools_registered(self, register_tools):
        """All four design tools should be registered with the MCP server."""
        assert "design_generate" in register_tools
        assert "design_import" in register_tools
        assert "design_review" in register_tools
        assert "design_get" in register_tools

    def test_registered_tools_are_callable(self, register_tools):
        """Each registered tool should be callable."""
        for name, func in register_tools.items():
            assert callable(func), f"Tool {name} is not callable"


# ============================================================
# Test: design_generate
# ============================================================

class TestDesignGenerate:
    """Test the design_generate tool."""

    def test_generate_feature_level_from_prd(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should generate a feature-level design doc when PRD exists."""
        # Write sample PRD
        prd_path = temp_project_dir / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"]()

        assert result["success"] is True
        assert "message" in result
        assert "design_doc" in result
        assert "file_path" in result

        # Check the design doc structure
        doc = result["design_doc"]
        assert "metadata" in doc
        assert "overview" in doc
        assert "architecture" in doc
        assert "interfaces" in doc
        assert "decisions" in doc
        assert "story_mappings" in doc

        # Check level detection
        assert doc["metadata"]["level"] == "feature"

    def test_generate_project_level_from_mega_plan(
        self, register_tools, temp_project_dir, sample_mega_plan
    ):
        """Should generate a project-level design doc when mega-plan exists."""
        # Write sample mega-plan
        plan_path = temp_project_dir / "mega-plan.json"
        with open(plan_path, "w", encoding="utf-8") as f:
            json.dump(sample_mega_plan, f)

        result = register_tools["design_generate"](level="project")

        assert result["success"] is True
        doc = result["design_doc"]
        assert doc["metadata"]["level"] == "project"
        assert "feature_mappings" in doc

    def test_generate_auto_detects_feature_level(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should auto-detect feature level when only PRD exists."""
        prd_path = temp_project_dir / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"](level="auto")

        assert result["success"] is True
        assert result["level"] == "feature"

    def test_generate_auto_detects_project_level(
        self, register_tools, temp_project_dir, sample_mega_plan
    ):
        """Should auto-detect project level when mega-plan exists."""
        plan_path = temp_project_dir / "mega-plan.json"
        with open(plan_path, "w", encoding="utf-8") as f:
            json.dump(sample_mega_plan, f)

        result = register_tools["design_generate"](level="auto")

        assert result["success"] is True
        assert result["level"] == "project"

    def test_generate_saves_file(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should save design_doc.json to project root."""
        prd_path = temp_project_dir / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"]()

        assert result["success"] is True
        design_doc_path = temp_project_dir / "design_doc.json"
        assert design_doc_path.exists()

        # Verify file contents match returned doc
        with open(design_doc_path, encoding="utf-8") as f:
            saved_doc = json.load(f)
        assert saved_doc == result["design_doc"]

    def test_generate_includes_story_mappings(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Generated feature doc should include mappings for all PRD stories."""
        prd_path = temp_project_dir / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"]()

        doc = result["design_doc"]
        story_ids = {s["id"] for s in sample_prd["stories"]}
        mapped_ids = set(doc["story_mappings"].keys())
        assert story_ids == mapped_ids

    def test_generate_no_source_files_error(self, register_tools):
        """Should return error when no PRD or mega-plan exists."""
        result = register_tools["design_generate"]()

        assert result["success"] is False
        assert "error" in result

    def test_generate_with_description_override(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should accept optional description override."""
        prd_path = temp_project_dir / "prd.json"
        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"](
            description="Custom design context"
        )

        assert result["success"] is True

    def test_generate_with_custom_prd_path(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should use custom PRD path when provided."""
        custom_path = temp_project_dir / "subdir"
        custom_path.mkdir()
        prd_file = custom_path / "custom-prd.json"
        with open(prd_file, "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_generate"](
            prd_path=str(prd_file)
        )

        assert result["success"] is True
        assert result["design_doc"]["metadata"]["level"] == "feature"


# ============================================================
# Test: design_import
# ============================================================

class TestDesignImport:
    """Test the design_import tool."""

    def test_import_markdown_file(self, register_tools, temp_project_dir):
        """Should convert a markdown file to design_doc.json format."""
        md_content = """# My Design Document

## Overview
This is a test design document for the authentication feature.

## Architecture
The system uses a layered architecture.

## Components
- AuthController: Handles HTTP requests
- AuthService: Business logic layer

## Decisions
ADR-001: Use JWT for authentication
We decided to use JWT tokens for stateless authentication.
"""
        md_path = temp_project_dir / "design.md"
        with open(md_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        result = register_tools["design_import"](source_path=str(md_path))

        assert result["success"] is True
        assert "design_doc" in result
        assert "file_path" in result
        assert result["source_format"] == "markdown"

        doc = result["design_doc"]
        assert doc["overview"]["title"] == "My Design Document"

    def test_import_json_file(self, register_tools, temp_project_dir):
        """Should convert a JSON file to design_doc.json format."""
        json_content = {
            "title": "API Gateway Design",
            "description": "Design for the API gateway service",
            "goals": ["Route requests", "Rate limiting"],
            "components": [
                {"name": "Router", "description": "Request routing"}
            ]
        }
        json_path = temp_project_dir / "design.json"
        with open(json_path, "w", encoding="utf-8") as f:
            json.dump(json_content, f)

        result = register_tools["design_import"](source_path=str(json_path))

        assert result["success"] is True
        assert result["source_format"] == "json"
        doc = result["design_doc"]
        assert doc["overview"]["title"] == "API Gateway Design"

    def test_import_html_file(self, register_tools, temp_project_dir):
        """Should convert an HTML file to design_doc.json format."""
        html_content = """
<html>
<head><title>Design Spec</title></head>
<body>
<h1>System Design</h1>
<h2>Overview</h2>
<p>A comprehensive system design.</p>
<h2>Goals</h2>
<ul>
<li>Performance</li>
<li>Reliability</li>
</ul>
</body>
</html>
"""
        html_path = temp_project_dir / "design.html"
        with open(html_path, "w", encoding="utf-8") as f:
            f.write(html_content)

        result = register_tools["design_import"](source_path=str(html_path))

        assert result["success"] is True
        assert result["source_format"] == "html"

    def test_import_saves_file(self, register_tools, temp_project_dir):
        """Should save the converted document to design_doc.json."""
        md_path = temp_project_dir / "design.md"
        with open(md_path, "w", encoding="utf-8") as f:
            f.write("# Simple Design\n\nA simple design document.")

        result = register_tools["design_import"](source_path=str(md_path))

        assert result["success"] is True
        design_doc_path = temp_project_dir / "design_doc.json"
        assert design_doc_path.exists()

    def test_import_nonexistent_file_error(self, register_tools, temp_project_dir):
        """Should return error for non-existent source file."""
        result = register_tools["design_import"](
            source_path=str(temp_project_dir / "nonexistent.md")
        )

        assert result["success"] is False
        assert "error" in result

    def test_import_with_level_parameter(self, register_tools, temp_project_dir):
        """Should accept level parameter for the imported document."""
        md_path = temp_project_dir / "design.md"
        with open(md_path, "w", encoding="utf-8") as f:
            f.write("# Project Design\n\nProject-level design.")

        result = register_tools["design_import"](
            source_path=str(md_path),
            level="project"
        )

        assert result["success"] is True
        doc = result["design_doc"]
        assert doc["metadata"]["level"] == "project"


# ============================================================
# Test: design_review
# ============================================================

class TestDesignReview:
    """Test the design_review tool."""

    def test_review_valid_design_doc(
        self, register_tools, temp_project_dir, sample_design_doc, sample_prd
    ):
        """Should return valid review for a complete design doc with PRD."""
        # Write both design doc and PRD
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)
        with open(temp_project_dir / "prd.json", "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_review"]()

        assert result["success"] is True
        assert "is_valid" in result
        assert "story_mapping_coverage" in result
        assert "component_completeness" in result
        assert "warnings" in result

    def test_review_story_mapping_coverage(
        self, register_tools, temp_project_dir, sample_design_doc, sample_prd
    ):
        """Should report story mapping coverage correctly."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)
        with open(temp_project_dir / "prd.json", "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_review"]()

        coverage = result["story_mapping_coverage"]
        assert coverage["total_stories"] == 3
        assert coverage["mapped_stories"] == 3
        assert len(coverage["unmapped"]) == 0

    def test_review_unmapped_stories_warning(
        self, register_tools, temp_project_dir, sample_prd
    ):
        """Should report unmapped stories when design doc is missing mappings."""
        # Create design doc without story-003 mapping
        incomplete_doc = {
            "metadata": {
                "created_at": "2026-01-01T00:00:00Z",
                "version": "1.0.0",
                "source": "ai-generated",
                "level": "feature",
                "prd_reference": "prd.json"
            },
            "overview": {"title": "Test", "summary": "", "goals": [], "non_goals": []},
            "architecture": {"components": [], "data_flow": "", "patterns": []},
            "interfaces": {"apis": [], "data_models": []},
            "decisions": [],
            "story_mappings": {
                "story-001": {"components": [], "decisions": [], "interfaces": []},
                "story-002": {"components": [], "decisions": [], "interfaces": []}
            }
        }
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(incomplete_doc, f)
        with open(temp_project_dir / "prd.json", "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_review"]()

        assert result["success"] is True
        coverage = result["story_mapping_coverage"]
        assert "story-003" in coverage["unmapped"]

    def test_review_component_completeness(
        self, register_tools, temp_project_dir, sample_design_doc, sample_prd
    ):
        """Should report component completeness metrics."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)
        with open(temp_project_dir / "prd.json", "w", encoding="utf-8") as f:
            json.dump(sample_prd, f)

        result = register_tools["design_review"]()

        comp = result["component_completeness"]
        assert comp["total"] == 1
        assert comp["with_files"] == 1

    def test_review_no_design_doc_error(self, register_tools):
        """Should return error when no design_doc.json exists."""
        result = register_tools["design_review"]()

        assert result["success"] is False
        assert "error" in result

    def test_review_returns_validation_errors(
        self, register_tools, temp_project_dir
    ):
        """Should return validation errors for malformed design doc."""
        # Write a malformed design doc (missing required sections)
        bad_doc = {"metadata": {"level": "feature", "source": "test"}}
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(bad_doc, f)

        result = register_tools["design_review"]()

        assert result["success"] is True
        assert result["is_valid"] is False
        assert len(result["errors"]) > 0


# ============================================================
# Test: design_get
# ============================================================

class TestDesignGet:
    """Test the design_get tool."""

    def test_get_full_design_doc(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Should return the full design doc when no story_id given."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"]()

        assert result["success"] is True
        assert "design_doc" in result
        assert result["filtered"] is False
        assert result["design_doc"] == sample_design_doc

    def test_get_filtered_by_story_id(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Should return filtered context for a specific story_id."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"](story_id="story-001")

        assert result["success"] is True
        assert result["filtered"] is True
        assert result["story_id"] == "story-001"

        # Should include the story mapping info
        doc = result["design_doc"]
        assert "story_mapping" in doc
        assert "TestRunner" in doc["story_mapping"]["components"]

    def test_get_filtered_includes_relevant_components(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Filtered result should include components referenced by the story."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"](story_id="story-001")

        doc = result["design_doc"]
        assert "components" in doc
        component_names = [c["name"] for c in doc["components"]]
        assert "TestRunner" in component_names

    def test_get_filtered_includes_relevant_decisions(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Filtered result should include decisions referenced by the story."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"](story_id="story-002")

        doc = result["design_doc"]
        assert "decisions" in doc
        decision_ids = [d["id"] for d in doc["decisions"]]
        assert "ADR-F001" in decision_ids

    def test_get_nonexistent_story_id(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Should return error for a story_id not in story_mappings."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"](story_id="story-999")

        assert result["success"] is False
        assert "error" in result

    def test_get_no_design_doc_error(self, register_tools):
        """Should return error when no design_doc.json exists."""
        result = register_tools["design_get"]()

        assert result["success"] is False
        assert "error" in result

    def test_get_returns_success_boolean(
        self, register_tools, temp_project_dir, sample_design_doc
    ):
        """Every response should include a 'success' boolean."""
        with open(temp_project_dir / "design_doc.json", "w", encoding="utf-8") as f:
            json.dump(sample_design_doc, f)

        result = register_tools["design_get"]()
        assert isinstance(result["success"], bool)


# ============================================================
# Test: Module Integration
# ============================================================

class TestModuleIntegration:
    """Test that the module integrates properly with the server."""

    def test_register_design_tools_is_callable(self):
        """register_design_tools should be a callable function."""
        assert callable(register_design_tools)

    def test_register_function_signature(self):
        """register_design_tools should accept (mcp, project_root) args."""
        import inspect
        sig = inspect.signature(register_design_tools)
        params = list(sig.parameters.keys())
        assert "mcp" in params
        assert "project_root" in params
