"""
Plan Cascade - AI-Powered Development Orchestration

A tool that enables AI-driven development workflows with support for:
- Claude Code as GUI (ClaudeCodeBackend)
- Standalone LLM execution (BuiltinBackend)
- Multi-agent collaboration
- PRD-driven task decomposition
"""

__version__ = "2.0.0"

from .backends import AgentBackend, ExecutionResult, BackendFactory
from .llm import LLMProvider, LLMResponse, LLMFactory

__all__ = [
    # Backends
    "AgentBackend",
    "ExecutionResult",
    "BackendFactory",
    # LLM
    "LLMProvider",
    "LLMResponse",
    "LLMFactory",
]
