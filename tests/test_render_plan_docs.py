#!/usr/bin/env python3
"""
Tests for render-plan-docs.py — JSON-to-Markdown rendering.

Tests the three pure rendering functions:
  - render_prd_md(prd: dict) -> str
  - render_design_doc_md(doc: dict) -> str
  - render_mega_plan_md(mega: dict) -> str

Also tests helper functions: _safe_list, _safe_str, _render_list,
_truncate, and calculate_batches.
"""

import importlib.util
from pathlib import Path

import pytest


# ---------------------------------------------------------------------------
# Module loading (script is not a package module)
# ---------------------------------------------------------------------------

def _load_render_module():
    spec = importlib.util.spec_from_file_location(
        "render_plan_docs",
        Path(__file__).parent.parent / "skills" / "hybrid-ralph" / "scripts" / "render-plan-docs.py",
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


mod = _load_render_module()

render_prd_md = mod.render_prd_md
render_design_doc_md = mod.render_design_doc_md
render_mega_plan_md = mod.render_mega_plan_md
calculate_batches = mod.calculate_batches
_safe_list = mod._safe_list
_safe_str = mod._safe_str
_render_list = mod._render_list
_truncate = mod._truncate


# ---------------------------------------------------------------------------
# Fixture factories
# ---------------------------------------------------------------------------

def make_story(
    id: str,
    title: str = "Story Title",
    *,
    priority: str = "medium",
    description: str = "A story description.",
    dependencies: list[str] | None = None,
    acceptance_criteria: list[str] | None = None,
    context_estimate: str = "",
) -> dict:
    """Build a minimal story dict."""
    story: dict = {
        "id": id,
        "title": title,
        "priority": priority,
        "description": description,
        "dependencies": dependencies or [],
    }
    if acceptance_criteria is not None:
        story["acceptance_criteria"] = acceptance_criteria
    if context_estimate:
        story["context_estimate"] = context_estimate
    return story


def make_prd(
    goal: str = "Build a feature",
    stories: list[dict] | None = None,
    objectives: list[str] | None = None,
    metadata: dict | None = None,
) -> dict:
    """Build a minimal PRD dict."""
    return {
        "goal": goal,
        "metadata": metadata or {"created_at": "2024-01-01T00:00:00Z"},
        "stories": stories or [],
        "objectives": objectives or [],
    }


def make_feature(
    id: str,
    title: str = "Feature Title",
    *,
    priority: str = "medium",
    description: str = "A feature description.",
    dependencies: list[str] | None = None,
) -> dict:
    """Build a minimal mega-plan feature dict."""
    return {
        "id": id,
        "title": title,
        "priority": priority,
        "description": description,
        "dependencies": dependencies or [],
    }


def make_design_doc(
    *,
    level: str = "feature",
    title: str = "Technical Design",
    summary: str = "A design document.",
    goals: list[str] | None = None,
    non_goals: list[str] | None = None,
    components: list[dict] | None = None,
    patterns: list[dict] | None = None,
    data_flow: str = "",
    system_overview: str = "",
    apis: list[dict] | None = None,
    data_models: list[dict] | None = None,
    decisions: list[dict] | None = None,
    story_mappings: dict | None = None,
    feature_mappings: dict | None = None,
) -> dict:
    """Build a minimal design_doc dict."""
    doc: dict = {
        "metadata": {"level": level},
        "overview": {
            "title": title,
            "summary": summary,
        },
        "architecture": {
            "components": components or [],
            "patterns": patterns or [],
        },
        "interfaces": {},
        "decisions": decisions or [],
    }
    if goals is not None:
        doc["overview"]["goals"] = goals
    if non_goals is not None:
        doc["overview"]["non_goals"] = non_goals
    if data_flow:
        doc["architecture"]["data_flow"] = data_flow
    if system_overview:
        doc["architecture"]["system_overview"] = system_overview
    if apis is not None:
        doc["interfaces"]["apis"] = apis
    if data_models is not None:
        doc["interfaces"]["data_models"] = data_models
    if story_mappings is not None:
        doc["story_mappings"] = story_mappings
    if feature_mappings is not None:
        doc["feature_mappings"] = feature_mappings
    return doc


def make_decision(
    id: str = "ADR-001",
    title: str = "Use REST",
    *,
    status: str = "accepted",
    context: str = "Need an API style",
    decision: str = "Use REST",
    rationale: str = "Widely supported",
    alternatives: list[str] | None = None,
) -> dict:
    d: dict = {
        "id": id,
        "title": title,
        "status": status,
        "context": context,
        "decision": decision,
        "rationale": rationale,
    }
    if alternatives is not None:
        d["alternatives_considered"] = alternatives
    return d


# =============================================================================
# Tests for helper functions
# =============================================================================


class TestSafeList:
    """Tests for _safe_list helper."""

    def test_list_passthrough(self):
        assert _safe_list(["a", "b"]) == ["a", "b"]

    def test_empty_list(self):
        assert _safe_list([]) == []

    def test_comma_separated_string(self):
        assert _safe_list("a, b, c") == ["a", "b", "c"]

    def test_single_string(self):
        assert _safe_list("hello") == ["hello"]

    def test_string_with_empty_parts(self):
        assert _safe_list("a,,b, ,c") == ["a", "b", "c"]

    def test_none(self):
        assert _safe_list(None) == []

    def test_integer(self):
        assert _safe_list(42) == []

    def test_dict(self):
        assert _safe_list({"key": "val"}) == []


class TestSafeStr:
    """Tests for _safe_str helper."""

    def test_normal_string(self):
        assert _safe_str("hello") == "hello"

    def test_none_default_empty(self):
        assert _safe_str(None) == ""

    def test_none_custom_default(self):
        assert _safe_str(None, "fallback") == "fallback"

    def test_strips_whitespace(self):
        assert _safe_str("  hello  ") == "hello"

    def test_integer(self):
        assert _safe_str(42) == "42"


class TestRenderList:
    """Tests for _render_list helper."""

    def test_normal_list(self):
        result = _render_list(["alpha", "beta"])
        assert result == "- alpha\n- beta\n"

    def test_empty_list(self):
        assert _render_list([]) == "_(none)_\n"

    def test_custom_prefix(self):
        result = _render_list(["x"], prefix="* ")
        assert result == "* x\n"


class TestTruncate:
    """Tests for _truncate helper."""

    def test_short_text(self):
        assert _truncate("hi", 10) == "hi"

    def test_exact_length(self):
        assert _truncate("abcde", 5) == "abcde"

    def test_truncated(self):
        assert _truncate("abcdefghij", 7) == "abcd..."

    def test_length_3(self):
        assert _truncate("abcdef", 3) == "..."


# =============================================================================
# Tests for calculate_batches
# =============================================================================


class TestCalculateBatches:
    """Tests for batch grouping based on dependencies."""

    def test_empty_items(self):
        assert calculate_batches([]) == []

    def test_no_dependencies(self):
        items = [
            {"id": "s1", "dependencies": []},
            {"id": "s2", "dependencies": []},
            {"id": "s3", "dependencies": []},
        ]
        batches = calculate_batches(items)
        assert len(batches) == 1
        assert len(batches[0]) == 3

    def test_linear_chain(self):
        items = [
            {"id": "s1", "dependencies": []},
            {"id": "s2", "dependencies": ["s1"]},
            {"id": "s3", "dependencies": ["s2"]},
        ]
        batches = calculate_batches(items)
        assert len(batches) == 3
        assert [b[0]["id"] for b in batches] == ["s1", "s2", "s3"]

    def test_diamond_dependency(self):
        """s1 -> s2, s1 -> s3, s2+s3 -> s4"""
        items = [
            {"id": "s1", "dependencies": []},
            {"id": "s2", "dependencies": ["s1"]},
            {"id": "s3", "dependencies": ["s1"]},
            {"id": "s4", "dependencies": ["s2", "s3"]},
        ]
        batches = calculate_batches(items)
        assert len(batches) == 3
        batch_ids = [[item["id"] for item in b] for b in batches]
        assert batch_ids[0] == ["s1"]
        assert set(batch_ids[1]) == {"s2", "s3"}
        assert batch_ids[2] == ["s4"]

    def test_priority_ordering_within_batch(self):
        items = [
            {"id": "s1", "priority": "low", "dependencies": []},
            {"id": "s2", "priority": "high", "dependencies": []},
            {"id": "s3", "priority": "medium", "dependencies": []},
        ]
        batches = calculate_batches(items)
        assert len(batches) == 1
        ids = [item["id"] for item in batches[0]]
        assert ids == ["s2", "s3", "s1"]

    def test_circular_dependency_does_not_loop_forever(self):
        """Circular deps should still complete (fallback picks remaining)."""
        items = [
            {"id": "s1", "dependencies": ["s2"]},
            {"id": "s2", "dependencies": ["s1"]},
        ]
        batches = calculate_batches(items)
        all_ids = {item["id"] for batch in batches for item in batch}
        assert all_ids == {"s1", "s2"}

    def test_custom_id_key(self):
        items = [
            {"fid": "f1", "dependencies": []},
            {"fid": "f2", "dependencies": ["f1"]},
        ]
        batches = calculate_batches(items, id_key="fid")
        assert len(batches) == 2
        assert batches[0][0]["fid"] == "f1"
        assert batches[1][0]["fid"] == "f2"


# =============================================================================
# Tests for render_prd_md
# =============================================================================


class TestRenderPrdMd:
    """Tests for PRD rendering."""

    def test_full_prd(self):
        """Normal case: full PRD with all fields populated."""
        prd = make_prd(
            goal="Implement Auth System",
            objectives=["Secure login", "Token refresh"],
            stories=[
                make_story(
                    "story-1",
                    "Login endpoint",
                    priority="high",
                    description="Build POST /login",
                    acceptance_criteria=["Returns JWT", "Validates credentials"],
                    context_estimate="medium",
                ),
                make_story(
                    "story-2",
                    "Token refresh",
                    priority="medium",
                    description="Build POST /refresh",
                    dependencies=["story-1"],
                    acceptance_criteria=["Rotates token"],
                ),
            ],
            metadata={"created_at": "2024-06-15T10:00:00Z"},
        )

        md = render_prd_md(prd)

        # Title
        assert "# PRD: Implement Auth System" in md
        # Metadata line
        assert "Generated: 2024-06-15T10:00:00Z" in md
        assert "Stories: 2" in md
        assert "Batches: 2" in md
        # Objectives
        assert "## Objectives" in md
        assert "- Secure login" in md
        assert "- Token refresh" in md
        # Stories
        assert "### story-1: Login endpoint [HIGH]" in md
        assert "Build POST /login" in md
        assert "[ ] Returns JWT" in md
        assert "[ ] Validates credentials" in md
        assert "**Estimated Size:** medium" in md
        assert "### story-2: Token refresh [MEDIUM]" in md
        assert "**Dependencies:** story-1" in md
        # Execution plan
        assert "## Execution Plan" in md
        assert "Batch 1: story-1" in md
        assert "Batch 2: story-2" in md

    def test_minimal_prd(self):
        """Minimal case: only required fields."""
        prd = {"stories": []}
        md = render_prd_md(prd)

        assert "# PRD: Untitled" in md
        assert "Stories: 0" in md
        assert "Batches: 0" in md
        assert "_(none)_" in md

    def test_prd_no_stories(self):
        """Empty stories list renders _(none)_."""
        prd = make_prd(stories=[])
        md = render_prd_md(prd)
        assert "_(none)_" in md
        assert "## Execution Plan" in md

    def test_prd_story_no_dependencies(self):
        """Story without dependencies omits Dependencies line."""
        prd = make_prd(stories=[make_story("s1", "Solo story", dependencies=[])])
        md = render_prd_md(prd)
        assert "**Dependencies:**" not in md

    def test_prd_story_multiple_dependencies(self):
        """Story with multiple dependencies lists them comma-separated."""
        prd = make_prd(
            stories=[
                make_story("s1", "First"),
                make_story("s2", "Second"),
                make_story("s3", "Third", dependencies=["s1", "s2"]),
            ]
        )
        md = render_prd_md(prd)
        assert "**Dependencies:** s1, s2" in md

    def test_prd_no_objectives(self):
        """Missing objectives section does not appear."""
        prd = make_prd(objectives=[])
        md = render_prd_md(prd)
        assert "## Objectives" not in md

    def test_prd_objectives_as_comma_string(self):
        """Objectives provided as comma-separated string via _safe_list."""
        prd = make_prd()
        prd["objectives"] = "Goal A, Goal B"
        md = render_prd_md(prd)
        assert "- Goal A" in md
        assert "- Goal B" in md

    def test_prd_none_goal(self):
        """None goal defaults to 'Untitled'."""
        prd = make_prd()
        prd["goal"] = None
        md = render_prd_md(prd)
        assert "# PRD: Untitled" in md

    def test_prd_missing_metadata(self):
        """Missing metadata does not crash."""
        prd = {"stories": [make_story("s1", "Solo")]}
        md = render_prd_md(prd)
        assert "Generated: N/A" in md

    def test_prd_story_no_acceptance_criteria(self):
        """Story without acceptance_criteria omits that section."""
        prd = make_prd(stories=[make_story("s1", "No AC")])
        md = render_prd_md(prd)
        assert "**Acceptance Criteria:**" not in md

    def test_prd_story_no_description(self):
        """Story with empty description still renders ID/title line."""
        story = make_story("s1", "Empty Desc", description="")
        prd = make_prd(stories=[story])
        md = render_prd_md(prd)
        assert "### s1: Empty Desc [MEDIUM]" in md

    def test_prd_batch_parallel_marker(self):
        """Batch with multiple stories gets (Parallel) marker."""
        prd = make_prd(
            stories=[
                make_story("s1", "A"),
                make_story("s2", "B"),
            ]
        )
        md = render_prd_md(prd)
        assert "(Parallel)" in md

    def test_prd_batch_single_no_parallel_marker(self):
        """Batch with single story does NOT get (Parallel) marker."""
        prd = make_prd(stories=[make_story("s1", "Solo")])
        md = render_prd_md(prd)
        assert "(Parallel)" not in md

    def test_prd_ends_with_newline(self):
        """Output ends with exactly one newline."""
        prd = make_prd(stories=[make_story("s1", "A")])
        md = render_prd_md(prd)
        assert md.endswith("\n")
        assert not md.endswith("\n\n")

    def test_prd_no_trailing_whitespace(self):
        """No lines have trailing whitespace."""
        prd = make_prd(
            stories=[
                make_story("s1", "A", acceptance_criteria=["AC1"]),
            ]
        )
        md = render_prd_md(prd)
        for line in md.splitlines():
            assert line == line.rstrip(), f"Trailing whitespace on: {line!r}"

    def test_prd_acceptance_criteria_as_string(self):
        """acceptance_criteria as comma-separated string is handled."""
        story = make_story("s1", "AC string")
        story["acceptance_criteria"] = "Criterion A, Criterion B"
        prd = make_prd(stories=[story])
        md = render_prd_md(prd)
        assert "[ ] Criterion A" in md
        assert "[ ] Criterion B" in md


# =============================================================================
# Tests for render_design_doc_md
# =============================================================================


class TestRenderDesignDocMd:
    """Tests for design document rendering."""

    def test_full_design_doc(self):
        """Normal case: full design doc with all sections populated."""
        doc = make_design_doc(
            level="feature",
            title="Auth Module Design",
            summary="Design for the authentication module.",
            goals=["Secure", "Scalable"],
            non_goals=["Mobile app"],
            components=[
                {
                    "name": "AuthController",
                    "description": "Handles auth routes",
                    "responsibilities": ["Login", "Logout"],
                    "dependencies": ["UserService"],
                    "files": ["src/auth/controller.ts"],
                },
            ],
            patterns=[
                {
                    "name": "Repository Pattern",
                    "description": "Abstract data access",
                    "rationale": "Testability",
                },
            ],
            data_flow="Request -> Controller -> Service -> DB",
            apis=[
                {"method": "POST", "path": "/login", "description": "User login"},
            ],
            data_models=[
                {
                    "name": "User",
                    "description": "User entity",
                    "fields": {"id": "string", "email": "string"},
                },
            ],
            decisions=[
                make_decision(
                    "ADR-001",
                    "Use JWT",
                    status="accepted",
                    context="Need stateless auth",
                    decision="Use JWT tokens",
                    rationale="No server-side sessions",
                    alternatives=["Session cookies", "OAuth only"],
                ),
            ],
            story_mappings={
                "story-1": {
                    "components": ["AuthController"],
                    "decisions": ["ADR-001"],
                    "interfaces": ["/login"],
                },
            },
        )

        md = render_design_doc_md(doc)

        # Header
        assert "# Technical Design: Auth Module Design" in md
        assert "Level: feature" in md
        assert "Components: 1" in md
        assert "Patterns: 1" in md
        assert "ADRs: 1" in md
        # Overview
        assert "## Overview" in md
        assert "Design for the authentication module." in md
        assert "### Goals" in md
        assert "- Secure" in md
        assert "### Non-Goals" in md
        assert "- Mobile app" in md
        # Architecture
        assert "## Architecture" in md
        assert "#### AuthController" in md
        assert "Handles auth routes" in md
        assert "Responsibilities: Login, Logout" in md
        assert "Dependencies: UserService" in md
        assert "`src/auth/controller.ts`" in md
        # Patterns
        assert "### Patterns" in md
        assert "**Repository Pattern**" in md
        assert "Abstract data access" in md
        assert "_Testability_" in md
        # Data Flow
        assert "### Data Flow" in md
        assert "Request -> Controller -> Service -> DB" in md
        # APIs
        assert "### APIs" in md
        assert "| POST | /login | User login |" in md
        # Data Models
        assert "### Data Models" in md
        assert "#### User" in md
        assert "| id | string |" in md
        assert "| email | string |" in md
        # ADRs
        assert "## Architecture Decisions" in md
        assert "### ADR-001: Use JWT [accepted]" in md
        assert "**Context**: Need stateless auth" in md
        assert "**Decision**: Use JWT tokens" in md
        assert "**Rationale**: No server-side sessions" in md
        assert "**Alternatives Considered**: Session cookies, OAuth only" in md
        # Story Mappings
        assert "## Story Mappings" in md
        assert "| story-1 | AuthController | ADR-001 | /login |" in md

    def test_minimal_design_doc(self):
        """Minimal case: only required structure, no optional sections."""
        doc = {
            "metadata": {},
            "overview": {},
            "architecture": {},
            "interfaces": {},
            "decisions": [],
        }
        md = render_design_doc_md(doc)

        assert "# Technical Design: Technical Design" in md
        assert "Level: feature" in md
        assert "Components: 0" in md
        # No crash, no optional sections
        assert "## Overview" not in md  # no summary
        assert "### Goals" not in md
        assert "### Components" not in md
        assert "### Patterns" not in md
        assert "## Interfaces" not in md
        assert "## Architecture Decisions" not in md

    def test_project_level_system_overview(self):
        """Project-level doc with system_overview renders it."""
        doc = make_design_doc(
            level="project",
            system_overview="Microservices architecture with API gateway.",
        )
        md = render_design_doc_md(doc)
        assert "Level: project" in md
        assert "Microservices architecture with API gateway." in md

    def test_feature_mappings(self):
        """feature_mappings used when story_mappings absent."""
        doc = make_design_doc(
            feature_mappings={
                "feat-1": {
                    "components": ["CompA"],
                    "decisions": ["ADR-001"],
                    "interfaces": ["/api"],
                },
            },
        )
        md = render_design_doc_md(doc)
        assert "## Story Mappings" in md
        assert "| feat-1 | CompA | ADR-001 | /api |" in md

    def test_story_mappings_takes_precedence(self):
        """story_mappings used even if feature_mappings also present."""
        doc = make_design_doc(
            story_mappings={
                "s1": {"components": ["X"], "decisions": [], "interfaces": []},
            },
            feature_mappings={
                "f1": {"components": ["Y"], "decisions": [], "interfaces": []},
            },
        )
        md = render_design_doc_md(doc)
        assert "| s1 |" in md
        assert "| f1 |" not in md

    def test_empty_components(self):
        """Empty components list omits the section."""
        doc = make_design_doc(components=[])
        md = render_design_doc_md(doc)
        assert "### Components" not in md

    def test_empty_patterns(self):
        """Empty patterns list omits the section."""
        doc = make_design_doc(patterns=[])
        md = render_design_doc_md(doc)
        assert "### Patterns" not in md

    def test_empty_decisions(self):
        """Empty decisions list omits the section."""
        doc = make_design_doc(decisions=[])
        md = render_design_doc_md(doc)
        assert "## Architecture Decisions" not in md

    def test_decision_no_alternatives(self):
        """Decision without alternatives_considered omits that line."""
        doc = make_design_doc(
            decisions=[make_decision("ADR-001", "Test")],
        )
        md = render_design_doc_md(doc)
        assert "**Alternatives Considered**" not in md

    def test_decision_none_alternatives(self):
        """Decision with alternatives_considered=None handled via _safe_list."""
        decision = make_decision("ADR-001", "Test")
        decision["alternatives_considered"] = None
        doc = make_design_doc(decisions=[decision])
        md = render_design_doc_md(doc)
        assert "**Alternatives Considered**" not in md

    def test_decision_alternatives_as_string(self):
        """alternatives_considered as comma-separated string is handled."""
        decision = make_decision("ADR-001", "Test")
        decision["alternatives_considered"] = "Option A, Option B"
        doc = make_design_doc(decisions=[decision])
        md = render_design_doc_md(doc)
        assert "**Alternatives Considered**: Option A, Option B" in md

    def test_data_models_fields_as_dict(self):
        """Data model with fields as dict renders correctly."""
        doc = make_design_doc(
            data_models=[
                {
                    "name": "Order",
                    "description": "An order",
                    "fields": {"id": "int", "total": "float"},
                },
            ],
        )
        md = render_design_doc_md(doc)
        assert "#### Order" in md
        assert "| id | int |" in md
        assert "| total | float |" in md

    def test_data_models_fields_as_list_of_dicts(self):
        """Data model with fields as list of {name, type} dicts."""
        doc = make_design_doc(
            data_models=[
                {
                    "name": "Item",
                    "fields": [
                        {"name": "sku", "type": "string"},
                        {"name": "qty", "type": "int"},
                    ],
                },
            ],
        )
        md = render_design_doc_md(doc)
        assert "| sku | string |" in md
        assert "| qty | int |" in md

    def test_data_models_fields_as_list_of_strings(self):
        """Data model with fields as list of plain strings."""
        doc = make_design_doc(
            data_models=[
                {
                    "name": "Simple",
                    "fields": ["field_a", "field_b"],
                },
            ],
        )
        md = render_design_doc_md(doc)
        assert "| field_a | - |" in md
        assert "| field_b | - |" in md

    def test_data_models_empty_fields(self):
        """Data model with empty fields dict omits table."""
        doc = make_design_doc(
            data_models=[{"name": "Empty", "fields": {}}],
        )
        md = render_design_doc_md(doc)
        assert "#### Empty" in md
        assert "| Field | Type |" not in md

    def test_shared_data_models_fallback(self):
        """shared_data_models used when data_models is empty (mega-level)."""
        doc = make_design_doc()
        doc["interfaces"]["shared_data_models"] = [
            {"name": "SharedModel", "description": "Shared", "fields": {"x": "int"}},
        ]
        md = render_design_doc_md(doc)
        assert "#### SharedModel" in md
        assert "| x | int |" in md

    def test_component_minimal_fields(self):
        """Component with only name renders without crash."""
        doc = make_design_doc(
            components=[{"name": "Minimal"}],
        )
        md = render_design_doc_md(doc)
        assert "#### Minimal" in md

    def test_component_all_fields(self):
        """Component with all optional fields renders everything."""
        doc = make_design_doc(
            components=[
                {
                    "name": "Full",
                    "description": "Full component",
                    "responsibilities": ["R1", "R2"],
                    "dependencies": ["D1"],
                    "files": ["a.ts", "b.ts"],
                },
            ],
        )
        md = render_design_doc_md(doc)
        assert "Full component" in md
        assert "Responsibilities: R1, R2" in md
        assert "Dependencies: D1" in md
        assert "`a.ts`" in md
        assert "`b.ts`" in md

    def test_pattern_without_rationale(self):
        """Pattern without rationale omits the italic rationale part."""
        doc = make_design_doc(
            patterns=[{"name": "Observer", "description": "Event-based"}],
        )
        md = render_design_doc_md(doc)
        assert "**Observer**: Event-based" in md
        assert "_" not in md.split("**Observer**")[1].split("\n")[0]

    def test_no_summary_omits_overview(self):
        """Empty summary omits the Overview section."""
        doc = make_design_doc(summary="")
        md = render_design_doc_md(doc)
        assert "## Overview" not in md

    def test_no_data_flow_omits_section(self):
        """Empty data_flow omits the Data Flow section."""
        doc = make_design_doc(data_flow="")
        md = render_design_doc_md(doc)
        assert "### Data Flow" not in md

    def test_mapping_with_empty_values(self):
        """Story mapping with empty component/decision/interface lists renders dashes."""
        doc = make_design_doc(
            story_mappings={
                "s1": {"components": [], "decisions": [], "interfaces": []},
            },
        )
        md = render_design_doc_md(doc)
        assert "| s1 | - | - | - |" in md

    def test_ends_with_newline(self):
        """Output ends with exactly one newline."""
        doc = make_design_doc()
        md = render_design_doc_md(doc)
        assert md.endswith("\n")
        assert not md.endswith("\n\n")

    def test_no_trailing_whitespace(self):
        """No lines have trailing whitespace."""
        doc = make_design_doc(
            components=[{"name": "C", "description": "D"}],
            decisions=[make_decision()],
        )
        md = render_design_doc_md(doc)
        for line in md.splitlines():
            assert line == line.rstrip(), f"Trailing whitespace on: {line!r}"

    def test_none_fields_in_overview(self):
        """None values in overview fields do not crash."""
        doc = make_design_doc()
        doc["overview"]["title"] = None
        doc["overview"]["summary"] = None
        md = render_design_doc_md(doc)
        assert "# Technical Design: Technical Design" in md

    def test_decision_missing_optional_fields(self):
        """Decision with no context/decision/rationale still renders header."""
        decision = {"id": "ADR-099", "title": "Bare", "status": "proposed"}
        doc = make_design_doc(decisions=[decision])
        md = render_design_doc_md(doc)
        assert "### ADR-099: Bare [proposed]" in md
        assert "**Context**" not in md
        assert "**Decision**" not in md
        assert "**Rationale**" not in md


# =============================================================================
# Tests for render_mega_plan_md
# =============================================================================


class TestRenderMegaPlanMd:
    """Tests for mega plan rendering."""

    def test_full_mega_plan(self):
        """Normal case: full mega plan with all fields populated."""
        mega = {
            "goal": "Build E-Commerce Platform",
            "metadata": {"created_at": "2024-06-15"},
            "execution_mode": "worktree",
            "features": [
                make_feature("feat-1", "User Auth", priority="high"),
                make_feature("feat-2", "Product Catalog", priority="medium"),
                make_feature(
                    "feat-3",
                    "Order System",
                    priority="medium",
                    dependencies=["feat-1", "feat-2"],
                ),
            ],
        }

        md = render_mega_plan_md(mega)

        # Header
        assert "# Mega Plan: Build E-Commerce Platform" in md
        assert "Features: 3" in md
        assert "Batches: 2" in md
        assert "Mode: worktree" in md
        # Features
        assert "### feat-1: User Auth [HIGH]" in md
        assert "### feat-2: Product Catalog [MEDIUM]" in md
        assert "### feat-3: Order System [MEDIUM]" in md
        assert "**Dependencies:** feat-1, feat-2" in md
        # Execution Batches
        assert "## Execution Batches" in md
        assert "Batch 1" in md
        assert "(Parallel)" in md
        assert "feat-1" in md
        assert "feat-2" in md
        assert "Batch 2" in md
        assert "feat-3" in md

    def test_minimal_mega_plan(self):
        """Minimal case: empty features."""
        mega = {"features": []}
        md = render_mega_plan_md(mega)

        assert "# Mega Plan: Untitled" in md
        assert "Features: 0" in md
        assert "Batches: 0" in md
        assert "Mode: auto" in md
        assert "_(none)_" in md

    def test_mega_plan_no_features(self):
        """Missing features key defaults gracefully."""
        mega = {"goal": "Empty"}
        md = render_mega_plan_md(mega)
        assert "_(none)_" in md
        assert "Features: 0" in md

    def test_mega_plan_feature_no_dependencies(self):
        """Feature without dependencies omits Dependencies line."""
        mega = {
            "features": [make_feature("f1", "Solo Feature")],
        }
        md = render_mega_plan_md(mega)
        assert "**Dependencies:**" not in md

    def test_mega_plan_feature_with_dependencies(self):
        """Feature with dependencies renders them."""
        mega = {
            "features": [
                make_feature("f1", "First"),
                make_feature("f2", "Second", dependencies=["f1"]),
            ],
        }
        md = render_mega_plan_md(mega)
        assert "**Dependencies:** f1" in md

    def test_mega_plan_none_goal(self):
        """None goal defaults to 'Untitled'."""
        mega = {"goal": None, "features": []}
        md = render_mega_plan_md(mega)
        assert "# Mega Plan: Untitled" in md

    def test_mega_plan_batch_merge_suffix(self):
        """Non-last batches get '(merge to target branch)' suffix."""
        mega = {
            "features": [
                make_feature("f1", "A"),
                make_feature("f2", "B", dependencies=["f1"]),
            ],
        }
        md = render_mega_plan_md(mega)
        # First batch (not last) should have merge suffix
        lines = md.splitlines()
        batch1_line = [l for l in lines if "Batch 1" in l][0]
        assert "(merge to target branch)" in batch1_line
        # Last batch should NOT have merge suffix
        batch2_line = [l for l in lines if "Batch 2" in l][0]
        assert "(merge to target branch)" not in batch2_line

    def test_mega_plan_single_batch_no_merge_suffix(self):
        """Single batch (also the last) has no merge suffix."""
        mega = {
            "features": [make_feature("f1", "Solo")],
        }
        md = render_mega_plan_md(mega)
        lines = md.splitlines()
        batch_lines = [l for l in lines if l.strip().startswith("- Batch")]
        assert len(batch_lines) == 1
        assert "(merge to target branch)" not in batch_lines[0]

    def test_mega_plan_parallel_marker(self):
        """Batch with multiple features gets (Parallel) marker."""
        mega = {
            "features": [
                make_feature("f1", "A"),
                make_feature("f2", "B"),
            ],
        }
        md = render_mega_plan_md(mega)
        assert "(Parallel)" in md

    def test_mega_plan_single_feature_batch_no_parallel(self):
        """Batch with single feature does NOT get (Parallel) marker."""
        mega = {
            "features": [make_feature("f1", "Solo")],
        }
        md = render_mega_plan_md(mega)
        assert "(Parallel)" not in md

    def test_mega_plan_empty_description(self):
        """Feature with empty description still renders."""
        feat = make_feature("f1", "No Desc", description="")
        mega = {"features": [feat]}
        md = render_mega_plan_md(mega)
        assert "### f1: No Desc [MEDIUM]" in md

    def test_mega_plan_ends_with_newline(self):
        """Output ends with exactly one newline."""
        mega = {"features": [make_feature("f1", "A")]}
        md = render_mega_plan_md(mega)
        assert md.endswith("\n")
        assert not md.endswith("\n\n")

    def test_mega_plan_no_trailing_whitespace(self):
        """No lines have trailing whitespace."""
        mega = {
            "features": [
                make_feature("f1", "A"),
                make_feature("f2", "B", dependencies=["f1"]),
            ],
        }
        md = render_mega_plan_md(mega)
        for line in md.splitlines():
            assert line == line.rstrip(), f"Trailing whitespace on: {line!r}"

    def test_mega_plan_missing_metadata(self):
        """Missing metadata does not crash."""
        mega = {"features": []}
        md = render_mega_plan_md(mega)
        # Should still render without error
        assert "# Mega Plan:" in md


# =============================================================================
# Integration-style: round-trip consistency
# =============================================================================


class TestRenderConsistency:
    """Cross-cutting tests for rendering consistency."""

    def test_prd_batch_count_matches_execution_plan(self):
        """Batch count in metadata matches actual execution plan lines."""
        stories = [
            make_story("s1", "A"),
            make_story("s2", "B", dependencies=["s1"]),
            make_story("s3", "C", dependencies=["s1"]),
            make_story("s4", "D", dependencies=["s2", "s3"]),
        ]
        prd = make_prd(stories=stories)
        md = render_prd_md(prd)

        # Extract batch count from metadata line
        assert "Batches: 3" in md
        # Count actual "Batch N" lines
        batch_lines = [l for l in md.splitlines() if l.strip().startswith("- Batch")]
        assert len(batch_lines) == 3

    def test_mega_batch_count_matches_execution_batches(self):
        """Batch count in header matches actual execution batch lines."""
        features = [
            make_feature("f1", "A"),
            make_feature("f2", "B"),
            make_feature("f3", "C", dependencies=["f1"]),
        ]
        mega = {"features": features}
        md = render_mega_plan_md(mega)

        assert "Batches: 2" in md
        batch_lines = [l for l in md.splitlines() if l.strip().startswith("- Batch")]
        assert len(batch_lines) == 2

    def test_design_doc_component_count_matches(self):
        """Component count in header matches actual component count."""
        components = [
            {"name": "A"},
            {"name": "B"},
            {"name": "C"},
        ]
        doc = make_design_doc(components=components)
        md = render_design_doc_md(doc)
        assert "Components: 3" in md
