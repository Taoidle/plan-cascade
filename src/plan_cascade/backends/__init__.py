"""
Plan Cascade Backends Module

Contains the backend abstraction layer for agent execution:
- AgentBackend: Abstract base class for all backends
- ExecutionResult: Standardized execution result dataclass
- AgentExecutor: Multi-agent execution with automatic fallback
- AgentMonitor: Process monitoring and status tracking
- CrossPlatformDetector: Platform-specific agent detection
- PhaseAgentManager: Phase-based agent selection
"""

from .base import AgentBackend, ExecutionResult
from .agent_executor import AgentExecutor
from .agent_monitor import AgentMonitor
from .cross_platform_detector import (
    CrossPlatformDetector,
    DetectorConfig,
    AgentInfo,
    Platform,
)
from .phase_config import (
    PhaseAgentManager,
    PhaseConfig,
    ExecutionPhase,
    StoryType,
    AgentOverrides,
)

__all__ = [
    # Base abstractions
    "AgentBackend",
    "ExecutionResult",
    # Execution
    "AgentExecutor",
    "AgentMonitor",
    # Detection
    "CrossPlatformDetector",
    "DetectorConfig",
    "AgentInfo",
    "Platform",
    # Phase configuration
    "PhaseAgentManager",
    "PhaseConfig",
    "ExecutionPhase",
    "StoryType",
    "AgentOverrides",
]
