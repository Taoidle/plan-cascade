//! Services
//!
//! Business logic services for the application.
//! Services handle the core functionality and are called by commands.

pub mod a2a;
pub mod agent;
pub mod agent_composer;
pub mod agent_executor;
pub mod analytics;
pub mod artifacts;
pub mod claude_code;
pub mod context;
pub mod core;
pub mod dependency;
pub mod design;
pub mod fallback;
pub mod file_change_tracker;
pub mod git;
pub mod graph_workflow;
pub mod guardrail;
pub mod iteration;
pub mod knowledge;
pub mod llm;
pub mod markdown;
pub mod mcp;
pub mod mega;
pub mod memory;
pub mod orchestrator;
pub mod persona;
pub mod phase;
pub mod plan_mode;
pub mod plugins;
pub mod project;
pub mod prompt;
pub mod proxy;
pub mod quality_gates;
pub mod recovery;
pub mod remote;
pub mod session;
pub mod settings_export;
pub mod skills;
pub mod spec_interview;
pub mod strategy;
pub mod streaming;
pub mod sync;
pub mod task_mode;
pub mod timeline;
pub mod tools;
pub mod webhook;
pub mod worktree;

pub use agent::AgentService;
pub use agent_composer::types::AgentEvent;
pub use agent_executor::{AgentExecutor, ExecutionHandle, ExecutorConfig, ToolFilter};
pub use context::{ContextError, ContextFilter, ContextFilterConfig, ContextTag, StoryContext};
pub use dependency::{Batch, DependencyAnalyzer, DependencyError};
pub use design::{
    DesignDocGenerator, DesignDocImporter, DesignDocLoader, DESIGN_DOC_FILENAME, WORKTREES_DIR,
};
pub use fallback::{
    AgentFallbackChain, FailureReason, FallbackAttempt, FallbackConfig, FallbackError,
    FallbackExecutionLog,
};
pub use git::{GitLlmAssist, GitService, GitWatcher};
pub use iteration::{IterationEvent, IterationLoop, IterationLoopConfig, IterationLoopError};
pub use mega::{MegaOrchestrator, MegaOrchestratorConfig, MegaOrchestratorError};
pub use phase::{Phase, PhaseConfig, PhaseError, PhaseManager};
pub use quality_gates::{ProjectDetector, QualityGateRunner, QualityGatesStore, ValidatorRegistry};
pub use recovery::{IncompleteTask, RecoveryDetector, ResumeEngine, ResumeEvent, ResumeResult};
pub use strategy::{
    analyze_task_for_mode, Benefit, ExecutionMode, ExecutionStrategy, Intent, IntentClassifier,
    IntentResult, RiskLevel, StrategyAnalysis, StrategyAnalyzer, StrategyDecision,
};
pub use sync::{start_default_watches, FileWatcherService, WatchTarget, WatcherConfig};
pub use worktree::{GitOps, PlanningConfigService, WorktreeManager};
