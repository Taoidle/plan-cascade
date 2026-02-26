"""
Plan Cascade - A structured approach to AI-driven development

Plan Cascade implements a multi-layer architecture for decomposing complex
development tasks into manageable stories with dependency resolution,
parallel execution, multi-agent collaboration, and quality assurance.

Architecture:
    - Mega Plan Layer: Project-level orchestration with multiple features
    - Hybrid Layer: PRD-based story management with dependency resolution
    - Execution Layer: Multi-agent task execution with quality gates

Supports:
    - Claude Code as GUI (ClaudeCodeBackend)
    - Standalone LLM execution (BuiltinBackend)
    - Multi-agent collaboration
    - PRD-driven task decomposition

Example usage:
    from plan_cascade import Orchestrator, PRDGenerator, StateManager

    # Load or generate a PRD
    generator = PRDGenerator()
    prd = generator.generate("Build a REST API with authentication")

    # Execute with automatic dependency resolution
    orchestrator = Orchestrator(project_root)
    results = orchestrator.execute_prd(prd)
"""

__version__ = "4.2.0"
__author__ = "Plan Cascade Team"

# Core orchestration
# Backend layer (from feature-002)
from .backends import (
    AgentBackend,
    BackendFactory,
    BuiltinBackend,
    ClaudeCodeBackend,
    ExecutionResult,
)
from .core.feature_orchestrator import FeatureOrchestrator
from .core.iteration_loop import (
    BatchResult,
    IterationCallbacks,
    IterationConfig,
    IterationLoop,
    IterationMode,
    IterationState,
    IterationStatus,
)
from .core.mega_generator import MegaPlanGenerator
from .core.mode import ModeConfig, UserMode
from .core.orchestrator import Orchestrator, StoryAgent
from .core.prd_generator import PRDGenerator, create_sample_prd
from .core.quality_gate import (
    CustomGate,
    Gate,
    GateConfig,
    GateOutput,
    GateType,
    LintGate,
    ProjectType,
    QualityGate,
    TestGate,
    TypeCheckGate,
)
from .core.retry_manager import (
    ErrorType,
    FailureRecord,
    RetryConfig,
    RetryManager,
    RetryState,
)
from .core.strategy import ExecutionStrategy, StrategyDecision

# LLM providers (from feature-002)
from .llm import (
    LLMFactory,
    LLMProvider,
    LLMResponse,
    TokenUsage,
    ToolCall,
)

# Settings management (from feature-004)
# Graceful degradation: keyring or other optional deps may not be installed
# when running from a directory without plan-cascade dependencies.
try:
    from .settings import (
        AgentConfig,
        BackendType,
        ConfigMigration,
        ConfigValidator,
        QualityGateConfig,
        Settings,
        SettingsStorage,
        SetupWizard,
        ValidationResult,
    )
    _SETTINGS_AVAILABLE = True
except ImportError:
    _SETTINGS_AVAILABLE = False
    # Provide placeholder values so downstream imports don't break immediately.
    # The CLI entry point checks _SETTINGS_AVAILABLE and exits gracefully.
    AgentConfig = None  # type: ignore[assignment,misc]
    BackendType = None  # type: ignore[assignment,misc]
    ConfigMigration = None  # type: ignore[assignment,misc]
    ConfigValidator = None  # type: ignore[assignment,misc]
    QualityGateConfig = None  # type: ignore[assignment,misc]
    Settings = None  # type: ignore[assignment,misc]
    SettingsStorage = None  # type: ignore[assignment,misc]
    SetupWizard = None  # type: ignore[assignment,misc]
    ValidationResult = None  # type: ignore[assignment,misc]
from .state.context_filter import ContextFilter
from .state.mega_state import MegaStateManager

# State management
from .state.state_manager import FileLock, StateManager

# Tools (from feature-002)
from .tools import Tool, ToolRegistry, ToolResult

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
    "BackendFactory",
    "ClaudeCodeBackend",
    "BuiltinBackend",
    # LLM
    "LLMProvider",
    "LLMResponse",
    "LLMFactory",
    "ToolCall",
    "TokenUsage",
    # Tools
    "ToolRegistry",
    "Tool",
    "ToolResult",
    # Settings
    "_SETTINGS_AVAILABLE",
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
