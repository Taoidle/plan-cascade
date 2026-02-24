//! Plan Cascade Tools
//!
//! Core types and trait definitions for the Plan Cascade tool executor system.
//!
//! This crate provides the foundational types that can be compiled independently:
//! - `ToolResult` - execution result type
//! - `ReadCacheEntry` - file read deduplication cache entry
//! - `Tool` trait - unified tool interface
//! - `ToolRegistry` - dynamic tool registration and dispatch
//! - `FunctionTool` - closure-based tool creation
//! - `ParsedToolCall` - prompt-fallback tool call parsing
//!
//! Tool implementations (Read, Write, Edit, Bash, Glob, Grep, etc.) and
//! orchestrator-coupled services (MCP, WebFetch, CodebaseSearch) live in the
//! main crate's `services::tools` module and are re-exported there.

pub mod executor;
pub mod prompt_fallback;
pub mod trait_def;

// Re-export core types
pub use executor::{ReadCacheEntry, ToolResult};
pub use prompt_fallback::{
    build_tool_call_instructions, extract_text_without_tool_calls, format_tool_result,
    parse_tool_calls, ParsedToolCall,
};
pub use trait_def::{FunctionTool, Tool, ToolFilterContext, ToolRegistry, Toolset};
