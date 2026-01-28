#!/usr/bin/env python3
"""
Agent Executor for Plan Cascade Multi-Agent Collaboration

Provides an abstraction layer for executing stories using different agents:
- claude-code (Task tool): Built-in, always available
- CLI agents (codex, amp-code, aider, cursor-cli): External CLI tools

Features:
- Automatic fallback to claude-code if CLI agent is unavailable
- Process management for CLI agents
- Agent status tracking via .agent-status.json
- Enhanced progress logging with agent info
"""

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple

# Import state manager for logging
try:
    from state_manager import StateManager
except ImportError:
    # Allow running standalone
    StateManager = None


class AgentExecutor:
    """
    Agent execution abstraction layer with automatic fallback.

    Supports two types of agents:
    - task-tool: Uses Claude Code's built-in Task tool
    - cli: Executes external CLI tools (codex, amp, aider, etc.)

    Automatically falls back to claude-code when:
    - Requested agent is not configured
    - CLI command is not found in PATH
    """

    # Default agent configuration
    DEFAULT_AGENTS = {
        "claude-code": {
            "type": "task-tool",
            "description": "Claude Code Task tool (built-in)",
            "subagent_type": "general-purpose"
        }
    }

    def __init__(
        self,
        project_root: Path,
        agents_config: Optional[Dict] = None,
        config_path: Optional[Path] = None
    ):
        """
        Initialize the AgentExecutor.

        Args:
            project_root: Root directory of the project
            agents_config: Direct agent configuration dict (optional)
            config_path: Path to agents.json file (optional, defaults to project_root)
        """
        self.project_root = Path(project_root)
        self.default_agent = "claude-code"
        self.agents = self.DEFAULT_AGENTS.copy()
        self.state_manager = StateManager(project_root) if StateManager else None

        # Agent status tracking
        self.agent_status_path = self.project_root / ".agent-status.json"

        # Load configuration
        if agents_config:
            self._load_config(agents_config)
        elif config_path:
            self._load_config_file(config_path)
        else:
            # Try default locations
            default_config = self.project_root / "agents.json"
            if default_config.exists():
                self._load_config_file(default_config)

    def _load_config(self, config: Dict) -> None:
        """Load configuration from a dictionary."""
        if "default_agent" in config:
            self.default_agent = config["default_agent"]

        if "agents" in config:
            self.agents.update(config["agents"])

    def _load_config_file(self, config_path: Path) -> None:
        """Load configuration from a JSON file."""
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                config = json.load(f)
                self._load_config(config)
        except (json.JSONDecodeError, IOError) as e:
            print(f"[Warning] Could not load agent config from {config_path}: {e}")

    def _resolve_agent(
        self,
        agent_name: Optional[str] = None,
        story_agent: Optional[str] = None,
        prd_agent: Optional[str] = None
    ) -> Tuple[str, Dict]:
        """
        Resolve agent with automatic fallback to claude-code.

        Priority order:
        1. agent_name parameter (explicit override)
        2. story_agent from story metadata
        3. prd_agent from PRD metadata
        4. default_agent from config
        5. "claude-code" as ultimate fallback

        Args:
            agent_name: Explicit agent name override
            story_agent: Agent specified in story metadata
            prd_agent: Default agent from PRD metadata

        Returns:
            Tuple of (resolved_agent_name, agent_config)
        """
        # Priority chain
        name = agent_name or story_agent or prd_agent or self.default_agent

        # claude-code is always available
        if name == "claude-code":
            return name, self.agents["claude-code"]

        # Check if agent is configured
        if name not in self.agents:
            self._log_fallback(name, "not configured")
            return "claude-code", self.agents["claude-code"]

        agent = self.agents[name]

        # For CLI agents, check if command is available
        if agent.get("type") == "cli":
            command = agent.get("command", "")
            if not self._check_cli_available(command):
                self._log_fallback(name, f"CLI '{command}' not found in PATH")
                return "claude-code", self.agents["claude-code"]

        return name, agent

    def _check_cli_available(self, command: str) -> bool:
        """Check if a CLI command is available in PATH."""
        return shutil.which(command) is not None

    def _log_fallback(self, agent_name: str, reason: str) -> None:
        """Log a fallback event."""
        message = f"[AgentExecutor] Agent '{agent_name}' unavailable ({reason}), falling back to claude-code"
        print(message)
        if self.state_manager:
            self.state_manager.append_progress(
                f"[FALLBACK] {agent_name} -> claude-code: {reason}"
            )

    def execute_story(
        self,
        story: Dict,
        context: Dict,
        agent_name: Optional[str] = None,
        prd_metadata: Optional[Dict] = None,
        task_callback: Optional[Callable] = None
    ) -> Dict[str, Any]:
        """
        Execute a story using the specified agent (with fallback).

        Args:
            story: Story dictionary with id, title, description, etc.
            context: Context for the story (dependencies, findings)
            agent_name: Optional explicit agent override
            prd_metadata: Optional PRD metadata for agent defaults
            task_callback: Callback for Task tool execution (for claude-code)

        Returns:
            Execution result dictionary
        """
        story_id = story.get("id", "unknown")

        # Resolve agent with priority chain
        story_agent = story.get("agent")
        prd_agent = (prd_metadata or {}).get("default_agent")

        resolved_name, agent_config = self._resolve_agent(
            agent_name=agent_name,
            story_agent=story_agent,
            prd_agent=prd_agent
        )

        # Build prompt
        prompt = self._build_story_prompt(story, context)

        # Execute based on agent type
        if agent_config.get("type") == "task-tool":
            return self._execute_via_task_tool(
                story_id=story_id,
                prompt=prompt,
                agent_config=agent_config,
                task_callback=task_callback
            )
        elif agent_config.get("type") == "cli":
            return self._execute_via_cli(
                story_id=story_id,
                prompt=prompt,
                agent_name=resolved_name,
                agent_config=agent_config,
                working_dir=str(self.project_root)
            )
        else:
            return {
                "success": False,
                "story_id": story_id,
                "error": f"Unknown agent type: {agent_config.get('type')}"
            }

    def _build_story_prompt(self, story: Dict, context: Dict) -> str:
        """Build the execution prompt for a story."""
        story_id = story.get("id", "unknown")
        title = story.get("title", "")
        description = story.get("description", "")
        acceptance_criteria = story.get("acceptance_criteria", [])

        # Format acceptance criteria
        ac_lines = "\n".join(f"- {c}" for c in acceptance_criteria)

        # Format dependencies
        deps = context.get("dependencies", [])
        if deps:
            dep_lines = "\n".join(
                f"  - {d.get('id')}: {d.get('title', '')} (status: {d.get('status', 'unknown')})"
                for d in deps
            )
        else:
            dep_lines = "  None"

        # Format findings
        findings = context.get("findings", [])
        if findings:
            # Take first 5 findings to avoid context overflow
            finding_lines = "\n".join(findings[:5])
        else:
            finding_lines = "  No previous findings"

        prompt = f"""You are executing story {story_id}: {title}

Description:
{description}

Acceptance Criteria:
{ac_lines}

Dependencies Summary:
{dep_lines}

Relevant Findings:
{finding_lines}

Your task:
1. Read the relevant code and documentation
2. Implement the story according to acceptance criteria
3. Test your implementation
4. Update findings.md with any discoveries (tag with <!-- @tags: {story_id} -->)
5. Mark as complete by appending to progress.txt: [COMPLETE] {story_id}

Work methodically and document your progress.
"""
        return prompt

    def _execute_via_task_tool(
        self,
        story_id: str,
        prompt: str,
        agent_config: Dict,
        task_callback: Optional[Callable] = None
    ) -> Dict[str, Any]:
        """
        Execute via Claude Code's Task tool.

        Note: Actual Task tool execution is handled by Claude Code.
        This method prepares the execution plan.
        """
        subagent_type = agent_config.get("subagent_type", "general-purpose")

        # Record start
        self._record_agent_start(
            story_id=story_id,
            agent_name="claude-code",
            pid=None  # No PID for Task tool
        )

        result = {
            "success": True,
            "story_id": story_id,
            "agent": "claude-code",
            "agent_type": "task-tool",
            "subagent_type": subagent_type,
            "prompt": prompt,
            "execution_mode": "task-tool"
        }

        # If callback provided, use it
        if task_callback:
            try:
                callback_result = task_callback(story_id, prompt, subagent_type)
                result.update(callback_result)
            except Exception as e:
                result["success"] = False
                result["error"] = str(e)
                self._record_agent_failure(story_id, "claude-code", str(e))

        return result

    def _execute_via_cli(
        self,
        story_id: str,
        prompt: str,
        agent_name: str,
        agent_config: Dict,
        working_dir: str
    ) -> Dict[str, Any]:
        """
        Execute via external CLI agent using the wrapper script.

        The wrapper script handles:
        - Process execution and monitoring
        - Status file updates (.agent-status.json)
        - Output logging (.agent-outputs/)
        - Result file creation (.agent-outputs/<story-id>.result.json)

        This ensures proper status tracking regardless of how the CLI agent exits.
        """
        timeout = agent_config.get("timeout", 600)

        # Locate wrapper script
        wrapper_script = self._get_wrapper_script_path()
        if not wrapper_script:
            # Fallback to direct execution if wrapper not found
            return self._execute_via_cli_direct(
                story_id, prompt, agent_name, agent_config, working_dir
            )

        # Create output directory
        output_dir = Path(working_dir) / ".agent-outputs"
        output_dir.mkdir(exist_ok=True)

        # Write prompt to file
        prompt_file = output_dir / f"{story_id}.prompt.txt"
        with open(prompt_file, "w", encoding="utf-8") as f:
            f.write(prompt)

        # Build wrapper command
        cmd = [
            sys.executable,  # Use current Python
            str(wrapper_script),
            "--story-id", story_id,
            "--agent", agent_name,
            "--project-root", working_dir,
            "--timeout", str(timeout),
            "--prompt-file", str(prompt_file)
        ]

        # Config file path
        config_path = Path(working_dir) / "agents.json"
        if config_path.exists():
            cmd.extend(["--config", str(config_path)])

        try:
            # Launch wrapper in background
            if sys.platform == "win32":
                # Windows: use CREATE_NO_WINDOW and CREATE_NEW_PROCESS_GROUP
                process = subprocess.Popen(
                    cmd,
                    cwd=working_dir,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    stdin=subprocess.DEVNULL,
                    creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP
                )
            else:
                # Unix: use start_new_session for proper detachment
                process = subprocess.Popen(
                    cmd,
                    cwd=working_dir,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    stdin=subprocess.DEVNULL,
                    start_new_session=True
                )

            output_file = output_dir / f"{story_id}.log"
            result_file = output_dir / f"{story_id}.result.json"

            return {
                "success": True,
                "story_id": story_id,
                "agent": agent_name,
                "agent_type": "cli",
                "wrapper_pid": process.pid,
                "output_file": str(output_file),
                "result_file": str(result_file),
                "execution_mode": "wrapper"
            }

        except Exception as e:
            error_msg = str(e)
            self._record_agent_failure(story_id, agent_name, error_msg)
            return {
                "success": False,
                "story_id": story_id,
                "agent": agent_name,
                "error": error_msg
            }

    def _get_wrapper_script_path(self) -> Optional[Path]:
        """Get path to agent-wrapper.py script."""
        # Try relative to this file
        this_dir = Path(__file__).parent
        wrapper = this_dir.parent / "scripts" / "agent-wrapper.py"
        if wrapper.exists():
            return wrapper

        # Try in project root
        wrapper = self.project_root / "skills" / "hybrid-ralph" / "scripts" / "agent-wrapper.py"
        if wrapper.exists():
            return wrapper

        return None

    def _execute_via_cli_direct(
        self,
        story_id: str,
        prompt: str,
        agent_name: str,
        agent_config: Dict,
        working_dir: str
    ) -> Dict[str, Any]:
        """
        Direct CLI execution fallback (when wrapper not available).

        Note: This does not have proper completion tracking.
        """
        command = agent_config.get("command", "")
        args_template = agent_config.get("args", [])
        env_vars = agent_config.get("env", {})

        # Build command with substitutions
        args = []
        for arg in args_template:
            if isinstance(arg, str):
                arg = arg.replace("{prompt}", prompt)
                arg = arg.replace("{working_dir}", working_dir)
                arg = arg.replace("{story_id}", story_id)
            args.append(arg)

        cmd = [command] + args

        # Prepare environment
        env = os.environ.copy()
        env.update(env_vars)

        # Create output directory
        output_dir = Path(working_dir) / ".agent-outputs"
        output_dir.mkdir(exist_ok=True)
        output_file = output_dir / f"{story_id}.log"

        try:
            # Open log file for output
            with open(output_file, "w", encoding="utf-8") as log_file:
                log_file.write(f"# Agent: {agent_name}\n")
                log_file.write(f"# Story: {story_id}\n")
                log_file.write(f"# Command: {' '.join(cmd[:2])}...\n")
                log_file.write(f"# Started: {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
                log_file.write(f"# WARNING: Running without wrapper - completion tracking limited\n")
                log_file.write("-" * 60 + "\n\n")

            # Launch process in background
            with open(output_file, "a", encoding="utf-8") as log_file:
                process = subprocess.Popen(
                    cmd,
                    cwd=working_dir,
                    stdout=log_file,
                    stderr=subprocess.STDOUT,
                    env=env,
                    creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
                )

            # Record agent start
            self._record_agent_start(
                story_id=story_id,
                agent_name=agent_name,
                pid=process.pid,
                output_file=str(output_file)
            )

            return {
                "success": True,
                "story_id": story_id,
                "agent": agent_name,
                "agent_type": "cli",
                "pid": process.pid,
                "output_file": str(output_file),
                "execution_mode": "direct",
                "warning": "Running without wrapper - completion tracking limited"
            }

        except FileNotFoundError:
            error_msg = f"CLI command '{command}' not found"
            self._record_agent_failure(story_id, agent_name, error_msg)
            return {
                "success": False,
                "story_id": story_id,
                "agent": agent_name,
                "error": error_msg
            }
        except Exception as e:
            error_msg = str(e)
            self._record_agent_failure(story_id, agent_name, error_msg)
            return {
                "success": False,
                "story_id": story_id,
                "agent": agent_name,
                "error": error_msg
            }

    # ========== Agent Status Tracking ==========

    def _read_agent_status(self) -> Dict:
        """Read .agent-status.json file."""
        if not self.agent_status_path.exists():
            return {
                "running": [],
                "completed": [],
                "failed": []
            }

        try:
            with open(self.agent_status_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return {
                "running": [],
                "completed": [],
                "failed": []
            }

    def _write_agent_status(self, status: Dict) -> None:
        """Write .agent-status.json file."""
        status["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")
        try:
            with open(self.agent_status_path, "w", encoding="utf-8") as f:
                json.dump(status, f, indent=2)
        except IOError as e:
            print(f"[Warning] Could not write agent status: {e}")

    def _record_agent_start(
        self,
        story_id: str,
        agent_name: str,
        pid: Optional[int],
        output_file: Optional[str] = None
    ) -> None:
        """Record agent start in .agent-status.json."""
        status = self._read_agent_status()

        # Remove any existing entry for this story
        status["running"] = [
            r for r in status["running"]
            if r.get("story_id") != story_id
        ]

        # Add new running entry
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")
        }
        if pid:
            entry["pid"] = pid
        if output_file:
            entry["output_file"] = output_file

        status["running"].append(entry)
        self._write_agent_status(status)

    def _record_agent_complete(self, story_id: str, agent_name: str) -> None:
        """Record agent completion in .agent-status.json."""
        status = self._read_agent_status()

        # Find and move from running to completed
        running_entry = None
        status["running"] = [
            r for r in status["running"]
            if r.get("story_id") != story_id or (running_entry := r) is None
        ]
        # Note: The walrus operator trick above filters and captures

        # Actually find the entry properly
        for i, r in enumerate(self._read_agent_status().get("running", [])):
            if r.get("story_id") == story_id:
                running_entry = r
                break

        # Reload and properly filter
        status = self._read_agent_status()
        status["running"] = [
            r for r in status["running"]
            if r.get("story_id") != story_id
        ]

        # Add to completed
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "completed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        status["completed"].append(entry)
        self._write_agent_status(status)

        # Update progress
        if self.state_manager:
            self.state_manager.append_progress(
                f"[COMPLETE] via {agent_name}",
                story_id=story_id
            )

    def _record_agent_failure(
        self,
        story_id: str,
        agent_name: str,
        error: str
    ) -> None:
        """Record agent failure in .agent-status.json."""
        status = self._read_agent_status()

        # Find and move from running to failed
        running_entry = None
        for r in status.get("running", []):
            if r.get("story_id") == story_id:
                running_entry = r
                break

        status["running"] = [
            r for r in status["running"]
            if r.get("story_id") != story_id
        ]

        # Add to failed
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "failed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "error": error
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        status["failed"].append(entry)
        self._write_agent_status(status)

        # Update progress
        if self.state_manager:
            self.state_manager.append_progress(
                f"[FAILED] via {agent_name}: {error}",
                story_id=story_id
            )

    def get_agent_status(self, check_processes: bool = True) -> Dict[str, Any]:
        """
        Get current agent status.

        Args:
            check_processes: If True, verify running processes and check for results

        Returns:
            Dict with running, completed, failed agents
        """
        # Use AgentMonitor for comprehensive checking
        if check_processes:
            try:
                from agent_monitor import AgentMonitor
                monitor = AgentMonitor(self.project_root)
                result = monitor.check_running_agents()
                return {
                    "running": result.get("running", []),
                    "completed": self._read_agent_status().get("completed", []),
                    "failed": self._read_agent_status().get("failed", []),
                    "updated": result.get("updated", []),
                    "newly_completed": result.get("newly_completed", []),
                    "newly_failed": result.get("newly_failed", [])
                }
            except ImportError:
                pass

        # Fallback to basic status reading
        status = self._read_agent_status()

        # Check if running processes are still alive
        updated_running = []
        for entry in status.get("running", []):
            pid = entry.get("pid") or entry.get("wrapper_pid")
            if pid:
                if self._is_process_alive(pid):
                    updated_running.append(entry)
                else:
                    # Process died - check result file
                    output_file = entry.get("output_file", "")
                    if output_file:
                        result_file = Path(output_file).parent / f"{entry['story_id']}.result.json"
                        if result_file.exists():
                            try:
                                with open(result_file, "r") as f:
                                    result = json.load(f)
                                if result.get("success"):
                                    entry["completed_at"] = result.get("completed_at", time.strftime("%Y-%m-%dT%H:%M:%SZ"))
                                    if "completed" not in status:
                                        status["completed"] = []
                                    status["completed"].append(entry)
                                else:
                                    entry["failed_at"] = result.get("completed_at", time.strftime("%Y-%m-%dT%H:%M:%SZ"))
                                    entry["error"] = result.get("error", "Unknown error")
                                    if "failed" not in status:
                                        status["failed"] = []
                                    status["failed"].append(entry)
                                continue
                            except (json.JSONDecodeError, IOError):
                                pass

                    # No result file - mark as unexpected exit
                    entry["failed_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")
                    entry["error"] = "Process exited unexpectedly"
                    if "failed" not in status:
                        status["failed"] = []
                    status["failed"].append(entry)
            else:
                # Task tool agents don't have PIDs
                updated_running.append(entry)

        status["running"] = updated_running
        self._write_agent_status(status)

        return status

    def get_agent_result(self, story_id: str) -> Optional[Dict[str, Any]]:
        """
        Get the result of a completed agent.

        Args:
            story_id: Story ID to get result for

        Returns:
            Result dict with success, exit_code, output, etc.
        """
        # Check result file first
        result_file = self.project_root / ".agent-outputs" / f"{story_id}.result.json"
        if result_file.exists():
            try:
                with open(result_file, "r", encoding="utf-8") as f:
                    return json.load(f)
            except (json.JSONDecodeError, IOError):
                pass

        # Check status file
        status = self._read_agent_status()

        for entry in status.get("completed", []):
            if entry.get("story_id") == story_id:
                return {
                    "story_id": story_id,
                    "success": True,
                    "agent": entry.get("agent"),
                    "exit_code": entry.get("exit_code", 0),
                    "completed_at": entry.get("completed_at"),
                    "output_file": entry.get("output_file")
                }

        for entry in status.get("failed", []):
            if entry.get("story_id") == story_id:
                return {
                    "story_id": story_id,
                    "success": False,
                    "agent": entry.get("agent"),
                    "error": entry.get("error"),
                    "exit_code": entry.get("exit_code", -1),
                    "failed_at": entry.get("failed_at"),
                    "output_file": entry.get("output_file")
                }

        # Check if still running
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                return {
                    "story_id": story_id,
                    "success": None,  # Still running
                    "status": "running",
                    "agent": entry.get("agent"),
                    "started_at": entry.get("started_at"),
                    "output_file": entry.get("output_file")
                }

        return None

    def get_agent_output(self, story_id: str, tail_lines: int = 50) -> Optional[str]:
        """
        Get the output log of an agent.

        Args:
            story_id: Story ID
            tail_lines: Number of lines from end (0 = all)

        Returns:
            Output content or None
        """
        output_file = self.project_root / ".agent-outputs" / f"{story_id}.log"
        if not output_file.exists():
            return None

        try:
            with open(output_file, "r", encoding="utf-8") as f:
                content = f.read()

            if tail_lines > 0:
                lines = content.split("\n")
                return "\n".join(lines[-tail_lines:])

            return content
        except IOError:
            return None

    def wait_for_agents(
        self,
        story_ids: Optional[List[str]] = None,
        timeout: int = 3600,
        poll_interval: int = 5,
        callback: Optional[Callable[[str, Dict], None]] = None
    ) -> Dict[str, Any]:
        """
        Wait for agents to complete with optional callback.

        Args:
            story_ids: Specific story IDs to wait for (None = all running)
            timeout: Maximum wait time in seconds
            poll_interval: Seconds between polls
            callback: Optional callback(story_id, result) on completion

        Returns:
            Dict with completed and failed agents
        """
        try:
            from agent_monitor import AgentMonitor
            monitor = AgentMonitor(self.project_root)
        except ImportError:
            # Basic polling without monitor
            return self._wait_for_agents_basic(story_ids, timeout, poll_interval, callback)

        start_time = time.time()
        completed = []
        failed = []
        result = {"running": []}

        while True:
            result = monitor.check_running_agents()

            # Process new completions
            for entry in result.get("newly_completed", []):
                completed.append(entry)
                if callback:
                    callback(entry["story_id"], {"success": True, **entry})

            for entry in result.get("newly_failed", []):
                failed.append(entry)
                if callback:
                    callback(entry["story_id"], {"success": False, **entry})

            # Check if done
            if story_ids:
                pending = set(story_ids) - set(e["story_id"] for e in completed + failed)
                if not pending:
                    break
            else:
                if result["total_running"] == 0:
                    break

            # Check timeout
            if time.time() - start_time > timeout:
                break

            time.sleep(poll_interval)

        return {
            "completed": completed,
            "failed": failed,
            "still_running": result.get("running", []),
            "elapsed_seconds": time.time() - start_time
        }

    def _wait_for_agents_basic(
        self,
        story_ids: Optional[List[str]],
        timeout: int,
        poll_interval: int,
        callback: Optional[Callable]
    ) -> Dict[str, Any]:
        """Basic wait implementation without AgentMonitor."""
        start_time = time.time()
        completed = []
        failed = []
        seen = set()
        running = []

        while True:
            status = self.get_agent_status(check_processes=True)

            for entry in status.get("completed", []):
                if entry["story_id"] not in seen:
                    if not story_ids or entry["story_id"] in story_ids:
                        completed.append(entry)
                        seen.add(entry["story_id"])
                        if callback:
                            callback(entry["story_id"], {"success": True, **entry})

            for entry in status.get("failed", []):
                if entry["story_id"] not in seen:
                    if not story_ids or entry["story_id"] in story_ids:
                        failed.append(entry)
                        seen.add(entry["story_id"])
                        if callback:
                            callback(entry["story_id"], {"success": False, **entry})

            # Check if done
            running = [e for e in status.get("running", [])
                      if not story_ids or e["story_id"] in story_ids]
            if not running:
                break

            if time.time() - start_time > timeout:
                break

            time.sleep(poll_interval)

        return {
            "completed": completed,
            "failed": failed,
            "still_running": running,
            "elapsed_seconds": time.time() - start_time
        }

    def _is_process_alive(self, pid: int) -> bool:
        """Check if a process is still running."""
        try:
            if sys.platform == "win32":
                # Windows: use subprocess to check
                result = subprocess.run(
                    ["tasklist", "/FI", f"PID eq {pid}"],
                    capture_output=True,
                    text=True
                )
                return str(pid) in result.stdout
            else:
                # Unix: send signal 0
                os.kill(pid, 0)
                return True
        except (OSError, subprocess.SubprocessError):
            return False

    def get_available_agents(self) -> Dict[str, Dict]:
        """
        Get all configured agents with availability status.

        Returns:
            Dict mapping agent names to their config with 'available' flag
        """
        result = {}
        for name, config in self.agents.items():
            agent_info = config.copy()
            if config.get("type") == "task-tool":
                agent_info["available"] = True
            elif config.get("type") == "cli":
                command = config.get("command", "")
                agent_info["available"] = self._check_cli_available(command)
            else:
                agent_info["available"] = False
            result[name] = agent_info
        return result

    def stop_agent(self, story_id: str) -> Dict[str, Any]:
        """
        Stop a running CLI agent.

        Args:
            story_id: Story ID of the agent to stop

        Returns:
            Result dict with success status
        """
        status = self._read_agent_status()

        # Find running agent
        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                running_entry = entry
                break

        if not running_entry:
            return {
                "success": False,
                "error": f"No running agent found for story {story_id}"
            }

        pid = running_entry.get("pid")
        if not pid:
            return {
                "success": False,
                "error": f"Agent for story {story_id} has no PID (Task tool agent)"
            }

        try:
            if sys.platform == "win32":
                subprocess.run(["taskkill", "/F", "/PID", str(pid)], check=True)
            else:
                os.kill(pid, 9)  # SIGKILL

            # Record as failed
            self._record_agent_failure(
                story_id,
                running_entry.get("agent", "unknown"),
                "Stopped by user"
            )

            return {
                "success": True,
                "message": f"Stopped agent for story {story_id} (PID: {pid})"
            }

        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to stop agent: {e}"
            }


