//! Individual Tool Implementations
//!
//! Each tool from the executor's match statement is implemented as a separate
//! struct implementing the `Tool` trait. This enables:
//! - Dynamic tool registration/unregistration
//! - Clean per-tool unit testing
//! - Auto-generated tool definitions from trait methods
//! - Foundation for MCP tool integration

pub mod analyze;
pub mod bash;
pub mod browser;
pub mod codebase_search;
pub mod cwd;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod notebook_edit;
pub mod read;
mod scan_utils;
pub mod search_knowledge;
pub mod task;
#[cfg(test)]
pub(crate) mod test_helpers;
pub mod text_utils;
pub mod web_fetch;
pub mod web_search;
pub mod write;

pub use analyze::AnalyzeTool;
pub use bash::BashTool;
pub use browser::BrowserTool;
pub use browser::{
    browser_availability, detect_browser, BrowserAction, BrowserActionResult, BrowserAvailability,
};
pub use codebase_search::CodebaseSearchTool;
pub use cwd::CwdTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use ls::LsTool;
pub use notebook_edit::NotebookEditTool;
pub use read::ReadTool;
pub use search_knowledge::SearchKnowledgeTool;
pub use task::TaskTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
pub use write::WriteTool;
