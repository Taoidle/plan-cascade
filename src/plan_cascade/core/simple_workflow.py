"""
Simple Mode Workflow for Plan Cascade

Provides one-click execution with automatic strategy selection and PRD generation.
User input description -> AI analysis -> Auto strategy -> Auto execution -> Done
"""

import asyncio
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

from .strategy_analyzer import (
    ExecutionStrategy,
    StrategyAnalyzer,
    StrategyDecision,
)

if TYPE_CHECKING:
    from ..backends.base import AgentBackend, ExecutionResult


@dataclass
class WorkflowResult:
    """Result from workflow execution."""
    success: bool
    strategy: ExecutionStrategy
    output: str = ""
    error: Optional[str] = None
    stories_completed: int = 0
    stories_total: int = 0
    iterations: int = 0
    duration_seconds: float = 0.0
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "success": self.success,
            "strategy": self.strategy.value,
            "output": self.output,
            "error": self.error,
            "stories_completed": self.stories_completed,
            "stories_total": self.stories_total,
            "iterations": self.iterations,
            "duration_seconds": self.duration_seconds,
            "metadata": self.metadata,
        }


@dataclass
class ProgressEvent:
    """Progress event for UI updates."""
    type: str  # strategy_decided, story_started, story_completed, story_failed, etc.
    data: Dict[str, Any] = field(default_factory=dict)

    @classmethod
    def strategy_decided(cls, decision: StrategyDecision) -> "ProgressEvent":
        return cls(type="strategy_decided", data=decision.to_dict())

    @classmethod
    def story_started(cls, story_id: str, title: str) -> "ProgressEvent":
        return cls(type="story_started", data={"story_id": story_id, "title": title})

    @classmethod
    def story_completed(cls, story_id: str, title: str) -> "ProgressEvent":
        return cls(type="story_completed", data={"story_id": story_id, "title": title})

    @classmethod
    def story_failed(cls, story_id: str, title: str, error: str) -> "ProgressEvent":
        return cls(type="story_failed", data={"story_id": story_id, "title": title, "error": error})

    @classmethod
    def batch_started(cls, batch_num: int, total: int, stories: List[str]) -> "ProgressEvent":
        return cls(type="batch_started", data={"batch": batch_num, "total": total, "stories": stories})

    @classmethod
    def execution_started(cls, strategy: str) -> "ProgressEvent":
        return cls(type="execution_started", data={"strategy": strategy})

    @classmethod
    def execution_completed(cls, success: bool) -> "ProgressEvent":
        return cls(type="execution_completed", data={"success": success})


# Type alias for progress callback
ProgressCallback = Callable[[ProgressEvent], Any]


