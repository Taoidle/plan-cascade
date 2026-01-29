"""
Builtin Backend

Implementation of AgentBackend that uses LLM APIs directly with a ReAct loop.
Enables Plan Cascade to run independently without Claude Code.

Key features:
- Direct LLM API calls (Claude, OpenAI, Ollama, DeepSeek)
- ReAct (Reasoning + Acting) loop for autonomous task execution
- Tool registry integration for file/shell operations
- Configurable iteration limits and timeouts
"""

import asyncio
from pathlib import Path
from typing import TYPE_CHECKING, Any

from ..core.react_engine import ReActConfig, ReActEngine
from ..tools.registry import ToolRegistry
from .base import AgentBackend, ExecutionResult

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


class BuiltinBackend(AgentBackend):
    """
    Builtin Backend - Standalone LLM execution with ReAct loop.

    This backend uses LLM APIs directly to execute development tasks,
    implementing a ReAct (Reasoning + Acting) loop for autonomous
    problem-solving.

    Example:
        backend = BuiltinBackend(
            provider="claude",
            api_key="sk-ant-...",
            model="claude-sonnet-4-20250514"
        )

        result = await backend.execute({
            "id": "story-001",
            "title": "Add login feature",
            "description": "Implement user login",
            "acceptance_criteria": ["Users can log in"]
        })
    """

    # System prompt for the ReAct agent
    SYSTEM_PROMPT = """You are a professional software development agent.

You have the following tools available:
- read_file: Read file contents with line numbers
- write_file: Create or overwrite a file
- edit_file: Edit a file by replacing specific text
- run_command: Execute shell commands
- search_files: Search for files by glob pattern
- grep_content: Search for content in files

Working principles:
1. Always read relevant code before making changes
2. Follow the project's existing code style and conventions
3. Make incremental changes and verify each step
4. Test your implementation when possible
5. Document significant changes or decisions

When you complete the task successfully, output "TASK_COMPLETE" on its own line.
If you encounter an unrecoverable error, output "TASK_FAILED: <reason>".
"""

    def __init__(
        self,
        provider: str = "claude",
        model: str | None = None,
        api_key: str | None = None,
        base_url: str | None = None,
        max_iterations: int = 50,
        project_root: Path | None = None,
        config: dict[str, Any] | None = None
    ):
        """
        Initialize the Builtin backend.

        Args:
            provider: LLM provider name ("claude", "openai", "ollama", "deepseek")
            model: Model identifier (uses provider default if not specified)
            api_key: API key for the provider
            base_url: Custom API base URL
            max_iterations: Maximum ReAct iterations (default: 50)
            project_root: Project root directory
            config: Additional configuration
        """
        super().__init__(project_root)

        self.provider_name = provider
        self.model = model
        self.api_key = api_key
        self.base_url = base_url
        self.max_iterations = max_iterations
        self.config = config or {}

        self._llm: LLMProvider | None = None
        self._tools = ToolRegistry()
        self._react_engine: ReActEngine | None = None
        self._running = False
        self._should_stop = False

    def _get_llm(self) -> "LLMProvider":
        """Get or create the LLM provider."""
        if self._llm is None:
            from ..llm.factory import LLMFactory

            self._llm = LLMFactory.create(
                provider=self.provider_name,
                model=self.model,
                api_key=self.api_key,
                base_url=self.base_url,
                **self.config
            )

        return self._llm

    def _get_react_engine(self) -> ReActEngine:
        """Get or create the ReAct engine."""
        if self._react_engine is None:
            config = ReActConfig(
                max_iterations=self.max_iterations,
                temperature=0.7,
                max_tokens=self.config.get("max_tokens", 8192),
            )

            self._react_engine = ReActEngine(
                llm=self._get_llm(),
                tools=self._tools,
                config=config,
                system_prompt=self.SYSTEM_PROMPT,
            )

        return self._react_engine

    async def execute(
        self,
        story: dict[str, Any],
        context: str = ""
    ) -> ExecutionResult:
        """
        Execute a story using the ReAct loop.

        Args:
            story: Story dictionary
            context: Additional context

        Returns:
            ExecutionResult with outcome
        """
        story_id = story.get("id", "unknown")
        self._running = True
        self._should_stop = False

        # Build task description from story
        task = self._build_prompt(story, context)

        # Get or create ReAct engine
        engine = self._get_react_engine()

        # Wire up callbacks
        engine.on_text = self.on_text
        engine.on_tool_call = self.on_tool_call
        engine.on_thinking = self.on_thinking

        try:
            # Execute using ReAct engine
            result = await engine.execute(task=task, context="")

            return ExecutionResult(
                success=result.success,
                output=result.output,
                iterations=result.iterations,
                error=result.error,
                story_id=story_id,
                agent=f"builtin-{self.provider_name}",
                tool_calls=result.tool_calls,
                metadata=result.metadata,
            )

        except Exception as e:
            return ExecutionResult(
                success=False,
                output="",
                iterations=0,
                error=str(e),
                story_id=story_id,
                agent=f"builtin-{self.provider_name}",
            )
        finally:
            self._running = False

    async def stop(self) -> None:
        """Stop the current execution."""
        self._should_stop = True
        if self._react_engine:
            self._react_engine.stop()
        # Wait a bit for the loop to notice
        await asyncio.sleep(0.1)

    def get_llm(self) -> "LLMProvider":
        """Get the LLM provider."""
        return self._get_llm()

    def get_name(self) -> str:
        """Get the backend name."""
        return "builtin"

    def get_status(self) -> dict[str, Any]:
        """Get current status."""
        return {
            "backend": self.get_name(),
            "project_root": str(self.project_root),
            "provider": self.provider_name,
            "model": self.model,
            "running": self._running,
            "max_iterations": self.max_iterations,
        }

    def set_tools(self, tools: ToolRegistry) -> None:
        """
        Set a custom tool registry.

        Args:
            tools: ToolRegistry instance
        """
        self._tools = tools
        # Reset ReAct engine so it picks up new tools
        self._react_engine = None

    def register_tool(self, tool: Any) -> None:
        """
        Register an additional tool.

        Args:
            tool: Tool to register
        """
        self._tools.register(tool)
        # Reset ReAct engine so it picks up new tools
        self._react_engine = None


class AsyncBuiltinBackend(BuiltinBackend):
    """
    Async-optimized version of BuiltinBackend.

    Provides the same functionality but with better async handling
    for long-running operations.
    """

    async def execute_with_progress(
        self,
        story: dict[str, Any],
        context: str = "",
        progress_callback: Any | None = None
    ) -> ExecutionResult:
        """
        Execute with progress reporting.

        Args:
            story: Story dictionary
            context: Additional context
            progress_callback: Async callback for progress updates

        Returns:
            ExecutionResult with outcome
        """
        # Store original callbacks
        original_tool_callback = self.on_tool_call
        original_text_callback = self.on_text

        # Wrap callbacks to report progress
        if progress_callback:
            iteration_count = [0]

            async def wrapped_tool_callback(data: dict[str, Any]) -> None:
                if original_tool_callback:
                    original_tool_callback(data)
                if data.get("type") != "tool_result":
                    iteration_count[0] += 1
                await progress_callback({
                    "type": "tool_call",
                    "iteration": iteration_count[0],
                    "data": data
                })

            def sync_tool_callback(data: dict[str, Any]) -> None:
                asyncio.create_task(wrapped_tool_callback(data))

            self.on_tool_call = sync_tool_callback

        try:
            return await self.execute(story, context)
        finally:
            self.on_tool_call = original_tool_callback
            self.on_text = original_text_callback
