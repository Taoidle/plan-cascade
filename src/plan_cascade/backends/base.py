"""
Base Backend Abstraction for Plan Cascade

Provides abstract base class for backend implementations and standardized result types.
This module supports the dual-backend architecture:

1. ClaudeCodeBackend: Plan Cascade as a GUI for Claude Code
   - Communicates via subprocess with Claude Code CLI
   - No API key required (uses Claude Code's authentication)
   - Provides visualization of Claude Code's actions

2. BuiltinBackend: Standalone LLM execution
   - Direct LLM API calls (Claude, OpenAI, Ollama)
   - ReAct loop for autonomous task execution
   - Requires API key configuration

Both backends implement the same interface, allowing seamless switching.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


@dataclass
class ExecutionResult:
    """
    Standardized result from agent execution.

    Attributes:
        success: Whether execution completed successfully
        output: Text output from the execution
        iterations: Number of tool call iterations (ReAct cycles)
        error: Error message if execution failed
        story_id: Story ID that was executed (if applicable)
        agent: Name of the agent/backend used
        tool_calls: List of tool calls made during execution
        metadata: Additional execution metadata
    """
    success: bool
    output: str = ""
    iterations: int = 0
    error: Optional[str] = None
    story_id: Optional[str] = None
    agent: str = ""
    tool_calls: List[Dict[str, Any]] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "success": self.success,
            "output": self.output,
            "iterations": self.iterations,
            "error": self.error,
            "story_id": self.story_id,
            "agent": self.agent,
            "tool_calls": self.tool_calls,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ExecutionResult":
        """Create from dictionary."""
        return cls(
            success=data.get("success", False),
            output=data.get("output", ""),
            iterations=data.get("iterations", 0),
            error=data.get("error"),
            story_id=data.get("story_id"),
            agent=data.get("agent", ""),
            tool_calls=data.get("tool_calls", []),
            metadata=data.get("metadata", {}),
        )

    @classmethod
    def failure(cls, error: str, **kwargs: Any) -> "ExecutionResult":
        """Create a failure result."""
        return cls(success=False, error=error, **kwargs)

    @classmethod
    def success_result(cls, output: str = "", **kwargs: Any) -> "ExecutionResult":
        """Create a success result."""
        return cls(success=True, output=output, **kwargs)


# Type alias for callbacks
OnToolCallCallback = Callable[[Dict[str, Any]], None]
OnTextCallback = Callable[[str], None]


class AgentBackend(ABC):
    """
    Abstract base class for agent backends.

    Backends provide the execution layer for running development tasks:
    - ClaudeCodeBackend: Uses Claude Code CLI for execution
    - BuiltinBackend: Uses LLM API directly with ReAct loop

    Subclasses must implement:
    - execute(): Execute a story/task
    - get_llm(): Get the LLM provider for PRD generation, etc.
    - get_name(): Return the backend name

    Optional overrides:
    - start_session(): Initialize a session
    - stop(): Stop ongoing execution
    - get_status(): Get execution status
    """

    def __init__(self, project_root: Optional[Path] = None):
        """
        Initialize the backend.

        Args:
            project_root: Root directory of the project (default: current directory)
        """
        self.project_root = Path(project_root) if project_root else Path.cwd()

        # Callbacks for UI integration
        self.on_tool_call: Optional[OnToolCallCallback] = None
        self.on_text: Optional[OnTextCallback] = None

    @abstractmethod
    async def execute(
        self,
        story: Dict[str, Any],
        context: str = ""
    ) -> ExecutionResult:
        """
        Execute a story/task.

        Args:
            story: Story dictionary with at minimum:
                - id: Story identifier
                - title: Story title
                - description: Story description
                - acceptance_criteria: List of acceptance criteria
            context: Additional context for execution

        Returns:
            ExecutionResult with the outcome
        """
        pass

    @abstractmethod
    def get_llm(self) -> "LLMProvider":
        """
        Get the LLM provider for this backend.

        Used for PRD generation, strategy analysis, and other
        LLM tasks outside of story execution.

        Returns:
            LLMProvider instance
        """
        pass

    @abstractmethod
    def get_name(self) -> str:
        """
        Get the name of this backend.

        Returns:
            Backend name (e.g., "claude-code", "builtin")
        """
        pass

    async def start_session(self, project_path: Optional[str] = None) -> None:
        """
        Start an execution session.

        Override in subclasses that need session initialization
        (e.g., ClaudeCodeBackend starts a subprocess).

        Args:
            project_path: Project path for the session
        """
        if project_path:
            self.project_root = Path(project_path)

    async def stop(self) -> None:
        """
        Stop any ongoing execution and cleanup.

        Override in subclasses that need cleanup logic.
        """
        pass

    def get_status(self) -> Dict[str, Any]:
        """
        Get the current execution status.

        Returns:
            Status dictionary
        """
        return {
            "backend": self.get_name(),
            "project_root": str(self.project_root),
        }

    def _build_prompt(self, story: Dict[str, Any], context: str = "") -> str:
        """
        Build the execution prompt for a story.

        Args:
            story: Story dictionary
            context: Additional context

        Returns:
            Formatted prompt string
        """
        story_id = story.get("id", "unknown")
        title = story.get("title", story_id)
        description = story.get("description", "")

        # Format acceptance criteria
        ac = story.get("acceptance_criteria", [])
        if isinstance(ac, list):
            ac_text = "\n".join(f"- {item}" for item in ac)
        else:
            ac_text = str(ac)

        prompt = f"""Please complete the following development task:

## Story: {title}
{description}

## Acceptance Criteria
{ac_text}

## Context
{context if context else "No additional context provided."}

Instructions:
1. Read relevant code and documentation first
2. Implement according to acceptance criteria
3. Follow project coding conventions
4. Test your implementation
5. Report completion when done
"""
        return prompt

    async def _emit_tool_call(self, data: Dict[str, Any]) -> None:
        """
        Emit a tool call event to the callback.

        Args:
            data: Tool call data
        """
        if self.on_tool_call:
            try:
                self.on_tool_call(data)
            except Exception:
                pass  # Don't let callback errors break execution

    async def _emit_text(self, text: str) -> None:
        """
        Emit a text event to the callback.

        Args:
            text: Text content
        """
        if self.on_text:
            try:
                self.on_text(text)
            except Exception:
                pass  # Don't let callback errors break execution
