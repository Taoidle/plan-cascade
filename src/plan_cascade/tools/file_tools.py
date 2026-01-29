"""
File Tools

Tools for reading, writing, and editing files.
"""

import os
from pathlib import Path
from typing import Any, Dict, Optional

from .registry import Tool, ToolResult


def read_file(
    file_path: str,
    offset: Optional[int] = None,
    limit: Optional[int] = None,
    encoding: str = "utf-8"
) -> ToolResult:
    """
    Read contents of a file.

    Args:
        file_path: Path to the file (absolute or relative)
        offset: Line number to start reading from (0-based)
        limit: Maximum number of lines to read
        encoding: File encoding (default: utf-8)

    Returns:
        ToolResult with file contents or error
    """
    try:
        path = Path(file_path)

        if not path.exists():
            return ToolResult(
                success=False,
                error=f"File not found: {file_path}"
            )

        if not path.is_file():
            return ToolResult(
                success=False,
                error=f"Not a file: {file_path}"
            )

        # Read file
        with open(path, "r", encoding=encoding) as f:
            if offset is not None or limit is not None:
                lines = f.readlines()
                start = offset or 0
                end = (start + limit) if limit else len(lines)
                content_lines = lines[start:end]

                # Add line numbers
                numbered_lines = [
                    f"{start + i + 1:6d}\t{line.rstrip()}"
                    for i, line in enumerate(content_lines)
                ]
                content = "\n".join(numbered_lines)

                return ToolResult(
                    success=True,
                    output=content,
                    metadata={
                        "total_lines": len(lines),
                        "start_line": start + 1,
                        "end_line": min(end, len(lines)),
                    }
                )
            else:
                content = f.read()
                lines = content.split("\n")

                # Add line numbers
                numbered_lines = [
                    f"{i + 1:6d}\t{line}"
                    for i, line in enumerate(lines)
                ]
                content = "\n".join(numbered_lines)

                return ToolResult(
                    success=True,
                    output=content,
                    metadata={"total_lines": len(lines)}
                )

    except UnicodeDecodeError:
        return ToolResult(
            success=False,
            error=f"Cannot decode file with {encoding} encoding: {file_path}"
        )
    except PermissionError:
        return ToolResult(
            success=False,
            error=f"Permission denied: {file_path}"
        )
    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error reading file: {e}"
        )


def write_file(
    file_path: str,
    content: str,
    encoding: str = "utf-8",
    create_dirs: bool = True
) -> ToolResult:
    """
    Write content to a file (creates or overwrites).

    Args:
        file_path: Path to the file
        content: Content to write
        encoding: File encoding (default: utf-8)
        create_dirs: Create parent directories if they don't exist

    Returns:
        ToolResult indicating success or failure
    """
    try:
        path = Path(file_path)

        # Create parent directories if needed
        if create_dirs and not path.parent.exists():
            path.parent.mkdir(parents=True, exist_ok=True)

        # Write file
        with open(path, "w", encoding=encoding) as f:
            f.write(content)

        lines = content.count("\n") + (1 if content and not content.endswith("\n") else 0)

        return ToolResult(
            success=True,
            output=f"Successfully wrote {len(content)} characters ({lines} lines) to {file_path}",
            metadata={
                "bytes_written": len(content.encode(encoding)),
                "lines": lines,
            }
        )

    except PermissionError:
        return ToolResult(
            success=False,
            error=f"Permission denied: {file_path}"
        )
    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error writing file: {e}"
        )


def edit_file(
    file_path: str,
    old_string: str,
    new_string: str,
    replace_all: bool = False,
    encoding: str = "utf-8"
) -> ToolResult:
    """
    Edit a file by replacing old_string with new_string.

    Args:
        file_path: Path to the file
        old_string: String to find and replace
        new_string: String to replace with
        replace_all: Replace all occurrences (default: first only)
        encoding: File encoding (default: utf-8)

    Returns:
        ToolResult indicating success or failure
    """
    try:
        path = Path(file_path)

        if not path.exists():
            return ToolResult(
                success=False,
                error=f"File not found: {file_path}"
            )

        # Read current content
        with open(path, "r", encoding=encoding) as f:
            content = f.read()

        # Check if old_string exists
        if old_string not in content:
            return ToolResult(
                success=False,
                error=f"String not found in file: {repr(old_string[:100])}"
            )

        # Count occurrences
        occurrences = content.count(old_string)

        if occurrences > 1 and not replace_all:
            # Check if it's unique enough
            return ToolResult(
                success=False,
                error=f"String found {occurrences} times. Use replace_all=true to replace all, "
                      f"or provide a more unique string."
            )

        # Perform replacement
        if replace_all:
            new_content = content.replace(old_string, new_string)
            replaced_count = occurrences
        else:
            new_content = content.replace(old_string, new_string, 1)
            replaced_count = 1

        # Write back
        with open(path, "w", encoding=encoding) as f:
            f.write(new_content)

        return ToolResult(
            success=True,
            output=f"Successfully replaced {replaced_count} occurrence(s) in {file_path}",
            metadata={
                "replacements": replaced_count,
                "old_length": len(old_string),
                "new_length": len(new_string),
            }
        )

    except PermissionError:
        return ToolResult(
            success=False,
            error=f"Permission denied: {file_path}"
        )
    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error editing file: {e}"
        )


# Tool definitions for the registry

READ_FILE_TOOL = Tool(
    name="read_file",
    description="Read the contents of a file. Returns the file content with line numbers.",
    parameters={
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "The path to the file to read (absolute or relative)"
            },
            "offset": {
                "type": "integer",
                "description": "Line number to start reading from (0-based)"
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of lines to read"
            }
        },
        "required": ["file_path"]
    },
    function=read_file
)

WRITE_FILE_TOOL = Tool(
    name="write_file",
    description="Create a new file or completely overwrite an existing file with new content.",
    parameters={
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "The path where to write the file"
            },
            "content": {
                "type": "string",
                "description": "The content to write to the file"
            }
        },
        "required": ["file_path", "content"]
    },
    function=write_file
)

EDIT_FILE_TOOL = Tool(
    name="edit_file",
    description="Edit a file by replacing a specific string with new content. "
                "The old_string must be unique in the file unless replace_all is true.",
    parameters={
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "The path to the file to edit"
            },
            "old_string": {
                "type": "string",
                "description": "The exact string to find and replace"
            },
            "new_string": {
                "type": "string",
                "description": "The string to replace old_string with"
            },
            "replace_all": {
                "type": "boolean",
                "description": "Replace all occurrences instead of just the first",
                "default": False
            }
        },
        "required": ["file_path", "old_string", "new_string"]
    },
    function=edit_file
)
