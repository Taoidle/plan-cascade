#!/usr/bin/env python3
"""
Mega Plan Tools for Plan Cascade MCP Server

Provides MCP tools for project-level mega-plan management:
- mega_generate: Generate mega-plan from description
- mega_add_feature: Add a feature to mega-plan
- mega_validate: Validate mega-plan structure
- mega_get_batches: Get feature execution batches
- mega_update_feature_status: Update feature status
- mega_get_merge_plan: Get ordered merge plan
"""

import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional
import importlib.util

# Add skills core directories to path for imports
PLUGIN_ROOT = Path(__file__).parent.parent.parent
SKILLS_MEGA_PLAN_CORE = PLUGIN_ROOT / "skills" / "mega-plan" / "core"


def _load_mega_plan_modules():
    """
    Load mega-plan core modules with proper handling of relative imports.

    The modules have internal relative imports, so we need to:
    1. Load base modules first (mega_generator, mega_state)
    2. Patch sys.modules to make them available for relative imports
    3. Then load modules that depend on them (merge_coordinator)
    """
    # Add core directory to path
    if str(SKILLS_MEGA_PLAN_CORE) not in sys.path:
        sys.path.insert(0, str(SKILLS_MEGA_PLAN_CORE))

    # Load mega_generator first (no relative imports)
    spec = importlib.util.spec_from_file_location(
        "mega_generator",
        SKILLS_MEGA_PLAN_CORE / "mega_generator.py"
    )
    mega_generator = importlib.util.module_from_spec(spec)
    sys.modules["mega_generator"] = mega_generator
    spec.loader.exec_module(mega_generator)

    # Load mega_state (has lazy relative import to mega_generator)
    spec = importlib.util.spec_from_file_location(
        "mega_state",
        SKILLS_MEGA_PLAN_CORE / "mega_state.py"
    )
    mega_state = importlib.util.module_from_spec(spec)
    sys.modules["mega_state"] = mega_state
    spec.loader.exec_module(mega_state)

    # Now load merge_coordinator with patched imports
    # Read the file and replace relative imports
    merge_coord_path = SKILLS_MEGA_PLAN_CORE / "merge_coordinator.py"
    with open(merge_coord_path, "r", encoding="utf-8") as f:
        source = f.read()

    # Replace relative imports with absolute
    source = source.replace("from .mega_state import", "from mega_state import")
    source = source.replace("from .mega_generator import", "from mega_generator import")

    # Compile and execute
    code = compile(source, str(merge_coord_path), "exec")
    merge_coordinator = type(sys)("merge_coordinator")
    merge_coordinator.__file__ = str(merge_coord_path)
    sys.modules["merge_coordinator"] = merge_coordinator
    exec(code, merge_coordinator.__dict__)

    return mega_generator, mega_state, merge_coordinator


# Load the modules
_mega_gen_mod, _mega_state_mod, _merge_coord_mod = _load_mega_plan_modules()

MegaPlanGenerator = _mega_gen_mod.MegaPlanGenerator
MegaStateManager = _mega_state_mod.MegaStateManager
MergeCoordinator = _merge_coord_mod.MergeCoordinator


