"""
Shell Tools

Tools for executing shell commands.
"""

import asyncio
import os
import subprocess
import sys
from pathlib import Path

from .registry import Tool, ToolResult


def run_command(
    command: str,
    working_dir: str | None = None,
    timeout: int = 120,
    shell: bool = True
) -> ToolResult:
    """
    Execute a shell command.

    Args:
        command: The command to execute
        working_dir: Working directory for the command (default: current directory)
        timeout: Timeout in seconds (default: 120)
        shell: Whether to run through shell (default: True)

    Returns:
        ToolResult with command output
    """
    try:
        # Resolve working directory
        cwd = Path(working_dir).resolve() if working_dir else Path.cwd()

        if not cwd.exists():
            return ToolResult(
                success=False,
                error=f"Working directory does not exist: {cwd}"
            )

        # Prepare environment
        env = os.environ.copy()

        # Execute command
        if shell:
            # Use shell execution
            if sys.platform == "win32":
                # Windows: use cmd.exe
                result = subprocess.run(
                    command,
                    shell=True,
                    cwd=str(cwd),
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    env=env,
                )
            else:
                # Unix: use bash if available
                result = subprocess.run(
                    command,
                    shell=True,
                    executable="/bin/bash" if os.path.exists("/bin/bash") else None,
                    cwd=str(cwd),
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    env=env,
                )
        else:
            # Direct execution (split command)
            import shlex
            args = shlex.split(command)
            result = subprocess.run(
                args,
                cwd=str(cwd),
                capture_output=True,
                text=True,
                timeout=timeout,
                env=env,
            )

        # Combine stdout and stderr
        output_parts = []
        if result.stdout:
            output_parts.append(result.stdout)
        if result.stderr:
            output_parts.append(f"[stderr]\n{result.stderr}")

        output = "\n".join(output_parts)

        # Truncate if too long
        max_output = 50000
        if len(output) > max_output:
            output = output[:max_output] + f"\n... (truncated, {len(output) - max_output} more characters)"

        success = result.returncode == 0

        return ToolResult(
            success=success,
            output=output if output else "(no output)",
            error=None if success else f"Command exited with code {result.returncode}",
            metadata={
                "exit_code": result.returncode,
                "working_dir": str(cwd),
            }
        )

    except subprocess.TimeoutExpired:
        return ToolResult(
            success=False,
            error=f"Command timed out after {timeout} seconds"
        )
    except FileNotFoundError as e:
        return ToolResult(
            success=False,
            error=f"Command not found: {e}"
        )
    except PermissionError:
        return ToolResult(
            success=False,
            error="Permission denied executing command"
        )
    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error executing command: {e}"
        )


async def run_command_async(
    command: str,
    working_dir: str | None = None,
    timeout: int = 120
) -> ToolResult:
    """
    Execute a shell command asynchronously.

    Args:
        command: The command to execute
        working_dir: Working directory for the command
        timeout: Timeout in seconds

    Returns:
        ToolResult with command output
    """
    try:
        cwd = Path(working_dir).resolve() if working_dir else Path.cwd()

        if not cwd.exists():
            return ToolResult(
                success=False,
                error=f"Working directory does not exist: {cwd}"
            )

        # Create subprocess
        if sys.platform == "win32":
            process = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=str(cwd),
            )
        else:
            process = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=str(cwd),
                executable="/bin/bash" if os.path.exists("/bin/bash") else None,
            )

        try:
            stdout, stderr = await asyncio.wait_for(
                process.communicate(),
                timeout=timeout
            )
        except asyncio.TimeoutError:
            process.kill()
            await process.wait()
            return ToolResult(
                success=False,
                error=f"Command timed out after {timeout} seconds"
            )

        # Decode output
        stdout_text = stdout.decode("utf-8", errors="replace") if stdout else ""
        stderr_text = stderr.decode("utf-8", errors="replace") if stderr else ""

        # Combine output
        output_parts = []
        if stdout_text:
            output_parts.append(stdout_text)
        if stderr_text:
            output_parts.append(f"[stderr]\n{stderr_text}")

        output = "\n".join(output_parts)

        # Truncate if too long
        max_output = 50000
        if len(output) > max_output:
            output = output[:max_output] + "\n... (truncated)"

        success = process.returncode == 0

        return ToolResult(
            success=success,
            output=output if output else "(no output)",
            error=None if success else f"Command exited with code {process.returncode}",
            metadata={
                "exit_code": process.returncode,
                "working_dir": str(cwd),
            }
        )

    except Exception as e:
        return ToolResult(
            success=False,
            error=f"Error executing command: {e}"
        )


# Tool definition for the registry

RUN_COMMAND_TOOL = Tool(
    name="run_command",
    description="Execute a shell command. Returns stdout and stderr. "
                "Use for running tests, git commands, build scripts, etc.",
    parameters={
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "The shell command to execute"
            },
            "working_dir": {
                "type": "string",
                "description": "Working directory for the command (optional)"
            },
            "timeout": {
                "type": "integer",
                "description": "Timeout in seconds (default: 120)",
                "default": 120
            }
        },
        "required": ["command"]
    },
    function=run_command
)
