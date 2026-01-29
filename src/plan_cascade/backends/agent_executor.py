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
- Cross-platform agent detection with caching
- Phase-based agent assignment
"""

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple, TYPE_CHECKING

from ..state.state_manager import StateManager
from .base import ExecutionResult

if TYPE_CHECKING:
    from .cross_platform_detector import CrossPlatformDetector
    from .phase_config import PhaseAgentManager, ExecutionPhase, AgentOverrides


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
        config_path: Optional[Path] = None,
        detector: Optional["CrossPlatformDetector"] = None,
        phase_manager: Optional["PhaseAgentManager"] = None
    ):
        """
        Initialize the AgentExecutor.

        Args:
            project_root: Root directory of the project
            agents_config: Direct agent configuration dict (optional)
            config_path: Path to agents.json file (optional, defaults to project_root)
            detector: CrossPlatformDetector instance (optional, created if not provided)
            phase_manager: PhaseAgentManager instance (optional, created if not provided)
        """
        self.project_root = Path(project_root)
        self.default_agent = "claude-code"
        self.agents = self.DEFAULT_AGENTS.copy()
        self.state_manager = StateManager(project_root)
        self._full_config: Dict[str, Any] = {}

        # Agent status tracking
        self.agent_status_path = self.project_root / ".agent-status.json"

        # Load configuration
        if agents_config:
            self._load_config(agents_config)
        elif config_path:
            self._load_config_file(config_path)
        else:
            default_config = self.project_root / "agents.json"
            if default_config.exists():
                self._load_config_file(default_config)

        # Initialize cross-platform detector
        try:
            from .cross_platform_detector import CrossPlatformDetector
            self.detector = detector or CrossPlatformDetector(project_root=self.project_root)
        except ImportError:
            self.detector = None

        # Initialize phase-based agent manager
        try:
            from .phase_config import PhaseAgentManager
            self.phase_manager = phase_manager or PhaseAgentManager(
                config=self._full_config,
                detector=self.detector
            )
        except ImportError:
            self.phase_manager = None

    def _load_config(self, config: Dict) -> None:
        """Load configuration from a dictionary."""
        self._full_config = config
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
        prd_agent: Optional[str] = None,
        phase: Optional["ExecutionPhase"] = None,
        story: Optional[Dict] = None,
        override: Optional["AgentOverrides"] = None
    ) -> Tuple[str, Dict]:
        """
        Resolve agent with automatic fallback to claude-code.

        Priority order:
        1. agent_name parameter (explicit override)
        2. Phase-based resolution (if phase and phase_manager available)
        3. story_agent from story metadata
        4. prd_agent from PRD metadata
        5. default_agent from config
        6. "claude-code" as ultimate fallback

        Returns:
            Tuple of (resolved_agent_name, agent_config)
        """
        # Use phase-based resolution if available
        if self.phase_manager and phase and story:
            try:
                from .phase_config import ExecutionPhase
                resolved_name = self.phase_manager.get_agent_for_story(
                    story=story,
                    phase=phase,
                    override=override
                )
                if resolved_name in self.agents:
                    agent = self.agents[resolved_name]
                    if agent.get("type") == "cli":
                        if self._check_cli_available(resolved_name, agent.get("command", "")):
                            return resolved_name, agent
                        else:
                            self._log_fallback(resolved_name, "CLI not available")
                    else:
                        return resolved_name, agent
            except ImportError:
                pass

        # Traditional priority chain
        name = agent_name or story_agent or prd_agent or self.default_agent

        if name == "claude-code":
            return name, self.agents["claude-code"]

        if name not in self.agents:
            self._log_fallback(name, "not configured")
            return "claude-code", self.agents["claude-code"]

        agent = self.agents[name]

        # For CLI agents, check if command is available
        if agent.get("type") == "cli":
            command = agent.get("command", "")
            if not self._check_cli_available(name, command):
                self._log_fallback(name, f"CLI '{command}' not found")
                return "claude-code", self.agents["claude-code"]

        return name, agent

    def _check_cli_available(self, agent_name: str, command: str) -> bool:
        """Check if a CLI command is available."""
        if self.detector:
            info = self.detector.detect_agent(agent_name)
            return info.available
        return shutil.which(command) is not None

    def _log_fallback(self, agent_name: str, reason: str) -> None:
        """Log a fallback event."""
        message = f"[AgentExecutor] Agent '{agent_name}' unavailable ({reason}), falling back to claude-code"
        print(message)
        self.state_manager.append_progress(f"[FALLBACK] {agent_name} -> claude-code: {reason}")

    def execute_story(
        self,
        story: Dict,
        context: Dict,
        agent_name: Optional[str] = None,
        prd_metadata: Optional[Dict] = None,
        task_callback: Optional[Callable] = None,
        phase: Optional["ExecutionPhase"] = None,
        override: Optional["AgentOverrides"] = None
    ) -> Dict[str, Any]:
        """
        Execute a story using the specified agent (with fallback).

        Args:
            story: Story dictionary with id, title, description, etc.
            context: Context for the story (dependencies, findings)
            agent_name: Optional explicit agent override
            prd_metadata: Optional PRD metadata for agent defaults
            task_callback: Callback for Task tool execution (for claude-code)
            phase: Execution phase for phase-based agent selection
            override: Command-line agent overrides

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
            prd_agent=prd_agent,
            phase=phase,
            story=story,
            override=override
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

        ac_lines = "\n".join(f"- {c}" for c in acceptance_criteria)

        deps = context.get("dependencies", [])
        dep_lines = "\n".join(
            f"  - {d.get('id')}: {d.get('title', '')} (status: {d.get('status', 'unknown')})"
            for d in deps
        ) if deps else "  None"

        findings = context.get("findings", [])
        finding_lines = "\n".join(findings[:5]) if findings else "  No previous findings"

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
        """Execute via Claude Code's Task tool."""
        subagent_type = agent_config.get("subagent_type", "general-purpose")

        self.state_manager.record_agent_start(story_id, "claude-code")

        result = {
            "success": True,
            "story_id": story_id,
            "agent": "claude-code",
            "agent_type": "task-tool",
            "subagent_type": subagent_type,
            "prompt": prompt,
            "execution_mode": "task-tool"
        }

        if task_callback:
            try:
                callback_result = task_callback(story_id, prompt, subagent_type)
                result.update(callback_result)
            except Exception as e:
                result["success"] = False
                result["error"] = str(e)
                self.state_manager.record_agent_failure(story_id, "claude-code", str(e))

        return result

    def _execute_via_cli(
        self,
        story_id: str,
        prompt: str,
        agent_name: str,
        agent_config: Dict,
        working_dir: str
    ) -> Dict[str, Any]:
        """Execute via external CLI agent."""
        command = agent_config.get("command", "")
        args_template = agent_config.get("args", [])
        env_vars = agent_config.get("env", {})
        timeout = agent_config.get("timeout", 600)

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
            with open(output_file, "w", encoding="utf-8") as log_file:
                log_file.write(f"# Agent: {agent_name}\n")
                log_file.write(f"# Story: {story_id}\n")
                log_file.write(f"# Command: {' '.join(cmd[:2])}...\n")
                log_file.write(f"# Started: {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
                log_file.write("-" * 60 + "\n\n")

            with open(output_file, "a", encoding="utf-8") as log_file:
                kwargs: Dict[str, Any] = {
                    "cwd": working_dir,
                    "stdout": log_file,
                    "stderr": subprocess.STDOUT,
                    "env": env,
                }
                if sys.platform == "win32":
                    kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

                process = subprocess.Popen(cmd, **kwargs)

            self.state_manager.record_agent_start(
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
                "execution_mode": "cli"
            }

        except FileNotFoundError:
            error_msg = f"CLI command '{command}' not found"
            self.state_manager.record_agent_failure(story_id, agent_name, error_msg)
            return {"success": False, "story_id": story_id, "agent": agent_name, "error": error_msg}
        except Exception as e:
            error_msg = str(e)
            self.state_manager.record_agent_failure(story_id, agent_name, error_msg)
            return {"success": False, "story_id": story_id, "agent": agent_name, "error": error_msg}

    def get_agent_status(self, check_processes: bool = True) -> Dict[str, Any]:
        """Get current agent status."""
        return self.state_manager.read_agent_status()

    def get_available_agents(self) -> Dict[str, Dict]:
        """Get all configured agents with availability status."""
        result = {}
        for name, config in self.agents.items():
            agent_info = config.copy()
            if config.get("type") == "task-tool":
                agent_info["available"] = True
            elif config.get("type") == "cli":
                command = config.get("command", "")
                agent_info["available"] = shutil.which(command) is not None
            else:
                agent_info["available"] = False
            result[name] = agent_info
        return result

    def stop_agent(self, story_id: str) -> Dict[str, Any]:
        """Stop a running CLI agent."""
        status = self.state_manager.read_agent_status()

        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                running_entry = entry
                break

        if not running_entry:
            return {"success": False, "error": f"No running agent found for story {story_id}"}

        pid = running_entry.get("pid")
        if not pid:
            return {"success": False, "error": f"Agent for story {story_id} has no PID"}

        try:
            if sys.platform == "win32":
                subprocess.run(["taskkill", "/F", "/PID", str(pid)], check=True)
            else:
                os.kill(pid, 9)

            self.state_manager.record_agent_failure(
                story_id,
                running_entry.get("agent", "unknown"),
                "Stopped by user"
            )

            return {"success": True, "message": f"Stopped agent for story {story_id} (PID: {pid})"}
        except Exception as e:
            return {"success": False, "error": f"Failed to stop agent: {e}"}
