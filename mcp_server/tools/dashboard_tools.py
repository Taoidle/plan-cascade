#!/usr/bin/env python3
"""
Dashboard and Session Recovery Tools for Plan Cascade MCP Server

Provides MCP tools for project status overview and session management:
- dashboard: Unified status dashboard aggregating info across all layers
- session_recover: Detect and prepare recovery for interrupted sessions
- get_configuration: Read current config from prd.json and agents.json
- update_configuration: Update flow, tdd, confirm, default_agent settings
"""

import json
from pathlib import Path
from typing import Any, Dict, List, Optional


def _read_json_safe(path: Path) -> Optional[Dict[str, Any]]:
    """Read a JSON file safely, returning None on any error."""
    if not path.exists():
        return None
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError, ValueError):
        return None


def _write_json_safe(path: Path, data: Dict[str, Any]) -> bool:
    """Write a JSON file safely. Returns True on success."""
    try:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        return True
    except OSError:
        return False


def _detect_mode(project_root: Path) -> Optional[str]:
    """
    Auto-detect active execution mode by checking for valid state files.

    Priority order:
    1. mega-plan.json -> "mega"
    2. .planning-config.json -> "hybrid-worktree"
    3. prd.json -> "hybrid-auto"
    4. None if no valid state files found

    Files must be valid JSON to trigger mode detection.
    """
    if _read_json_safe(project_root / "mega-plan.json") is not None:
        return "mega"
    if _read_json_safe(project_root / ".planning-config.json") is not None:
        return "hybrid-worktree"
    if _read_json_safe(project_root / "prd.json") is not None:
        return "hybrid-auto"
    return None


def _get_prd_progress(prd: Dict[str, Any]) -> Dict[str, Any]:
    """Extract PRD story progress from prd.json data."""
    stories = prd.get("stories", [])
    total = len(stories)
    complete = sum(1 for s in stories if s.get("status") == "complete")
    in_progress = sum(1 for s in stories if s.get("status") == "in_progress")
    failed = sum(1 for s in stories if s.get("status") == "failed")
    pending = total - complete - in_progress - failed
    return {
        "total_stories": total,
        "complete": complete,
        "in_progress": in_progress,
        "failed": failed,
        "pending": pending,
        "progress_percent": int((complete / total) * 100) if total > 0 else 0,
    }


def _get_mega_progress(mega: Dict[str, Any]) -> Dict[str, Any]:
    """Extract mega-plan feature progress from mega-plan.json data."""
    features = mega.get("features", [])
    total = len(features)
    complete = sum(1 for f in features if f.get("status") == "complete")
    in_progress = sum(1 for f in features if f.get("status") == "in_progress")
    failed = sum(1 for f in features if f.get("status") == "failed")
    pending = total - complete - in_progress - failed
    return {
        "total_features": total,
        "complete": complete,
        "in_progress": in_progress,
        "failed": failed,
        "pending": pending,
        "progress_percent": int((complete / total) * 100) if total > 0 else 0,
    }


def _get_agent_status_summary(agent_status: Dict[str, Any]) -> Dict[str, int]:
    """Summarize agent status counts."""
    return {
        "running": len(agent_status.get("running", [])),
        "completed": len(agent_status.get("completed", [])),
        "failed": len(agent_status.get("failed", [])),
    }


def _get_design_doc_status(project_root: Path) -> Dict[str, Any]:
    """Get design_doc.json status."""
    design_doc = _read_json_safe(project_root / "design_doc.json")
    if not design_doc:
        return {"exists": False}
    return {
        "exists": True,
        "level": design_doc.get("metadata", {}).get("level", "unknown"),
        "components_count": len(
            design_doc.get("architecture", {}).get("components", [])
        ),
        "mapped_stories": len(design_doc.get("story_mappings", {})),
    }


def _recommend_action(
    mode: Optional[str],
    prd: Optional[Dict[str, Any]],
    mega: Optional[Dict[str, Any]],
) -> str:
    """Generate a recommended next action based on current state."""
    if mode is None:
        return "Start a new project with /plan-cascade:auto or /plan-cascade:hybrid-auto"

    if mode == "mega":
        if mega:
            features = mega.get("features", [])
            in_progress = [f for f in features if f.get("status") == "in_progress"]
            all_complete = all(f.get("status") == "complete" for f in features) if features else False
            if all_complete:
                return "All features complete. Run /plan-cascade:mega-complete to finish."
            if in_progress:
                return f"Continue working on {in_progress[0].get('title', 'current feature')}."
            return "Resume mega-plan execution with /plan-cascade:resume"
        return "Resume mega-plan execution with /plan-cascade:resume"

    if mode in ("hybrid-auto", "hybrid-worktree"):
        if prd:
            stories = prd.get("stories", [])
            in_progress = [s for s in stories if s.get("status") == "in_progress"]
            pending = [s for s in stories if s.get("status") == "pending"]
            all_complete = all(s.get("status") == "complete" for s in stories) if stories else False
            if all_complete:
                return "All stories complete. Review and finalize the feature."
            if in_progress:
                return f"Continue working on {in_progress[0].get('title', 'current story')}."
            if pending:
                return f"Start next story: {pending[0].get('title', 'pending story')}."
        return "Resume execution with /plan-cascade:resume"

    return "Check project status and decide next steps."


