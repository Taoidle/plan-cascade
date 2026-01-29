"""
ReAct Engine

Independent implementation of the ReAct (Reasoning + Acting) loop
for autonomous task execution.

This module provides a standalone ReAct engine that can be used by
different backends (Builtin, Claude Max, etc.) for LLM-driven
task execution with tool use.

The ReAct pattern:
1. Think: LLM reasons about the task and decides next action
2. Act: Execute the decided tool/action
3. Observe: Process the result and update state
4. Repeat until task completion or max iterations
"""

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ..llm.base import LLMProvider, ToolCall
    from ..tools.registry import ToolRegistry


# Type aliases for callbacks
OnTextCallback = Callable[[str], None]
OnToolCallCallback = Callable[[dict[str, Any]], None]
OnThinkingCallback = Callable[[str], None]


@dataclass
class ReActResult:
    """
    Result from a ReAct execution.

    Attributes:
        success: Whether the task completed successfully
        output: Accumulated text output from the execution
        iterations: Number of Think-Act-Observe cycles
        error: Error message if execution failed
        tool_calls: List of all tool calls made
        final_response: The final LLM response content
        metadata: Additional execution metadata
    """
    success: bool
    output: str = ""
    iterations: int = 0
    error: str | None = None
    tool_calls: list[dict[str, Any]] = field(default_factory=list)
    final_response: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass
class ReActConfig:
    """
    Configuration for the ReAct engine.

    Attributes:
        max_iterations: Maximum Think-Act-Observe cycles
        temperature: LLM sampling temperature
        max_tokens: Maximum tokens per LLM response
        completion_markers: Strings that indicate task completion
        failure_markers: Strings that indicate task failure
        stop_on_end_turn: Stop if LLM signals end_turn without tool calls
    """
    max_iterations: int = 50
    temperature: float = 0.7
    max_tokens: int = 8192
    completion_markers: list[str] = field(
        default_factory=lambda: ["TASK_COMPLETE", "Task completed", "Done."]
    )
    failure_markers: list[str] = field(
        default_factory=lambda: ["TASK_FAILED:", "Cannot complete", "Error:"]
    )
    stop_on_end_turn: bool = True


