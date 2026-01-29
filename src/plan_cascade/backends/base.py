"""
Base Backend Abstraction for Plan Cascade

Provides abstract base class for backend implementations and standardized result types.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional


@dataclass
class ExecutionResult:
    """Standardized result from agent execution."""
    success: bool
    story_id: str
    agent: str
    agent_type: str  # "task-tool" or "cli"
    error: Optional[str] = None
    exit_code: Optional[int] = None
    output: Optional[str] = None
    output_file: Optional[str] = None
    pid: Optional[int] = None
    started_at: Optional[str] = None
    completed_at: Optional[str] = None
    duration_seconds: Optional[float] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "success": self.success,
            "story_id": self.story_id,
            "agent": self.agent,
            "agent_type": self.agent_type,
            "error": self.error,
            "exit_code": self.exit_code,
            "output": self.output,
            "output_file": self.output_file,
            "pid": self.pid,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "duration_seconds": self.duration_seconds,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ExecutionResult":
        """Create from dictionary."""
        return cls(
            success=data.get("success", False),
            story_id=data.get("story_id", ""),
            agent=data.get("agent", ""),
            agent_type=data.get("agent_type", ""),
            error=data.get("error"),
            exit_code=data.get("exit_code"),
            output=data.get("output"),
            output_file=data.get("output_file"),
            pid=data.get("pid"),
            started_at=data.get("started_at"),
            completed_at=data.get("completed_at"),
            duration_seconds=data.get("duration_seconds"),
            metadata=data.get("metadata", {}),
        )


class AgentBackend(ABC):
    """
    Abstract base class for agent backends.

    Backends provide the execution layer for running agents:
    - ClaudeCodeBackend: Uses Claude Code's built-in Task tool
    - CLIBackend: Executes external CLI tools (codex, aider, etc.)
    """

    def __init__(self, project_root: Path):
        """
        Initialize the backend.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)

    @abstractmethod
    def is_available(self) -> bool:
        """
        Check if this backend is available.

        Returns:
            True if backend can be used
        """
        pass

    @abstractmethod
    def execute(
        self,
        story_id: str,
        prompt: str,
        context: Optional[Dict[str, Any]] = None,
    ) -> ExecutionResult:
        """
        Execute a story with this backend.

        Args:
            story_id: Story ID being executed
            prompt: Execution prompt
            context: Optional execution context

        Returns:
            ExecutionResult with outcome
        """
        pass

    @abstractmethod
    def get_name(self) -> str:
        """Get the name of this backend."""
        pass

    @abstractmethod
    def get_type(self) -> str:
        """Get the type of this backend ('task-tool' or 'cli')."""
        pass

    def get_status(self, story_id: str) -> Optional[Dict[str, Any]]:
        """
        Get execution status for a story.

        Args:
            story_id: Story ID to check

        Returns:
            Status dict or None if not found
        """
        return None

    def stop(self, story_id: str) -> bool:
        """
        Stop execution of a story.

        Args:
            story_id: Story ID to stop

        Returns:
            True if successfully stopped
        """
        return False
