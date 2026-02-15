#!/usr/bin/env python3
"""
PRD Story Orchestrator for Plan Cascade

Executes stories from a PRD in batches with dependency resolution.
Handles parallel agent execution with automatic fallback and context injection.

Optionally integrates with StageStateMachine for unified stage tracking.
"""

import json
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any, TYPE_CHECKING

from ..state.context_filter import ContextFilter
from ..state.state_manager import StateManager

if TYPE_CHECKING:
    from ..backends.agent_executor import AgentExecutor
    from ..backends.phase_config import AgentOverrides, ExecutionPhase
    from ..state.path_resolver import PathResolver
    from .stage_state import StageStateMachine, ExecutionStage


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
        stage_machine: "StageStateMachine | None" = None,
        agent_executor: "AgentExecutor | None" = None,
        agent_override: "AgentOverrides | None" = None,
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
            stage_machine: Optional StageStateMachine for unified stage tracking.
                When provided, the orchestrator will update stage states during execution.
                When None (default), the orchestrator behaves as before (backward compatible).
            agent_executor: Optional AgentExecutor for actual story execution.
                When provided, execute_story will use this to actually run stories.
                When None (default), stories are only marked as started (legacy behavior).
            agent_override: Optional phase-based agent overrides applied to all stories.
        """
        self.project_root = Path(project_root)
        self.agents = agents or self.DEFAULT_AGENTS
        self._stage_machine = stage_machine
        self._agent_executor = agent_executor
        self._agent_override = agent_override

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

    @property
    def stage_machine(self) -> "StageStateMachine | None":
        """Get the StageStateMachine instance if configured."""
        return self._stage_machine

    def set_stage_machine(self, machine: "StageStateMachine") -> None:
        """
        Set or update the stage machine.

        Args:
            machine: StageStateMachine instance to use
        """
        self._stage_machine = machine

    @property
    def agent_executor(self) -> "AgentExecutor | None":
        """Get the AgentExecutor instance if configured."""
        return self._agent_executor

    def set_agent_executor(self, executor: "AgentExecutor") -> None:
        """
        Set or update the agent executor.

        Args:
            executor: AgentExecutor instance to use for story execution
        """
        self._agent_executor = executor

    @property
    def agent_override(self) -> "AgentOverrides | None":
        """Get global agent overrides used for execution."""
        return self._agent_override

    def set_agent_override(self, override: "AgentOverrides | None") -> None:
        """
        Set or clear global agent overrides.

        Args:
            override: AgentOverrides instance, or None to clear overrides.
        """
        self._agent_override = override

    def get_stage_status(self) -> dict[str, Any] | None:
        """
        Get current stage status from the stage machine.

        Returns:
            Stage progress summary dict if stage machine is configured, None otherwise
        """
        if self._stage_machine is None:
            return None
        return self._stage_machine.get_progress_summary()

    def _update_stage(
        self,
        action: str,
        stage: "ExecutionStage | None" = None,
        outputs: dict[str, Any] | None = None,
        errors: list[str] | None = None,
    ) -> None:
        """
        Internal helper to update stage state if stage machine is configured.

        Args:
            action: One of 'start', 'complete', 'fail'
            stage: Stage to update (uses EXECUTE if not specified)
            outputs: Stage outputs (for complete action)
            errors: Error messages (for fail action)
        """
        if self._stage_machine is None:
            return

        from .stage_state import ExecutionStage

        target_stage = stage or ExecutionStage.EXECUTE

        try:
            if action == "start":
                self._stage_machine.start_stage(target_stage)
            elif action == "complete":
                self._stage_machine.complete_stage(target_stage, outputs or {})
            elif action == "fail":
                self._stage_machine.fail_stage(target_stage, errors or [])
        except ValueError:
            # Stage transition not valid - this is expected in some cases
            # e.g., stage already started by external caller
            pass

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
        task_callback: Callable | None = None,
        phase: "ExecutionPhase | None" = None,
    ) -> tuple[bool, str]:
        """
        Execute a single story.

        Args:
            story: Story dictionary
            agent: Agent to use (auto-selected if not provided)
            dry_run: If True, don't actually execute
            task_callback: Optional callback for Task tool execution (for claude-code)
            phase: Optional execution phase for phase-based agent routing.

        Returns:
            Tuple of (success, message)
        """
        story_id = story.get("id", "unknown")

        if dry_run:
            agent_label = agent.name if agent else "auto"
            return True, f"[DRY RUN] Would execute {story_id} with {agent_label}"

        try:
            # Use AgentExecutor if available for actual execution
            if self._agent_executor:
                # Get context for the story
                context = self.context_filter.get_context_for_story(story_id)

                # Load PRD metadata (optional hints for backend selection)
                prd = self.state_manager.read_prd() or {}
                prd_metadata = prd.get("metadata", {}) if isinstance(prd, dict) else {}

                # Default to implementation phase for backend agent selection
                resolved_phase = phase
                if resolved_phase is None:
                    try:
                        from ..backends.phase_config import ExecutionPhase

                        resolved_phase = ExecutionPhase.IMPLEMENTATION
                    except Exception:
                        resolved_phase = None

                # Execute via the backend (let it resolve the agent unless explicitly provided)
                requested_agent = agent.name if agent else None
                result = self._agent_executor.execute_story(
                    story=story,
                    context=context,
                    agent_name=requested_agent,
                    prd_metadata=prd_metadata,
                    task_callback=task_callback,
                    phase=resolved_phase,
                    override=self._agent_override,
                )

                success = result.get("success", False)
                resolved_agent = result.get("agent") or requested_agent or "unknown"
                message = result.get("error") or f"Executed {story_id} with {resolved_agent}"

                if success:
                    self.state_manager.record_agent_complete(story_id, resolved_agent)
                else:
                    self.state_manager.record_agent_failure(story_id, resolved_agent, message)

                return success, message
            else:
                # Get or select agent
                if not agent:
                    agent = self.get_available_agent()

                if not agent:
                    return False, "No agent available"

                # Mark story as in progress
                self.state_manager.record_agent_start(story_id, agent.name)

                # Legacy behavior: just mark as started (for polling-based execution)
                return True, f"Started {story_id} with {agent.name}"
        except Exception as e:
            self.state_manager.record_agent_failure(
                story_id,
                agent.name if agent else "unknown",
                str(e),
            )
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

        # Record batch results to stage machine if configured
        self._record_batch_to_stage(batch_num, results)

        return results

    def _record_batch_to_stage(
        self,
        batch_num: int,
        results: dict[str, tuple[bool, str]],
    ) -> None:
        """
        Record batch execution results to stage machine outputs.

        Args:
            batch_num: Batch number
            results: Batch execution results
        """
        if self._stage_machine is None:
            return

        from .stage_state import ExecutionStage, StageStatus

        execute_state = self._stage_machine.get_stage_state(ExecutionStage.EXECUTE)
        if execute_state.status != StageStatus.IN_PROGRESS:
            return

        # Accumulate results in stage outputs
        batch_outputs = execute_state.outputs.get("batch_results", {})
        batch_outputs[f"batch_{batch_num}"] = {
            "completed": [sid for sid, (ok, _) in results.items() if ok],
            "failed": [sid for sid, (ok, _) in results.items() if not ok],
        }
        execute_state.outputs["batch_results"] = batch_outputs
        execute_state.outputs["batches_executed"] = batch_num

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

        When a stage_machine is configured, this method updates the EXECUTE stage:
        - Starts EXECUTE stage at the beginning (if not already started)
        - Records batch results as stage outputs
        - Completes or fails EXECUTE stage based on results

        Args:
            dry_run: If True, don't actually execute
            callback: Optional callback after each batch

        Returns:
            Summary dictionary with results
        """
        batches = self.analyze_dependencies()
        if not batches:
            return {"success": False, "error": "No batches to execute"}

        # Start EXECUTE stage if stage machine is configured
        self._update_stage("start")

        all_results: dict[str, tuple[bool, str]] = {}
        failed_stories: list[str] = []
        completed_stories: list[str] = []

        try:
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

            # Prepare summary
            summary = {
                "success": len(failed_stories) == 0,
                "total_batches": len(batches),
                "total_stories": len(all_results),
                "completed": len(completed_stories),
                "failed": len(failed_stories),
                "failed_stories": failed_stories,
                "results": all_results,
            }

            # Complete or fail EXECUTE stage based on results
            if len(failed_stories) == 0:
                self._update_stage(
                    "complete",
                    outputs={
                        "completed_stories": completed_stories,
                        "batches_executed": len(batches),
                    }
                )
            else:
                self._update_stage(
                    "fail",
                    errors=[f"Stories failed: {', '.join(failed_stories)}"]
                )

            return summary

        except Exception as e:
            # Fail EXECUTE stage on exception
            self._update_stage("fail", errors=[str(e)])
            raise

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