def main():
    """CLI interface for testing AgentExecutor."""
    import argparse

    parser = argparse.ArgumentParser(description="Agent Executor CLI")
    parser.add_argument("command", choices=["list", "status", "check", "stop"])
    parser.add_argument("--story-id", help="Story ID (for stop command)")
    parser.add_argument("--agent", help="Agent name (for check command)")

    args = parser.parse_args()
    project_root = Path.cwd()
    executor = AgentExecutor(project_root)

    if args.command == "list":
        agents = executor.get_available_agents()
        print("\nConfigured Agents:")
        print("-" * 60)
        for name, config in agents.items():
            avail = "available" if config.get("available") else "NOT AVAILABLE"
            agent_type = config.get("type", "unknown")
            desc = config.get("description", "")
            print(f"  {name}: [{agent_type}] {avail}")
            print(f"    {desc}")
        print()

    elif args.command == "status":
        status = executor.get_agent_status()
        print("\nAgent Status:")
        print("-" * 60)

        print(f"\nRunning ({len(status.get('running', []))}):")
        for entry in status.get("running", []):
            pid_str = f" (PID: {entry['pid']})" if entry.get("pid") else ""
            print(f"  - {entry['story_id']}: {entry['agent']}{pid_str}")
            print(f"    Started: {entry.get('started_at', 'unknown')}")

        print(f"\nCompleted ({len(status.get('completed', []))}):")
        for entry in status.get("completed", [])[-5:]:  # Last 5
            print(f"  - {entry['story_id']}: {entry['agent']}")

        print(f"\nFailed ({len(status.get('failed', []))}):")
        for entry in status.get("failed", [])[-5:]:  # Last 5
            print(f"  - {entry['story_id']}: {entry['agent']}")
            print(f"    Error: {entry.get('error', 'unknown')}")
        print()

    elif args.command == "check":
        agent_name = args.agent or "codex"
        _, agent_config = executor._resolve_agent(agent_name=agent_name)
        if agent_config == executor.agents.get("claude-code"):
            print(f"Agent '{agent_name}' would fallback to claude-code")
        else:
            print(f"Agent '{agent_name}' is available")

    elif args.command == "stop":
        if not args.story_id:
            print("Error: --story-id required for stop command")
            sys.exit(1)
        result = executor.stop_agent(args.story_id)
        if result["success"]:
            print(result["message"])
        else:
            print(f"Error: {result['error']}")


if __name__ == "__main__":
    main()
