"""
Tool Registry

Provides registration, management, and execution of tools for the builtin backend.
"""

from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Union
import asyncio
import traceback


@dataclass
class ToolResult:
    """
    Result from tool execution.

    Attributes:
        success: Whether the tool executed successfully
        output: Output from the tool (string or structured data)
        error: Error message if execution failed
        metadata: Additional execution metadata
    """
    success: bool
    output: Any = None
    error: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_string(self) -> str:
        """Convert result to string for LLM consumption."""
        if self.success:
            if isinstance(self.output, str):
                return self.output
            elif self.output is not None:
                import json
                try:
                    return json.dumps(self.output, indent=2)
                except (TypeError, ValueError):
                    return str(self.output)
            return "Success"
        else:
            return f"Error: {self.error}"


@dataclass
class Tool:
    """
    Tool definition with execution function.

    Attributes:
        name: Unique tool name
        description: Human-readable description
        parameters: JSON Schema for tool parameters
        function: The callable to execute
        is_async: Whether the function is async
    """
    name: str
    description: str
    parameters: Dict[str, Any]
    function: Callable
    is_async: bool = False

    def get_definition(self) -> Dict[str, Any]:
        """Get tool definition for LLM."""
        return {
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        }


class ToolRegistry:
    """
    Registry for tool management and execution.

    Provides:
    - Tool registration with validation
    - Tool execution with error handling
    - Tool definition export for LLM
    - Built-in tools for common operations

    Example:
        registry = ToolRegistry()

        # Register a custom tool
        registry.register(Tool(
            name="my_tool",
            description="Does something",
            parameters={"type": "object", "properties": {...}},
            function=my_function
        ))

        # Execute a tool
        result = await registry.execute("my_tool", arg1="value")

        # Get definitions for LLM
        definitions = registry.get_definitions()
    """

    def __init__(self, include_defaults: bool = True):
        """
        Initialize the tool registry.

        Args:
            include_defaults: Whether to include default file/shell tools
        """
        self._tools: Dict[str, Tool] = {}

        if include_defaults:
            self._register_default_tools()

    def _register_default_tools(self) -> None:
        """Register the default set of tools."""
        from .file_tools import READ_FILE_TOOL, WRITE_FILE_TOOL, EDIT_FILE_TOOL
        from .shell_tools import RUN_COMMAND_TOOL
        from .search_tools import SEARCH_FILES_TOOL, GREP_CONTENT_TOOL

        for tool in [
            READ_FILE_TOOL,
            WRITE_FILE_TOOL,
            EDIT_FILE_TOOL,
            RUN_COMMAND_TOOL,
            SEARCH_FILES_TOOL,
            GREP_CONTENT_TOOL,
        ]:
            self._tools[tool.name] = tool

    def register(self, tool: Tool) -> None:
        """
        Register a tool.

        Args:
            tool: Tool to register

        Raises:
            ValueError: If tool name is invalid or already registered
        """
        if not tool.name:
            raise ValueError("Tool name cannot be empty")

        if not tool.function:
            raise ValueError(f"Tool '{tool.name}' must have a function")

        self._tools[tool.name] = tool

    def unregister(self, name: str) -> None:
        """
        Unregister a tool.

        Args:
            name: Tool name to unregister
        """
        self._tools.pop(name, None)

    def get(self, name: str) -> Optional[Tool]:
        """
        Get a tool by name.

        Args:
            name: Tool name

        Returns:
            Tool if found, None otherwise
        """
        return self._tools.get(name)

    def has(self, name: str) -> bool:
        """
        Check if a tool is registered.

        Args:
            name: Tool name

        Returns:
            True if tool exists
        """
        return name in self._tools

    def list_tools(self) -> List[str]:
        """
        List all registered tool names.

        Returns:
            List of tool names
        """
        return list(self._tools.keys())

    async def execute(self, name: str, **kwargs: Any) -> ToolResult:
        """
        Execute a tool by name.

        Args:
            name: Tool name
            **kwargs: Tool arguments

        Returns:
            ToolResult with execution outcome
        """
        tool = self._tools.get(name)
        if not tool:
            return ToolResult(
                success=False,
                error=f"Unknown tool: {name}"
            )

        try:
            # Execute the tool function
            if tool.is_async or asyncio.iscoroutinefunction(tool.function):
                result = await tool.function(**kwargs)
            else:
                # Run sync function in executor to avoid blocking
                loop = asyncio.get_event_loop()
                result = await loop.run_in_executor(
                    None,
                    lambda: tool.function(**kwargs)
                )

            # Normalize result
            if isinstance(result, ToolResult):
                return result
            elif isinstance(result, dict) and "success" in result:
                return ToolResult(
                    success=result.get("success", True),
                    output=result.get("output"),
                    error=result.get("error"),
                    metadata=result.get("metadata", {})
                )
            else:
                return ToolResult(success=True, output=result)

        except Exception as e:
            return ToolResult(
                success=False,
                error=f"{type(e).__name__}: {str(e)}",
                metadata={"traceback": traceback.format_exc()}
            )

    def get_definitions(self) -> List[Dict[str, Any]]:
        """
        Get tool definitions for LLM.

        Returns:
            List of tool definition dictionaries
        """
        return [tool.get_definition() for tool in self._tools.values()]

    def get_definition(self, name: str) -> Optional[Dict[str, Any]]:
        """
        Get a single tool definition.

        Args:
            name: Tool name

        Returns:
            Tool definition or None
        """
        tool = self._tools.get(name)
        return tool.get_definition() if tool else None
