"""
Search Tools

Tools for searching files and content.
"""

import fnmatch
import os
import re
from pathlib import Path
from typing import Any, Dict, List, Optional

from .registry import Tool, ToolResult


def search_files(
    pattern: str,
    path: Optional[str] = None,
    max_results: int = 100,
    include_hidden: bool = False
) -> ToolResult:
    """
    Search for files matching a glob pattern.

    Args:
        pattern: Glob pattern to match (e.g., "**/*.py", "src/**/*.ts")
        path: Directory to search in (default: current directory)
        max_results: Maximum number of results to return
        include_hidden: Include hidden files/directories

    Returns:
        ToolResult with list of matching files
    """
    try:
        base_path = Path(path).resolve() if path else Path.cwd()

        if not base_path.exists():
            return ToolResult(
                success=False,
                error=f"Path does not exist: {base_path}"
            )

        if not base_path.is_dir():
            return ToolResult(
                success=False,
                error=f"Path is not a directory: {base_path}"
            )

        # Collect matching files
        matches = []
        truncated = False

        for match in base_path.glob(pattern):
            # Skip hidden files/directories if not requested
            if not include_hidden:
                parts = match.relative_to(base_path).parts
                if any(part.startswith(".") for part in parts):
                    continue

            if match.is_file():
                rel_path = str(match.relative_to(base_path))
                matches.append(rel_path)

                if len(matches) >= max_results:
                    truncated = True
                    break

        # Sort by modification time (most recent first)
        matches.sort(key=lambda p: os.path.getmtime(base_path / p), reverse=True)

        if not matches:
            return ToolResult(
                success=True,
                output="No files found matching the pattern",
                metadata={"count": 0, "pattern": pattern}
            )

        output_lines = [f"Found {len(matches)} file(s):"]
        output_lines.extend(f"  {m}" for m in matches)

        if truncated:
            output_lines.append(f"\n(Results truncated at {max_results})")

        return ToolResult(
            success=True,
            output="\n".join(output_lines),
            metadata={
                "count": len(matches),
                "pattern": pattern,
                "truncated": truncated,
                "files": matches,
            }
        )

    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error searching files: {e}"
        )


def grep_content(
    pattern: str,
    path: Optional[str] = None,
    file_pattern: str = "**/*",
    case_sensitive: bool = True,
    max_results: int = 100,
    context_lines: int = 0
) -> ToolResult:
    """
    Search for content matching a pattern in files.

    Args:
        pattern: Regex pattern to search for
        path: Directory to search in (default: current directory)
        file_pattern: Glob pattern for files to search (default: all files)
        case_sensitive: Whether search is case sensitive
        max_results: Maximum number of matches to return
        context_lines: Number of context lines before/after match

    Returns:
        ToolResult with matching lines
    """
    try:
        base_path = Path(path).resolve() if path else Path.cwd()

        if not base_path.exists():
            return ToolResult(
                success=False,
                error=f"Path does not exist: {base_path}"
            )

        # Compile regex
        flags = 0 if case_sensitive else re.IGNORECASE
        try:
            regex = re.compile(pattern, flags)
        except re.error as e:
            return ToolResult(
                success=False,
                error=f"Invalid regex pattern: {e}"
            )

        # Search files
        matches = []
        files_searched = 0
        truncated = False

        # Skip binary and large files
        skip_extensions = {
            ".pyc", ".pyo", ".so", ".dll", ".exe", ".bin",
            ".jpg", ".jpeg", ".png", ".gif", ".ico", ".pdf",
            ".zip", ".tar", ".gz", ".rar", ".7z",
            ".woff", ".woff2", ".ttf", ".eot",
        }

        for file_path in base_path.glob(file_pattern):
            if not file_path.is_file():
                continue

            # Skip binary files
            if file_path.suffix.lower() in skip_extensions:
                continue

            # Skip hidden files
            if any(part.startswith(".") for part in file_path.relative_to(base_path).parts):
                continue

            try:
                # Skip large files
                if file_path.stat().st_size > 1_000_000:  # 1MB
                    continue

                with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                    lines = f.readlines()
                    files_searched += 1

                    for i, line in enumerate(lines):
                        if regex.search(line):
                            rel_path = str(file_path.relative_to(base_path))
                            line_num = i + 1

                            # Collect context
                            context_before = []
                            context_after = []

                            if context_lines > 0:
                                start = max(0, i - context_lines)
                                end = min(len(lines), i + context_lines + 1)
                                context_before = [
                                    (j + 1, lines[j].rstrip())
                                    for j in range(start, i)
                                ]
                                context_after = [
                                    (j + 1, lines[j].rstrip())
                                    for j in range(i + 1, end)
                                ]

                            matches.append({
                                "file": rel_path,
                                "line": line_num,
                                "content": line.rstrip(),
                                "context_before": context_before,
                                "context_after": context_after,
                            })

                            if len(matches) >= max_results:
                                truncated = True
                                break

                if truncated:
                    break

            except (IOError, OSError):
                continue

        if not matches:
            return ToolResult(
                success=True,
                output=f"No matches found for pattern: {pattern}",
                metadata={
                    "count": 0,
                    "files_searched": files_searched,
                    "pattern": pattern
                }
            )

        # Format output
        output_lines = [f"Found {len(matches)} match(es) in {files_searched} file(s):"]

        current_file = None
        for match in matches:
            if match["file"] != current_file:
                current_file = match["file"]
                output_lines.append(f"\n{current_file}:")

            # Context before
            for ln, content in match.get("context_before", []):
                output_lines.append(f"  {ln:6d}  {content}")

            # Match line (highlighted)
            output_lines.append(f"  {match['line']:6d}> {match['content']}")

            # Context after
            for ln, content in match.get("context_after", []):
                output_lines.append(f"  {ln:6d}  {content}")

        if truncated:
            output_lines.append(f"\n(Results truncated at {max_results})")

        return ToolResult(
            success=True,
            output="\n".join(output_lines),
            metadata={
                "count": len(matches),
                "files_searched": files_searched,
                "pattern": pattern,
                "truncated": truncated,
            }
        )

    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error searching content: {e}"
        )


# Tool definitions for the registry

SEARCH_FILES_TOOL = Tool(
    name="search_files",
    description="Search for files matching a glob pattern. "
                "Use patterns like '**/*.py' for Python files, 'src/**/*.ts' for TypeScript in src.",
    parameters={
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Glob pattern to match files (e.g., '**/*.py', 'src/**/*.ts')"
            },
            "path": {
                "type": "string",
                "description": "Directory to search in (default: current directory)"
            },
            "max_results": {
                "type": "integer",
                "description": "Maximum number of results (default: 100)",
                "default": 100
            }
        },
        "required": ["pattern"]
    },
    function=search_files
)

GREP_CONTENT_TOOL = Tool(
    name="grep_content",
    description="Search for content matching a regex pattern in files. "
                "Returns matching lines with file locations.",
    parameters={
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Regex pattern to search for"
            },
            "path": {
                "type": "string",
                "description": "Directory to search in (default: current directory)"
            },
            "file_pattern": {
                "type": "string",
                "description": "Glob pattern for files to search (default: '**/*')",
                "default": "**/*"
            },
            "case_sensitive": {
                "type": "boolean",
                "description": "Whether search is case sensitive (default: true)",
                "default": True
            },
            "context_lines": {
                "type": "integer",
                "description": "Number of context lines before/after match (default: 0)",
                "default": 0
            }
        },
        "required": ["pattern"]
    },
    function=grep_content
)
