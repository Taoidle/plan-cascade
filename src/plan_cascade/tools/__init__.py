"""
Plan Cascade Tools Module

Provides the tool abstraction layer for the builtin backend.
Tools are used by the ReAct loop to interact with the file system
and execute commands.

Key Components:
- ToolRegistry: Tool registration and execution
- Tool definitions for file operations, shell commands, and search
"""

from .file_tools import (
    EDIT_FILE_TOOL,
    READ_FILE_TOOL,
    WRITE_FILE_TOOL,
    edit_file,
    read_file,
    write_file,
)
from .registry import Tool, ToolRegistry, ToolResult
from .search_tools import (
    GREP_CONTENT_TOOL,
    SEARCH_FILES_TOOL,
    grep_content,
    search_files,
)
from .shell_tools import (
    RUN_COMMAND_TOOL,
    run_command,
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
