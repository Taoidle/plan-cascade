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
