"""
Settings data models for Plan Cascade.

This module defines all configuration-related data classes including:
- BackendType: Supported backend types enumeration
- AgentConfig: Configuration for execution agents
- QualityGateConfig: Quality gate settings
- Settings: Main settings class aggregating all configuration options
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import List, Optional


class BackendType(Enum):
    """
    Supported backend types for Plan Cascade.

    - CLAUDE_CODE: Claude Code mode (zero configuration required)
    - CLAUDE_API: Anthropic Claude API (requires API key)
    - OPENAI: OpenAI API (requires API key)
    - DEEPSEEK: DeepSeek API (requires API key)
    - OLLAMA: Local Ollama instance (optional API key)
    """

    CLAUDE_CODE = "claude-code"
    CLAUDE_API = "claude-api"
    OPENAI = "openai"
    DEEPSEEK = "deepseek"
    OLLAMA = "ollama"


@dataclass
class AgentConfig:
    """
    Configuration for an execution agent.

    Attributes:
        name: Unique identifier for the agent.
        enabled: Whether this agent is available for use.
        command: The command used to invoke this agent.
        is_default: Whether this is the default agent for execution.
    """

    name: str
    enabled: bool = True
    command: str = ""
    is_default: bool = False


@dataclass
class QualityGateConfig:
    """
    Quality gate configuration settings.

    Controls which quality checks are performed after story completion.

    Attributes:
        typecheck: Enable type checking (e.g., mypy, pyright).
        test: Enable test execution.
        lint: Enable code linting (e.g., ruff, flake8).
        custom: Enable custom script execution.
        custom_script: Path to custom quality check script.
        max_retries: Maximum retry attempts when quality checks fail.
    """

    typecheck: bool = True
    test: bool = True
    lint: bool = True
    custom: bool = False
    custom_script: str = ""
    max_retries: int = 3


@dataclass
class Settings:
    """
    Global settings for Plan Cascade.

    Aggregates all configuration options with sensible defaults:
    - Default backend: CLAUDE_CODE (zero configuration)
    - Default agent: claude-code
    - Default mode: simple

    Attributes:
        backend: The backend type to use for execution.
        provider: The LLM provider name (claude, openai, deepseek, ollama).
        model: Specific model to use (empty string for provider default).
        agents: List of configured execution agents.
        agent_selection: Strategy for selecting agents ("smart", "prefer_default", "manual").
        default_agent: Name of the default agent to use.
        quality_gates: Quality gate configuration.
        max_parallel_stories: Maximum concurrent story executions.
        max_iterations: Maximum iterations per story execution.
        timeout_seconds: Timeout for each story execution.
        default_mode: Default UI mode ("simple" or "expert").
        theme: UI theme preference ("light", "dark", "system").
    """

    # Backend configuration
    backend: BackendType = BackendType.CLAUDE_CODE
    provider: str = "claude"
    model: str = ""

    # Execution agents
    agents: List[AgentConfig] = field(
        default_factory=lambda: [
            AgentConfig(
                name="claude-code", enabled=True, command="claude", is_default=True
            ),
            AgentConfig(name="aider", enabled=False, command="aider"),
            AgentConfig(name="codex", enabled=False, command="codex"),
        ]
    )

    # Agent selection strategy
    agent_selection: str = "prefer_default"
    default_agent: str = "claude-code"

    # Quality gates
    quality_gates: QualityGateConfig = field(default_factory=QualityGateConfig)

    # Execution configuration
    max_parallel_stories: int = 3
    max_iterations: int = 50
    timeout_seconds: int = 300

    # UI configuration
    default_mode: str = "simple"
    theme: str = "system"

    def get_enabled_agents(self) -> List[AgentConfig]:
        """Return a list of all enabled agents."""
        return [agent for agent in self.agents if agent.enabled]

    def get_default_agent(self) -> Optional[AgentConfig]:
        """Return the default agent configuration, if any."""
        for agent in self.agents:
            if agent.name == self.default_agent and agent.enabled:
                return agent
        return None

    def get_agent_by_name(self, name: str) -> Optional[AgentConfig]:
        """Return an agent configuration by name."""
        for agent in self.agents:
            if agent.name == name:
                return agent
        return None
