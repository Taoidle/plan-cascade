//! Services
//!
//! Business logic services for the application.
//! Services handle the core functionality and are called by commands.

pub mod agent;
pub mod agent_executor;
pub mod analytics;
pub mod claude_code;
pub mod context;
pub mod dependency;
pub mod design;
pub mod fallback;
pub mod iteration;
pub mod llm;
pub mod markdown;
pub mod mcp;
pub mod mega;
pub mod orchestrator;
pub mod phase;
pub mod project;
pub mod quality_gates;
pub mod session;
pub mod streaming;
pub mod sync;
pub mod timeline;
pub mod tools;
pub mod worktree;

pub use agent::AgentService;
pub use agent_executor::{AgentExecutor, ExecutorConfig, AgentEvent, ExecutionHandle, ToolFilter};
pub use dependency::{DependencyAnalyzer, Batch, DependencyError};
pub use design::{DesignDocLoader, DESIGN_DOC_FILENAME, WORKTREES_DIR};
pub use iteration::{IterationLoop, IterationLoopConfig, IterationLoopError, IterationEvent};
pub use mega::{MegaOrchestrator, MegaOrchestratorConfig, MegaOrchestratorError};
pub use phase::{Phase, PhaseConfig, PhaseManager, PhaseError};
pub use context::{ContextFilter, ContextFilterConfig, ContextTag, StoryContext, ContextError};
pub use fallback::{AgentFallbackChain, FallbackConfig, FallbackError, FailureReason, FallbackAttempt, FallbackExecutionLog};
pub use quality_gates::{ProjectDetector, ValidatorRegistry, QualityGateRunner, QualityGatesStore};
pub use sync::{FileWatcherService, WatcherConfig, WatchTarget, start_default_watches};
pub use worktree::{WorktreeManager, PlanningConfigService, GitOps};
