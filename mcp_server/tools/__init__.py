"""
MCP Tools for Plan Cascade

This package provides MCP tools for:
- PRD (Product Requirements Document) management
- Mega Plan (project-level) management
- Execution and state management
"""

from .prd_tools import register_prd_tools
from .mega_tools import register_mega_tools
from .execution_tools import register_execution_tools

__all__ = [
    "register_prd_tools",
    "register_mega_tools",
    "register_execution_tools",
]
