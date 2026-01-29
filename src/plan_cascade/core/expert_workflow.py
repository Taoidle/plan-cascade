"""
Expert Mode Workflow for Plan Cascade

Provides interactive PRD editing, strategy selection, and agent assignment.
Gives users full control over the execution process.
"""

import asyncio
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

from .strategy_analyzer import (
    ExecutionStrategy,
    StrategyAnalyzer,
    StrategyDecision,
    override_strategy,
)

if TYPE_CHECKING:
    from ..backends.base import AgentBackend, ExecutionResult


@dataclass
class PRD:
    """Product Requirements Document."""
    metadata: Dict[str, Any]
    goal: str
    objectives: List[str]
    stories: List[Dict[str, Any]]

    def __post_init__(self):
        if self.metadata is None:
            self.metadata = {}
        if self.objectives is None:
            self.objectives = []
        if self.stories is None:
            self.stories = []

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "PRD":
        """Create from dictionary."""
        return cls(
            metadata=data.get("metadata", {}),
            goal=data.get("goal", ""),
            objectives=data.get("objectives", []),
            stories=data.get("stories", []),
        )

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "metadata": self.metadata,
            "goal": self.goal,
            "objectives": self.objectives,
            "stories": self.stories,
        }

    def get_story(self, story_id: str) -> Optional[Dict[str, Any]]:
        """Get a story by ID."""
        for story in self.stories:
            if story.get("id") == story_id:
                return story
        return None

    def update_story(self, story_id: str, updates: Dict[str, Any]) -> bool:
        """Update a story by ID."""
        for story in self.stories:
            if story.get("id") == story_id:
                story.update(updates)
                return True
        return False

    def add_story(self, story: Dict[str, Any]) -> None:
        """Add a new story."""
        self.stories.append(story)

    def remove_story(self, story_id: str) -> bool:
        """Remove a story by ID."""
        for i, story in enumerate(self.stories):
            if story.get("id") == story_id:
                self.stories.pop(i)
                return True
        return False

    def reorder_stories(self, story_ids: List[str]) -> None:
        """Reorder stories based on ID list."""
        story_map = {s["id"]: s for s in self.stories}
        self.stories = [story_map[sid] for sid in story_ids if sid in story_map]

    def get_dependency_graph(self) -> Dict[str, List[str]]:
        """Get dependency graph as adjacency list."""
        return {
            story["id"]: story.get("dependencies", [])
            for story in self.stories
        }

    def validate(self) -> List[str]:
        """Validate the PRD and return list of errors."""
        errors = []

        if not self.goal:
            errors.append("Missing goal")

        story_ids = set()
        for i, story in enumerate(self.stories):
            story_id = story.get("id")
            if not story_id:
                errors.append(f"Story {i}: Missing ID")
            elif story_id in story_ids:
                errors.append(f"Duplicate story ID: {story_id}")
            else:
                story_ids.add(story_id)

            if not story.get("title"):
                errors.append(f"Story {story_id or i}: Missing title")

            if not story.get("description"):
                errors.append(f"Story {story_id or i}: Missing description")

            # Validate dependencies
            for dep in story.get("dependencies", []):
                if dep not in story_ids:
                    errors.append(f"Story {story_id}: Unknown dependency '{dep}'")

        return errors


@dataclass
class ExpertWorkflowState:
    """State for expert workflow."""
    description: str
    prd: Optional[PRD] = None
    strategy_decision: Optional[StrategyDecision] = None
    selected_strategy: Optional[ExecutionStrategy] = None
    agent_assignments: Dict[str, str] = field(default_factory=dict)
    execution_started: bool = False
    execution_completed: bool = False
    completed_stories: List[str] = field(default_factory=list)
    failed_stories: List[str] = field(default_factory=list)


