#!/usr/bin/env python3
"""
Orchestrator for Hybrid Ralph + Planning-with-Files

Manages parallel execution of user stories using Claude Code's Task tool
or external CLI agents (codex, amp-code, aider, etc.).
Handles dependency resolution, batch execution, and failure recovery.

Extended for multi-agent collaboration support with:
- Automatic iteration loop
- Quality gates
- Retry management
- Phase-based agent selection
"""

import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple, TYPE_CHECKING

try:
    from .context_filter import ContextFilter
    from .state_manager import StateManager
    from .prd_generator import PRDGenerator
    from .agent_executor import AgentExecutor
except ImportError:
    from context_filter import ContextFilter
    from state_manager import StateManager
    from prd_generator import PRDGenerator
    from agent_executor import AgentExecutor

# Import new modules for iteration, quality gates, and retry management
try:
    from .iteration_loop import IterationLoop, IterationConfig, IterationMode, IterationCallbacks, IterationState
    from .quality_gate import QualityGate, GateConfig, GateType
    from .retry_manager import RetryManager, RetryConfig, ErrorType
    from .phase_config import PhaseAgentManager, ExecutionPhase, AgentOverrides
    from .cross_platform_detector import CrossPlatformDetector
except ImportError:
    try:
        from iteration_loop import IterationLoop, IterationConfig, IterationMode, IterationCallbacks, IterationState
        from quality_gate import QualityGate, GateConfig, GateType
        from retry_manager import RetryManager, RetryConfig, ErrorType
        from phase_config import PhaseAgentManager, ExecutionPhase, AgentOverrides
        from cross_platform_detector import CrossPlatformDetector
    except ImportError:
        IterationLoop = None
        QualityGate = None
        RetryManager = None
        PhaseAgentManager = None
        ExecutionPhase = None
        AgentOverrides = None
        CrossPlatformDetector = None


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

    def __init__(
        self,
        project_root: Path,
        agents_config: Optional[Dict] = None,
        default_agent: Optional[str] = None,
        iteration_config: Optional["IterationConfig"] = None,
        quality_gate: Optional["QualityGate"] = None,
        retry_config: Optional["RetryConfig"] = None,
        agent_overrides: Optional["AgentOverrides"] = None
    ):
        """
        Initialize the orchestrator.

        Args:
            project_root: Root directory of the project
            agents_config: Optional agent configuration dict
            default_agent: Optional default agent name override
            iteration_config: Configuration for automatic iteration
            quality_gate: Quality gate instance
            retry_config: Configuration for retry management
            agent_overrides: Command-line agent overrides
        """
        self.project_root = Path(project_root)
        self.context_filter = ContextFilter(project_root)
        self.state_manager = StateManager(project_root)
        self.prd_generator = PRDGenerator(project_root)

        # Initialize agent executor
        self.agent_executor = AgentExecutor(
            project_root,
            agents_config=agents_config
        )
        if default_agent:
            self.agent_executor.default_agent = default_agent

        # Track running agents
        self.running_agents: Dict[str, StoryAgent] = {}
        self.completed_stories: set = set()
        self.failed_stories: set = set()

        # Store agent overrides
        self.agent_overrides = agent_overrides

        # Initialize quality gate
        if quality_gate:
            self.quality_gate = quality_gate
        elif QualityGate:
            # Try to create from PRD
            prd = self.state_manager.read_prd()
            if prd and prd.get("quality_gates", {}).get("enabled"):
                self.quality_gate = QualityGate.from_prd(project_root, prd)
            else:
                self.quality_gate = None
        else:
            self.quality_gate = None

        # Initialize retry manager
        if RetryManager:
            self.retry_manager = RetryManager(
                project_root,
                config=retry_config
            )
        else:
            self.retry_manager = None

        # Initialize iteration loop (created on demand in start_auto_run)
        self.iteration_config = iteration_config
        self.iteration_loop: Optional["IterationLoop"] = None

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

    def launch_agent(
        self,
        story: Dict,
        batch_num: int,
        agent_name: Optional[str] = None,
        task_callback: Optional[Callable] = None
    ) -> StoryAgent:
        """
        Launch a background agent for a story.

        Supports both Task tool agents (claude-code) and CLI agents.

        Args:
            story: Story dictionary
            batch_num: Current batch number
            agent_name: Optional specific agent to use
            task_callback: Callback for Task tool execution

        Returns:
            StoryAgent instance
        """
        story_id = story["id"]

        # Get context for this story
        context = self.context_filter.get_context_for_story(story_id)

        # Create agent wrapper
        agent = StoryAgent(story_id, story, context)
        agent.status = "running"

        # Generate output file path
        output_dir = self.project_root / ".agent-outputs"
        output_dir.mkdir(exist_ok=True)
        agent.output_file = output_dir / f"{story_id}.log"

        # Get PRD metadata for agent defaults
        prd = self.state_manager.read_prd()
        prd_metadata = prd.get("metadata", {}) if prd else {}

        # Execute via AgentExecutor
        result = self.agent_executor.execute_story(
            story=story,
            context=context,
            agent_name=agent_name,
            prd_metadata=prd_metadata,
            task_callback=task_callback
        )

        # Update agent with execution result
        if result.get("success"):
            agent.task_id = result.get("pid") or result.get("task_id")
            if result.get("output_file"):
                agent.output_file = Path(result["output_file"])
        else:
            agent.status = "failed"

        self.running_agents[story_id] = agent
        return agent

    def launch_agent_for_story(
        self,
        story_id: str,
        agent_name: Optional[str] = None,
        task_callback: Optional[Callable] = None
    ) -> Dict[str, Any]:
        """
        Launch an agent for a specific story by ID.

        Args:
            story_id: Story ID to execute
            agent_name: Optional specific agent to use
            task_callback: Callback for Task tool execution

        Returns:
            Execution result dict
        """
        # Get story from PRD
        prd = self.state_manager.read_prd()
        if not prd:
            return {"success": False, "error": "No PRD found"}

        story = None
        for s in prd.get("stories", []):
            if s.get("id") == story_id:
                story = s
                break

        if not story:
            return {"success": False, "error": f"Story {story_id} not found"}

        # Get context
        context = self.context_filter.get_context_for_story(story_id)

        # Execute
        return self.agent_executor.execute_story(
            story=story,
            context=context,
            agent_name=agent_name,
            prd_metadata=prd.get("metadata", {}),
            task_callback=task_callback
        )

    def execute_batch(
        self,
        batch: List[Dict],
        batch_num: int,
        dry_run: bool = False,
        agent_name: Optional[str] = None,
        task_callback: Optional[Callable] = None
    ) -> bool:
        """
        Execute a batch of stories in parallel.

        Args:
            batch: List of stories in this batch
            batch_num: Batch number
            dry_run: If True, only print what would be done
            agent_name: Optional agent to use for all stories
            task_callback: Callback for Task tool execution

        Returns:
            True if all stories succeeded, False otherwise
        """
        print(f"\n{'='*60}")
        print(f"Executing Batch {batch_num}")
        print(f"{'='*60}")

        if dry_run:
            print("\nStories in this batch:")
            for story in batch:
                story_agent = story.get("agent", agent_name or self.agent_executor.default_agent)
                print(f"  - {story['id']}: {story['title']} [agent: {story_agent}]")
            return True

        # Launch all agents in parallel
        agents = []
        results = []
        for story in batch:
            # Determine agent for this story
            story_agent_name = story.get("agent") or agent_name

            agent = self.launch_agent(
                story,
                batch_num,
                agent_name=story_agent_name,
                task_callback=task_callback
            )
            agents.append(agent)

            # Get resolved agent name from executor
            resolved_name, _ = self.agent_executor._resolve_agent(
                agent_name=story_agent_name,
                story_agent=story.get("agent")
            )
            print(f"Launched agent for {story['id']}: {story['title']} [via {resolved_name}]")

        # In real execution, agents run in background
        print("\nAgents launched. Monitor progress with:")
        for agent in agents:
            if agent.output_file:
                print(f"  tail -f {agent.output_file}")

        print("\nCheck status with: /hybrid:status or /agent-status")

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

    def print_status(self, show_agents: bool = True):
        """
        Print current execution status.

        Args:
            show_agents: Include agent information in output
        """
        batches = self.analyze_dependencies()

        print("\n" + "=" * 60)
        print("EXECUTION STATUS")
        print("=" * 60)

        # Show agent summary if enabled
        if show_agents:
            agent_summary = self.state_manager.get_agent_summary()
            print(f"\nAgents: {agent_summary['running']} running, "
                  f"{agent_summary['completed']} completed, "
                  f"{agent_summary['failed']} failed")

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

                # Get agent info if available
                agent_info = ""
                if show_agents:
                    agent_entry = self.state_manager.get_agent_for_story(story_id)
                    if agent_entry:
                        agent_info = f" [via {agent_entry.get('agent', 'unknown')}]"

                print(f"  {status_symbol} {story_id}: {title} [{status}]{agent_info}")

        print("\n" + "=" * 60)

    def get_agent_status(self) -> Dict[str, Any]:
        """
        Get current agent execution status.

        Returns:
            Dict with running, completed, failed agents
        """
        return self.agent_executor.get_agent_status()

    def get_available_agents(self) -> Dict[str, Dict]:
        """
        Get all configured agents with availability status.

        Returns:
            Dict mapping agent names to their config with 'available' flag
        """
        return self.agent_executor.get_available_agents()

    def stop_agent(self, story_id: str) -> Dict[str, Any]:
        """
        Stop a running CLI agent.

        Args:
            story_id: Story ID of the agent to stop

        Returns:
            Result dict with success status
        """
        return self.agent_executor.stop_agent(story_id)

    def set_default_agent(self, agent_name: str) -> bool:
        """
        Set the default agent for execution.

        Args:
            agent_name: Agent name to set as default

        Returns:
            True if agent exists and was set
        """
        if agent_name in self.agent_executor.agents:
            self.agent_executor.default_agent = agent_name
            return True
        return False

    # ========== Auto-Run / Iteration Loop Methods ==========

    def start_auto_run(
        self,
        mode: Optional["IterationMode"] = None,
        max_iterations: int = 50,
        callbacks: Optional["IterationCallbacks"] = None,
        dry_run: bool = False
    ) -> Optional["IterationState"]:
        """
        Start automatic iteration through all batches.

        Args:
            mode: Iteration mode (until_complete, max_iterations, batch_complete)
            max_iterations: Maximum iterations (for max_iterations mode)
            callbacks: Optional callbacks for iteration events
            dry_run: If True, don't actually execute

        Returns:
            IterationState on completion, None if not available
        """
        if not IterationLoop:
            print("[Error] IterationLoop not available")
            return None

        # Create iteration config
        config = self.iteration_config or IterationConfig()
        if mode:
            config.mode = mode
        config.max_iterations = max_iterations

        # Load quality gates from PRD if enabled
        prd = self.state_manager.read_prd()
        if prd:
            prd_iteration = prd.get("iteration_config", {})
            if prd_iteration:
                config = IterationConfig.from_dict(prd_iteration)

        # Create iteration loop
        self.iteration_loop = IterationLoop(
            project_root=self.project_root,
            config=config,
            orchestrator=self,
            quality_gate=self.quality_gate,
            retry_manager=self.retry_manager
        )

        # Start iteration
        try:
            state = self.iteration_loop.start(callbacks=callbacks, dry_run=dry_run)
            return state
        except Exception as e:
            print(f"[Error] Auto-run failed: {e}")
            return None

    def pause_auto_run(self, reason: Optional[str] = None) -> None:
        """Pause the automatic iteration loop."""
        if self.iteration_loop:
            self.iteration_loop.pause(reason)

    def resume_auto_run(self) -> Optional["IterationState"]:
        """Resume a paused iteration loop."""
        if self.iteration_loop:
            return self.iteration_loop.resume()
        return None

    def stop_auto_run(self) -> None:
        """Stop the automatic iteration loop."""
        if self.iteration_loop:
            self.iteration_loop.stop()

    def get_iteration_state(self) -> Optional[Dict[str, Any]]:
        """Get the current iteration state."""
        if self.iteration_loop:
            return self.iteration_loop.get_progress_summary()
        # Try to load from state file
        return self.state_manager.get_iteration_progress()

    # ========== Quality Gate Methods ==========

    def execute_batch_with_quality_gates(
        self,
        batch: List[Dict],
        batch_num: int,
        agent_name: Optional[str] = None,
        task_callback: Optional[Callable] = None
    ) -> Dict[str, Any]:
        """
        Execute a batch with quality gate verification after each story.

        Args:
            batch: List of stories in this batch
            batch_num: Batch number
            agent_name: Optional agent to use for all stories
            task_callback: Callback for Task tool execution

        Returns:
            Dict with execution results including quality gate outcomes
        """
        results = {
            "batch_num": batch_num,
            "stories_launched": 0,
            "stories_completed": 0,
            "stories_failed": 0,
            "quality_gate_failures": 0,
            "story_results": {}
        }

        # Execute batch
        self.execute_batch(batch, batch_num, agent_name=agent_name, task_callback=task_callback)
        results["stories_launched"] = len(batch)

        # Wait for completion and run quality gates
        if self.quality_gate:
            for story in batch:
                story_id = story["id"]

                # Wait for story to complete (simple polling)
                max_wait = 3600  # 1 hour
                poll_interval = 10
                elapsed = 0

                while elapsed < max_wait:
                    status = self.get_story_status(story_id)
                    if status in ["complete", "failed"]:
                        break
                    time.sleep(poll_interval)
                    elapsed += poll_interval

                if status == "complete":
                    # Run quality gates
                    gate_results = self.quality_gate.execute_all(story_id, {"story": story})
                    results["story_results"][story_id] = {
                        "status": "complete",
                        "quality_gates": {
                            name: output.passed
                            for name, output in gate_results.items()
                        }
                    }

                    if not self.quality_gate.should_allow_progression(gate_results):
                        results["quality_gate_failures"] += 1

                        # Handle failure with retry if enabled
                        if self.retry_manager:
                            self.handle_story_failure(
                                story_id=story_id,
                                agent_name=agent_name or self.agent_executor.default_agent,
                                error_type="quality_gate",
                                error_message=self.quality_gate.get_failure_summary(gate_results) or "Quality gate failed",
                                quality_gate_results=gate_results
                            )
                    else:
                        results["stories_completed"] += 1
                else:
                    results["stories_failed"] += 1
                    results["story_results"][story_id] = {"status": "failed"}

        return results

    def run_quality_gates(self, story_id: str) -> Optional[Dict[str, Any]]:
        """
        Run quality gates for a specific story.

        Args:
            story_id: Story ID to verify

        Returns:
            Quality gate results or None if not configured
        """
        if not self.quality_gate:
            return None

        # Get story
        prd = self.state_manager.read_prd()
        story = None
        if prd:
            for s in prd.get("stories", []):
                if s.get("id") == story_id:
                    story = s
                    break

        results = self.quality_gate.execute_all(story_id, {"story": story or {}})
        return {
            name: {
                "passed": output.passed,
                "exit_code": output.exit_code,
                "duration": output.duration_seconds,
                "error_summary": output.error_summary
            }
            for name, output in results.items()
        }

    # ========== Retry Management Methods ==========

    def handle_story_failure(
        self,
        story_id: str,
        agent_name: str,
        error_type: str,
        error_message: str,
        quality_gate_results: Optional[Dict] = None,
        exit_code: Optional[int] = None,
        output_excerpt: Optional[str] = None
    ) -> bool:
        """
        Handle a story failure with potential retry.

        Args:
            story_id: ID of the failed story
            agent_name: Agent that failed
            error_type: Type of error ("timeout", "exit_code", "quality_gate")
            error_message: Error message
            quality_gate_results: Quality gate results if applicable
            exit_code: Process exit code if applicable
            output_excerpt: Output excerpt if applicable

        Returns:
            True if retry was initiated, False otherwise
        """
        if not self.retry_manager or not ErrorType:
            return False

        # Map error type
        error_type_enum = {
            "timeout": ErrorType.TIMEOUT,
            "exit_code": ErrorType.EXIT_CODE,
            "quality_gate": ErrorType.QUALITY_GATE,
            "process_crash": ErrorType.PROCESS_CRASH,
        }.get(error_type, ErrorType.UNKNOWN)

        # Convert quality gate results for storage
        qg_results = None
        if quality_gate_results:
            qg_results = {
                name: {"passed": result.passed if hasattr(result, "passed") else result.get("passed", False)}
                for name, result in quality_gate_results.items()
            }

        # Record failure
        self.retry_manager.record_failure(
            story_id=story_id,
            agent=agent_name,
            error_type=error_type_enum,
            error_message=error_message,
            quality_gate_results=qg_results,
            exit_code=exit_code,
            output_excerpt=output_excerpt
        )

        # Check if we can retry
        if not self.retry_manager.can_retry(story_id):
            return False

        # Get retry agent (may switch agents)
        retry_agent = self.retry_manager.get_retry_agent(story_id, agent_name)

        # Log retry
        retry_count = self.retry_manager.get_retry_count(story_id)
        self.state_manager.append_progress(
            f"[RETRY] Attempt {retry_count} with {retry_agent}",
            story_id=story_id
        )

        # Initiate retry (will be picked up by iteration loop or manual trigger)
        return True

    def get_retry_summary(self) -> Dict[str, Any]:
        """Get summary of all retry states."""
        if not self.retry_manager:
            return {"enabled": False}

        return {
            "enabled": True,
            **self.retry_manager.get_all_states()
        }

    # ========== Agent Override Methods ==========

    def set_agent_overrides(self, overrides: "AgentOverrides") -> None:
        """Set agent overrides for current session."""
        self.agent_overrides = overrides
        if self.agent_executor and hasattr(self.agent_executor, "phase_manager"):
            # Overrides will be passed to execute_story
            pass

    def get_resolution_chain(self, story_id: str) -> Optional[List[Dict[str, Any]]]:
        """
        Get the agent resolution chain for a story.

        Args:
            story_id: Story ID

        Returns:
            List of resolution steps or None
        """
        if not self.agent_executor.phase_manager or not ExecutionPhase:
            return None

        # Get story
        prd = self.state_manager.read_prd()
        story = None
        if prd:
            for s in prd.get("stories", []):
                if s.get("id") == story_id:
                    story = s
                    break

        if not story:
            return None

        return self.agent_executor.phase_manager.get_resolution_chain(
            story=story,
            phase=ExecutionPhase.IMPLEMENTATION,
            override=self.agent_overrides
        )


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
