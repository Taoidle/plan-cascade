"""
Plan Cascade CLI Module

Contains the command-line interface:
- main: CLI entry point with typer
- Commands: run, config, status, worktree, mega, design
- Simple mode: One-click execution
- Expert mode: Interactive PRD editing
- Worktree: Parallel task development with Git worktrees
- Mega: Multi-feature project orchestration
- Design: Design document management
"""

from .main import app, main

try:
    from .worktree import WorktreeManager, WorktreeState, WorktreeStatus, worktree_app
except ImportError:
    WorktreeManager = None
    WorktreeState = None
    WorktreeStatus = None
    worktree_app = None

try:
    from .mega import mega_app
except ImportError:
    mega_app = None

try:
    from .design import design_app
except ImportError:
    design_app = None

__all__ = [
    "app",
    "main",
    "WorktreeManager",
    "WorktreeState",
    "WorktreeStatus",
    "worktree_app",
    "mega_app",
    "design_app",
]
