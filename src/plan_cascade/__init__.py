"""
Plan Cascade - A structured approach to AI-driven development

Plan Cascade implements a multi-layer architecture for decomposing complex
development tasks into manageable stories with dependency resolution,
parallel execution, multi-agent collaboration, and quality assurance.

Architecture:
    - Mega Plan Layer: Project-level orchestration with multiple features
    - Hybrid Layer: PRD-based story management with dependency resolution
    - Execution Layer: Multi-agent task execution with quality gates

Example usage:
    from plan_cascade import Orchestrator, PRDGenerator, StateManager

    # Load or generate a PRD
    generator = PRDGenerator()
    prd = generator.generate("Build a REST API with authentication")

    # Execute with automatic dependency resolution
    orchestrator = Orchestrator(project_root)
    results = orchestrator.execute_prd(prd)
"""

__version__ = "2.0.0"
__author__ = "Plan Cascade Team"

# Core orchestration
from .core.orchestrator import Orchestrator, StoryAgent
from .core.prd_generator import PRDGenerator, create_sample_prd
from .core.mega_generator import MegaPlanGenerator
from .core.feature_orchestrator import FeatureOrchestrator
from .core.iteration_loop import (
    IterationLoop,
    IterationConfig,
    IterationMode,
    IterationStatus,
    IterationState,
    IterationCallbacks,
    BatchResult,
)
from .core.quality_gate import (
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
from .core.retry_manager import (
    RetryManager,
    RetryConfig,
    RetryState,
    FailureRecord,
    ErrorType,
)
from .core.mode import UserMode, ModeConfig
from .core.strategy import ExecutionStrategy, StrategyDecision

# State management
from .state.state_manager import StateManager, FileLock
from .state.context_filter import ContextFilter
from .state.mega_state import MegaStateManager

# Backend layer
from .backends.base import AgentBackend, ExecutionResult
from .backends.agent_executor import AgentExecutor
from .backends.agent_monitor import AgentMonitor
from .backends.cross_platform_detector import (
    CrossPlatformDetector,
    DetectorConfig,
    AgentInfo,
    Platform,
)
from .backends.phase_config import (
    PhaseAgentManager,
    PhaseConfig,
    ExecutionPhase,
    StoryType,
    AgentOverrides,
)

# Settings management
from .settings import (
    BackendType,
    AgentConfig,
    QualityGateConfig,
    Settings,
    SettingsStorage,
    ConfigMigration,
    ConfigValidator,
    ValidationResult,
    SetupWizard,
)

__all__ = [
    # Version info
    "__version__",
    "__author__",
    # Core orchestration
    "Orchestrator",
    "StoryAgent",
    "PRDGenerator",
    "create_sample_prd",
    "MegaPlanGenerator",
    "FeatureOrchestrator",
    # Iteration
    "IterationLoop",
    "IterationConfig",
    "IterationMode",
    "IterationStatus",
    "IterationState",
    "IterationCallbacks",
    "BatchResult",
    # Quality gates
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
    # Retry management
    "RetryManager",
    "RetryConfig",
    "RetryState",
    "FailureRecord",
    "ErrorType",
    # Mode and strategy
    "UserMode",
    "ModeConfig",
    "ExecutionStrategy",
    "StrategyDecision",
    # State management
    "StateManager",
    "FileLock",
    "ContextFilter",
    "MegaStateManager",
    # Backend layer
    "AgentBackend",
    "ExecutionResult",
    "AgentExecutor",
    "AgentMonitor",
    "CrossPlatformDetector",
    "DetectorConfig",
    "AgentInfo",
    "Platform",
    "PhaseAgentManager",
    "PhaseConfig",
    "ExecutionPhase",
    "StoryType",
    "AgentOverrides",
    # Settings
    "BackendType",
    "AgentConfig",
    "QualityGateConfig",
    "Settings",
    "SettingsStorage",
    "ConfigMigration",
    "ConfigValidator",
    "ValidationResult",
    "SetupWizard",
]
