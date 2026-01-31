"""
Plan Cascade Core Module

Contains the core orchestration logic including:
- Orchestrator: Batch execution with dependency resolution
- PRDGenerator: AI-driven PRD creation and validation
- MegaPlanGenerator: Project-level multi-feature planning
- FeatureOrchestrator: Feature-level coordination
- IterationLoop: Automatic iteration with quality gates
- ParallelExecutor: Parallel story execution with asyncio
- QualityGate: Code verification (typecheck, test, lint)
- RetryManager: Failure handling with exponential backoff
- Mode: Simple/Expert user mode configuration
- Strategy: Execution strategy selection (Direct/Hybrid/Mega)
- DesignDocGenerator: Technical design document generation
- DesignDocConverter: External document conversion
- ExternalSkillLoader: Framework-specific skill loading
"""

from .design_doc_converter import DesignDocConverter
from .design_doc_generator import DesignDocGenerator
from .external_skill_loader import ExternalSkillLoader, LoadedSkill
from .feature_orchestrator import FeatureOrchestrator
from .iteration_loop import (
    BatchResult,
    IterationCallbacks,
    IterationConfig,
    IterationLoop,
    IterationMode,
    IterationState,
    IterationStatus,
)
from .parallel_executor import (
    BatchProgress,
    ParallelExecutionConfig,
    ParallelExecutor,
    ParallelProgressDisplay,
    StoryProgress,
    StoryStatus,
    run_parallel_batch,
)
from .mega_generator import MegaPlanGenerator
from .mode import ModeConfig, UserMode
from .orchestrator import Orchestrator, StoryAgent
from .prd_generator import PRDGenerator, create_sample_prd
from .quality_gate import (
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
from .retry_manager import (
    ErrorType,
    FailureRecord,
    RetryConfig,
    RetryManager,
    RetryState,
)
from .strategy import ExecutionStrategy, StrategyDecision

__all__ = [
    # Orchestration
    "Orchestrator",
    "StoryAgent",
    "PRDGenerator",
    "create_sample_prd",
    "MegaPlanGenerator",
    "FeatureOrchestrator",
    # Design documents
    "DesignDocGenerator",
    "DesignDocConverter",
    # External skills
    "ExternalSkillLoader",
    "LoadedSkill",
    # Iteration
    "IterationLoop",
    "IterationConfig",
    "IterationMode",
    "IterationStatus",
    "IterationState",
    "IterationCallbacks",
    "BatchResult",
    # Parallel execution
    "ParallelExecutor",
    "ParallelExecutionConfig",
    "ParallelProgressDisplay",
    "StoryProgress",
    "StoryStatus",
    "BatchProgress",
    "run_parallel_batch",
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
