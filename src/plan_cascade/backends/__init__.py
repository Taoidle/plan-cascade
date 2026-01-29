"""
Plan Cascade Backends Module

Contains the backend abstraction layer for agent execution.
Supports dual-mode operation:
- ClaudeCodeBackend: Plan Cascade as GUI for Claude Code
- BuiltinBackend: Standalone operation with direct LLM API calls

Key Components:
- AgentBackend: Abstract base class for all backends
- ExecutionResult: Standardized execution result
- BackendFactory: Backend instantiation factory
"""

from .base import AgentBackend, ExecutionResult
from .builtin import BuiltinBackend
from .claude_code import ClaudeCodeBackend
from .factory import BackendFactory

__all__ = [
    # Base classes
    "AgentBackend",
    "ExecutionResult",
    # Factory
    "BackendFactory",
    # Implementations
    "ClaudeCodeBackend",
    "BuiltinBackend",
]