class ReActEngine:
    """
    ReAct (Reasoning + Acting) Engine for autonomous task execution.

    The engine orchestrates the Think → Act → Observe loop:

    1. Think: Send task/context to LLM, get reasoning and tool call decision
    2. Act: Execute the tool(s) requested by the LLM
    3. Observe: Feed tool results back to LLM
    4. Repeat until completion criteria or max iterations

    Example:
        from plan_cascade.llm.factory import LLMFactory
        from plan_cascade.tools.registry import ToolRegistry

        llm = LLMFactory.create("claude", api_key="sk-...")
        tools = ToolRegistry()

        engine = ReActEngine(llm, tools, max_iterations=30)

        # Set up callbacks for UI
        engine.on_text = lambda text: print(text, end="")
        engine.on_tool_call = lambda data: print(f"Tool: {data['name']}")

        result = await engine.execute(
            task="Implement a login function",
            context="Project uses Python Flask"
        )

        if result.success:
            print(f"Completed in {result.iterations} iterations")
    """

    # Default system prompt for the ReAct agent
    DEFAULT_SYSTEM_PROMPT = """You are a professional software development agent.

You have tools available for file operations, code search, and command execution.
Use them to complete the given task.

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
        llm: "LLMProvider",
        tools: "ToolRegistry",
        config: ReActConfig | None = None,
        system_prompt: str | None = None,
    ):
        """
        Initialize the ReAct engine.

        Args:
            llm: LLM provider for generating responses
            tools: Tool registry for executing actions
            config: Engine configuration (uses defaults if not provided)
            system_prompt: Custom system prompt (uses default if not provided)
        """
        self.llm = llm
        self.tools = tools
        self.config = config or ReActConfig()
        self.system_prompt = system_prompt or self.DEFAULT_SYSTEM_PROMPT

        # Callbacks for UI integration
        self.on_text: OnTextCallback | None = None
        self.on_tool_call: OnToolCallCallback | None = None
        self.on_thinking: OnThinkingCallback | None = None

        # Execution state
        self._running = False
        self._should_stop = False

    async def execute(
        self,
        task: str,
        context: str = "",
        initial_messages: list[dict[str, Any]] | None = None,
    ) -> ReActResult:
        """
        Execute a task using the ReAct loop.

        Args:
            task: Task description to execute
            context: Additional context for the task
            initial_messages: Pre-existing message history (for continuation)

        Returns:
            ReActResult with execution outcome
        """
        self._running = True
        self._should_stop = False

        # Build initial messages
        if initial_messages:
            messages = list(initial_messages)
        else:
            messages = [
                {"role": "system", "content": self.system_prompt},
            ]

        # Add the task as user message
        task_prompt = self._build_task_prompt(task, context)
        messages.append({"role": "user", "content": task_prompt})

        # Get tool definitions
        tool_definitions = self.tools.get_definitions()

        # Track execution
        all_tool_calls: list[dict[str, Any]] = []
        output_text = ""
        iteration = 0

        try:
            for iteration in range(self.config.max_iterations):
                # Check for stop signal
                if self._should_stop:
                    return ReActResult(
                        success=False,
                        output=output_text,
                        iterations=iteration,
                        error="Execution stopped by user",
                        tool_calls=all_tool_calls,
                    )

                # THINK: Get LLM response
                response = await self.llm.complete(
                    messages=messages,
                    tools=tool_definitions,
                    temperature=self.config.temperature,
                    max_tokens=self.config.max_tokens,
                )

                # Handle text output
                if response.content:
                    output_text += response.content + "\n"
                    await self._emit_text(response.content)

                    # Check for completion markers
                    completion_result = self._check_completion(response.content)
                    if completion_result is not None:
                        return ReActResult(
                            success=completion_result["success"],
                            output=output_text,
                            iterations=iteration + 1,
                            error=completion_result.get("error"),
                            tool_calls=all_tool_calls,
                            final_response=response.content,
                        )

                # Check for natural end (no tool calls and end_turn)
                if (self.config.stop_on_end_turn and
                    response.stop_reason == "end_turn" and
                    not response.tool_calls):
                    return ReActResult(
                        success=True,
                        output=output_text,
                        iterations=iteration + 1,
                        tool_calls=all_tool_calls,
                        final_response=response.content,
                    )

                # ACT: Execute tool calls
                if response.tool_calls:
                    # Add assistant message with tool calls to history
                    messages.append({
                        "role": "assistant",
                        "content": response.content,
                        "tool_calls": [tc.to_dict() for tc in response.tool_calls],
                    })

                    # Execute each tool
                    tool_results = await self._execute_tools(
                        response.tool_calls,
                        all_tool_calls,
                        iteration,
                    )

                    # OBSERVE: Add tool results to messages
                    for tr in tool_results:
                        messages.append({
                            "role": "tool",
                            "tool_call_id": tr["tool_call_id"],
                            "content": tr["output"],
                        })
                else:
                    # No tool calls but not end_turn - unusual but handle gracefully
                    break

            # Max iterations reached
            return ReActResult(
                success=False,
                output=output_text,
                iterations=self.config.max_iterations,
                error="Maximum iterations reached without completion",
                tool_calls=all_tool_calls,
            )

        except Exception as e:
            return ReActResult(
                success=False,
                output=output_text,
                iterations=iteration,
                error=str(e),
                tool_calls=all_tool_calls,
                metadata={"exception_type": type(e).__name__},
            )
        finally:
            self._running = False

    async def _execute_tools(
        self,
        tool_calls: list["ToolCall"],
        all_tool_calls: list[dict[str, Any]],
        iteration: int,
    ) -> list[dict[str, str]]:
        """
        Execute a list of tool calls.

        Args:
            tool_calls: Tool calls from LLM response
            all_tool_calls: Accumulator for all tool calls
            iteration: Current iteration number

        Returns:
            List of tool result dictionaries
        """
        tool_results = []

        for tc in tool_calls:
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
            result = await self.tools.execute(tc.name, **tc.arguments)

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

        return tool_results

    def _build_task_prompt(self, task: str, context: str = "") -> str:
        """
        Build the task prompt.

        Args:
            task: Task description
            context: Additional context

        Returns:
            Formatted prompt string
        """
        parts = [f"## Task\n{task}"]

        if context:
            parts.append(f"\n## Context\n{context}")

        parts.append("\nPlease complete this task. Use the available tools as needed.")

        return "\n".join(parts)

    def _check_completion(self, content: str) -> dict[str, Any] | None:
        """
        Check if content contains completion or failure markers.

        Args:
            content: LLM response content

        Returns:
            Dict with success status and optional error, or None if no marker found
        """
        # Check for completion markers
        for marker in self.config.completion_markers:
            if marker in content:
                return {"success": True}

        # Check for failure markers
        for marker in self.config.failure_markers:
            if marker in content:
                # Extract error message after the marker
                idx = content.find(marker)
                error_msg = content[idx + len(marker):].strip()
                # Take first line or first 200 chars
                error_msg = error_msg.split("\n")[0][:200]
                return {"success": False, "error": error_msg or "Task failed"}

        return None

    async def _emit_text(self, text: str) -> None:
        """Emit text to callback."""
        if self.on_text:
            try:
                self.on_text(text)
            except Exception:
                pass

    async def _emit_tool_call(self, data: dict[str, Any]) -> None:
        """Emit tool call to callback."""
        if self.on_tool_call:
            try:
                self.on_tool_call(data)
            except Exception:
                pass

    async def _emit_thinking(self, text: str) -> None:
        """Emit thinking to callback."""
        if self.on_thinking:
            try:
                self.on_thinking(text)
            except Exception:
                pass

    def stop(self) -> None:
        """Signal the engine to stop execution."""
        self._should_stop = True

    def is_running(self) -> bool:
        """Check if the engine is currently running."""
        return self._running

    def get_status(self) -> dict[str, Any]:
        """
        Get the current engine status.

        Returns:
            Status dictionary
        """
        return {
            "running": self._running,
            "should_stop": self._should_stop,
            "max_iterations": self.config.max_iterations,
            "tools_available": self.tools.list_tools(),
        }


class ReActEngineBuilder:
    """
    Builder for creating configured ReActEngine instances.

    Example:
        engine = (ReActEngineBuilder()
            .with_llm(llm)
            .with_tools(tools)
            .with_max_iterations(30)
            .with_system_prompt("Custom prompt...")
            .on_text(print)
            .build())
    """

    def __init__(self):
        self._llm = None
        self._tools = None
        self._config = ReActConfig()
        self._system_prompt = None
        self._on_text = None
        self._on_tool_call = None
        self._on_thinking = None

    def with_llm(self, llm: "LLMProvider") -> "ReActEngineBuilder":
        """Set the LLM provider."""
        self._llm = llm
        return self

    def with_tools(self, tools: "ToolRegistry") -> "ReActEngineBuilder":
        """Set the tool registry."""
        self._tools = tools
        return self

    def with_config(self, config: ReActConfig) -> "ReActEngineBuilder":
        """Set the full configuration."""
        self._config = config
        return self

    def with_max_iterations(self, max_iterations: int) -> "ReActEngineBuilder":
        """Set maximum iterations."""
        self._config.max_iterations = max_iterations
        return self

    def with_temperature(self, temperature: float) -> "ReActEngineBuilder":
        """Set LLM temperature."""
        self._config.temperature = temperature
        return self

    def with_max_tokens(self, max_tokens: int) -> "ReActEngineBuilder":
        """Set maximum tokens per response."""
        self._config.max_tokens = max_tokens
        return self

    def with_system_prompt(self, prompt: str) -> "ReActEngineBuilder":
        """Set custom system prompt."""
        self._system_prompt = prompt
        return self

    def on_text(self, callback: OnTextCallback) -> "ReActEngineBuilder":
        """Set text callback."""
        self._on_text = callback
        return self

    def on_tool_call(self, callback: OnToolCallCallback) -> "ReActEngineBuilder":
        """Set tool call callback."""
        self._on_tool_call = callback
        return self

    def on_thinking(self, callback: OnThinkingCallback) -> "ReActEngineBuilder":
        """Set thinking callback."""
        self._on_thinking = callback
        return self

    def build(self) -> ReActEngine:
        """
        Build the ReActEngine.

        Returns:
            Configured ReActEngine instance

        Raises:
            ValueError: If required components are missing
        """
        if self._llm is None:
            raise ValueError("LLM provider is required")
        if self._tools is None:
            raise ValueError("Tool registry is required")

        engine = ReActEngine(
            llm=self._llm,
            tools=self._tools,
            config=self._config,
            system_prompt=self._system_prompt,
        )

        if self._on_text:
            engine.on_text = self._on_text
        if self._on_tool_call:
            engine.on_tool_call = self._on_tool_call
        if self._on_thinking:
            engine.on_thinking = self._on_thinking

        return engine
