"""
Plan Cascade State Module

Contains state management and context filtering:
- StateManager: Thread-safe state management with file locking
- FileLock: Cross-platform file locking mechanism
- ContextFilter: Story context extraction and filtering
- MegaStateManager: Mega-plan state management
- ContextRecoveryManager: Context detection and recovery for interrupted tasks
- PathResolver: Unified path resolution for runtime files
- ConfigManager: Hierarchical configuration management
- ProjectLinkManager: Project discovery via link files in project roots
- Resume detection: Unified detection of incomplete state and resume suggestions
"""

from .config_manager import ConfigManager
from .context_filter import ContextFilter
from .context_recovery import (
    ContextRecoveryManager,
    ContextRecoveryState,
    ContextType,
    PrdStatus,
    RecoveryAction,
    RecoveryPlan,
    TaskState,
)
from .mega_state import MegaStateManager
from .path_resolver import PathResolver
from .project_link import LINK_FILE_NAME, ProjectLinkManager
from .resume_detector import (
    IncompleteStateInfo,
    ResumeReason,
    ResumeSuggestion,
    check_and_suggest_resume,
    detect_incomplete_state,
    format_resume_display,
    get_resume_suggestion,
)
from .state_manager import FileLock, StateManager

__all__ = [
    "StateManager",
    "FileLock",
    "ContextFilter",
    "MegaStateManager",
    "ContextRecoveryManager",
    "ContextRecoveryState",
    "ContextType",
    "TaskState",
    "PrdStatus",
    "RecoveryAction",
    "RecoveryPlan",
    "PathResolver",
    "ConfigManager",
    "ProjectLinkManager",
    "LINK_FILE_NAME",
    # Resume detector exports
    "IncompleteStateInfo",
    "ResumeReason",
    "ResumeSuggestion",
    "detect_incomplete_state",
    "get_resume_suggestion",
    "format_resume_display",
    "check_and_suggest_resume",
]
