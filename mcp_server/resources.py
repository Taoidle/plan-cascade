#!/usr/bin/env python3
"""
MCP Resources for Plan Cascade

Provides read-only access to Plan Cascade state files:
- prd.json: Current PRD
- mega-plan.json: Current mega-plan
- findings.md: Development findings
- progress.txt: Progress timeline
- .mega-status.json: Mega-plan execution status
- mega-findings.md: Project-level findings
- design_doc.json: Design document (full or filtered by story)
- .hybrid-execution-context.md / .mega-execution-context.md: Execution context
- .planning-config.json: Worktree planning configuration
- spec.json: Specification document
- .state/spec-interview.json: Spec interview state
"""

import json
from pathlib import Path
from typing import Any


def register_resources(mcp: Any, project_root: Path) -> None:
    """
    Register all MCP resources with the server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.resource("plan-cascade://prd")
    def get_prd_resource() -> str:
        """
        Get the current PRD (Product Requirements Document).

        Returns the full prd.json content as formatted JSON.
        """
        prd_path = project_root / "prd.json"

        if not prd_path.exists():
            return json.dumps({
                "error": "No PRD found",
                "hint": "Use prd_generate tool to create a PRD"
            }, indent=2)

        try:
            with open(prd_path, "r", encoding="utf-8") as f:
                prd = json.load(f)
            return json.dumps(prd, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://mega-plan")
    def get_mega_plan_resource() -> str:
        """
        Get the current mega-plan (project-level plan).

        Returns the full mega-plan.json content as formatted JSON.
        """
        plan_path = project_root / "mega-plan.json"

        if not plan_path.exists():
            return json.dumps({
                "error": "No mega-plan found",
                "hint": "Use mega_generate tool to create a mega-plan"
            }, indent=2)

        try:
            with open(plan_path, "r", encoding="utf-8") as f:
                plan = json.load(f)
            return json.dumps(plan, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://findings")
    def get_findings_resource() -> str:
        """
        Get the development findings.

        Returns the full findings.md content.
        """
        findings_path = project_root / "findings.md"

        if not findings_path.exists():
            return "# Findings\n\nNo findings recorded yet.\n\nUse the `append_findings` tool to record findings during development."

        try:
            with open(findings_path, "r", encoding="utf-8") as f:
                return f.read()
        except Exception as e:
            return f"Error reading findings: {e}"

    @mcp.resource("plan-cascade://progress")
    def get_progress_resource() -> str:
        """
        Get the progress timeline.

        Returns the full progress.txt content showing the development timeline.
        """
        progress_path = project_root / "progress.txt"

        if not progress_path.exists():
            return "# Progress\n\nNo progress recorded yet.\n\nProgress is automatically tracked when using mark_story_complete and other execution tools."

        try:
            with open(progress_path, "r", encoding="utf-8") as f:
                return f.read()
        except Exception as e:
            return f"Error reading progress: {e}"

    @mcp.resource("plan-cascade://mega-status")
    def get_mega_status_resource() -> str:
        """
        Get the mega-plan execution status.

        Returns the .mega-status.json content showing current feature statuses.
        """
        status_path = project_root / ".mega-status.json"

        if not status_path.exists():
            return json.dumps({
                "error": "No mega-status found",
                "hint": "Mega-status is created automatically when working with mega-plans"
            }, indent=2)

        try:
            with open(status_path, "r", encoding="utf-8") as f:
                status = json.load(f)
            return json.dumps(status, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://mega-findings")
    def get_mega_findings_resource() -> str:
        """
        Get the project-level mega-findings.

        Returns the mega-findings.md content with cross-feature discoveries.
        """
        findings_path = project_root / "mega-findings.md"

        if not findings_path.exists():
            return "# Mega Plan Findings\n\nNo mega-findings recorded yet.\n\nMega-findings are shared across all features in a mega-plan."

        try:
            with open(findings_path, "r", encoding="utf-8") as f:
                return f.read()
        except Exception as e:
            return f"Error reading mega-findings: {e}"

    @mcp.resource("plan-cascade://story/{story_id}")
    def get_story_resource(story_id: str) -> str:
        """
        Get details for a specific story.

        Args:
            story_id: Story ID (e.g., "story-001")

        Returns the story details as formatted JSON.
        """
        prd_path = project_root / "prd.json"

        if not prd_path.exists():
            return json.dumps({
                "error": "No PRD found",
                "hint": "Use prd_generate tool to create a PRD"
            }, indent=2)

        try:
            with open(prd_path, "r", encoding="utf-8") as f:
                prd = json.load(f)

            for story in prd.get("stories", []):
                if story.get("id") == story_id:
                    return json.dumps(story, indent=2)

            return json.dumps({
                "error": f"Story {story_id} not found",
                "available_stories": [s["id"] for s in prd.get("stories", [])]
            }, indent=2)

        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://feature/{feature_id}")
    def get_feature_resource(feature_id: str) -> str:
        """
        Get details for a specific feature.

        Args:
            feature_id: Feature ID (e.g., "feature-001") or name (e.g., "feature-auth")

        Returns the feature details as formatted JSON.
        """
        plan_path = project_root / "mega-plan.json"

        if not plan_path.exists():
            return json.dumps({
                "error": "No mega-plan found",
                "hint": "Use mega_generate tool to create a mega-plan"
            }, indent=2)

        try:
            with open(plan_path, "r", encoding="utf-8") as f:
                plan = json.load(f)

            for feature in plan.get("features", []):
                if feature.get("id") == feature_id or feature.get("name") == feature_id:
                    return json.dumps(feature, indent=2)

            return json.dumps({
                "error": f"Feature {feature_id} not found",
                "available_features": [
                    {"id": f["id"], "name": f["name"]}
                    for f in plan.get("features", [])
                ]
            }, indent=2)

        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    # ==================================================================
    # New Resources (story-003)
    # ==================================================================

    @mcp.resource("plan-cascade://design-doc")
    def get_design_doc_resource() -> str:
        """
        Get the current design document.

        Returns the full design_doc.json content as formatted JSON.
        """
        design_path = project_root / "design_doc.json"

        if not design_path.exists():
            return json.dumps({
                "error": "No design_doc.json found",
                "hint": "Use the design_generate tool to create a design document"
            }, indent=2)

        try:
            with open(design_path, "r", encoding="utf-8") as f:
                design_doc = json.load(f)
            return json.dumps(design_doc, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://design-doc/{story_id}")
    def get_design_doc_story_resource(story_id: str) -> str:
        """
        Get filtered design context for a specific story.

        Uses story_mappings to return only the components, decisions, and
        interfaces relevant to the given story.

        Args:
            story_id: Story ID (e.g., "story-001")

        Returns filtered design context as formatted JSON.
        """
        design_path = project_root / "design_doc.json"

        if not design_path.exists():
            return json.dumps({
                "error": "No design_doc.json found",
                "hint": "Use the design_generate tool to create a design document"
            }, indent=2)

        try:
            with open(design_path, "r", encoding="utf-8") as f:
                design_doc = json.load(f)

            # Check if story_id exists in story_mappings
            story_mappings = design_doc.get("story_mappings", {})
            if story_id not in story_mappings:
                available = list(story_mappings.keys())
                return json.dumps({
                    "error": f"Story {story_id} not found in design document story_mappings",
                    "available_stories": available
                }, indent=2)

            mapping = story_mappings[story_id]

            # Filter components
            all_components = design_doc.get("architecture", {}).get("components", [])
            mapped_component_names = mapping.get("components", [])
            relevant_components = [
                c for c in all_components
                if c.get("name") in mapped_component_names
            ]

            # Filter decisions
            all_decisions = design_doc.get("decisions", [])
            mapped_decision_ids = mapping.get("decisions", [])
            relevant_decisions = [
                d for d in all_decisions
                if d.get("id") in mapped_decision_ids
            ]

            # Filter interfaces (APIs)
            all_apis = design_doc.get("interfaces", {}).get("apis", [])
            mapped_interface_refs = mapping.get("interfaces", [])
            relevant_apis = [
                a for a in all_apis
                if a.get("id") in mapped_interface_refs
            ]

            # Filter data models
            all_models = design_doc.get("interfaces", {}).get("data_models", [])
            relevant_models = [
                m for m in all_models
                if m.get("name") in mapped_interface_refs
            ]

            # Build filtered context
            filtered_doc = {
                "metadata": design_doc.get("metadata", {}),
                "overview": design_doc.get("overview", {}),
                "story_id": story_id,
                "story_mapping": mapping,
                "components": relevant_components,
                "decisions": relevant_decisions,
                "apis": relevant_apis,
                "data_models": relevant_models,
                "patterns": design_doc.get("architecture", {}).get("patterns", [])
            }

            return json.dumps(filtered_doc, indent=2)

        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://execution-context")
    def get_execution_context_resource() -> str:
        """
        Get the current execution context.

        Reads .hybrid-execution-context.md (preferred) or
        .mega-execution-context.md as fallback.
        """
        hybrid_path = project_root / ".hybrid-execution-context.md"
        mega_path = project_root / ".mega-execution-context.md"

        # Prefer hybrid context
        if hybrid_path.exists():
            try:
                with open(hybrid_path, "r", encoding="utf-8") as f:
                    return f.read()
            except Exception as e:
                return f"Error reading hybrid execution context: {e}"

        # Fall back to mega context
        if mega_path.exists():
            try:
                with open(mega_path, "r", encoding="utf-8") as f:
                    return f.read()
            except Exception as e:
                return f"Error reading mega execution context: {e}"

        return (
            "No execution context found. "
            "Neither .hybrid-execution-context.md nor .mega-execution-context.md exists.\n\n"
            "Execution context is automatically generated during story execution. "
            "Use the hybrid-auto or mega-plan commands to start execution."
        )

    @mcp.resource("plan-cascade://worktree-config/{task_name}")
    def get_worktree_config_resource(task_name: str) -> str:
        """
        Get the planning configuration for a worktree task.

        Resolves the worktree path via the .worktree/ directory and returns
        the .planning-config.json content.

        Args:
            task_name: Name of the worktree task (e.g., "feature-login")

        Returns the worktree planning config as formatted JSON.
        """
        worktree_path = project_root / ".worktree" / task_name

        if not worktree_path.exists():
            return json.dumps({
                "error": f"Worktree '{task_name}' not found",
                "hint": "Use the worktree_create tool to create a worktree, "
                        "or check worktree_list for available worktrees"
            }, indent=2)

        config_path = worktree_path / ".planning-config.json"

        if not config_path.exists():
            return json.dumps({
                "error": f"No .planning-config.json found in worktree '{task_name}'",
                "hint": "The worktree exists but has no planning configuration. "
                        "Use worktree_create to set up a properly configured worktree."
            }, indent=2)

        try:
            with open(config_path, "r", encoding="utf-8") as f:
                config = json.load(f)
            return json.dumps(config, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://spec")
    def get_spec_resource() -> str:
        """
        Get the current specification document.

        Returns the full spec.json content as formatted JSON.
        """
        spec_path = project_root / "spec.json"

        if not spec_path.exists():
            return json.dumps({
                "error": "No spec.json found",
                "hint": "Use the spec_start tool to begin a spec interview, "
                        "which will generate spec.json upon completion"
            }, indent=2)

        try:
            with open(spec_path, "r", encoding="utf-8") as f:
                spec = json.load(f)
            return json.dumps(spec, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)

    @mcp.resource("plan-cascade://spec-interview-state")
    def get_spec_interview_state_resource() -> str:
        """
        Get the current spec interview state.

        Returns the .state/spec-interview.json content showing interview
        progress, questions asked, and answers recorded.
        """
        state_path = project_root / ".state" / "spec-interview.json"

        if not state_path.exists():
            return json.dumps({
                "error": "No spec interview state found",
                "hint": "Use the spec_start tool to begin a spec interview. "
                        "Interview state is stored in .state/spec-interview.json"
            }, indent=2)

        try:
            with open(state_path, "r", encoding="utf-8") as f:
                state = json.load(f)
            return json.dumps(state, indent=2)
        except Exception as e:
            return json.dumps({"error": str(e)}, indent=2)