class ExpertWorkflow:
    """
    Expert mode workflow with full user control.

    Provides:
    - Interactive PRD generation and editing
    - Strategy selection and override
    - Agent assignment per story
    - Dependency visualization
    - Step-by-step execution control
    """

    def __init__(
        self,
        backend: "AgentBackend",
        project_path: Optional[Path] = None,
        available_agents: Optional[List[str]] = None
    ):
        """
        Initialize the expert workflow.

        Args:
            backend: Backend for execution
            project_path: Project root path
            available_agents: List of available agent names
        """
        self.backend = backend
        self.project_path = Path(project_path) if project_path else Path.cwd()
        self.available_agents = available_agents or ["claude-code", "builtin"]
        self.default_agent = "claude-code"
        self.strategy_analyzer = StrategyAnalyzer(
            llm=backend.get_llm() if hasattr(backend, 'get_llm') else None,
            fallback_to_heuristic=True
        )
        self.state: Optional[ExpertWorkflowState] = None

    async def start(self, description: str) -> ExpertWorkflowState:
        """
        Start the expert workflow with a task description.

        Args:
            description: Task description

        Returns:
            Initial workflow state
        """
        self.state = ExpertWorkflowState(description=description)
        return self.state

    async def analyze_strategy(self) -> StrategyDecision:
        """
        Analyze and suggest a strategy.

        Returns:
            Strategy decision
        """
        if not self.state:
            raise ValueError("Workflow not started")

        decision = await self.strategy_analyzer.analyze(
            description=self.state.description,
            project_path=self.project_path
        )

        self.state.strategy_decision = decision
        self.state.selected_strategy = decision.strategy

        return decision

    def select_strategy(
        self,
        strategy: ExecutionStrategy,
        reason: str = "User selection"
    ) -> StrategyDecision:
        """
        Override the strategy selection.

        Args:
            strategy: Selected strategy
            reason: Reason for selection

        Returns:
            Updated strategy decision
        """
        if not self.state:
            raise ValueError("Workflow not started")

        if self.state.strategy_decision:
            self.state.strategy_decision = override_strategy(
                self.state.strategy_decision,
                strategy,
                reason
            )
        else:
            self.state.strategy_decision = StrategyDecision(
                strategy=strategy,
                use_worktree=False,
                estimated_stories=1,
                confidence=1.0,
                reasoning=reason,
            )

        self.state.selected_strategy = strategy
        return self.state.strategy_decision

    async def generate_prd(self) -> PRD:
        """
        Generate a PRD based on the description and strategy.

        Returns:
            Generated PRD
        """
        if not self.state:
            raise ValueError("Workflow not started")

        llm = self.backend.get_llm()

        strategy = self.state.selected_strategy or ExecutionStrategy.HYBRID_AUTO

        prompt = f"""Generate a detailed PRD for the following task.

## Task Description
{self.state.description}

## Strategy
Using {strategy.value} strategy.

## Requirements
Create a comprehensive JSON PRD:
```json
{{
    "metadata": {{
        "version": "1.0.0",
        "description": "<brief description>",
        "strategy": "{strategy.value}"
    }},
    "goal": "<main goal of this task>",
    "objectives": ["<objective 1>", "<objective 2>"],
    "stories": [
        {{
            "id": "story-001",
            "title": "<clear, concise title>",
            "description": "<detailed description of what needs to be done>",
            "priority": "high" | "medium" | "low",
            "dependencies": [],
            "status": "pending",
            "acceptance_criteria": [
                "<specific, measurable criterion 1>",
                "<specific, measurable criterion 2>"
            ],
            "estimated_hours": 1.0,
            "tags": ["<tag1>", "<tag2>"]
        }}
    ]
}}
```

Guidelines:
- Create well-structured stories with clear scope
- Include specific acceptance criteria for each story
- Set realistic priorities and dependencies
- Estimate hours for each story
- Add relevant tags for categorization

Return ONLY the JSON, no additional text."""

        response = await llm.complete([{"role": "user", "content": prompt}])

        # Parse JSON from response
        import re

        json_match = re.search(r'\{[\s\S]*\}', response.content)
        if not json_match:
            raise ValueError("No JSON found in LLM response")

        prd_data = json.loads(json_match.group())
        self.state.prd = PRD.from_dict(prd_data)

        return self.state.prd

    def load_prd(self, prd_path: Path) -> PRD:
        """
        Load an existing PRD from file.

        Args:
            prd_path: Path to PRD JSON file

        Returns:
            Loaded PRD
        """
        if not self.state:
            raise ValueError("Workflow not started")

        with open(prd_path, "r", encoding="utf-8") as f:
            prd_data = json.load(f)

        self.state.prd = PRD.from_dict(prd_data)
        return self.state.prd

    def save_prd(self, prd_path: Optional[Path] = None) -> Path:
        """
        Save the current PRD to file.

        Args:
            prd_path: Path to save (default: project_path/prd.json)

        Returns:
            Path where PRD was saved
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD to save")

        if prd_path is None:
            prd_path = self.project_path / "prd.json"

        with open(prd_path, "w", encoding="utf-8") as f:
            json.dump(self.state.prd.to_dict(), f, indent=2)

        return prd_path

    def edit_story(self, story_id: str, updates: Dict[str, Any]) -> bool:
        """
        Edit a story in the PRD.

        Args:
            story_id: ID of story to edit
            updates: Dictionary of updates

        Returns:
            True if story was found and updated
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD loaded")

        return self.state.prd.update_story(story_id, updates)

    def add_story(self, story: Dict[str, Any]) -> None:
        """
        Add a new story to the PRD.

        Args:
            story: Story dictionary
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD loaded")

        # Ensure story has required fields
        if "id" not in story:
            existing_ids = [s["id"] for s in self.state.prd.stories]
            max_num = 0
            for sid in existing_ids:
                try:
                    num = int(sid.split("-")[-1])
                    max_num = max(max_num, num)
                except (ValueError, IndexError):
                    pass
            story["id"] = f"story-{max_num + 1:03d}"

        if "status" not in story:
            story["status"] = "pending"

        self.state.prd.add_story(story)

    def remove_story(self, story_id: str) -> bool:
        """
        Remove a story from the PRD.

        Args:
            story_id: ID of story to remove

        Returns:
            True if story was found and removed
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD loaded")

        # Also update dependencies in other stories
        for story in self.state.prd.stories:
            deps = story.get("dependencies", [])
            if story_id in deps:
                deps.remove(story_id)

        return self.state.prd.remove_story(story_id)

    def reorder_stories(self, story_ids: List[str]) -> None:
        """
        Reorder stories in the PRD.

        Args:
            story_ids: List of story IDs in desired order
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD loaded")

        self.state.prd.reorder_stories(story_ids)

    def assign_agent(self, story_id: str, agent: str) -> None:
        """
        Assign an agent to a story.

        Args:
            story_id: Story ID
            agent: Agent name
        """
        if not self.state:
            raise ValueError("Workflow not started")

        if agent not in self.available_agents:
            raise ValueError(f"Unknown agent: {agent}. Available: {self.available_agents}")

        self.state.agent_assignments[story_id] = agent

    def get_agent_for_story(self, story_id: str) -> str:
        """
        Get the agent assigned to a story.

        Args:
            story_id: Story ID

        Returns:
            Agent name
        """
        if not self.state:
            raise ValueError("Workflow not started")

        return self.state.agent_assignments.get(story_id, self.default_agent)

    def get_dependency_graph_ascii(self) -> str:
        """
        Get ASCII representation of dependency graph.

        Returns:
            ASCII graph string
        """
        if not self.state or not self.state.prd:
            return "No PRD loaded"

        lines = ["Dependency Graph:", ""]

        # Build adjacency list
        graph = self.state.prd.get_dependency_graph()
        story_titles = {s["id"]: s["title"] for s in self.state.prd.stories}

        # Find root stories (no dependencies)
        roots = [sid for sid, deps in graph.items() if not deps]

        # Build reverse graph (dependents)
        dependents: Dict[str, List[str]] = {sid: [] for sid in graph}
        for sid, deps in graph.items():
            for dep in deps:
                if dep in dependents:
                    dependents[dep].append(sid)

        # DFS to print tree
        visited = set()

        def print_tree(story_id: str, prefix: str = "", is_last: bool = True):
            if story_id in visited:
                return

            connector = "\\-- " if is_last else "|-- "
            title = story_titles.get(story_id, story_id)
            status = ""
            if story_id in self.state.completed_stories:
                status = " [DONE]"
            elif story_id in self.state.failed_stories:
                status = " [FAILED]"

            lines.append(f"{prefix}{connector}{story_id}: {title}{status}")
            visited.add(story_id)

            children = dependents.get(story_id, [])
            for i, child in enumerate(children):
                child_prefix = prefix + ("    " if is_last else "|   ")
                print_tree(child, child_prefix, i == len(children) - 1)

        for i, root in enumerate(roots):
            print_tree(root, "", i == len(roots) - 1)

        # Show any not reached (cycles or orphans)
        orphans = [sid for sid in graph if sid not in visited]
        if orphans:
            lines.append("")
            lines.append("Orphaned/Cyclic stories:")
            for sid in orphans:
                title = story_titles.get(sid, sid)
                lines.append(f"  - {sid}: {title}")

        return "\n".join(lines)

    def validate_prd(self) -> List[str]:
        """
        Validate the current PRD.

        Returns:
            List of validation errors
        """
        if not self.state or not self.state.prd:
            return ["No PRD loaded"]

        return self.state.prd.validate()

    def get_execution_batches(self) -> List[List[Dict[str, Any]]]:
        """
        Get execution batches based on dependencies.

        Returns:
            List of batches
        """
        if not self.state or not self.state.prd:
            return []

        stories = self.state.prd.stories
        completed = set()
        batches = []

        while len(completed) < len(stories):
            batch = []

            for story in stories:
                story_id = story["id"]
                if story_id in completed:
                    continue

                deps = story.get("dependencies", [])
                if all(dep in completed for dep in deps):
                    batch.append(story)

            if not batch:
                # Handle cycles by adding remaining
                batch = [s for s in stories if s["id"] not in completed]

            batches.append(batch)
            completed.update(s["id"] for s in batch)

        return batches

    async def execute_story(
        self,
        story_id: str,
        context: str = ""
    ) -> "ExecutionResult":
        """
        Execute a single story.

        Args:
            story_id: Story ID to execute
            context: Additional context

        Returns:
            ExecutionResult
        """
        if not self.state or not self.state.prd:
            raise ValueError("No PRD loaded")

        story = self.state.prd.get_story(story_id)
        if not story:
            raise ValueError(f"Story not found: {story_id}")

        # Check dependencies
        deps = story.get("dependencies", [])
        unmet_deps = [d for d in deps if d not in self.state.completed_stories]
        if unmet_deps:
            raise ValueError(f"Unmet dependencies: {unmet_deps}")

        # Update status
        story["status"] = "in_progress"

        # Execute
        result = await self.backend.execute(story, context)

        # Update status based on result
        if result.success:
            story["status"] = "complete"
            self.state.completed_stories.append(story_id)
        else:
            story["status"] = "failed"
            self.state.failed_stories.append(story_id)

        return result

    async def execute_batch(
        self,
        batch: List[Dict[str, Any]],
        context: str = "",
        parallel: bool = False
    ) -> List["ExecutionResult"]:
        """
        Execute a batch of stories.

        Args:
            batch: List of stories to execute
            context: Additional context
            parallel: Execute in parallel (not yet supported)

        Returns:
            List of results
        """
        results = []

        for story in batch:
            result = await self.execute_story(story["id"], context)
            results.append(result)

        return results

    async def execute_all(
        self,
        context: str = ""
    ) -> Dict[str, Any]:
        """
        Execute all stories in dependency order.

        Args:
            context: Additional context

        Returns:
            Summary of execution
        """
        if not self.state:
            raise ValueError("Workflow not started")

        self.state.execution_started = True

        batches = self.get_execution_batches()
        all_results = []

        for batch in batches:
            results = await self.execute_batch(batch, context)
            all_results.extend(results)

        self.state.execution_completed = True

        return {
            "total_stories": len(all_results),
            "completed": len(self.state.completed_stories),
            "failed": len(self.state.failed_stories),
            "success": len(self.state.failed_stories) == 0,
        }

    def get_status(self) -> Dict[str, Any]:
        """
        Get current workflow status.

        Returns:
            Status dictionary
        """
        if not self.state:
            return {"status": "not_started"}

        return {
            "status": "running" if self.state.execution_started and not self.state.execution_completed else "ready",
            "has_prd": self.state.prd is not None,
            "strategy": self.state.selected_strategy.value if self.state.selected_strategy else None,
            "total_stories": len(self.state.prd.stories) if self.state.prd else 0,
            "completed_stories": len(self.state.completed_stories),
            "failed_stories": len(self.state.failed_stories),
            "agent_assignments": self.state.agent_assignments,
        }


class ExpertWorkflowInteractive:
    """
    Interactive wrapper for expert workflow.

    Provides menu-driven interaction for CLI usage.
    """

    def __init__(self, workflow: ExpertWorkflow):
        """
        Initialize interactive wrapper.

        Args:
            workflow: ExpertWorkflow instance
        """
        self.workflow = workflow

    def get_menu_options(self) -> List[str]:
        """
        Get available menu options based on current state.

        Returns:
            List of option strings
        """
        options = []

        if not self.workflow.state:
            return ["start"]

        if not self.workflow.state.prd:
            options.extend(["generate", "load"])
        else:
            options.extend(["view", "edit", "agent", "validate", "graph"])

            if not self.workflow.state.execution_started:
                options.extend(["run", "save"])

        options.append("quit")
        return options

    def get_menu_descriptions(self) -> Dict[str, str]:
        """
        Get descriptions for menu options.

        Returns:
            Dictionary of option -> description
        """
        return {
            "start": "Start workflow with task description",
            "generate": "Generate PRD from description",
            "load": "Load existing PRD file",
            "view": "View current PRD",
            "edit": "Edit PRD stories",
            "agent": "Assign agents to stories",
            "validate": "Validate PRD",
            "graph": "Show dependency graph",
            "run": "Execute all stories",
            "save": "Save PRD to file",
            "quit": "Exit workflow",
        }
