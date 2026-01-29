"""
Plan Cascade Tools Module

Provides the tool abstraction layer for the builtin backend.
Tools are used by the ReAct loop to interact with the file system
and execute commands.

Key Components:
- ToolRegistry: Tool registration and execution
- Tool definitions for file operations, shell commands, and search
"""

from .registry import ToolRegistry, ToolResult, Tool
from .file_tools import (
    read_file,
    write_file,
    edit_file,
    READ_FILE_TOOL,
    WRITE_FILE_TOOL,
    EDIT_FILE_TOOL,
)
from .shell_tools import (
    run_command,
    RUN_COMMAND_TOOL,
)
from .search_tools import (
    search_files,
    grep_content,
    SEARCH_FILES_TOOL,
    GREP_CONTENT_TOOL,
)

__all__ = [
    # Registry
    "ToolRegistry",
    "ToolResult",
    "Tool",
    # File tools
    "read_file",
    "write_file",
    "edit_file",
    "READ_FILE_TOOL",
    "WRITE_FILE_TOOL",
    "EDIT_FILE_TOOL",
    # Shell tools
    "run_command",
    "RUN_COMMAND_TOOL",
    # Search tools
    "search_files",
    "grep_content",
    "SEARCH_FILES_TOOL",
    "GREP_CONTENT_TOOL",
]
