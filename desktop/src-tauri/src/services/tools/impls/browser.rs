//! Browser Automation Tool
//!
//! Provides headless browser automation capabilities for agents.
//! Types and trait are defined unconditionally; the actual browser
//! implementation is gated behind `#[cfg(feature = "browser")]` to
//! avoid pulling in heavy chromiumoxide dependencies by default.
//!
//! ## Tools
//! - `navigate(url)` - Navigate to a URL
//! - `click(selector)` - Click an element matching a CSS selector
//! - `type_text(selector, text)` - Type text into an input element
//! - `screenshot()` - Take a screenshot of the current page
//! - `extract_text(selector)` - Extract text content from an element
//! - `wait_for(selector, timeout)` - Wait for an element to appear
//!
//! Story 005: Browser automation tool types and feature-gated implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

// ============================================================================
// Browser Action Types (unconditional)
// ============================================================================

/// Actions supported by the browser tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to a URL.
    Navigate {
        /// Target URL to navigate to.
        url: String,
    },
    /// Click an element matching a CSS selector.
    Click {
        /// CSS selector for the target element.
        selector: String,
    },
    /// Type text into an input element.
    TypeText {
        /// CSS selector for the input element.
        selector: String,
        /// Text to type.
        text: String,
    },
    /// Take a screenshot of the current page.
    Screenshot,
    /// Extract text content from elements matching a CSS selector.
    ExtractText {
        /// CSS selector for the target element(s).
        selector: String,
    },
    /// Wait for an element matching a CSS selector to appear.
    WaitFor {
        /// CSS selector to wait for.
        selector: String,
        /// Maximum wait time in milliseconds (default: 5000).
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
}

fn default_timeout() -> u64 {
    5000
}

/// Result of a browser action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Output data (e.g., extracted text, screenshot path).
    pub output: Option<String>,
    /// Current page URL after the action.
    pub current_url: Option<String>,
    /// Current page title after the action.
    pub page_title: Option<String>,
}

// ============================================================================
// BrowserTool (unconditional struct, feature-gated internals)
// ============================================================================

/// Browser automation tool that wraps headless browser functionality.
///
/// Uses lazy initialization to avoid starting the browser process
/// until the first action is requested. The actual browser instance
/// is gated behind `#[cfg(feature = "browser")]`.
pub struct BrowserTool {
    /// Whether the tool has been lazily initialized.
    _initialized: std::sync::atomic::AtomicBool,
}

