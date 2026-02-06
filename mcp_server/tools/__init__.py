"""
MCP Tools for Plan Cascade

This package provides MCP tools for:
- PRD (Product Requirements Document) management
- Mega Plan (project-level) management
- Execution and state management
- Worktree lifecycle management
- Design Document lifecycle management
- Spec Interview lifecycle management
- Dashboard and session recovery
"""

from .prd_tools import register_prd_tools
from .mega_tools import register_mega_tools
from .execution_tools import register_execution_tools
from .worktree_tools import register_worktree_tools
from .design_tools import register_design_tools
from .spec_tools import register_spec_tools
from .dashboard_tools import register_dashboard_tools

__all__ = [
    "register_prd_tools",
    "register_mega_tools",
    "register_execution_tools",
    "register_worktree_tools",
    "register_design_tools",
    "register_spec_tools",
    "register_dashboard_tools",
]
