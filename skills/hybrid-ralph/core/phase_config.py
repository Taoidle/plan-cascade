"""
Phase-Based Agent Assignment

Manages agent selection based on execution phase, story type, and command-line overrides.
Implements a resolution priority chain for flexible agent assignment.
"""

import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from .cross_platform_detector import CrossPlatformDetector


class ExecutionPhase(Enum):
    """Execution phases for story processing."""
    PLANNING = "planning"
    IMPLEMENTATION = "implementation"
    RETRY = "retry"
    REFACTOR = "refactor"
    REVIEW = "review"


class StoryType(Enum):
    """Types of stories for agent selection."""
    FEATURE = "feature"
    BUGFIX = "bugfix"
    REFACTOR = "refactor"
    TEST = "test"
    DOCUMENTATION = "documentation"
    INFRASTRUCTURE = "infrastructure"
    UNKNOWN = "unknown"


@dataclass
class AgentOverrides:
    """
    Command-line temporary overrides for agent selection.

    These override the default resolution chain when specified.
    """
    global_agent: Optional[str] = None      # --agent (overrides all phases)
    planning_agent: Optional[str] = None    # --planning-agent
    impl_agent: Optional[str] = None        # --impl-agent
    retry_agent: Optional[str] = None       # --retry-agent
    review_agent: Optional[str] = None      # --review-agent
    no_fallback: bool = False               # --no-fallback (disable auto fallback)

    def get_override_for_phase(self, phase: ExecutionPhase) -> Optional[str]:
        """Get the override agent for a specific phase."""
        if self.global_agent:
            return self.global_agent

        phase_overrides = {
            ExecutionPhase.PLANNING: self.planning_agent,
            ExecutionPhase.IMPLEMENTATION: self.impl_agent,
            ExecutionPhase.RETRY: self.retry_agent,
            ExecutionPhase.REVIEW: self.review_agent,
        }
        return phase_overrides.get(phase)


@dataclass
class PhaseConfig:
    """Configuration for a single execution phase."""
    default_agent: str
    fallback_chain: List[str] = field(default_factory=list)
    story_type_overrides: Dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "default_agent": self.default_agent,
            "fallback_chain": self.fallback_chain,
            "story_type_overrides": self.story_type_overrides,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "PhaseConfig":
        """Create from dictionary."""
        return cls(
            default_agent=data.get("default_agent", "claude-code"),
            fallback_chain=data.get("fallback_chain", []),
            story_type_overrides=data.get("story_type_overrides", {}),
        )