def register_dashboard_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all dashboard and session recovery tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def dashboard() -> Dict[str, Any]:
        """
        Unified status dashboard aggregating info across all Plan Cascade layers.

        Auto-detects mode (mega/hybrid-worktree/hybrid-auto) by checking for
        mega-plan.json, .planning-config.json, prd.json. Returns active mode,
        PRD/mega progress, current batch, active worktrees, agent status,
        design doc status, and recommended next action.

        Returns:
            Comprehensive dashboard status with 'success' boolean
        """
        mode = _detect_mode(project_root)

        # Read state files
        prd = _read_json_safe(project_root / "prd.json")
        mega = _read_json_safe(project_root / "mega-plan.json")
        planning_config = _read_json_safe(project_root / ".planning-config.json")
        agent_status_data = _read_json_safe(project_root / ".agent-status.json")

        # Build result
        result: Dict[str, Any] = {
            "success": True,
            "active_mode": mode,
        }

        # PRD progress
        if prd:
            result["prd_progress"] = _get_prd_progress(prd)
        else:
            result["prd_progress"] = None

        # Mega progress
        if mega:
            result["mega_progress"] = _get_mega_progress(mega)
        else:
            result["mega_progress"] = None

        # Current batch (derive from PRD stories)
        if prd:
            stories = prd.get("stories", [])
            # Find the first batch with non-complete stories
            # Simple approach: find first in_progress or pending story
            current_batch_num = 0
            for i, story in enumerate(stories):
                if story.get("status") in ("in_progress", "pending"):
                    current_batch_num = i + 1
                    break
            result["current_batch"] = current_batch_num
        else:
            result["current_batch"] = None

        # Active worktrees
        if planning_config:
            worktrees = planning_config.get("worktrees", [])
            active_worktrees = len([
                w for w in worktrees
                if w.get("status") == "active"
            ])
            result["active_worktrees"] = active_worktrees
        else:
            result["active_worktrees"] = 0

        # Agent status
        if agent_status_data:
            result["agent_status"] = _get_agent_status_summary(agent_status_data)
        else:
            result["agent_status"] = {"running": 0, "completed": 0, "failed": 0}

        # Design doc status
        result["design_doc_status"] = _get_design_doc_status(project_root)

        # Recommended action
        result["recommended_action"] = _recommend_action(mode, prd, mega)

        return result

    @mcp.tool()
    def session_recover() -> Dict[str, Any]:
        """
        Detect and prepare recovery for interrupted sessions.

        Checks for state files (.hybrid-execution-context.md,
        .mega-execution-context.md, prd.json, mega-plan.json,
        .planning-config.json) and returns detected mode, current state,
        incomplete work items, and recommended resume action.

        Returns:
            Recovery status with 'success' boolean and 'recovery_needed' flag
        """
        recovery_needed = False
        detected_mode: Optional[str] = None
        current_state: Optional[str] = None
        incomplete_items: List[Dict[str, str]] = []
        resume_action: Optional[str] = None

        # Check mode detection files (same priority order)
        mega = _read_json_safe(project_root / "mega-plan.json")
        planning_config = _read_json_safe(project_root / ".planning-config.json")
        prd = _read_json_safe(project_root / "prd.json")

        # Check context recovery files
        has_hybrid_context = (project_root / ".hybrid-execution-context.md").exists()
        has_mega_context = (project_root / ".mega-execution-context.md").exists()

        # Determine mode and incomplete items
        if mega:
            detected_mode = "mega"
            features = mega.get("features", [])
            incomplete = [
                f for f in features
                if f.get("status") in ("in_progress", "pending")
            ]
            if incomplete:
                recovery_needed = True
                incomplete_items = [
                    {"id": f.get("id", "unknown"), "title": f.get("title", "Untitled"), "status": f.get("status", "unknown")}
                    for f in incomplete
                ]
                current_state = f"Mega plan with {len(features)} features, {len(incomplete)} incomplete"
                resume_action = "/plan-cascade:resume"
        elif planning_config:
            detected_mode = "hybrid-worktree"
            worktrees = planning_config.get("worktrees", [])
            active = [w for w in worktrees if w.get("status") == "active"]
            if active:
                recovery_needed = True
                incomplete_items = [
                    {"id": w.get("branch", "unknown"), "title": w.get("branch", "Untitled"), "status": "active"}
                    for w in active
                ]
                current_state = f"Worktree mode with {len(active)} active worktrees"
                resume_action = "/plan-cascade:resume"
        elif prd:
            detected_mode = "hybrid-auto"
            stories = prd.get("stories", [])
            incomplete = [
                s for s in stories
                if s.get("status") in ("in_progress", "pending")
            ]
            if incomplete:
                recovery_needed = True
                incomplete_items = [
                    {"id": s.get("id", "unknown"), "title": s.get("title", "Untitled"), "status": s.get("status", "unknown")}
                    for s in incomplete
                ]
                current_state = f"Hybrid-auto with {len(stories)} stories, {len(incomplete)} incomplete"
                resume_action = "/plan-cascade:resume"

        # Check context files as fallback signals
        if not recovery_needed:
            if has_mega_context:
                recovery_needed = True
                detected_mode = detected_mode or "mega"
                current_state = current_state or "Mega execution context file found"
                resume_action = resume_action or "/plan-cascade:resume"
            elif has_hybrid_context:
                recovery_needed = True
                detected_mode = detected_mode or "hybrid-auto"
                current_state = current_state or "Hybrid execution context file found"
                resume_action = resume_action or "/plan-cascade:resume"

        return {
            "success": True,
            "recovery_needed": recovery_needed,
            "detected_mode": detected_mode,
            "current_state": current_state,
            "incomplete_items": incomplete_items,
            "resume_action": resume_action,
        }

    @mcp.tool()
    def get_configuration() -> Dict[str, Any]:
        """
        Read current configuration from prd.json and agents.json.

        Returns flow_config, tdd_config, and execution_config from prd.json,
        and the full agents.json configuration.

        Returns:
            Configuration data with 'success' boolean
        """
        prd = _read_json_safe(project_root / "prd.json")
        agents = _read_json_safe(project_root / "agents.json")

        prd_config = None
        if prd:
            prd_config = {
                "flow_config": prd.get("flow_config"),
                "tdd_config": prd.get("tdd_config"),
                "execution_config": prd.get("execution_config"),
            }

        return {
            "success": True,
            "prd_config": prd_config,
            "agents_config": agents,
        }

    @mcp.tool()
    def update_configuration(
        flow: Optional[str] = None,
        tdd: Optional[bool] = None,
        confirm: Optional[bool] = None,
        default_agent: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Update flow, tdd, confirm, and default_agent settings.

        Updates settings in prd.json (flow_config, tdd_config, execution_config)
        and/or agents.json as appropriate. Creates config sections if they
        don't exist in prd.json.

        Args:
            flow: Flow level to set (e.g., "quick", "standard", "full")
            tdd: Whether TDD mode is enabled
            confirm: Whether confirmation is required before execution
            default_agent: Default agent name (e.g., "sonnet", "opus")

        Returns:
            Update result with 'success' boolean and list of changes
        """
        # Check if any changes were requested
        if flow is None and tdd is None and confirm is None and default_agent is None:
            return {
                "success": True,
                "message": "No changes specified",
                "changes": [],
            }

        prd_path = project_root / "prd.json"
        agents_path = project_root / "agents.json"

        prd = _read_json_safe(prd_path)
        agents = _read_json_safe(agents_path)

        # Need at least one config file to exist
        if prd is None and agents is None:
            return {
                "success": False,
                "error": "No configuration files found. Neither prd.json nor agents.json exists.",
            }

        # Cannot update prd-specific settings without prd.json
        if prd is None and (flow is not None or tdd is not None or confirm is not None):
            # Only agents.json exists, but flow/tdd/confirm need prd.json
            if agents is not None and default_agent is not None:
                # Can still update agents.json for default_agent
                pass
            else:
                return {
                    "success": False,
                    "error": "Cannot update flow/tdd/confirm: prd.json not found.",
                }

        changes: List[str] = []

        # Update prd.json settings
        if prd is not None:
            if flow is not None:
                if "flow_config" not in prd:
                    prd["flow_config"] = {}
                prd["flow_config"]["flow"] = flow
                changes.append(f"flow set to '{flow}'")

            if tdd is not None:
                if "tdd_config" not in prd:
                    prd["tdd_config"] = {}
                prd["tdd_config"]["enabled"] = tdd
                changes.append(f"tdd set to {tdd}")

            if confirm is not None:
                if "flow_config" not in prd:
                    prd["flow_config"] = {}
                prd["flow_config"]["confirm"] = confirm
                changes.append(f"confirm set to {confirm}")

            if default_agent is not None:
                if "execution_config" not in prd:
                    prd["execution_config"] = {}
                prd["execution_config"]["default_agent"] = default_agent
                changes.append(f"default_agent set to '{default_agent}' in prd.json")

            if not _write_json_safe(prd_path, prd):
                return {
                    "success": False,
                    "error": "Failed to write prd.json",
                }

        # Update agents.json settings
        if agents is not None and default_agent is not None:
            agents["default_agent"] = default_agent
            changes.append(f"default_agent set to '{default_agent}' in agents.json")
            if not _write_json_safe(agents_path, agents):
                return {
                    "success": False,
                    "error": "Failed to write agents.json",
                }

        return {
            "success": True,
            "message": f"Configuration updated: {', '.join(changes)}",
            "changes": changes,
        }
