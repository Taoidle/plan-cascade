"""
Hybrid Ralph Core Modules

This package provides the core functionality for the Hybrid Ralph + Planning-with-Files system.

Extended with multi-agent collaboration support including:
- Automatic iteration loop
- Quality gates
- Retry management
- Cross-platform agent detection
- Phase-based agent assignment
"""

from .context_filter import ContextFilter
from .state_manager import StateManager, FileLock
from .prd_generator import PRDGenerator, create_sample_prd
from .orchestrator import Orchestrator, StoryAgent
from .agent_executor import AgentExecutor
from .agent_monitor import AgentMonitor

# New modules for iteration, quality gates, and retry management
from .iteration_loop import (
    IterationLoop,
    IterationConfig,
    IterationMode,
    IterationStatus,
    IterationState,
    IterationCallbacks,
    BatchResult,
)
from .quality_gate import (
    QualityGate,
    Gate,
    GateConfig,
    GateOutput,
    GateType,
    TypeCheckGate,
    TestGate,
    LintGate,
    CustomGate,
    ProjectType,
)
from .retry_manager import (
    RetryManager,
    RetryConfig,
    RetryState,
    FailureRecord,
    ErrorType,
)
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
    # Original modules
    "ContextFilter",
    "StateManager",
    "FileLock",
    "PRDGenerator",
    "create_sample_prd",
    "Orchestrator",
    "StoryAgent",
    "AgentExecutor",
    "AgentMonitor",
    # Iteration loop
    "IterationLoop",
    "IterationConfig",
    "IterationMode",
    "IterationStatus",
    "IterationState",
    "IterationCallbacks",
    "BatchResult",
    # Quality gate
    "QualityGate",
    "Gate",
    "GateConfig",
    "GateOutput",
    "GateType",
    "TypeCheckGate",
    "TestGate",
    "LintGate",
    "CustomGate",
    "ProjectType",
    # Retry manager
    "RetryManager",
    "RetryConfig",
    "RetryState",
    "FailureRecord",
    "ErrorType",
    # Cross-platform detector
    "CrossPlatformDetector",
    "DetectorConfig",
    "AgentInfo",
    "Platform",
    # Phase config
    "PhaseAgentManager",
    "PhaseConfig",
    "ExecutionPhase",
    "StoryType",
    "AgentOverrides",
]
