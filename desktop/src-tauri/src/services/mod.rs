//! Services
//!
//! Business logic services for the application.
//! Services handle the core functionality and are called by commands.

pub mod agent;
pub mod agent_composer;
pub mod agent_executor;
pub mod analytics;
pub mod claude_code;
pub mod context;
pub mod dependency;
pub mod design;
pub mod fallback;
pub mod guardrail;
pub mod iteration;
pub mod llm;
pub mod markdown;
pub mod mcp;
pub mod mega;
pub mod memory;
pub mod orchestrator;
pub mod plugins;
pub mod phase;
pub mod project;
pub mod proxy;
pub mod quality_gates;
pub mod recovery;
pub mod session;
pub mod skills;
pub mod spec_interview;
pub mod strategy;
pub mod streaming;
pub mod sync;
pub mod timeline;
pub mod tools;
pub mod remote;
pub mod worktree;

pub use agent::AgentService;
pub use agent_executor::{AgentEvent, AgentExecutor, ExecutionHandle, ExecutorConfig, ToolFilter};
pub use context::{ContextError, ContextFilter, ContextFilterConfig, ContextTag, StoryContext};
pub use dependency::{Batch, DependencyAnalyzer, DependencyError};
pub use design::{
    DesignDocGenerator, DesignDocImporter, DesignDocLoader, DESIGN_DOC_FILENAME, WORKTREES_DIR,
};
pub use fallback::{
    AgentFallbackChain, FailureReason, FallbackAttempt, FallbackConfig, FallbackError,
    FallbackExecutionLog,
};
pub use iteration::{IterationEvent, IterationLoop, IterationLoopConfig, IterationLoopError};
pub use mega::{MegaOrchestrator, MegaOrchestratorConfig, MegaOrchestratorError};
pub use phase::{Phase, PhaseConfig, PhaseError, PhaseManager};
pub use quality_gates::{ProjectDetector, QualityGateRunner, QualityGatesStore, ValidatorRegistry};
pub use recovery::{IncompleteTask, RecoveryDetector, ResumeEngine, ResumeEvent, ResumeResult};
pub use strategy::{
    ExecutionStrategy, Intent, IntentClassifier, IntentResult, StrategyAnalyzer, StrategyDecision,
};
pub use sync::{start_default_watches, FileWatcherService, WatchTarget, WatcherConfig};
pub use worktree::{GitOps, PlanningConfigService, WorktreeManager};
