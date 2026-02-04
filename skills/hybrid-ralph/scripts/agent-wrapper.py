#!/usr/bin/env python3
"""
Agent Wrapper for Plan Cascade Multi-Agent Collaboration

This script wraps external CLI agents (codex, amp-code, aider, etc.) to:
1. Receive story context and prompt
2. Execute the CLI agent as a subprocess
3. Monitor process execution and capture output
4. Update status files (.agent-status.json, progress.txt) on completion
5. Handle timeouts and errors gracefully

Usage:
    python agent-wrapper.py --story-id <id> --agent <name> --config <path> [--timeout <seconds>]

The wrapper reads the prompt from stdin or a prompt file, executes the agent,
and updates status files based on the result.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, Optional


# Try to use PathResolver and FileLock from plan_cascade for consistency
_PATH_RESOLVER_AVAILABLE = False
_FILE_LOCK_AVAILABLE = False
_PathResolver = None
_FileLock = None


def _setup_import_path():
    """Setup path to import plan_cascade modules."""
    global _PATH_RESOLVER_AVAILABLE, _FILE_LOCK_AVAILABLE, _PathResolver, _FileLock

    # Find the plugin root
    candidates = [
        Path(__file__).parent.parent.parent.parent / "src",  # Standard location
        Path.cwd() / "src",  # Development mode
    ]
    for src_path in candidates:
        if src_path.exists():
            sys.path.insert(0, str(src_path))
            try:
                from plan_cascade.state.path_resolver import PathResolver
                _PathResolver = PathResolver
                _PATH_RESOLVER_AVAILABLE = True
            except ImportError:
                pass
            try:
                from plan_cascade.state.state_manager import FileLock
                _FileLock = FileLock
                _FILE_LOCK_AVAILABLE = True
            except ImportError:
                pass
            break

_setup_import_path()


class AgentWrapper:
    """Wraps CLI agent execution with status tracking."""

    def __init__(
        self,
        story_id: str,
        agent_name: str,
        project_root: Path,
        config_path: Optional[Path] = None,
        timeout: int = 600
    ):
        self.story_id = story_id
        self.agent_name = agent_name
        self.project_root = Path(project_root)
        self.timeout = timeout

        # Initialize PathResolver if available
        self._path_resolver = None
        if _PATH_RESOLVER_AVAILABLE and _PathResolver:
            self._path_resolver = _PathResolver(self.project_root, legacy_mode=True)

        # Set up paths - use PathResolver if available, fallback to legacy
        if self._path_resolver and not self._path_resolver.is_legacy_mode():
            # New mode: internal files in ~/.plan-cascade/<project-id>/.state/
            state_dir = self._path_resolver.get_state_dir()
            self.agent_status_path = self._path_resolver.get_state_file_path("agent-status.json")
            self.output_dir = state_dir / "agent-outputs"
        else:
            # Legacy mode: match StateManager legacy layout (project root)
            self.agent_status_path = self.project_root / ".agent-status.json"
            self.output_dir = self.project_root / ".agent-outputs"

        # Progress file always in project root (user-visible)
        self.progress_path = self.project_root / "progress.txt"
        self.output_file = self.output_dir / f"{story_id}.log"
        self.result_file = self.output_dir / f"{story_id}.result.json"

        # Ensure directories exist
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.agent_status_path.parent.mkdir(parents=True, exist_ok=True)

        # Load agent config
        self.config = self._load_config(config_path)
        self.agent_config = self.config.get("agents", {}).get(agent_name, {})

        if not self.agent_config:
            raise ValueError(f"Agent '{agent_name}' not found in configuration")

        if self.agent_config.get("type") != "cli":
            raise ValueError(f"Agent '{agent_name}' is not a CLI agent")

    def _load_config(self, config_path: Optional[Path]) -> Dict:
        """Load agent configuration."""
        if config_path and config_path.exists():
            with open(config_path, "r", encoding="utf-8") as f:
                return json.load(f)

        # Try default location
        default_path = self.project_root / "agents.json"
        if default_path.exists():
            with open(default_path, "r", encoding="utf-8") as f:
                return json.load(f)

        return {"agents": {}}

    def _timestamp(self) -> str:
        """Get ISO timestamp."""
        return datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    def _read_status(self) -> Dict:
        """Read .agent-status.json."""
        if not self.agent_status_path.exists():
            return {"running": [], "completed": [], "failed": []}

        try:
            with open(self.agent_status_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return {"running": [], "completed": [], "failed": []}

    def _write_status(self, status: Dict) -> None:
        """Write .agent-status.json with file locking for concurrent safety."""
        status["updated_at"] = self._timestamp()

        max_retries = 5
        for attempt in range(max_retries):
            try:
                # Create parent directory if needed
                self.agent_status_path.parent.mkdir(parents=True, exist_ok=True)

                # Use unified FileLock from StateManager if available
                if _FILE_LOCK_AVAILABLE and _FileLock:
                    if self._path_resolver:
                        lock_dir = self._path_resolver.get_locks_dir()
                    else:
                        lock_dir = self.agent_status_path.parent / ".locks"
                    lock_dir.mkdir(parents=True, exist_ok=True)
                    lock_path = lock_dir / f"{self.agent_status_path.stem}.lock"

                    with _FileLock(lock_path, timeout=30):
                        self._write_status_unlocked(status)
                    return  # Success

                # Fallback to inline locking
                mode = "r+" if self.agent_status_path.exists() else "w"
                with open(self.agent_status_path, mode, encoding="utf-8") as f:
                    # Acquire exclusive lock
                    if sys.platform != "win32":
                        import fcntl
                        fcntl.flock(f.fileno(), fcntl.LOCK_EX)
                    else:
                        import msvcrt
                        # Use LK_LOCK (blocking) for consistency with Unix
                        msvcrt.locking(f.fileno(), msvcrt.LK_LOCK, 1)

                    try:
                        # Read existing content if file existed
                        if mode == "r+":
                            f.seek(0)
                            try:
                                current = json.load(f)
                            except json.JSONDecodeError:
                                current = {"running": [], "completed": [], "failed": []}
                        else:
                            current = {"running": [], "completed": [], "failed": []}

                        # Merge status into current
                        for key in ["running", "completed", "failed"]:
                            if key in status:
                                current[key] = status[key]
                        current["updated_at"] = status["updated_at"]

                        # Write back
                        f.seek(0)
                        f.truncate()
                        json.dump(current, f, indent=2)

                    finally:
                        # Release lock
                        if sys.platform != "win32":
                            import fcntl
                            fcntl.flock(f.fileno(), fcntl.LOCK_UN)
                        else:
                            import msvcrt
                            try:
                                f.seek(0)
                                msvcrt.locking(f.fileno(), msvcrt.LK_UNLCK, 1)
                            except Exception:
                                pass  # Ignore unlock errors on Windows

                return  # Success

            except (IOError, BlockingIOError, PermissionError) as e:
                if attempt < max_retries - 1:
                    time.sleep(0.1 * (attempt + 1))  # Exponential backoff
                else:
                    print(f"[Warning] Could not write status after {max_retries} attempts: {e}",
                          file=sys.stderr)

    def _write_status_unlocked(self, status: Dict) -> None:
        """Write status file without locking (caller must hold lock)."""
        current = {"running": [], "completed": [], "failed": []}
        if self.agent_status_path.exists():
            try:
                with open(self.agent_status_path, "r", encoding="utf-8") as f:
                    current = json.load(f)
            except (json.JSONDecodeError, IOError):
                pass

        # Merge status into current
        for key in ["running", "completed", "failed"]:
            if key in status:
                current[key] = status[key]
        current["updated_at"] = status["updated_at"]

        with open(self.agent_status_path, "w", encoding="utf-8") as f:
            json.dump(current, f, indent=2)

    def _append_progress(self, message: str) -> None:
        """Append to progress.txt."""
        try:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            with open(self.progress_path, "a", encoding="utf-8") as f:
                f.write(f"[{timestamp}] {self.story_id}: {message}\n")
        except IOError as e:
            print(f"[Warning] Could not write progress: {e}", file=sys.stderr)

    def _record_start(self, pid: int) -> None:
        """Record agent start."""
        status = self._read_status()

        # Remove any existing entry for this story
        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != self.story_id
        ]

        # Add new entry
        status["running"].append({
            "story_id": self.story_id,
            "agent": self.agent_name,
            "pid": pid,
            "started_at": self._timestamp(),
            "output_file": str(self.output_file),
            "timeout": self.timeout
        })

        self._write_status(status)
        self._append_progress(f"[START] via {self.agent_name} (pid:{pid})")

    def _record_complete(self, exit_code: int, duration: float) -> None:
        """Record agent completion."""
        status = self._read_status()

        # Find and remove from running
        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == self.story_id:
                running_entry = entry
                break

        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != self.story_id
        ]

        # Add to completed
        entry = {
            "story_id": self.story_id,
            "agent": self.agent_name,
            "exit_code": exit_code,
            "completed_at": self._timestamp(),
            "duration_seconds": round(duration, 2),
            "output_file": str(self.output_file),
            "result_file": str(self.result_file)
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        if "completed" not in status:
            status["completed"] = []
        status["completed"].append(entry)

        self._write_status(status)
        self._append_progress(f"[COMPLETE] via {self.agent_name} (exit:{exit_code}, duration:{duration:.1f}s)")

    def _record_failure(self, error: str, duration: float) -> None:
        """Record agent failure."""
        status = self._read_status()

        # Find and remove from running
        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == self.story_id:
                running_entry = entry
                break

        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != self.story_id
        ]

        # Add to failed
        entry = {
            "story_id": self.story_id,
            "agent": self.agent_name,
            "error": error,
            "failed_at": self._timestamp(),
            "duration_seconds": round(duration, 2),
            "output_file": str(self.output_file)
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        if "failed" not in status:
            status["failed"] = []
        status["failed"].append(entry)

        self._write_status(status)
        self._append_progress(f"[FAILED] via {self.agent_name}: {error}")

    def _write_result(self, success: bool, exit_code: int, error: Optional[str] = None) -> None:
        """Write result JSON file for the main session to read."""
        result = {
            "story_id": self.story_id,
            "agent": self.agent_name,
            "success": success,
            "exit_code": exit_code,
            "completed_at": self._timestamp(),
            "output_file": str(self.output_file)
        }
        if error:
            result["error"] = error

        try:
            with open(self.result_file, "w", encoding="utf-8") as f:
                json.dump(result, f, indent=2)
        except IOError as e:
            print(f"[Warning] Could not write result: {e}", file=sys.stderr)

    def _build_command(self, prompt: str) -> list:
        """Build the CLI command with substitutions."""
        command = self.agent_config.get("command", "")
        args_template = self.agent_config.get("args", [])

        args = []
        for arg in args_template:
            if isinstance(arg, str):
                arg = arg.replace("{prompt}", prompt)
                arg = arg.replace("{working_dir}", str(self.project_root))
                arg = arg.replace("{story_id}", self.story_id)
            args.append(arg)

        return [command] + args

    def execute(self, prompt: str) -> int:
        """
        Execute the CLI agent with the given prompt.

        Args:
            prompt: The prompt/instructions for the agent

        Returns:
            Exit code (0 = success, non-zero = failure)
        """
        # Ensure output directory exists
        self.output_dir.mkdir(exist_ok=True)

        # Build command
        cmd = self._build_command(prompt)
        command_str = cmd[0]

        # Prepare environment
        env = os.environ.copy()
        env.update(self.agent_config.get("env", {}))

        # Write prompt to a temp file for agents that need file input
        prompt_file = self.output_dir / f"{self.story_id}.prompt.txt"
        with open(prompt_file, "w", encoding="utf-8") as f:
            f.write(prompt)

        start_time = time.time()

        try:
            # Open output file
            with open(self.output_file, "w", encoding="utf-8") as log_file:
                # Write header
                log_file.write(f"# Agent Wrapper Log\n")
                log_file.write(f"# Story: {self.story_id}\n")
                log_file.write(f"# Agent: {self.agent_name}\n")
                log_file.write(f"# Command: {command_str}\n")
                log_file.write(f"# Started: {self._timestamp()}\n")
                log_file.write(f"# Timeout: {self.timeout}s\n")
                log_file.write("-" * 60 + "\n\n")
                log_file.flush()

                # Start process
                process = subprocess.Popen(
                    cmd,
                    cwd=str(self.project_root),
                    stdout=log_file,
                    stderr=subprocess.STDOUT,
                    stdin=subprocess.PIPE,
                    env=env,
                    # Platform-specific: don't create console window on Windows
                    creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
                )

                # Record start
                self._record_start(process.pid)

                # Wait for completion with timeout
                try:
                    # Some agents might read from stdin
                    stdout, _ = process.communicate(
                        input=prompt.encode("utf-8"),
                        timeout=self.timeout
                    )
                    exit_code = process.returncode
                except subprocess.TimeoutExpired:
                    # Kill on timeout
                    process.kill()
                    process.wait()
                    duration = time.time() - start_time
                    error = f"Timeout after {self.timeout}s"
                    log_file.write(f"\n\n[TIMEOUT] {error}\n")
                    self._record_failure(error, duration)
                    self._write_result(False, -1, error)
                    return 1

            # Process completed
            duration = time.time() - start_time

            # Append completion info to log
            with open(self.output_file, "a", encoding="utf-8") as log_file:
                log_file.write(f"\n\n" + "-" * 60 + "\n")
                log_file.write(f"# Completed: {self._timestamp()}\n")
                log_file.write(f"# Exit Code: {exit_code}\n")
                log_file.write(f"# Duration: {duration:.2f}s\n")

            if exit_code == 0:
                self._record_complete(exit_code, duration)
                self._write_result(True, exit_code)
                return 0
            else:
                error = f"Exit code {exit_code}"
                self._record_failure(error, duration)
                self._write_result(False, exit_code, error)
                return exit_code

        except FileNotFoundError:
            duration = time.time() - start_time
            error = f"Command '{command_str}' not found"
            self._record_failure(error, duration)
            self._write_result(False, -1, error)
            return 1

        except Exception as e:
            duration = time.time() - start_time
            error = str(e)
            self._record_failure(error, duration)
            self._write_result(False, -1, error)
            return 1


def main():
    parser = argparse.ArgumentParser(
        description="Agent Wrapper for Plan Cascade",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Execute with prompt from stdin
  echo "Implement feature X" | python agent-wrapper.py --story-id story-001 --agent codex

  # Execute with prompt from file
  python agent-wrapper.py --story-id story-001 --agent codex --prompt-file prompt.txt

  # Execute with inline prompt
  python agent-wrapper.py --story-id story-001 --agent codex --prompt "Implement feature X"
        """
    )

    parser.add_argument("--story-id", required=True, help="Story ID (e.g., story-001)")
    parser.add_argument("--agent", required=True, help="Agent name (e.g., codex, amp-code)")
    parser.add_argument("--project-root", default=".", help="Project root directory")
    parser.add_argument("--config", help="Path to agents.json config file")
    parser.add_argument("--timeout", type=int, default=600, help="Timeout in seconds (default: 600)")
    parser.add_argument("--prompt", help="Prompt string (alternative to stdin)")
    parser.add_argument("--prompt-file", help="Path to prompt file (alternative to stdin)")

    args = parser.parse_args()

    # Get prompt
    if args.prompt:
        prompt = args.prompt
    elif args.prompt_file:
        with open(args.prompt_file, "r", encoding="utf-8") as f:
            prompt = f.read()
    else:
        # Read from stdin
        prompt = sys.stdin.read()

    if not prompt.strip():
        print("Error: No prompt provided", file=sys.stderr)
        sys.exit(1)

    # Create wrapper and execute
    try:
        wrapper = AgentWrapper(
            story_id=args.story_id,
            agent_name=args.agent,
            project_root=Path(args.project_root).resolve(),
            config_path=Path(args.config) if args.config else None,
            timeout=args.timeout
        )

        exit_code = wrapper.execute(prompt)
        sys.exit(exit_code)

    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