class SimpleWorkflow:
    """
    Simple mode workflow for one-click execution.

    Analyzes the task, selects strategy automatically, generates PRD if needed,
    and executes all stories with dependency resolution.
    """

    def __init__(
        self,
        backend: "AgentBackend",
        project_path: Optional[Path] = None,
        on_progress: Optional[ProgressCallback] = None
    ):
        """
        Initialize the simple workflow.

        Args:
            backend: Backend for execution
            project_path: Project root path
            on_progress: Callback for progress updates
        """
        self.backend = backend
        self.project_path = Path(project_path) if project_path else Path.cwd()
        self.on_progress = on_progress
        self.strategy_analyzer = StrategyAnalyzer(
            llm=backend.get_llm() if hasattr(backend, 'get_llm') else None,
            fallback_to_heuristic=True
        )

    async def run(
        self,
        description: str,
        context: str = ""
    ) -> WorkflowResult:
        """
        Execute the workflow in simple mode.

        Args:
            description: Task description
            context: Additional context

        Returns:
            WorkflowResult with execution outcome
        """
        import time
        start_time = time.time()

        try:
            # 1. Analyze strategy
            decision = await self.strategy_analyzer.analyze(
                description=description,
                context=context,
                project_path=self.project_path
            )
            await self._emit_progress(ProgressEvent.strategy_decided(decision))

            # 2. Execute based on strategy
            await self._emit_progress(ProgressEvent.execution_started(decision.strategy.value))

            if decision.strategy == ExecutionStrategy.DIRECT:
                result = await self._execute_direct(description, context)
            elif decision.strategy == ExecutionStrategy.HYBRID_AUTO:
                result = await self._execute_hybrid(
                    description, context, decision.use_worktree
                )
            elif decision.strategy == ExecutionStrategy.MEGA_PLAN:
                result = await self._execute_mega(description, context)
            else:
                result = await self._execute_hybrid(description, context, False)

            # 3. Calculate duration
            duration = time.time() - start_time

            await self._emit_progress(ProgressEvent.execution_completed(result.success))

            return WorkflowResult(
                success=result.success,
                strategy=decision.strategy,
                output=result.output,
                error=result.error,
                stories_completed=result.stories_completed,
                stories_total=result.stories_total,
                iterations=result.iterations,
                duration_seconds=duration,
                metadata={
                    "decision": decision.to_dict(),
                }
            )

        except Exception as e:
            duration = time.time() - start_time
            return WorkflowResult(
                success=False,
                strategy=ExecutionStrategy.DIRECT,
                error=str(e),
                duration_seconds=duration,
            )

    async def _execute_direct(
        self,
        description: str,
        context: str
    ) -> "ExecutionResult":
        """
        Execute a simple task directly without PRD.

        Args:
            description: Task description
            context: Additional context

        Returns:
            ExecutionResult
        """
        # Create a simple story from the description
        story = {
            "id": "direct-001",
            "title": "Direct Execution",
            "description": description,
            "acceptance_criteria": [],
            "status": "pending",
        }

        await self._emit_progress(ProgressEvent.story_started(story["id"], story["title"]))

        result = await self.backend.execute(story, context)

        if result.success:
            await self._emit_progress(ProgressEvent.story_completed(story["id"], story["title"]))
        else:
            await self._emit_progress(ProgressEvent.story_failed(
                story["id"], story["title"], result.error or "Unknown error"
            ))

        # Convert to internal result format
        from dataclasses import dataclass

        @dataclass
        class InternalResult:
            success: bool
            output: str
            error: Optional[str]
            stories_completed: int
            stories_total: int
            iterations: int

        return InternalResult(
            success=result.success,
            output=result.output,
            error=result.error,
            stories_completed=1 if result.success else 0,
            stories_total=1,
            iterations=result.iterations,
        )

    async def _execute_hybrid(
        self,
        description: str,
        context: str,
        use_worktree: bool = False
    ) -> Any:
        """
        Execute using Hybrid mode with auto-generated PRD.

        Args:
            description: Task description
            context: Additional context
            use_worktree: Whether to use git worktree

        Returns:
            ExecutionResult-like object
        """
        from dataclasses import dataclass

        @dataclass
        class InternalResult:
            success: bool
            output: str
            error: Optional[str]
            stories_completed: int
            stories_total: int
            iterations: int

        # Setup worktree if needed
        if use_worktree:
            await self._setup_worktree()

        # Generate PRD using LLM
        try:
            prd = await self._generate_prd(description, context)
        except Exception as e:
            return InternalResult(
                success=False,
                output="",
                error=f"Failed to generate PRD: {e}",
                stories_completed=0,
                stories_total=0,
                iterations=0,
            )

        stories = prd.get("stories", [])
        if not stories:
            return InternalResult(
                success=False,
                output="",
                error="No stories generated in PRD",
                stories_completed=0,
                stories_total=0,
                iterations=0,
            )

        # Generate execution batches
        batches = self._generate_batches(stories)

        # Execute batches
        completed = 0
        total_iterations = 0
        outputs = []
        last_error = None

        for batch_idx, batch in enumerate(batches):
            story_titles = [s["title"] for s in batch]
            await self._emit_progress(ProgressEvent.batch_started(
                batch_idx + 1, len(batches), story_titles
            ))

            # Execute stories in batch (could be parallel in future)
            for story in batch:
                await self._emit_progress(ProgressEvent.story_started(
                    story["id"], story["title"]
                ))

                result = await self.backend.execute(story, context)
                total_iterations += result.iterations

                if result.success:
                    completed += 1
                    outputs.append(result.output)
                    await self._emit_progress(ProgressEvent.story_completed(
                        story["id"], story["title"]
                    ))
                else:
                    last_error = result.error
                    await self._emit_progress(ProgressEvent.story_failed(
                        story["id"], story["title"], result.error or "Unknown error"
                    ))
                    # Continue with other stories despite failure

        return InternalResult(
            success=completed == len(stories),
            output="\n\n".join(outputs),
            error=last_error if completed < len(stories) else None,
            stories_completed=completed,
            stories_total=len(stories),
            iterations=total_iterations,
        )

    async def _execute_mega(
        self,
        description: str,
        context: str
    ) -> Any:
        """
        Execute using Mega Plan mode for large projects.

        Args:
            description: Task description
            context: Additional context

        Returns:
            ExecutionResult-like object
        """
        from dataclasses import dataclass

        @dataclass
        class InternalResult:
            success: bool
            output: str
            error: Optional[str]
            stories_completed: int
            stories_total: int
            iterations: int

        # Generate mega plan
        try:
            mega_plan = await self._generate_mega_plan(description, context)
        except Exception as e:
            return InternalResult(
                success=False,
                output="",
                error=f"Failed to generate mega plan: {e}",
                stories_completed=0,
                stories_total=0,
                iterations=0,
            )

        features = mega_plan.get("features", [])
        if not features:
            return InternalResult(
                success=False,
                output="",
                error="No features generated in mega plan",
                stories_completed=0,
                stories_total=0,
                iterations=0,
            )

        # Execute each feature as a hybrid workflow
        completed_features = 0
        total_stories_completed = 0
        total_stories = 0
        total_iterations = 0
        outputs = []
        last_error = None

        completed_features_list = []

        for feature in features:
            feature_desc = feature.get("description", "")
            feature_context = context + f"\n\nCompleted features: {', '.join(completed_features_list)}"

            result = await self._execute_hybrid(
                description=feature_desc,
                context=feature_context,
                use_worktree=True
            )

            total_stories_completed += result.stories_completed
            total_stories += result.stories_total
            total_iterations += result.iterations

            if result.success:
                completed_features += 1
                completed_features_list.append(feature.get("name", ""))
                outputs.append(f"Feature '{feature.get('name')}' completed:\n{result.output}")
            else:
                last_error = result.error
                outputs.append(f"Feature '{feature.get('name')}' failed: {result.error}")

        return InternalResult(
            success=completed_features == len(features),
            output="\n\n---\n\n".join(outputs),
            error=last_error if completed_features < len(features) else None,
            stories_completed=total_stories_completed,
            stories_total=total_stories,
            iterations=total_iterations,
        )

    async def _generate_prd(
        self,
        description: str,
        context: str
    ) -> Dict[str, Any]:
        """
        Generate a PRD from the description using LLM.

        Args:
            description: Task description
            context: Additional context

        Returns:
            PRD dictionary
        """
        llm = self.backend.get_llm()

        prompt = f"""Generate a PRD (Product Requirements Document) for the following task.

## Task Description
{description}

## Context
{context or "No additional context."}

## Requirements
Create a JSON PRD with the following structure:
```json
{{
    "metadata": {{
        "version": "1.0.0",
        "description": "<brief description>"
    }},
    "goal": "<main goal>",
    "stories": [
        {{
            "id": "story-001",
            "title": "<story title>",
            "description": "<detailed description>",
            "priority": "high" | "medium" | "low",
            "dependencies": [],
            "status": "pending",
            "acceptance_criteria": ["<criterion 1>", "<criterion 2>"]
        }}
    ]
}}
```

Guidelines:
- Break the task into 2-6 user stories
- Order stories by dependency (foundational first)
- Include clear acceptance criteria
- Use high priority for blocking/critical stories

Return ONLY the JSON, no additional text."""

        response = await llm.complete([{"role": "user", "content": prompt}])

        # Parse JSON from response
        import json
        import re

        json_match = re.search(r'\{[\s\S]*\}', response.content)
        if not json_match:
            raise ValueError("No JSON found in LLM response")

        return json.loads(json_match.group())

    async def _generate_mega_plan(
        self,
        description: str,
        context: str
    ) -> Dict[str, Any]:
        """
        Generate a mega plan for large projects.

        Args:
            description: Project description
            context: Additional context

        Returns:
            Mega plan dictionary
        """
        llm = self.backend.get_llm()

        prompt = f"""Generate a Mega Plan for the following large project.

## Project Description
{description}

## Context
{context or "No additional context."}

## Requirements
Create a JSON mega plan breaking this into independent features:
```json
{{
    "metadata": {{
        "version": "1.0.0",
        "description": "<brief description>"
    }},
    "goal": "<main goal>",
    "features": [
        {{
            "id": "feature-001",
            "name": "<feature name>",
            "description": "<detailed feature description>",
            "priority": "high" | "medium" | "low",
            "dependencies": [],
            "estimated_stories": 3
        }}
    ]
}}
```

Guidelines:
- Break into 2-5 independent features
- Order by dependency
- Each feature should be self-contained
- Estimate 2-5 stories per feature

Return ONLY the JSON, no additional text."""

        response = await llm.complete([{"role": "user", "content": prompt}])

        # Parse JSON from response
        import json
        import re

        json_match = re.search(r'\{[\s\S]*\}', response.content)
        if not json_match:
            raise ValueError("No JSON found in LLM response")

        return json.loads(json_match.group())

    def _generate_batches(self, stories: List[Dict]) -> List[List[Dict]]:
        """
        Generate execution batches from stories based on dependencies.

        Args:
            stories: List of story dictionaries

        Returns:
            List of batches (each batch is a list of stories)
        """
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
                # Circular dependency - add remaining
                batch = [s for s in stories if s["id"] not in completed]

            batches.append(batch)
            completed.update(s["id"] for s in batch)

        return batches

    async def _setup_worktree(self) -> None:
        """Setup git worktree for isolated development."""
        # This would create a git worktree for the feature
        # For now, this is a placeholder
        pass

    async def _emit_progress(self, event: ProgressEvent) -> None:
        """
        Emit a progress event to the callback.

        Args:
            event: Progress event to emit
        """
        if self.on_progress:
            try:
                result = self.on_progress(event)
                if asyncio.iscoroutine(result):
                    await result
            except Exception:
                pass  # Don't let callback errors break workflow


async def run_simple_workflow(
    description: str,
    backend: "AgentBackend",
    project_path: Optional[Path] = None,
    on_progress: Optional[ProgressCallback] = None
) -> WorkflowResult:
    """
    Convenience function to run simple workflow.

    Args:
        description: Task description
        backend: Backend for execution
        project_path: Project root path
        on_progress: Progress callback

    Returns:
        WorkflowResult
    """
    workflow = SimpleWorkflow(
        backend=backend,
        project_path=project_path,
        on_progress=on_progress
    )
    return await workflow.run(description)
