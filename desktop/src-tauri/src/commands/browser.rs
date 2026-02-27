//! Browser Automation Commands
//!
//! Tauri commands for headless browser automation with runtime Chrome detection.
//! Provides `execute_browser_action` for frontend access and
//! `get_browser_status` for settings UI availability display.

use serde::{Deserialize, Serialize};

use crate::models::response::CommandResponse;
use crate::services::tools::impls::{
    browser_availability, BrowserAction, BrowserActionResult, BrowserAvailability,
};

/// Execute a browser automation action.
///
/// Accepts a `BrowserAction` enum variant and returns the result.
/// When no browser is detected at runtime, returns a clear error message
/// instead of crashing.
///
/// # Arguments
/// * `action` - The browser action to execute (navigate, click, type_text, etc.)
///
/// # Returns
/// * `CommandResponse<BrowserActionResult>` - Result of the browser action
#[tauri::command]
pub async fn execute_browser_action(
    action: BrowserAction,
) -> Result<CommandResponse<BrowserActionResult>, String> {
    use crate::services::tools::impls::BrowserTool;
    use crate::services::tools::trait_def::Tool;

    let availability = browser_availability();

    if !availability.browser_detected {
        return Ok(CommandResponse::err(format!(
            "No Chrome or Chromium browser found on this system. \
             Please install Google Chrome or Chromium to use browser automation. \
             Platform: {}",
            std::env::consts::OS
        )));
    }

    if !availability.feature_compiled {
        return Ok(CommandResponse::err(format!(
            "Chrome/Chromium detected at '{}', but the 'browser' feature was not compiled in. \
             Rebuild with `--features browser` to enable browser automation.",
            availability
                .browser_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )));
    }

    // Both feature compiled and browser found — execute the action
    let tool = BrowserTool::new();
    let ctx = make_minimal_ctx();
    let args =
        serde_json::to_value(&action).map_err(|e| format!("Failed to serialize action: {}", e))?;

    let result = tool.execute(&ctx, args).await;

    if result.success {
        let action_result: BrowserActionResult = if let Some(output) = &result.output {
            serde_json::from_str(output).unwrap_or(BrowserActionResult {
                success: true,
                output: result.output.clone(),
                current_url: None,
                page_title: None,
            })
        } else {
            BrowserActionResult {
                success: true,
                output: None,
                current_url: None,
                page_title: None,
            }
        };
        Ok(CommandResponse::ok(action_result))
    } else {
        Ok(CommandResponse::err(
            result
                .error
                .unwrap_or_else(|| "Unknown browser error".to_string()),
        ))
    }
}

/// Get the current browser automation availability status.
///
/// Returns whether the browser feature is compiled in and whether a
/// Chrome/Chromium binary was detected at runtime. Used by the settings
/// UI to show browser availability status.
#[tauri::command]
pub async fn get_browser_status() -> Result<CommandResponse<BrowserAvailability>, String> {
    let availability = browser_availability();
    Ok(CommandResponse::ok(availability))
}

/// Create a minimal ToolExecutionContext for browser command usage.
///
/// The browser tool does not need most of the context fields (project root,
/// read cache, etc.), so we create a lightweight context.
fn make_minimal_ctx() -> crate::services::tools::trait_def::ToolExecutionContext {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    crate::services::tools::trait_def::ToolExecutionContext {
        session_id: "browser-command".to_string(),
        project_root: PathBuf::from("."),
        working_directory: Arc::new(Mutex::new(PathBuf::from("."))),
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
        core_context: None,
        file_change_tracker: None,
        permission_gate: None,
        knowledge_pipeline: None,
        knowledge_project_id: None,
        knowledge_collection_filter: None,
        knowledge_document_filter: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_browser_status_returns_availability() {
        // browser_availability() should not panic
        let avail = browser_availability();
        // Verify all fields are accessible
        let _ = avail.feature_compiled;
        let _ = avail.browser_detected;
        let _ = avail.browser_path;
        let _ = avail.is_available();
        let msg = avail.status_message();
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_make_minimal_ctx() {
        let ctx = make_minimal_ctx();
        assert_eq!(ctx.session_id, "browser-command");
    }

    #[test]
    fn test_browser_action_deserializable_for_command() {
        // Verify that BrowserAction can be deserialized from JSON
        // as Tauri commands receive JSON from the frontend
        let json = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let action: BrowserAction = serde_json::from_value(json).unwrap();
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_browser_action_result_serializable_for_command() {
        // Verify that BrowserActionResult can be serialized as JSON
        // for Tauri command response
        let result = BrowserActionResult {
            success: true,
            output: Some("Page loaded".to_string()),
            current_url: Some("https://example.com".to_string()),
            page_title: Some("Example".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.is_empty());
    }

    #[tokio::test]
    async fn test_execute_browser_action_command_structure() {
        // Test that the command works without panicking
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let result = execute_browser_action(action).await;
        // Should return Ok (the error is inside CommandResponse, not the Result)
        assert!(result.is_ok());
        let response = result.unwrap();
        // Response will be error if no browser found or feature not compiled
        // but it should always be a valid CommandResponse
        let _ = response.success;
        let _ = response.data;
        let _ = response.error;
    }

    #[tokio::test]
    async fn test_get_browser_status_command_structure() {
        let result = get_browser_status().await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let data = response.data.unwrap();
        // Verify availability fields
        let _ = data.feature_compiled;
        let _ = data.browser_detected;
    }
}
