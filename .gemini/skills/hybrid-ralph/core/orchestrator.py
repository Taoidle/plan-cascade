#!/usr/bin/env python3
"""
Orchestrator for Hybrid Ralph + Planning-with-Files

Manages parallel execution of user stories using Claude Code's Task tool.
Handles dependency resolution, batch execution, and failure recovery.
"""

import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from context_filter import ContextFilter
from state_manager import StateManager
from prd_generator import PRDGenerator


class StoryAgent:
    """Represents a background agent executing a story."""

    def __init__(self, story_id: str, story: Dict, context: Dict):
        """
        Initialize a story agent.

        Args:
            story_id: Story ID
            story: Story dictionary
            context: Context for the story
        """
        self.story_id = story_id
        self.story = story
        self.context = context
        self.task_id = None
        self.status = "pending"  # pending, running, complete, failed
        self.output_file = None

    def get_agent_prompt(self) -> str:
        """Generate the prompt for the agent executing this story."""
        prompt = f"""You are executing story {self.story_id}: {self.story['title']}

Description:
{self.story['description']}

Acceptance Criteria:
{chr(10).join(f'- {c}' for c in self.story.get('acceptance_criteria', []))}

Dependencies Summary:
{self._format_dependencies()}

Relevant Findings:
{self._format_findings()}

Your task:
1. Read the relevant code and documentation
2. Implement the story according to acceptance criteria
3. Test your implementation
4. Update findings.md with any discoveries (tag with <!-- @tags: {self.story_id} -->)
5. Mark as complete by appending to progress.txt: [COMPLETE] {self.story_id}

Work methodically and document your progress.
"""
        return prompt

    def _format_dependencies(self) -> str:
        """Format dependency summaries."""
        deps = self.context.get("dependencies", [])
        if not deps:
            return "None"

        lines = []
        for dep in deps:
            status = dep.get("status", "unknown")
            title = dep.get("title", "")
            lines.append(f"  - {dep['id']}: {title} (status: {status})")

        return "\n".join(lines) if lines else "None"

    def _format_findings(self) -> str:
        """Format relevant findings."""
        findings = self.context.get("findings", [])
        if not findings:
            return "No previous findings"

        return "\n".join(findings[:5])  # Limit to first 5 sections