impl BrowserTool {
    /// Create a new BrowserTool (lazy initialization).
    pub fn new() -> Self {
        Self {
            _initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Parse a BrowserAction from tool arguments.
    fn parse_action(args: &Value) -> Result<BrowserAction, String> {
        let action_str = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required 'action' parameter".to_string())?;

        match action_str {
            "navigate" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'url' parameter for navigate action".to_string())?;
                Ok(BrowserAction::Navigate {
                    url: url.to_string(),
                })
            }
            "click" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'selector' parameter for click action".to_string())?;
                Ok(BrowserAction::Click {
                    selector: selector.to_string(),
                })
            }
            "type_text" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for type_text action".to_string()
                    })?;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'text' parameter for type_text action".to_string()
                    })?;
                Ok(BrowserAction::TypeText {
                    selector: selector.to_string(),
                    text: text.to_string(),
                })
            }
            "screenshot" => Ok(BrowserAction::Screenshot),
            "extract_text" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for extract_text action".to_string()
                    })?;
                Ok(BrowserAction::ExtractText {
                    selector: selector.to_string(),
                })
            }
            "wait_for" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for wait_for action".to_string()
                    })?;
                let timeout_ms = args
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(default_timeout());
                Ok(BrowserAction::WaitFor {
                    selector: selector.to_string(),
                    timeout_ms,
                })
            }
            other => Err(format!(
                "Unknown action '{}'. Supported: navigate, click, type_text, screenshot, extract_text, wait_for",
                other
            )),
        }
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "Browser"
    }

    fn description(&self) -> &str {
        "Headless browser automation tool. Supports actions: navigate(url), click(selector), \
         type_text(selector, text), screenshot(), extract_text(selector), wait_for(selector, timeout_ms). \
         Requires the 'browser' feature to be enabled for actual browser interaction."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            ParameterSchema::string(Some(
                "The browser action: navigate, click, type_text, screenshot, extract_text, wait_for",
            )),
        );
        properties.insert(
            "url".to_string(),
            ParameterSchema::string(Some("URL to navigate to (for 'navigate' action)")),
        );
        properties.insert(
            "selector".to_string(),
            ParameterSchema::string(Some(
                "CSS selector for the target element (for click, type_text, extract_text, wait_for)",
            )),
        );
        properties.insert(
            "text".to_string(),
            ParameterSchema::string(Some("Text to type (for 'type_text' action)")),
        );
        properties.insert(
            "timeout_ms".to_string(),
            ParameterSchema::integer(Some("Max wait time in ms (for 'wait_for', default: 5000)")),
        );

        ParameterSchema::object(
            Some("Browser automation parameters"),
            properties,
            vec!["action".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // Parse the action from arguments
        let action = match Self::parse_action(&args) {
            Ok(a) => a,
            Err(e) => return ToolResult::err(e),
        };

        // Feature-gated execution
        #[cfg(feature = "browser")]
        {
            // When the browser feature is enabled, this would delegate to
            // chromiumoxide or similar. For now, return a placeholder.
            let _ = action;
            return ToolResult::err(
                "Browser feature is enabled but the browser backend is not yet implemented."
                    .to_string(),
            );
        }

        #[cfg(not(feature = "browser"))]
        {
            // When the browser feature is disabled, return an informative error.
            let action_name = match &action {
                BrowserAction::Navigate { .. } => "navigate",
                BrowserAction::Click { .. } => "click",
                BrowserAction::TypeText { .. } => "type_text",
                BrowserAction::Screenshot => "screenshot",
                BrowserAction::ExtractText { .. } => "extract_text",
                BrowserAction::WaitFor { .. } => "wait_for",
            };
            ToolResult::err(format!(
                "Browser tool action '{}' requires the 'browser' feature to be enabled. \
                 Recompile with `--features browser` to use browser automation.",
                action_name
            ))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn make_ctx() -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: PathBuf::from("/tmp/test"),
            working_directory: Arc::new(Mutex::new(PathBuf::from("/tmp/test"))),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::new(crate::services::tools::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            task_context: None,
        }
    }

    // ── Tool identity tests ──────────────────────────────────────────

    #[test]
    fn test_browser_tool_name() {
        let tool = BrowserTool::new();
        assert_eq!(tool.name(), "Browser");
    }

    #[test]
    fn test_browser_tool_description() {
        let tool = BrowserTool::new();
        assert!(tool.description().contains("browser automation"));
    }

    #[test]
    fn test_browser_tool_is_long_running() {
        let tool = BrowserTool::new();
        assert!(tool.is_long_running());
    }

    #[test]
    fn test_browser_tool_default() {
        let tool = BrowserTool::default();
        assert_eq!(tool.name(), "Browser");
    }

    // ── Action parsing tests ─────────────────────────────────────────

    #[test]
    fn test_parse_navigate_action() {
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_parse_click_action() {
        let args = serde_json::json!({
            "action": "click",
            "selector": "#submit-btn"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::Click { selector } => assert_eq!(selector, "#submit-btn"),
            _ => panic!("Expected Click"),
        }
    }

    #[test]
    fn test_parse_type_text_action() {
        let args = serde_json::json!({
            "action": "type_text",
            "selector": "input[name='email']",
            "text": "test@example.com"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::TypeText { selector, text } => {
                assert_eq!(selector, "input[name='email']");
                assert_eq!(text, "test@example.com");
            }
            _ => panic!("Expected TypeText"),
        }
    }

    #[test]
    fn test_parse_screenshot_action() {
        let args = serde_json::json!({"action": "screenshot"});
        let action = BrowserTool::parse_action(&args).unwrap();
        assert!(matches!(action, BrowserAction::Screenshot));
    }

    #[test]
    fn test_parse_extract_text_action() {
        let args = serde_json::json!({
            "action": "extract_text",
            "selector": ".main-content"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::ExtractText { selector } => assert_eq!(selector, ".main-content"),
            _ => panic!("Expected ExtractText"),
        }
    }

    #[test]
    fn test_parse_wait_for_action() {
        let args = serde_json::json!({
            "action": "wait_for",
            "selector": ".loaded",
            "timeout_ms": 10000
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::WaitFor {
                selector,
                timeout_ms,
            } => {
                assert_eq!(selector, ".loaded");
                assert_eq!(timeout_ms, 10000);
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    #[test]
    fn test_parse_wait_for_default_timeout() {
        let args = serde_json::json!({
            "action": "wait_for",
            "selector": ".loaded"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::WaitFor { timeout_ms, .. } => {
                assert_eq!(timeout_ms, 5000);
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    #[test]
    fn test_parse_unknown_action() {
        let args = serde_json::json!({"action": "fly_to_moon"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[test]
    fn test_parse_missing_action() {
        let args = serde_json::json!({"url": "https://example.com"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required 'action'"));
    }

    #[test]
    fn test_parse_navigate_missing_url() {
        let args = serde_json::json!({"action": "navigate"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'url'"));
    }

    #[test]
    fn test_parse_click_missing_selector() {
        let args = serde_json::json!({"action": "click"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'selector'"));
    }

    #[test]
    fn test_parse_type_text_missing_text() {
        let args = serde_json::json!({
            "action": "type_text",
            "selector": "#input"
        });
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'text'"));
    }

    // ── Execution tests (feature-gated) ──────────────────────────────

    #[tokio::test]
    async fn test_execute_without_browser_feature() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let result = tool.execute(&ctx, args).await;
        // Without the browser feature, it should return an error
        assert!(!result.success);
        assert!(result.error.unwrap().contains("browser"));
    }

    #[tokio::test]
    async fn test_execute_with_bad_args() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let args = serde_json::json!({});
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing required"));
    }

    // ── BrowserAction serialization tests ────────────────────────────

    #[test]
    fn test_browser_action_navigate_serde() {
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"navigate\""));
        assert!(json.contains("\"url\":\"https://example.com\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_browser_action_screenshot_serde() {
        let action = BrowserAction::Screenshot;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"screenshot\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, BrowserAction::Screenshot));
    }

    #[test]
    fn test_browser_action_result_serde() {
        let result = BrowserActionResult {
            success: true,
            output: Some("Page loaded".to_string()),
            current_url: Some("https://example.com".to_string()),
            page_title: Some("Example".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: BrowserActionResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.output, Some("Page loaded".to_string()));
        assert_eq!(parsed.current_url, Some("https://example.com".to_string()));
        assert_eq!(parsed.page_title, Some("Example".to_string()));
    }

    // ── Parameters schema test ───────────────────────────────────────

    #[test]
    fn test_parameters_schema_has_action() {
        let tool = BrowserTool::new();
        let schema = tool.parameters_schema();
        // The schema should have 'action' as required
        let json = serde_json::to_value(&schema).unwrap();
        let required = json.get("required").and_then(|v| v.as_array());
        assert!(required.is_some());
        let required_list: Vec<&str> = required
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(required_list.contains(&"action"));
    }
}
