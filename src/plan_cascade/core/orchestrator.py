#!/usr/bin/env python3
"""
PRD Story Orchestrator for Plan Cascade

Executes stories from a PRD in batches with dependency resolution.
Handles parallel agent execution with automatic fallback and context injection.
"""

import json
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any, TYPE_CHECKING

from ..state.context_filter import ContextFilter
from ..state.state_manager import StateManager

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver


class StoryAgent:
    """Represents an agent capable of executing stories."""

    def __init__(
        self,
        name: str,
        command_template: str,
        check_available: Callable[[], bool] | None = None,
        priority: int = 0,
    ):
        """
        Initialize a story agent.

        Args:
            name: Agent name (e.g., "claude-code", "aider")
            command_template: Command template with placeholders
            check_available: Function to check if agent is available
            priority: Higher priority agents are tried first
        """
        self.name = name
        self.command_template = command_template
        self.check_available = check_available or (lambda: True)
        self.priority = priority

    def is_available(self) -> bool:
        """Check if this agent is available."""
        try:
            return self.check_available()
        except Exception:
            return False


class Orchestrator:
    """Orchestrates the execution of PRD stories in batches."""

    DEFAULT_AGENTS = [
        StoryAgent(
            name="claude-code",
            command_template='claude --chat "{prompt}"',
            priority=100,
        ),
        StoryAgent(
            name="aider",
            command_template='aider --message "{prompt}"',
            priority=50,
        ),
    ]

    def __init__(
        self,
        project_root: Path,
        agents: list[StoryAgent] | None = None,
        state_manager: StateManager | None = None,
        context_filter: ContextFilter | None = None,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the orchestrator.

        Args:
            project_root: Root directory of the project
            agents: List of available agents
            state_manager: StateManager instance (created if not provided)
            context_filter: ContextFilter instance (created if not provided)
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for all paths (backward compatible).
                If None, defaults to True when path_resolver is not provided.
        """
        self.project_root = Path(project_root)
        self.agents = agents or self.DEFAULT_AGENTS

        # Set up PathResolver
        if path_resolver is not None:
            self._path_resolver = path_resolver
        else:
            # Default to legacy mode for backward compatibility
            if legacy_mode is None:
                legacy_mode = True
            from ..state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Create or use provided StateManager/ContextFilter with PathResolver
        self.state_manager = state_manager or StateManager(
            self.project_root,
            path_resolver=self._path_resolver,
        )
        self.context_filter = context_filter or ContextFilter(
            self.project_root,
            path_resolver=self._path_resolver,
        )

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

        # Sort agents by priority (highest first)
        self.agents.sort(key=lambda a: a.priority, reverse=True)

    def load_prd(self) -> dict | None:
        """Load the PRD from the state manager."""
        return self.state_manager.read_prd()

    def analyze_dependencies(self) -> list[list[dict]]:
        """
        Analyze story dependencies and create execution batches.

        Returns:
            List of batches, where each batch is a list of stories
        """
        prd = self.load_prd()
        if not prd:
            return []

        stories = prd.get("stories", [])
        if not stories:
            return []

        # Build dependency graph
        completed: set = set()
        batches: list[list[dict]] = []
        story_map = {s["id"]: s for s in stories}

        # Get already completed stories
        statuses = self.state_manager.get_all_story_statuses()
        for story_id, status in statuses.items():
            if status == "complete":
                completed.add(story_id)

        while len(completed) < len(stories):
            # Find ready stories
            ready = []
            for story in stories:
                story_id = story["id"]
                if story_id in completed:
                    continue

                # Check dependencies
                deps = story.get("dependencies", [])
                if all(dep in completed for dep in deps):
                    ready.append(story)

            if not ready:
                # Circular dependency or all remaining have unmet deps
                remaining = [s for s in stories if s["id"] not in completed]
                if remaining:
                    print(f"Warning: Could not resolve dependencies for: {[s['id'] for s in remaining]}")
                    # Add remaining as final batch anyway
                    ready = remaining
                else:
                    break

            # Sort by priority within batch
            priority_order = {"high": 0, "medium": 1, "low": 2}
            ready.sort(key=lambda s: priority_order.get(s.get("priority", "medium"), 1))

            batches.append(ready)
            completed.update(s["id"] for s in ready)

        return batches

    def get_available_agent(self) -> StoryAgent | None:
        """Get the highest priority available agent."""
        for agent in self.agents:
            if agent.is_available():
                return agent
        return None

    def build_story_prompt(self, story: dict) -> str:
        """
        Build the execution prompt for a story.

        Args:
            story: Story dictionary from PRD

        Returns:
            Formatted prompt string
        """
        story_id = story.get("id", "unknown")
        title = story.get("title", "")
        description = story.get("description", "")
        acceptance_criteria = story.get("acceptance_criteria", [])

        # Get context from context filter
        context = self.context_filter.get_context_for_story(story_id)

        # Build prompt
        lines = [
            f"## Story: {title}",
            f"ID: {story_id}",
            "",
            "### Description",
            description,
            "",
        ]

        # Add acceptance criteria
        if acceptance_criteria:
            lines.extend([
                "### Acceptance Criteria",
                "",
            ])
            for i, criterion in enumerate(acceptance_criteria, 1):
                lines.append(f"{i}. {criterion}")
            lines.append("")

        # Add dependency context
        dependencies = context.get("dependencies", [])
        if dependencies:
            lines.extend([
                "### Dependencies (Context)",
                "",
            ])
            for dep in dependencies:
                lines.append(f"- {dep.get('id')}: {dep.get('title')} [{dep.get('status')}]")
                if dep.get("summary"):
                    lines.append(f"  Summary: {dep.get('summary')[:200]}")
            lines.append("")

        # Add findings context
        findings = context.get("findings", [])
        if findings:
            lines.extend([
                "### Relevant Findings",
                "",
            ])
            for finding in findings[:3]:  # Limit to 3 most relevant
                lines.append(f"- {finding[:500]}")
            lines.append("")

        # Add instructions
        lines.extend([
            "### Instructions",
            "",
            "Please implement this story according to the description and acceptance criteria.",
            "Ensure all acceptance criteria are met before marking the story complete.",
            "",
        ])

        return "\n".join(lines)

    def execute_story(
        self,
        story: dict,
        agent: StoryAgent | None = None,
        dry_run: bool = False,
    ) -> tuple[bool, str]:
        """
        Execute a single story.

        Args:
            story: Story dictionary
            agent: Agent to use (auto-selected if not provided)
            dry_run: If True, don't actually execute

        Returns:
            Tuple of (success, message)
        """
        story_id = story.get("id", "unknown")

        # Get or select agent
        if not agent:
            agent = self.get_available_agent()

        if not agent:
            return False, "No agent available"

        # Build prompt
        prompt = self.build_story_prompt(story)

        if dry_run:
            return True, f"[DRY RUN] Would execute {story_id} with {agent.name}"

        # Mark story as in progress
        self.state_manager.record_agent_start(story_id, agent.name)

        # Execute (in real implementation, this would launch the agent)
        # For now, we just return success to allow the structure to work
        try:
            # The actual execution is handled by the backend layer
            # This method just coordinates the high-level flow
            return True, f"Started {story_id} with {agent.name}"
        except Exception as e:
            self.state_manager.record_agent_failure(story_id, agent.name, str(e))
            return False, str(e)

    def execute_batch(
        self,
        batch: list[dict],
        batch_num: int,
        dry_run: bool = False,
    ) -> dict[str, tuple[bool, str]]:
        """
        Execute a batch of stories.

        Args:
            batch: List of stories to execute
            batch_num: Batch number for logging
            dry_run: If True, don't actually execute

        Returns:
            Dictionary mapping story IDs to (success, message) tuples
        """
        results: dict[str, tuple[bool, str]] = {}

        print(f"\n{'='*60}")
        print(f"Batch {batch_num}: {len(batch)} stories")
        print(f"{'='*60}")

        for story in batch:
            story_id = story.get("id", "unknown")
            title = story.get("title", "")

            print(f"\n  [{story_id}] {title}")

            success, message = self.execute_story(story, dry_run=dry_run)
            results[story_id] = (success, message)

            status = "OK" if success else "FAIL"
            print(f"    -> {status}: {message}")

        return results

    def check_batch_complete(self, batch: list[dict]) -> bool:
        """
        Check if all stories in a batch are complete.

        Args:
            batch: List of stories

        Returns:
            True if all stories are complete
        """
        statuses = self.state_manager.get_all_story_statuses()

        for story in batch:
            story_id = story.get("id")
            status = statuses.get(story_id, "pending")
            if status != "complete":
                return False

        return True

    def execute_all(
        self,
        dry_run: bool = False,
        callback: Callable[[int, list[dict], dict], None] | None = None,
    ) -> dict[str, Any]:
        """
        Execute all stories in dependency order.

        Args:
            dry_run: If True, don't actually execute
            callback: Optional callback after each batch

        Returns:
            Summary dictionary with results
        """
        batches = self.analyze_dependencies()
        if not batches:
            return {"success": False, "error": "No batches to execute"}

        all_results: dict[str, tuple[bool, str]] = {}
        failed_stories: list[str] = []
        completed_stories: list[str] = []

        for batch_num, batch in enumerate(batches, 1):
            results = self.execute_batch(batch, batch_num, dry_run)
            all_results.update(results)

            # Track successes and failures
            for story_id, (success, _) in results.items():
                if success:
                    completed_stories.append(story_id)
                else:
                    failed_stories.append(story_id)

            # Call callback if provided
            if callback:
                callback(batch_num, batch, results)

        return {
            "success": len(failed_stories) == 0,
            "total_batches": len(batches),
            "total_stories": len(all_results),
            "completed": len(completed_stories),
            "failed": len(failed_stories),
            "failed_stories": failed_stories,
            "results": all_results,
        }

    def get_execution_plan(self) -> str:
        """
        Generate a human-readable execution plan.

        Returns:
            Execution plan as formatted string
        """
        batches = self.analyze_dependencies()
        if not batches:
            return "No stories to execute"

        prd = self.load_prd()
        goal = prd.get("goal", "N/A") if prd else "N/A"

        lines = [
            "=" * 60,
            "EXECUTION PLAN",
            "=" * 60,
            "",
            f"Goal: {goal}",
            f"Total Batches: {len(batches)}",
            f"Total Stories: {sum(len(b) for b in batches)}",
            "",
        ]

        for i, batch in enumerate(batches, 1):
            lines.append(f"Batch {i}:")
            for story in batch:
                deps = story.get("dependencies", [])
                dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                status = story.get("status", "pending")
                lines.append(f"  [{status}] {story.get('id')}: {story.get('title')}{dep_str}")
            lines.append("")

        lines.append("=" * 60)
        return "\n".join(lines)

    def print_status(self) -> None:
        """Print current execution status."""
        batches = self.analyze_dependencies()
        statuses = self.state_manager.get_all_story_statuses()
        agent_summary = self.state_manager.get_agent_summary()

        print("\n" + "=" * 60)
        print("EXECUTION STATUS")
        print("=" * 60)

        print("\nAgent Summary:")
        print(f"  Running: {agent_summary['running']}")
        print(f"  Completed: {agent_summary['completed']}")
        print(f"  Failed: {agent_summary['failed']}")

        print("\nStories by Batch:")
        for i, batch in enumerate(batches, 1):
            print(f"\n  Batch {i}:")
            for story in batch:
                story_id = story.get("id")
                title = story.get("title", "")
                status = statuses.get(story_id, "pending")

                symbol = {
                    "pending": " ",
                    "in_progress": ">",
                    "complete": "X",
                    "failed": "!",
                }.get(status, "?")

                print(f"    [{symbol}] {story_id}: {title}")

        print("\n" + "=" * 60)


def main():
    """CLI interface for testing orchestrator."""
    if len(sys.argv) < 2:
        print("Usage: orchestrator.py <command>")
        print("Commands:")
        print("  plan       - Show execution plan")
        print("  status     - Show execution status")
        print("  execute    - Execute all stories")
        print("  dry-run    - Dry run execution")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    orchestrator = Orchestrator(project_root)

    if command == "plan":
        print(orchestrator.get_execution_plan())

    elif command == "status":
        orchestrator.print_status()

    elif command == "execute":
        results = orchestrator.execute_all()
        print("\nExecution Results:")
        print(json.dumps(results, indent=2, default=str))

    elif command == "dry-run":
        results = orchestrator.execute_all(dry_run=True)
        print("\nDry Run Results:")
        print(json.dumps(results, indent=2, default=str))

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