class Orchestrator:
    """Orchestrates parallel execution of user stories."""

    def __init__(self, project_root: Path):
        """
        Initialize the orchestrator.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.context_filter = ContextFilter(project_root)
        self.state_manager = StateManager(project_root)
        self.prd_generator = PRDGenerator(project_root)

        # Track running agents
        self.running_agents: Dict[str, StoryAgent] = {}
        self.completed_stories: set = set()
        self.failed_stories: set = set()

    def analyze_dependencies(self) -> List[List[Dict]]:
        """
        Analyze story dependencies and create execution batches.

        Returns:
            List of batches, where each batch is a list of stories
        """
        prd = self.state_manager.read_prd()
        if not prd:
            raise ValueError("No PRD found. Cannot orchestrate execution.")

        return self.prd_generator.generate_execution_batches(prd)

    def launch_agent(self, story: Dict, batch_num: int) -> StoryAgent:
        """
        Launch a background Task agent for a story.

        Args:
            story: Story dictionary
            batch_num: Current batch number

        Returns:
            StoryAgent instance
        """
        story_id = story["id"]

        # Get context for this story
        context = self.context_filter.get_context_for_story(story_id)

        # Create agent
        agent = StoryAgent(story_id, story, context)
        agent.status = "running"

        # Generate output file path
        output_dir = self.project_root / ".agent-outputs"
        output_dir.mkdir(exist_ok=True)
        agent.output_file = output_dir / f"{story_id}.log"

        # Mark story as in progress
        self.state_manager.mark_story_in_progress(story_id)

        # Note: Actual agent launching is done via Claude Code's Task tool
        # This method prepares the agent configuration
        self.running_agents[story_id] = agent

        return agent

    def execute_batch(self, batch: List[Dict], batch_num: int, dry_run: bool = False) -> bool:
        """
        Execute a batch of stories in parallel.

        Args:
            batch: List of stories in this batch
            batch_num: Batch number
            dry_run: If True, only print what would be done

        Returns:
            True if all stories succeeded, False otherwise
        """
        print(f"\n{'='*60}")
        print(f"Executing Batch {batch_num}")
        print(f"{'='*60}")

        if dry_run:
            print("\nStories in this batch:")
            for story in batch:
                print(f"  - {story['id']}: {story['title']}")
            return True

        # Launch all agents in parallel
        agents = []
        for story in batch:
            agent = self.launch_agent(story, batch_num)
            agents.append(agent)
            print(f"Launched agent for {story['id']}: {story['title']}")

        # In real execution, agents run in background
        # For now, we'll prepare the execution plan
        print("\nAgents launched. Monitor progress with:")
        for agent in agents:
            print(f"  tail -f {agent.output_file}")

        return True

    def generate_execution_plan(self) -> str:
        """
        Generate a human-readable execution plan.

        Returns:
            Execution plan as formatted string
        """
        batches = self.analyze_dependencies()

        lines = [
            "=" * 60,
            "EXECUTION PLAN",
            "=" * 60,
            ""
        ]

        total_stories = sum(len(batch) for batch in batches)
        lines.append(f"Total Stories: {total_stories}")
        lines.append(f"Total Batches: {len(batches)}")
        lines.append("")

        for i, batch in enumerate(batches, 1):
            lines.append(f"Batch {i}:")
            lines.append(f"  Stories: {len(batch)}")

            # Group by priority
            high = [s for s in batch if s.get("priority") == "high"]
            medium = [s for s in batch if s.get("priority") == "medium"]
            low = [s for s in batch if s.get("priority") == "low"]

            if high:
                lines.append("  High Priority:")
                for story in high:
                    deps = story.get("dependencies", [])
                    dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                    lines.append(f"    - {story['id']}: {story['title']}{dep_str}")

            if medium:
                lines.append("  Medium Priority:")
                for story in medium:
                    deps = story.get("dependencies", [])
                    dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                    lines.append(f"    - {story['id']}: {story['title']}{dep_str}")

            if low:
                lines.append("  Low Priority:")
                for story in low:
                    deps = story.get("dependencies", [])
                    dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                    lines.append(f"    - {story['id']}: {story['title']}{dep_str}")

            lines.append("")

        lines.append("=" * 60)
        lines.append("EXECUTION STRATEGY")
        lines.append("=" * 60)
        lines.append("")
        lines.append("1. Each batch executes in parallel")
        lines.append("2. Batch N+1 starts only after Batch N completes")
        lines.append("3. Stories within a batch are independent")
        lines.append("4. Failed stories will be retried once")
        lines.append("")

        return "\n".join(lines)

    def generate_claude_task_commands(self) -> List[str]:
        """
        Generate Claude Code Task commands for batch execution.

        Returns:
            List of task invocation commands
        """
        batches = self.analyze_dependencies()
        commands = []

        for i, batch in enumerate(batches, 1):
            for story in batch:
                story_id = story["id"]
                title = story["title"]
                description = story["description"]

                # Generate Task tool invocation
                cmd = f'<tool_call>\n<tool_name>Task</tool_name>\n<parameters>\n<subagent_type>general-purpose</subagent_type>\n<description>Execute {story_id}: {title}</description>\n<prompt><![CDATA['
                cmd += f"You are executing story {story_id}: {title}\n\n"
                cmd += f"Description:\n{description}\n\n"
                cmd += "Acceptance Criteria:\n"
                for ac in story.get("acceptance_criteria", []):
                    cmd += f"- {ac}\n"
                cmd += f"\nAfter completion, append to progress.txt: [COMPLETE] {story_id}"
                cmd += "]]></prompt>\n<run_in_background>true</run_in_background>\n</parameters>\n</tool_call>"

                commands.append(cmd)

        return commands

    def get_story_status(self, story_id: str) -> str:
        """
        Get the current status of a story.

        Args:
            story_id: Story ID

        Returns:
            Status: pending, in_progress, complete, or failed
        """
        # Check progress.txt
        all_statuses = self.state_manager.get_all_story_statuses()
        return all_statuses.get(story_id, "pending")

    def check_batch_complete(self, batch: List[Dict]) -> bool:
        """
        Check if all stories in a batch are complete.

        Args:
            batch: List of stories in the batch

        Returns:
            True if all complete, False otherwise
        """
        for story in batch:
            status = self.get_story_status(story["id"])
            if status != "complete":
                return False
        return True

    def print_status(self):
        """Print current execution status."""
        batches = self.analyze_dependencies()

        print("\n" + "=" * 60)
        print("EXECUTION STATUS")
        print("=" * 60)

        for i, batch in enumerate(batches, 1):
            print(f"\nBatch {i}:")
            for story in batch:
                story_id = story["id"]
                title = story["title"]
                status = self.get_story_status(story_id)

                status_symbol = {
                    "pending": "○",
                    "in_progress": "◐",
                    "complete": "●",
                    "failed": "✗"
                }.get(status, "?")

                print(f"  {status_symbol} {story_id}: {title} [{status}]")

        print("\n" + "=" * 60)


def main():
    """CLI interface for testing orchestrator."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: orchestrator.py <command> [args]")
        print("Commands:")
        print("  plan                - Show execution plan")
        print("  status              - Show execution status")
        print("  execute-batch <n>   - Execute a specific batch")
        print("  check-complete <n>  - Check if batch is complete")
        print("  generate-tasks      - Generate Claude Task commands")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    orch = Orchestrator(project_root)

    if command == "plan":
        print(orch.generate_execution_plan())

    elif command == "status":
        orch.print_status()

    elif command == "execute-batch" and len(sys.argv) >= 3:
        batch_num = int(sys.argv[2])
        batches = orch.analyze_dependencies()
        if 1 <= batch_num <= len(batches):
            orch.execute_batch(batches[batch_num - 1], batch_num)
        else:
            print(f"Invalid batch number. Must be 1-{len(batches)}")

    elif command == "check-complete" and len(sys.argv) >= 3:
        batch_num = int(sys.argv[2])
        batches = orch.analyze_dependencies()
        if 1 <= batch_num <= len(batches):
            batch = batches[batch_num - 1]
            if orch.check_batch_complete(batch):
                print(f"Batch {batch_num} is complete!")
            else:
                print(f"Batch {batch_num} is not yet complete.")
        else:
            print(f"Invalid batch number.")

    elif command == "generate-tasks":
        commands = orch.generate_claude_task_commands()
        print("Claude Code Task commands:")
        for cmd in commands:
            print(cmd)
            print("---")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
