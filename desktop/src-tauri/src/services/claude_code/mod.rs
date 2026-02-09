//! Claude Code Service
//!
//! Comprehensive integration with Claude Code CLI for GUI mode.
//! Provides process management, session handling, stream processing,
//! thinking management, and tool tracking.

pub mod chat;
pub mod events;
pub mod executor;
pub mod session_manager;
pub mod thinking;
pub mod tools;

pub use chat::{ChatHandler, JsonLineBuffer, SendMessageResult};
pub use events::{
    channels, ClaudeCodeEventEmitter, SessionUpdateEvent, StreamEventPayload, ThinkingUpdateEvent,
    ToolUpdateEvent,
};
pub use executor::{ClaudeCodeExecutor, ClaudeCodeProcess, SpawnConfig};
pub use session_manager::ActiveSessionManager;
pub use thinking::{ThinkingBlock, ThinkingManager};
pub use tools::{ToolExecution, ToolStatus, ToolTracker};
