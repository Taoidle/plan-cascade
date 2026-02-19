//! Individual Tool Implementations
//!
//! Each tool from the executor's match statement is implemented as a separate
//! struct implementing the `Tool` trait. This enables:
//! - Dynamic tool registration/unregistration
//! - Clean per-tool unit testing
//! - Auto-generated tool definitions from trait methods
//! - Foundation for MCP tool integration

pub mod read;
pub mod write;
pub mod edit;
pub mod bash;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod cwd;
pub mod analyze;
pub mod task;
pub mod web_fetch;
pub mod web_search;
pub mod notebook_edit;
pub mod codebase_search;
pub mod browser;

pub use read::ReadTool;
pub use write::WriteTool;
pub use edit::EditTool;
pub use bash::BashTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use ls::LsTool;
pub use cwd::CwdTool;
pub use analyze::AnalyzeTool;
pub use task::TaskTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
pub use notebook_edit::NotebookEditTool;
pub use codebase_search::CodebaseSearchTool;
pub use browser::BrowserTool;
pub use browser::{detect_browser, browser_availability, BrowserAvailability, BrowserAction, BrowserActionResult};
