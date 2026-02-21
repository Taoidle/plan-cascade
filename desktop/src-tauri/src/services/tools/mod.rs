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
//! - MCP Tools: Dynamically loaded tools from MCP servers

pub mod definitions;
pub mod executor;
pub mod file_parsers;
pub mod impls;
pub mod mcp_adapter;
pub mod mcp_client;
pub mod mcp_manager;
pub mod mcp_schema;
pub mod notebook_edit;
pub mod prompt_fallback;
pub mod system_prompt;
pub mod task_spawner;
pub mod trait_def;
pub mod web_fetch;
pub mod web_search;

pub use definitions::{
    get_basic_tool_definitions_from_registry, get_tool_definitions_from_registry,
};
pub use executor::{ReadCacheEntry, ToolExecutor, ToolResult};
pub use mcp_adapter::McpToolAdapter;
pub use mcp_client::{McpClient, McpServerConfig, McpToolInfo, McpTransportConfig};
pub use mcp_manager::{ConnectedServerInfo, McpManager};
pub use mcp_schema::sanitize_schema;
pub use prompt_fallback::{
    build_tool_call_instructions, extract_text_without_tool_calls, format_tool_result,
    parse_tool_calls, ParsedToolCall,
};
pub use system_prompt::{
    build_memory_section, build_project_summary, build_skills_section,
    build_sub_agent_tool_guidance, build_system_prompt, build_system_prompt_with_memories,
    detect_language, merge_system_prompts,
};
pub use task_spawner::{TaskContext, TaskExecutionResult, TaskSpawner};
pub use trait_def::{Tool, ToolExecutionContext, ToolRegistry};
