"""
Builtin Backend

Implementation of AgentBackend that uses LLM APIs directly with a ReAct loop.
Enables Plan Cascade to run independently without Claude Code.

Key features:
- Direct LLM API calls (Claude, OpenAI, Ollama)
- ReAct (Reasoning + Acting) loop for autonomous task execution
- Tool registry integration for file/shell operations
- Configurable iteration limits and timeouts
"""

import asyncio
from pathlib import Path
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .base import AgentBackend, ExecutionResult
from ..tools.registry import ToolRegistry

if TYPE_CHECKING:
    from ..llm.base import LLMProvider, LLMResponse


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
        model: Optional[str] = None,
        api_key: Optional[str] = None,
        base_url: Optional[str] = None,
        max_iterations: int = 50,
        project_root: Optional[Path] = None,
        config: Optional[Dict[str, Any]] = None
    ):
        """
        Initialize the Builtin backend.

        Args:
            provider: LLM provider name ("claude", "openai", "ollama")
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

        self._llm: Optional["LLMProvider"] = None
        self._tools = ToolRegistry()
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

    async def execute(
        self,
        story: Dict[str, Any],
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

        # Build initial prompt
        prompt = self._build_prompt(story, context)

        # Initialize messages
        messages: List[Dict[str, Any]] = [
            {"role": "system", "content": self.SYSTEM_PROMPT},
            {"role": "user", "content": prompt}
        ]

        # Get tool definitions
        tool_definitions = self._tools.get_definitions()

        # Track execution
        all_tool_calls: List[Dict[str, Any]] = []
        output_text = ""
        iteration = 0

        try:
            llm = self._get_llm()

            for iteration in range(self.max_iterations):
                if self._should_stop:
                    return ExecutionResult(
                        success=False,
                        output=output_text,
                        iterations=iteration,
                        error="Execution stopped by user",
                        story_id=story_id,
                        agent=f"builtin-{self.provider_name}",
                        tool_calls=all_tool_calls,
                    )

                # Get LLM response
                response = await llm.complete(
                    messages=messages,
                    tools=tool_definitions,
                    temperature=0.7,
                    max_tokens=self.config.get("max_tokens", 8192),
                )

                # Handle text output
                if response.content:
                    output_text += response.content + "\n"
                    await self._emit_text(response.content)

                    # Check for completion markers
                    if "TASK_COMPLETE" in response.content:
                        return ExecutionResult(
                            success=True,
                            output=output_text,
                            iterations=iteration + 1,
                            story_id=story_id,
                            agent=f"builtin-{self.provider_name}",
                            tool_calls=all_tool_calls,
                        )

                    if "TASK_FAILED:" in response.content:
                        error_msg = response.content.split("TASK_FAILED:")[-1].strip()
                        return ExecutionResult(
                            success=False,
                            output=output_text,
                            iterations=iteration + 1,
                            error=error_msg,
                            story_id=story_id,
                            agent=f"builtin-{self.provider_name}",
                            tool_calls=all_tool_calls,
                        )

                # Check if we should stop (no tool calls and end_turn)
                if response.stop_reason == "end_turn" and not response.tool_calls:
                    return ExecutionResult(
                        success=True,
                        output=output_text,
                        iterations=iteration + 1,
                        story_id=story_id,
                        agent=f"builtin-{self.provider_name}",
                        tool_calls=all_tool_calls,
                    )

                # Execute tool calls
                if response.tool_calls:
                    # Add assistant message with tool calls
                    messages.append({
                        "role": "assistant",
                        "content": response.content,
                        "tool_calls": [tc.to_dict() for tc in response.tool_calls],
                    })

                    # Execute each tool call
                    tool_results = []
                    for tc in response.tool_calls:
                        # Record tool call
                        tool_data = {
                            "name": tc.name,
                            "arguments": tc.arguments,
                            "id": tc.id,
                            "iteration": iteration,
                        }
                        all_tool_calls.append(tool_data)
                        await self._emit_tool_call(tool_data)

                        # Execute tool
                        result = await self._tools.execute(tc.name, **tc.arguments)

                        # Record result
                        result_data = {
                            "tool_call_id": tc.id,
                            "name": tc.name,
                            "success": result.success,
                            "output": result.to_string()[:2000],  # Truncate for context window
                        }
                        await self._emit_tool_call({
                            "type": "tool_result",
                            **result_data
                        })

                        tool_results.append(result_data)

                    # Add tool results to messages
                    for tr in tool_results:
                        messages.append({
                            "role": "tool",
                            "tool_call_id": tr["tool_call_id"],
                            "content": tr["output"],
                        })

                else:
                    # No tool calls but stop_reason was not end_turn
                    # This shouldn't normally happen, but handle it
                    break

            # Max iterations reached
            return ExecutionResult(
                success=False,
                output=output_text,
                iterations=self.max_iterations,
                error="Maximum iterations reached without completion",
                story_id=story_id,
                agent=f"builtin-{self.provider_name}",
                tool_calls=all_tool_calls,
            )

        except Exception as e:
            return ExecutionResult(
                success=False,
                output=output_text,
                iterations=iteration,
                error=str(e),
                story_id=story_id,
                agent=f"builtin-{self.provider_name}",
                tool_calls=all_tool_calls,
            )
        finally:
            self._running = False

    async def stop(self) -> None:
        """Stop the current execution."""
        self._should_stop = True
        # Wait a bit for the loop to notice
        await asyncio.sleep(0.1)

    def get_llm(self) -> "LLMProvider":
        """Get the LLM provider."""
        return self._get_llm()

    def get_name(self) -> str:
        """Get the backend name."""
        return "builtin"

    def get_status(self) -> Dict[str, Any]:
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

    def register_tool(self, tool: Any) -> None:
        """
        Register an additional tool.

        Args:
            tool: Tool to register
        """
        self._tools.register(tool)


class AsyncBuiltinBackend(BuiltinBackend):
    """
    Async-optimized version of BuiltinBackend.

    Provides the same functionality but with better async handling
    for long-running operations.
    """

    async def execute_with_progress(
        self,
        story: Dict[str, Any],
        context: str = "",
        progress_callback: Optional[Any] = None
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

            async def wrapped_tool_callback(data: Dict[str, Any]) -> None:
                if original_tool_callback:
                    original_tool_callback(data)
                if data.get("type") != "tool_result":
                    iteration_count[0] += 1
                await progress_callback({
                    "type": "tool_call",
                    "iteration": iteration_count[0],
                    "data": data
                })

            def sync_tool_callback(data: Dict[str, Any]) -> None:
                asyncio.create_task(wrapped_tool_callback(data))

            self.on_tool_call = sync_tool_callback

        try:
            return await self.execute(story, context)
        finally:
            self.on_tool_call = original_tool_callback
            self.on_text = original_text_callback
