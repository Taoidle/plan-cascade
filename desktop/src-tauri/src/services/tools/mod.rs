//! Tool Executor Module
//!
//! Implements core tools for agentic operation:
//! - Read: File reading with line ranges (PDF, DOCX, XLSX, Jupyter, Image support)
//! - Write: File creation/overwrite
//! - Edit: String replacement
//! - Bash: Command execution with timeout
//! - Glob: File pattern matching
//! - Grep: Content search with regex
//! - LS: Directory listing
//! - Cwd: Current working directory
//! - WebFetch: Web page fetching with HTML-to-markdown conversion
//! - WebSearch: Pluggable web search (Tavily, Brave, DuckDuckGo)
//! - NotebookEdit: Jupyter notebook cell editing

pub mod definitions;
pub mod executor;
pub mod file_parsers;
pub mod notebook_edit;
pub mod prompt_fallback;
pub mod system_prompt;
pub mod task_spawner;
pub mod web_fetch;
pub mod web_search;

pub use definitions::{get_basic_tool_definitions, get_tool_definitions};
pub use executor::{ToolExecutor, ToolResult};
pub use prompt_fallback::{
    build_tool_call_instructions, extract_text_without_tool_calls, format_tool_result,
    parse_tool_calls, ParsedToolCall,
};
pub use system_prompt::{build_system_prompt, merge_system_prompts};
pub use task_spawner::{TaskContext, TaskExecutionResult, TaskSpawner};
