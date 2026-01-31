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
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any, Optional

from ..state.state_manager import StateManager

if TYPE_CHECKING:
    from .cross_platform_detector import CrossPlatformDetector
    from .phase_config import AgentOverrides, ExecutionPhase, PhaseAgentManager


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
        agents_config: dict | None = None,
        config_path: Path | None = None,
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
        self._full_config: dict[str, Any] = {}

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

    def _load_config(self, config: dict) -> None:
        """Load configuration from a dictionary."""
        self._full_config = config
        if "default_agent" in config:
            self.default_agent = config["default_agent"]
        if "agents" in config:
            self.agents.update(config["agents"])

    def _load_config_file(self, config_path: Path) -> None:
        """Load configuration from a JSON file."""
        try:
            with open(config_path, encoding="utf-8") as f:
                config = json.load(f)
                self._load_config(config)
        except (OSError, json.JSONDecodeError) as e:
            print(f"[Warning] Could not load agent config from {config_path}: {e}")

    def _resolve_agent(
        self,
        agent_name: str | None = None,
        story_agent: str | None = None,
        prd_agent: str | None = None,
        phase: Optional["ExecutionPhase"] = None,
        story: dict | None = None,
        override: Optional["AgentOverrides"] = None
    ) -> tuple[str, dict]:
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
        story: dict,
        context: dict,
        agent_name: str | None = None,
        prd_metadata: dict | None = None,
        task_callback: Callable | None = None,
        phase: Optional["ExecutionPhase"] = None,
        override: Optional["AgentOverrides"] = None
    ) -> dict[str, Any]:
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

    def _build_story_prompt(self, story: dict, context: dict) -> str:
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

        # Build design context section if available
        design_context = context.get("design", {})
        design_section = self._format_design_context(design_context)

        prompt = f"""You are executing story {story_id}: {title}

Description:
{description}

Acceptance Criteria:
{ac_lines}

Dependencies Summary:
{dep_lines}

Relevant Findings:
{finding_lines}
{design_section}
Your task:
1. Read the relevant code and documentation
2. Implement the story according to acceptance criteria
3. **Follow the architectural patterns and decisions above**
4. Test your implementation
5. Update findings.md with any discoveries (tag with <!-- @tags: {story_id} -->)
6. Mark as complete by appending to progress.txt: [COMPLETE] {story_id}

Work methodically and document your progress.
"""
        return prompt

    def _format_design_context(self, design_context: dict) -> str:
        """
        Format design context for inclusion in the story prompt.

        Args:
            design_context: Design context dictionary from ContextFilter

        Returns:
            Formatted string for prompt inclusion
        """
        if not design_context:
            return ""

        sections = []

        # Overview summary (if available)
        overview = design_context.get("overview", {})
        if overview.get("summary"):
            sections.append(f"Project Context: {overview.get('summary')}")

        # Architectural patterns to follow
        patterns = design_context.get("patterns", [])
        if patterns:
            pattern_lines = []
            for p in patterns:
                name = p.get("name", "")
                desc = p.get("description", "")
                rationale = p.get("rationale", "")
                if name:
                    line = f"  - **{name}**: {desc}"
                    if rationale:
                        line += f" (Reason: {rationale})"
                    pattern_lines.append(line)
            if pattern_lines:
                sections.append("Architectural Patterns:\n" + "\n".join(pattern_lines))

        # Relevant components
        components = design_context.get("components", [])
        if components:
            comp_lines = []
            for c in components:
                name = c.get("name", "")
                desc = c.get("description", "")
                files = c.get("files", [])
                if name:
                    line = f"  - **{name}**: {desc}"
                    if files:
                        line += f" (Files: {', '.join(files)})"
                    comp_lines.append(line)
            if comp_lines:
                sections.append("Relevant Components:\n" + "\n".join(comp_lines))

        # Architectural decisions (ADRs)
        decisions = design_context.get("decisions", [])
        if decisions:
            dec_lines = []
            for d in decisions:
                adr_id = d.get("id", "")
                title = d.get("title", "")
                decision = d.get("decision", "")
                if adr_id and title:
                    line = f"  - **{adr_id}: {title}**"
                    if decision:
                        line += f"\n    Decision: {decision}"
                    dec_lines.append(line)
            if dec_lines:
                sections.append("Architectural Decisions:\n" + "\n".join(dec_lines))

        # Relevant APIs
        apis = design_context.get("apis", [])
        if apis:
            api_lines = []
            for api in apis:
                api_id = api.get("id", "")
                method = api.get("method", "")
                path = api.get("path", "")
                desc = api.get("description", "")
                if path:
                    line = f"  - {method} {path}"
                    if desc:
                        line += f" - {desc}"
                    api_lines.append(line)
            if api_lines:
                sections.append("API Interfaces:\n" + "\n".join(api_lines))

        # Relevant data models
        data_models = design_context.get("data_models", [])
        if data_models:
            model_lines = []
            for model in data_models:
                name = model.get("name", "")
                desc = model.get("description", "")
                if name:
                    line = f"  - **{name}**: {desc}"
                    model_lines.append(line)
            if model_lines:
                sections.append("Data Models:\n" + "\n".join(model_lines))

        # Data flow
        data_flow = design_context.get("data_flow", "")
        if data_flow:
            sections.append(f"Data Flow: {data_flow}")

        # Inherited context from project-level design document
        inherited = design_context.get("inherited", {})
        if inherited:
            inherited_sections = []

            # Inherited patterns
            inherited_patterns = inherited.get("patterns", [])
            if inherited_patterns:
                pattern_names = []
                for p in inherited_patterns:
                    name = p.get("name", "") if isinstance(p, dict) else str(p)
                    if name:
                        pattern_names.append(name)
                if pattern_names:
                    inherited_sections.append(f"  - Patterns: {', '.join(pattern_names)}")

            # Inherited decisions
            inherited_decisions = inherited.get("decisions", [])
            if inherited_decisions:
                dec_lines = []
                for d in inherited_decisions:
                    if isinstance(d, dict):
                        adr_id = d.get("id", "")
                        title = d.get("title", "")
                        decision = d.get("decision", "")
                        if adr_id:
                            line = f"    - **{adr_id}: {title}**"
                            if decision:
                                line += f" - {decision}"
                            dec_lines.append(line)
                    else:
                        dec_lines.append(f"    - {d}")
                if dec_lines:
                    inherited_sections.append("  - Decisions:\n" + "\n".join(dec_lines))

            # Shared models
            shared_models = inherited.get("shared_models", [])
            if shared_models:
                model_names = []
                for m in shared_models:
                    name = m.get("name", "") if isinstance(m, dict) else str(m)
                    if name:
                        model_names.append(name)
                if model_names:
                    inherited_sections.append(f"  - Shared Models: {', '.join(model_names)}")

            # API standards
            api_standards = inherited.get("api_standards", {})
            if api_standards:
                std_parts = []
                if api_standards.get("style"):
                    std_parts.append(f"Style: {api_standards['style']}")
                if api_standards.get("authentication"):
                    std_parts.append(f"Auth: {api_standards['authentication']}")
                if std_parts:
                    inherited_sections.append(f"  - API Standards: {', '.join(std_parts)}")

            if inherited_sections:
                sections.append("Inherited from Project:\n" + "\n".join(inherited_sections))

        if not sections:
            return ""

        return "\n\n## Technical Design Context\n" + "\n\n".join(sections) + "\n"

    def _execute_via_task_tool(
        self,
        story_id: str,
        prompt: str,
        agent_config: dict,
        task_callback: Callable | None = None
    ) -> dict[str, Any]:
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
        agent_config: dict,
        working_dir: str
    ) -> dict[str, Any]:
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
                kwargs: dict[str, Any] = {
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

    def get_agent_status(self, check_processes: bool = True) -> dict[str, Any]:
        """Get current agent status."""
        return self.state_manager.read_agent_status()

    def get_available_agents(self) -> dict[str, dict]:
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

    def stop_agent(self, story_id: str) -> dict[str, Any]:
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
