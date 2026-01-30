//! Services
//!
//! Business logic services for the application.
//! Services handle the core functionality and are called by commands.

pub mod agent;
pub mod agent_executor;
pub mod analytics;
pub mod claude_code;
pub mod markdown;
pub mod mcp;
pub mod project;
pub mod quality_gates;
pub mod session;
pub mod streaming;
pub mod llm;
pub mod tools;
pub mod orchestrator;
pub mod timeline;
pub mod sync;
pub mod worktree;

pub use agent::AgentService;
pub use agent_executor::{AgentExecutor, ExecutorConfig, AgentEvent, ExecutionHandle, ToolFilter};
pub use quality_gates::{ProjectDetector, ValidatorRegistry, QualityGateRunner, QualityGatesStore};
pub use sync::{FileWatcherService, WatcherConfig, WatchTarget, start_default_watches};
pub use worktree::{WorktreeManager, PlanningConfigService, GitOps};
