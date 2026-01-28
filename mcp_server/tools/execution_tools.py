#!/usr/bin/env python3
"""
Execution Tools for Plan Cascade MCP Server

Provides MCP tools for story execution and state management:
- get_story_context: Get context for a specific story
- get_execution_status: Get overall execution status
- append_findings: Record findings during development
- mark_story_complete: Mark a story as complete
- get_progress: Get progress summary
- cleanup_locks: Clean up stale lock files
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
from context_filter import ContextFilter
from state_manager import StateManager
from prd_generator import PRDGenerator


def register_execution_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all execution-related tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def get_story_context(story_id: str) -> Dict[str, Any]:
        """
        Get all relevant context for a specific story.

        This provides everything needed to work on a story:
        - Story details (title, description, acceptance criteria)
        - Dependency information with completion status
        - Tagged findings from previous work
        - Context size estimate

        Args:
            story_id: Story ID to get context for (e.g., "story-001")

        Returns:
            Comprehensive context for the story including dependencies and findings
        """
        context_filter = ContextFilter(project_root)
        context = context_filter.get_context_for_story(story_id)

        if "error" in context:
            return {
                "success": False,
                "error": context["error"]
            }

        # Add additional status information
        story = context.get("story", {})

        return {
            "success": True,
            "story_id": story_id,
            "story": {
                "id": story.get("id"),
                "title": story.get("title"),
                "description": story.get("description"),
                "priority": story.get("priority", "medium"),
                "status": story.get("status", "pending"),
                "acceptance_criteria": story.get("acceptance_criteria", []),
                "tags": story.get("tags", [])
            },
            "dependencies": context.get("dependencies", []),
            "findings": context.get("findings", []),
            "context_estimate": context.get("context_estimate", "medium"),
            "dependent_stories": context_filter.get_dependent_stories(story_id)
        }

    @mcp.tool()
    def get_execution_status() -> Dict[str, Any]:
        """
        Get the overall execution status of the PRD.

        Shows:
        - Total stories and their status breakdown
        - Current execution batch
        - Stories ready to execute
        - Blocked stories

        Returns:
            Comprehensive execution status including batches and progress
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "error": "No PRD found. Run prd_generate first."
            }

        # Get story statuses from progress.txt
        progress_statuses = state_manager.get_all_story_statuses()

        # Merge with PRD statuses
        stories = prd.get("stories", [])
        status_counts = {"pending": 0, "in_progress": 0, "complete": 0}
        story_details = []

        for story in stories:
            story_id = story["id"]
            # Progress.txt takes precedence
            status = progress_statuses.get(story_id, story.get("status", "pending"))
            status_counts[status] = status_counts.get(status, 0) + 1

            story_details.append({
                "id": story_id,
                "title": story["title"],
                "status": status,
                "dependencies": story.get("dependencies", [])
            })

        # Generate batches to find current batch
        generator = PRDGenerator(project_root)
        batches = generator.generate_execution_batches(prd)

        # Find current batch (first batch with incomplete stories)
        current_batch = 0
        ready_stories = []
        blocked_stories = []

        for i, batch in enumerate(batches, 1):
            batch_complete = True
            for story in batch:
                status = progress_statuses.get(story["id"], story.get("status", "pending"))
                if status != "complete":
                    batch_complete = False
                    if i == current_batch or current_batch == 0:
                        current_batch = i
                        # Check if dependencies are satisfied
                        deps = story.get("dependencies", [])
                        deps_complete = all(
                            progress_statuses.get(d, "pending") == "complete"
                            for d in deps
                        )
                        if deps_complete:
                            ready_stories.append({
                                "id": story["id"],
                                "title": story["title"]
                            })
                        else:
                            blocked_stories.append({
                                "id": story["id"],
                                "title": story["title"],
                                "waiting_for": [d for d in deps if progress_statuses.get(d, "pending") != "complete"]
                            })

            if not batch_complete and current_batch == 0:
                current_batch = i

        total = len(stories)
        complete = status_counts.get("complete", 0)
        percentage = int((complete / total) * 100) if total > 0 else 0

        return {
            "success": True,
            "total_stories": total,
            "status_counts": status_counts,
            "progress_percentage": percentage,
            "current_batch": current_batch,
            "total_batches": len(batches),
            "ready_to_execute": ready_stories,
            "blocked_stories": blocked_stories,
            "all_stories": story_details
        }

    @mcp.tool()
    def append_findings(
        content: str,
        story_id: Optional[str] = None,
        tags: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Append findings to findings.md.

        Use this to record discoveries, decisions, and notes during development.
        Findings can be tagged with story IDs for context filtering.

        Args:
            content: The finding content (markdown supported)
            story_id: Optional story ID to associate with (shorthand for tags)
            tags: Optional list of tags (e.g., ["story-001", "story-002", "api"])

        Returns:
            Confirmation of findings being recorded
        """
        state_manager = StateManager(project_root)

        # Combine story_id and tags
        all_tags = list(tags) if tags else []
        if story_id and story_id not in all_tags:
            all_tags.insert(0, story_id)

        try:
            state_manager.append_findings(content, all_tags if all_tags else None)

            return {
                "success": True,
                "message": "Findings recorded successfully",
                "tags": all_tags,
                "file_path": str(project_root / "findings.md")
            }
        except Exception as e:
            return {
                "success": False,
                "error": str(e)
            }

    @mcp.tool()
    def mark_story_complete(story_id: str) -> Dict[str, Any]:
        """
        Mark a story as complete.

        This updates both the PRD status and progress.txt tracking.
        Use this when all acceptance criteria have been met.

        Args:
            story_id: Story ID to mark complete (e.g., "story-001")

        Returns:
            Updated status and information about next steps
        """
        state_manager = StateManager(project_root)
        prd = state_manager.read_prd()

        if not prd:
            return {
                "success": False,
                "error": "No PRD found."
            }

        # Find the story
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

        try:
            # Update PRD status
            state_manager.update_story_status(story_id, "complete")

            # Mark complete in progress.txt
            state_manager.mark_story_complete(story_id)

            # Find newly unblocked stories
            context_filter = ContextFilter(project_root)
            dependents = context_filter.get_dependent_stories(story_id)

            # Check which dependents are now ready
            progress_statuses = state_manager.get_all_story_statuses()
            progress_statuses[story_id] = "complete"  # Include current completion

            newly_ready = []
            for dep_id in dependents:
                dep_story = context_filter.get_story(dep_id)
                if dep_story:
                    deps = dep_story.get("dependencies", [])
                    if all(progress_statuses.get(d, "pending") == "complete" for d in deps):
                        newly_ready.append({
                            "id": dep_id,
                            "title": dep_story.get("title", "")
                        })

            return {
                "success": True,
                "message": f"Story {story_id} marked as complete",
                "story": {
                    "id": story_id,
                    "title": story.get("title")
                },
                "newly_unblocked_stories": newly_ready
            }

        except Exception as e:
            return {
                "success": False,
                "error": str(e)
            }

    @mcp.tool()
    def get_progress() -> Dict[str, Any]:
        """
        Get a progress summary of the current development.

        Returns the contents of progress.txt showing the timeline of work.

        Returns:
            Progress timeline and summary statistics
        """
        state_manager = StateManager(project_root)

        progress_content = state_manager.read_progress()
        statuses = state_manager.get_all_story_statuses()

        # Count statuses
        complete_count = sum(1 for s in statuses.values() if s == "complete")
        in_progress_count = sum(1 for s in statuses.values() if s == "in_progress")

        # Parse progress entries
        entries = []
        for line in progress_content.split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                entries.append(line)

        return {
            "success": True,
            "stories_complete": complete_count,
            "stories_in_progress": in_progress_count,
            "stories_tracked": len(statuses),
            "story_statuses": statuses,
            "recent_entries": entries[-20:] if len(entries) > 20 else entries,  # Last 20 entries
            "total_entries": len(entries),
            "file_path": str(project_root / "progress.txt")
        }

    @mcp.tool()
    def cleanup_locks() -> Dict[str, Any]:
        """
        Clean up stale lock files.

        Lock files older than 1 hour are considered stale and will be removed.
        Use this if you encounter lock errors due to interrupted operations.

        Returns:
            Cleanup result
        """
        state_manager = StateManager(project_root)

        try:
            state_manager.cleanup_locks()

            # Also check mega-plan locks if they exist
            mega_locks_dir = project_root / ".locks"
            removed_count = 0

            if mega_locks_dir.exists():
                import time
                for lock_file in mega_locks_dir.glob("*.lock"):
                    try:
                        if lock_file.stat().st_mtime < time.time() - 3600:
                            lock_file.unlink()
                            removed_count += 1
                    except Exception:
                        pass

            return {
                "success": True,
                "message": f"Lock cleanup complete. Removed {removed_count} stale lock files.",
                "locks_removed": removed_count
            }
        except Exception as e:
            return {
                "success": False,
                "error": str(e)
            }