class PhaseAgentManager:
    """
    Manages agent selection based on phase, story type, and overrides.

    Resolution Priority (highest to lowest):
    1. --agent command parameter (AgentOverrides.global_agent)
    2. Phase-specific command parameter (AgentOverrides.impl_agent, etc.)
    3. story.agent in PRD
    4. Story type override for phase
    5. Phase default agent
    6. Fallback chain (if agent unavailable)
    7. claude-code (always available)
    """

    # Default phase configuration
    DEFAULT_PHASE_CONFIG: Dict[ExecutionPhase, PhaseConfig] = {
        ExecutionPhase.PLANNING: PhaseConfig(
            default_agent="codex",
            fallback_chain=["claude-code"],
            story_type_overrides={},
        ),
        ExecutionPhase.IMPLEMENTATION: PhaseConfig(
            default_agent="claude-code",
            fallback_chain=["codex", "aider"],
            story_type_overrides={
                "refactor": "aider",
                "bugfix": "codex",
            },
        ),
        ExecutionPhase.RETRY: PhaseConfig(
            default_agent="claude-code",
            fallback_chain=["aider"],
            story_type_overrides={},
        ),
        ExecutionPhase.REFACTOR: PhaseConfig(
            default_agent="aider",
            fallback_chain=["claude-code"],
            story_type_overrides={},
        ),
        ExecutionPhase.REVIEW: PhaseConfig(
            default_agent="claude-code",
            fallback_chain=["codex"],
            story_type_overrides={},
        ),
    }

    # Default story type agent preferences
    DEFAULT_STORY_TYPE_AGENTS: Dict[StoryType, str] = {
        StoryType.FEATURE: "claude-code",
        StoryType.BUGFIX: "codex",
        StoryType.REFACTOR: "aider",
        StoryType.TEST: "claude-code",
        StoryType.DOCUMENTATION: "claude-code",
        StoryType.INFRASTRUCTURE: "claude-code",
        StoryType.UNKNOWN: "claude-code",
    }

    # Keywords for inferring story type
    STORY_TYPE_KEYWORDS: Dict[StoryType, List[str]] = {
        StoryType.BUGFIX: [
            "fix", "bug", "error", "issue", "crash", "broken",
            "patch", "repair", "resolve", "debug", "hotfix",
        ],
        StoryType.REFACTOR: [
            "refactor", "restructure", "reorganize", "cleanup",
            "improve", "optimize", "simplify", "modernize",
        ],
        StoryType.TEST: [
            "test", "spec", "unit test", "integration test",
            "e2e", "coverage", "assertion", "mock",
        ],
        StoryType.DOCUMENTATION: [
            "doc", "readme", "documentation", "comment",
            "jsdoc", "docstring", "api doc", "guide",
        ],
        StoryType.INFRASTRUCTURE: [
            "ci", "cd", "pipeline", "deploy", "docker",
            "kubernetes", "terraform", "ansible", "config",
            "devops", "infrastructure", "build", "workflow",
        ],
        StoryType.FEATURE: [
            "add", "create", "implement", "new", "feature",
            "introduce", "build", "develop", "support",
        ],
    }

    def __init__(
        self,
        config: Optional[Dict[str, Any]] = None,
        detector: Optional["CrossPlatformDetector"] = None,
    ):
        """
        Initialize the phase agent manager.

        Args:
            config: Configuration from agents.json phase_defaults section
            detector: CrossPlatformDetector instance for checking agent availability
        """
        self.detector = detector
        self._phase_configs: Dict[ExecutionPhase, PhaseConfig] = {}
        self._story_type_defaults: Dict[StoryType, str] = {}

        # Load configuration
        self._load_config(config or {})

    def _load_config(self, config: Dict[str, Any]) -> None:
        """Load configuration from agents.json format."""
        # Load phase defaults
        phase_defaults = config.get("phase_defaults", {})
        for phase in ExecutionPhase:
            phase_key = phase.value
            if phase_key in phase_defaults:
                self._phase_configs[phase] = PhaseConfig.from_dict(phase_defaults[phase_key])
            else:
                self._phase_configs[phase] = self.DEFAULT_PHASE_CONFIG.get(
                    phase,
                    PhaseConfig(default_agent="claude-code"),
                )

        # Load story type defaults
        story_type_defaults = config.get("story_type_defaults", {})
        for story_type in StoryType:
            type_key = story_type.value
            if type_key in story_type_defaults:
                self._story_type_defaults[story_type] = story_type_defaults[type_key]
            else:
                self._story_type_defaults[story_type] = self.DEFAULT_STORY_TYPE_AGENTS.get(
                    story_type, "claude-code"
                )

    def get_agent_for_story(
        self,
        story: Dict[str, Any],
        phase: ExecutionPhase,
        override: Optional[AgentOverrides] = None,
    ) -> str:
        """
        Get the appropriate agent for a story in a given phase.

        Resolution Priority:
        1. --agent command parameter (global override)
        2. Phase-specific command parameter
        3. story.agent in PRD
        4. Story type override for phase
        5. Phase default agent
        6. Fallback chain (if agent unavailable)
        7. claude-code (always available)

        Args:
            story: Story dictionary from PRD
            phase: Current execution phase
            override: Command-line overrides

        Returns:
            Agent name to use
        """
        override = override or AgentOverrides()
        phase_config = self._phase_configs.get(
            phase,
            PhaseConfig(default_agent="claude-code"),
        )

        # Priority 1 & 2: Command-line overrides
        override_agent = override.get_override_for_phase(phase)
        if override_agent:
            if self._is_agent_available(override_agent) or override.no_fallback:
                return override_agent

        # Priority 3: Story-level agent
        story_agent = story.get("agent")
        if story_agent:
            if self._is_agent_available(story_agent) or override.no_fallback:
                return story_agent

        # Priority 4: Story type override for phase
        story_type = self.infer_story_type(story)
        type_override = phase_config.story_type_overrides.get(story_type.value)
        if type_override:
            if self._is_agent_available(type_override) or override.no_fallback:
                return type_override

        # Priority 5: Phase default agent
        default_agent = phase_config.default_agent
        if self._is_agent_available(default_agent) or override.no_fallback:
            return default_agent

        # Priority 6: Fallback chain
        if not override.no_fallback:
            for fallback in phase_config.fallback_chain:
                if self._is_agent_available(fallback):
                    return fallback

        # Priority 7: Ultimate fallback
        return "claude-code"

    def infer_story_type(self, story: Dict[str, Any]) -> StoryType:
        """
        Infer the story type from title, description, and tags.

        Args:
            story: Story dictionary from PRD

        Returns:
            Inferred StoryType
        """
        # Check explicit tags first
        tags = story.get("tags", [])
        for tag in tags:
            tag_lower = tag.lower()
            for story_type in StoryType:
                if story_type.value in tag_lower:
                    return story_type

        # Analyze title and description
        title = story.get("title", "").lower()
        description = story.get("description", "").lower()
        text = f"{title} {description}"

        # Score each story type based on keyword matches
        scores: Dict[StoryType, int] = {st: 0 for st in StoryType}

        for story_type, keywords in self.STORY_TYPE_KEYWORDS.items():
            for keyword in keywords:
                if keyword in text:
                    scores[story_type] += 1
                    # Bonus for title matches
                    if keyword in title:
                        scores[story_type] += 2

        # Return highest scoring type
        max_score = max(scores.values())
        if max_score > 0:
            for story_type, score in scores.items():
                if score == max_score:
                    return story_type

        return StoryType.UNKNOWN

    def _is_agent_available(self, agent_name: str) -> bool:
        """Check if an agent is available."""
        # claude-code is always available
        if agent_name == "claude-code":
            return True

        if self.detector:
            info = self.detector.detect_agent(agent_name)
            return info.available

        # Without detector, assume available (will fail at execution time if not)
        return True

    def get_phase_config(self, phase: ExecutionPhase) -> PhaseConfig:
        """Get configuration for a specific phase."""
        return self._phase_configs.get(
            phase,
            PhaseConfig(default_agent="claude-code"),
        )

    def get_story_type_default(self, story_type: StoryType) -> str:
        """Get default agent for a story type."""
        return self._story_type_defaults.get(story_type, "claude-code")

    def get_all_phase_configs(self) -> Dict[str, Dict[str, Any]]:
        """Get all phase configurations as a dictionary."""
        return {
            phase.value: config.to_dict()
            for phase, config in self._phase_configs.items()
        }

    def get_resolution_chain(
        self,
        story: Dict[str, Any],
        phase: ExecutionPhase,
        override: Optional[AgentOverrides] = None,
    ) -> List[Dict[str, Any]]:
        """
        Get the full resolution chain for debugging/display.

        Returns list of resolution steps with agent and availability.
        """
        override = override or AgentOverrides()
        phase_config = self._phase_configs.get(
            phase,
            PhaseConfig(default_agent="claude-code"),
        )
        story_type = self.infer_story_type(story)

        chain = []

        # Priority 1 & 2: Command-line overrides
        override_agent = override.get_override_for_phase(phase)
        if override_agent:
            chain.append({
                "priority": 1,
                "source": "command_override",
                "agent": override_agent,
                "available": self._is_agent_available(override_agent),
            })

        # Priority 3: Story-level agent
        story_agent = story.get("agent")
        if story_agent:
            chain.append({
                "priority": 3,
                "source": "story.agent",
                "agent": story_agent,
                "available": self._is_agent_available(story_agent),
            })

        # Priority 4: Story type override
        type_override = phase_config.story_type_overrides.get(story_type.value)
        if type_override:
            chain.append({
                "priority": 4,
                "source": f"story_type_override ({story_type.value})",
                "agent": type_override,
                "available": self._is_agent_available(type_override),
            })

        # Priority 5: Phase default
        chain.append({
            "priority": 5,
            "source": f"phase_default ({phase.value})",
            "agent": phase_config.default_agent,
            "available": self._is_agent_available(phase_config.default_agent),
        })

        # Priority 6: Fallback chain
        for i, fallback in enumerate(phase_config.fallback_chain):
            chain.append({
                "priority": 6,
                "source": f"fallback_chain[{i}]",
                "agent": fallback,
                "available": self._is_agent_available(fallback),
            })

        # Priority 7: Ultimate fallback
        chain.append({
            "priority": 7,
            "source": "ultimate_fallback",
            "agent": "claude-code",
            "available": True,
        })

        return chain
