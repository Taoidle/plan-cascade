//! Services
//!
//! Business logic services for the application.
//! Services handle the core functionality and are called by commands.

pub mod claude_code;
pub mod markdown;
pub mod mcp;
pub mod project;
pub mod session;
pub mod streaming;
pub mod llm;
pub mod tools;
pub mod orchestrator;
pub mod timeline;

// Service modules will be added as they are implemented
// pub mod agent;
// pub mod analytics;
