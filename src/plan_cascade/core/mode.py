"""
User Mode Configuration for Plan Cascade

Provides Simple and Expert mode configurations for different user needs.
Simple mode: AI-driven automatic execution
Expert mode: Interactive PRD editing and agent selection
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class UserMode(Enum):
    """User mode for Plan Cascade operation."""
    SIMPLE = "simple"    # AI-driven automatic execution
    EXPERT = "expert"    # Interactive PRD editing and agent selection


@dataclass
class ModeConfig:
    """Configuration for user mode behavior."""

    mode: UserMode = UserMode.SIMPLE

    # Simple mode settings
    auto_approve_prd: bool = True
    auto_start_execution: bool = True
    auto_iterate: bool = True
    show_progress_updates: bool = True

    # Expert mode settings
    allow_prd_editing: bool = True
    allow_agent_selection: bool = True
    allow_story_reordering: bool = True
    show_dependency_graph: bool = True
    require_explicit_approval: bool = True

    # Quality gate settings
    quality_gates_enabled: bool = True
    auto_retry_on_failure: bool = True
    max_retries: int = 3

    # Backend settings
    preferred_backend: str | None = None
    fallback_backends: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "mode": self.mode.value,
            "auto_approve_prd": self.auto_approve_prd,
            "auto_start_execution": self.auto_start_execution,
            "auto_iterate": self.auto_iterate,
            "show_progress_updates": self.show_progress_updates,
            "allow_prd_editing": self.allow_prd_editing,
            "allow_agent_selection": self.allow_agent_selection,
            "allow_story_reordering": self.allow_story_reordering,
            "show_dependency_graph": self.show_dependency_graph,
            "require_explicit_approval": self.require_explicit_approval,
            "quality_gates_enabled": self.quality_gates_enabled,
            "auto_retry_on_failure": self.auto_retry_on_failure,
            "max_retries": self.max_retries,
            "preferred_backend": self.preferred_backend,
            "fallback_backends": self.fallback_backends,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ModeConfig":
        """Create from dictionary."""
        mode = data.get("mode", "simple")
        if isinstance(mode, str):
            mode = UserMode(mode)

        return cls(
            mode=mode,
            auto_approve_prd=data.get("auto_approve_prd", True),
            auto_start_execution=data.get("auto_start_execution", True),
            auto_iterate=data.get("auto_iterate", True),
            show_progress_updates=data.get("show_progress_updates", True),
            allow_prd_editing=data.get("allow_prd_editing", True),
            allow_agent_selection=data.get("allow_agent_selection", True),
            allow_story_reordering=data.get("allow_story_reordering", True),
            show_dependency_graph=data.get("show_dependency_graph", True),
            require_explicit_approval=data.get("require_explicit_approval", True),
            quality_gates_enabled=data.get("quality_gates_enabled", True),
            auto_retry_on_failure=data.get("auto_retry_on_failure", True),
            max_retries=data.get("max_retries", 3),
            preferred_backend=data.get("preferred_backend"),
            fallback_backends=data.get("fallback_backends", []),
        )

    @classmethod
    def simple(cls) -> "ModeConfig":
        """Create configuration for simple mode."""
        return cls(
            mode=UserMode.SIMPLE,
            auto_approve_prd=True,
            auto_start_execution=True,
            auto_iterate=True,
            show_progress_updates=True,
            require_explicit_approval=False,
        )

    @classmethod
    def expert(cls) -> "ModeConfig":
        """Create configuration for expert mode."""
        return cls(
            mode=UserMode.EXPERT,
            auto_approve_prd=False,
            auto_start_execution=False,
            auto_iterate=False,
            show_progress_updates=True,
            allow_prd_editing=True,
            allow_agent_selection=True,
            allow_story_reordering=True,
            show_dependency_graph=True,
            require_explicit_approval=True,
        )

    def is_simple(self) -> bool:
        """Check if in simple mode."""
        return self.mode == UserMode.SIMPLE

    def is_expert(self) -> bool:
        """Check if in expert mode."""
        return self.mode == UserMode.EXPERT

    def should_auto_execute(self) -> bool:
        """Check if execution should start automatically."""
        return self.auto_approve_prd and self.auto_start_execution

    def should_show_prd_editor(self) -> bool:
        """Check if PRD editor should be shown."""
        return self.allow_prd_editing and not self.auto_approve_prd
