"""
Plan Cascade Core Module

Contains the core orchestration logic including:
- Orchestrator: Batch execution with dependency resolution
- PRDGenerator: AI-driven PRD creation and validation
- MegaPlanGenerator: Project-level multi-feature planning
- FeatureOrchestrator: Feature-level coordination
- IterationLoop: Automatic iteration with quality gates
- QualityGate: Code verification (typecheck, test, lint)
- RetryManager: Failure handling with exponential backoff
- Mode: Simple/Expert user mode configuration
- Strategy: Execution strategy selection (Direct/Hybrid/Mega)
"""

from .orchestrator import Orchestrator, StoryAgent
from .prd_generator import PRDGenerator, create_sample_prd
from .mega_generator import MegaPlanGenerator
from .feature_orchestrator import FeatureOrchestrator
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
from .mode import UserMode, ModeConfig
from .strategy import ExecutionStrategy, StrategyDecision

__all__ = [
    # Orchestration
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
]
