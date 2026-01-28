#!/usr/bin/env python3
"""
PRD Tools for Plan Cascade MCP Server

Provides MCP tools for PRD (Product Requirements Document) management:
- prd_generate: Generate PRD from description
- prd_add_story: Add a story to PRD
- prd_validate: Validate PRD structure
- prd_get_batches: Get execution batches
- prd_update_story_status: Update story status
- prd_detect_dependencies: Auto-detect dependencies
"""

import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

# Add skills core directories to path for imports
PLUGIN_ROOT = Path(__file__).parent.parent.parent
SKILLS_HYBRID_RALPH_CORE = PLUGIN_ROOT / "skills" / "hybrid-ralph" / "core"

# Add core directory to path so relative imports work within modules
if str(SKILLS_HYBRID_RALPH_CORE) not in sys.path:
    sys.path.insert(0, str(SKILLS_HYBRID_RALPH_CORE))

# Now import the modules
from prd_generator import PRDGenerator
from state_manager import StateManager


def register_prd_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all PRD-related tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def prd_generate(
        description: str,
        goal: Optional[str] = None,
        objectives: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Generate a PRD (Product Requirements Document) from a task description.

        This creates the initial PRD structure. After generation, use prd_add_story
        to add user stories, then prd_validate to verify the PRD.

        Args:
            description: Detailed description of the feature/task to implement
            goal: Optional explicit goal (extracted from description if not provided)
            objectives: Optional list of objectives

        Returns:
            Generated PRD structure with metadata, goal, objectives, and empty stories list
        """
        generator = PRDGenerator(project_root)
        prd = generator.generate_prd(description)

        # Override goal if explicitly provided
        if goal:
            prd["goal"] = goal

        # Override objectives if provided
        if objectives:
            prd["objectives"] = objectives

        # Save to file
        state_manager = StateManager(project_root)
        state_manager.write_prd(prd)

        return {
            "success": True,
            "message": "PRD generated successfully",
            "prd": prd,
            "file_path": str(project_root / "prd.json")
        }

    @mcp.tool()
    def prd_add_story(
        title: str,
        description: str,
        priority: str = "medium",
        dependencies: Optional[List[str]] = None,
        acceptance_criteria: Optional[List[str]] = None,
        context_estimate: str = "medium",
        tags: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Add a user story to the existing PRD.

        Call this after prd_generate to add stories to the PRD.
        Stories can have dependencies on other stories (by story ID).

        Args:
            title: Short descriptive title for the story
            description: Detailed description of what needs to be done
            priority: Priority level - "high", "medium", or "low"
            dependencies: List of story IDs this story depends on (e.g., ["story-001"])
            acceptance_criteria: List of conditions that must be met for completion
            context_estimate: Estimated context size - "small", "medium", "large", "xlarge"
            tags: Optional tags for categorization

        Returns:
            Updated PRD with the new story added
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "error": "No PRD found. Run prd_generate first."
            }

        generator = PRDGenerator(project_root)
        # Sync story counter with existing stories
        if prd.get("stories"):
            max_id = max(int(s["id"].split("-")[1]) for s in prd["stories"])
            generator.story_counter = max_id

        prd = generator.add_story(
            prd=prd,
            title=title,
            description=description,
            priority=priority,
            dependencies=dependencies,
            acceptance_criteria=acceptance_criteria,
            context_estimate=context_estimate,
            tags=tags
        )

        state_manager.write_prd(prd)

        # Get the newly added story
        new_story = prd["stories"][-1]

        return {
            "success": True,
            "message": f"Story {new_story['id']} added successfully",
            "story": new_story,
            "total_stories": len(prd["stories"])
        }

    @mcp.tool()
    def prd_validate() -> Dict[str, Any]:
        """
        Validate the current PRD for correctness.

        Checks for:
        - Required fields (metadata, goal, stories)
        - Valid story structure (id, title, description)
        - No duplicate story IDs
        - Valid dependency references
        - Valid priority and status values

        Returns:
            Validation result with is_valid flag and list of errors if any
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "is_valid": False,
                "errors": ["No PRD found. Run prd_generate first."]
            }

        generator = PRDGenerator(project_root)
        is_valid, errors = generator.validate_prd(prd)

        return {
            "success": True,
            "is_valid": is_valid,
            "errors": errors,
            "story_count": len(prd.get("stories", []))
        }

    @mcp.tool()
    def prd_get_batches() -> Dict[str, Any]:
        """
        Get execution batches for parallel story execution.

        Stories are organized into batches based on their dependencies:
        - Batch 1: Stories with no dependencies (can run in parallel)
        - Batch 2+: Stories whose dependencies are in previous batches

        Returns:
            List of batches, where each batch contains stories that can run in parallel
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "error": "No PRD found. Run prd_generate first."
            }

        generator = PRDGenerator(project_root)
        batches = generator.generate_execution_batches(prd)

        # Format batches for output
        formatted_batches = []
        for i, batch in enumerate(batches, 1):
            batch_info = {
                "batch_number": i,
                "story_count": len(batch),
                "stories": [
                    {
                        "id": s["id"],
                        "title": s["title"],
                        "priority": s.get("priority", "medium"),
                        "status": s.get("status", "pending"),
                        "dependencies": s.get("dependencies", [])
                    }
                    for s in batch
                ]
            }
            formatted_batches.append(batch_info)

        return {
            "success": True,
            "total_batches": len(batches),
            "total_stories": len(prd.get("stories", [])),
            "batches": formatted_batches
        }

    @mcp.tool()
    def prd_update_story_status(story_id: str, status: str) -> Dict[str, Any]:
        """
        Update the status of a story in the PRD.

        Use this to track progress as stories are completed.

        Args:
            story_id: Story ID to update (e.g., "story-001")
            status: New status - "pending", "in_progress", or "complete"

        Returns:
            Updated story information
        """
        valid_statuses = ["pending", "in_progress", "complete"]
        if status not in valid_statuses:
            return {
                "success": False,
                "error": f"Invalid status '{status}'. Must be one of: {valid_statuses}"
            }

        state_manager = StateManager(project_root)

        try:
            state_manager.update_story_status(story_id, status)

            # Get updated story
            prd = state_manager.read_prd()
            story = None
            for s in prd.get("stories", []):
                if s["id"] == story_id:
                    story = s
                    break

            if not story:
                return {
                    "success": False,
                    "error": f"Story {story_id} not found in PRD"
                }

            return {
                "success": True,
                "message": f"Story {story_id} status updated to '{status}'",
                "story": story
            }

        except Exception as e:
            return {
                "success": False,
                "error": str(e)
            }

    @mcp.tool()
    def prd_detect_dependencies() -> Dict[str, Any]:
        """
        Automatically detect dependencies between stories.

        Analyzes story descriptions for dependency keywords like:
        "after", "once", "depends on", "requires", "following", "based on", "building on", "extends"

        Updates the PRD with detected dependencies.

        Returns:
            Stories with their detected dependencies
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "error": "No PRD found. Run prd_generate first."
            }

        stories = prd.get("stories", [])
        if not stories:
            return {
                "success": False,
                "error": "No stories in PRD. Add stories first."
            }

        generator = PRDGenerator(project_root)

        # Detect dependencies
        updated_stories = generator.detect_dependencies(stories.copy())

        # Update PRD
        prd["stories"] = updated_stories
        state_manager.write_prd(prd)

        # Format output
        dependency_summary = []
        for story in updated_stories:
            deps = story.get("dependencies", [])
            dependency_summary.append({
                "id": story["id"],
                "title": story["title"],
                "dependencies": deps,
                "has_dependencies": len(deps) > 0
            })

        return {
            "success": True,
            "message": "Dependencies detected and updated",
            "stories": dependency_summary,
            "stories_with_deps": sum(1 for s in dependency_summary if s["has_dependencies"])
        }