def register_mega_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all Mega Plan-related tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def mega_generate(
        description: str,
        execution_mode: str = "auto",
        target_branch: str = "main",
        goal: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Generate a mega-plan (project-level plan) from a project description.

        Mega-plans break large projects into features that can be developed in parallel.
        Each feature becomes a separate Git worktree with its own PRD.

        Args:
            description: Detailed description of the project to implement
            execution_mode: "auto" for automatic PRD approval, "manual" for manual review
            target_branch: Git branch to merge features into when complete
            goal: Optional explicit goal (extracted from description if not provided)

        Returns:
            Generated mega-plan structure
        """
        generator = MegaPlanGenerator(project_root)
        plan = generator.generate_mega_plan(
            description=description,
            execution_mode=execution_mode,
            target_branch=target_branch
        )

        # Override goal if explicitly provided
        if goal:
            plan["goal"] = goal

        # Save to file
        state_manager = MegaStateManager(project_root)
        state_manager.write_mega_plan(plan)

        return {
            "success": True,
            "message": "Mega-plan generated successfully",
            "mega_plan": plan,
            "file_path": str(project_root / "mega-plan.json")
        }

    @mcp.tool()
    def mega_add_feature(
        name: str,
        title: str,
        description: str,
        priority: str = "medium",
        dependencies: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Add a feature to the existing mega-plan.

        Features are major components of the project that can be developed independently.
        Each feature will get its own Git worktree and PRD.

        Args:
            name: Feature name (lowercase, alphanumeric with hyphens, e.g., "feature-auth")
            title: Human-readable feature title
            description: Detailed description for PRD generation
            priority: Priority level - "high", "medium", or "low"
            dependencies: List of feature IDs this feature depends on (e.g., ["feature-001"])

        Returns:
            Updated mega-plan with the new feature added
        """
        state_manager = MegaStateManager(project_root)
        plan = state_manager.read_mega_plan()

        if not plan:
            return {
                "success": False,
                "error": "No mega-plan found. Run mega_generate first."
            }

        # Validate name format
        import re
        if not re.match(r'^[a-z0-9][a-z0-9-]*$', name):
            return {
                "success": False,
                "error": f"Invalid feature name '{name}'. Use lowercase alphanumeric with hyphens (e.g., 'feature-auth')"
            }

        generator = MegaPlanGenerator(project_root)
        # Sync feature counter with existing features
        if plan.get("features"):
            max_id = max(int(f["id"].split("-")[1]) for f in plan["features"])
            generator.feature_counter = max_id

        plan = generator.add_feature(
            plan=plan,
            name=name,
            title=title,
            description=description,
            priority=priority,
            dependencies=dependencies
        )

        state_manager.write_mega_plan(plan)

        # Get the newly added feature
        new_feature = plan["features"][-1]

        return {
            "success": True,
            "message": f"Feature {new_feature['id']} ({name}) added successfully",
            "feature": new_feature,
            "total_features": len(plan["features"])
        }

    @mcp.tool()
    def mega_validate() -> Dict[str, Any]:
        """
        Validate the current mega-plan for correctness.

        Checks for:
        - Required fields (metadata, goal, features)
        - Valid feature structure (id, name, title, description)
        - No duplicate feature IDs or names
        - Valid name format (lowercase alphanumeric with hyphens)
        - Valid dependency references
        - No circular dependencies
        - Valid priority and status values

        Returns:
            Validation result with is_valid flag and list of errors if any
        """
        state_manager = MegaStateManager(project_root)
        plan = state_manager.read_mega_plan()

        if not plan:
            return {
                "success": False,
                "is_valid": False,
                "errors": ["No mega-plan found. Run mega_generate first."]
            }

        generator = MegaPlanGenerator(project_root)
        is_valid, errors = generator.validate_mega_plan(plan)

        return {
            "success": True,
            "is_valid": is_valid,
            "errors": errors,
            "feature_count": len(plan.get("features", [])),
            "execution_mode": plan.get("execution_mode", "auto"),
            "target_branch": plan.get("target_branch", "main")
        }

    @mcp.tool()
    def mega_get_batches() -> Dict[str, Any]:
        """
        Get execution batches for parallel feature development.

        Features are organized into batches based on their dependencies:
        - Batch 1: Features with no dependencies (can develop in parallel)
        - Batch 2+: Features whose dependencies are in previous batches

        Returns:
            List of batches, where each batch contains features that can be developed in parallel
        """
        state_manager = MegaStateManager(project_root)
        plan = state_manager.read_mega_plan()

        if not plan:
            return {
                "success": False,
                "error": "No mega-plan found. Run mega_generate first."
            }

        generator = MegaPlanGenerator(project_root)
        batches = generator.generate_feature_batches(plan)

        # Format batches for output
        formatted_batches = []
        for i, batch in enumerate(batches, 1):
            batch_info = {
                "batch_number": i,
                "feature_count": len(batch),
                "features": [
                    {
                        "id": f["id"],
                        "name": f["name"],
                        "title": f["title"],
                        "priority": f.get("priority", "medium"),
                        "status": f.get("status", "pending"),
                        "dependencies": f.get("dependencies", [])
                    }
                    for f in batch
                ]
            }
            formatted_batches.append(batch_info)

        # Calculate progress
        progress = generator.calculate_progress(plan)

        return {
            "success": True,
            "total_batches": len(batches),
            "total_features": len(plan.get("features", [])),
            "progress": progress,
            "batches": formatted_batches
        }

    @mcp.tool()
    def mega_update_feature_status(feature_id: str, status: str) -> Dict[str, Any]:
        """
        Update the status of a feature in the mega-plan.

        Use this to track progress as features are developed.

        Args:
            feature_id: Feature ID to update (e.g., "feature-001")
            status: New status - one of:
                - "pending": Not started
                - "prd_generated": PRD has been generated
                - "approved": PRD approved for development
                - "in_progress": Development in progress
                - "complete": Feature is complete
                - "failed": Development failed

        Returns:
            Updated feature information
        """
        valid_statuses = ["pending", "prd_generated", "approved", "in_progress", "complete", "failed"]
        if status not in valid_statuses:
            return {
                "success": False,
                "error": f"Invalid status '{status}'. Must be one of: {valid_statuses}"
            }

        state_manager = MegaStateManager(project_root)

        try:
            state_manager.update_feature_status(feature_id, status)

            # Get updated feature
            plan = state_manager.read_mega_plan()
            feature = None
            for f in plan.get("features", []):
                if f["id"] == feature_id:
                    feature = f
                    break

            if not feature:
                return {
                    "success": False,
                    "error": f"Feature {feature_id} not found in mega-plan"
                }

            # Calculate updated progress
            generator = MegaPlanGenerator(project_root)
            progress = generator.calculate_progress(plan)

            return {
                "success": True,
                "message": f"Feature {feature_id} status updated to '{status}'",
                "feature": feature,
                "progress": progress
            }

        except Exception as e:
            return {
                "success": False,
                "error": str(e)
            }

    @mcp.tool()
    def mega_get_merge_plan() -> Dict[str, Any]:
        """
        Get the ordered merge plan for all features.

        When all features are complete, they need to be merged in dependency order.
        This tool generates the correct merge sequence.

        Returns:
            Ordered list of features to merge and verification status
        """
        state_manager = MegaStateManager(project_root)
        plan = state_manager.read_mega_plan()

        if not plan:
            return {
                "success": False,
                "error": "No mega-plan found. Run mega_generate first."
            }

        coordinator = MergeCoordinator(project_root)

        # Verify completion status
        all_complete, incomplete = coordinator.verify_all_features_complete()

        # Generate merge order
        merge_order = coordinator.generate_merge_plan()

        # Format output
        merge_plan = []
        for i, feature in enumerate(merge_order, 1):
            merge_plan.append({
                "order": i,
                "id": feature["id"],
                "name": feature["name"],
                "title": feature["title"],
                "status": feature.get("status", "pending"),
                "dependencies": feature.get("dependencies", []),
                "branch_name": f"mega-{feature['name']}"
            })

        return {
            "success": True,
            "all_features_complete": all_complete,
            "incomplete_features": incomplete,
            "target_branch": plan.get("target_branch", "main"),
            "total_features": len(merge_plan),
            "merge_order": merge_plan,
            "message": "All features complete - ready to merge!" if all_complete else f"Waiting for: {', '.join(incomplete)}"
        }
