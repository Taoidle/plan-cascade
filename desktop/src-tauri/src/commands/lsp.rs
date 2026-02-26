//! LSP Tauri Commands
//!
//! IPC commands for language server detection and enrichment management.
//! These commands are called from the frontend Settings UI.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::standalone::StandaloneState;
use crate::models::response::CommandResponse;
use crate::services::orchestrator::index_store::IndexStore;
use crate::services::orchestrator::lsp_enricher::{EnrichmentReport, LspEnricher};
use crate::services::orchestrator::lsp_registry::LspServerRegistry;
use crate::state::AppState;

/// LSP state managed by Tauri.
pub struct LspState {
    pub registry: Arc<LspServerRegistry>,
    pub enricher: Arc<RwLock<Option<LspEnricher>>>,
    pub last_report: Arc<RwLock<Option<EnrichmentReport>>>,
}

impl LspState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LspServerRegistry::new()),
            enricher: Arc::new(RwLock::new(None)),
            last_report: Arc::new(RwLock::new(None)),
        }
    }
}

/// Per-language server status for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerStatus {
    pub language: String,
    pub server_name: String,
    pub detected: bool,
    pub binary_path: Option<String>,
    pub version: Option<String>,
    pub install_hint: String,
}

/// Detect installed language servers.
///
/// Runs PATH + fallback detection for all 5 supported languages and caches
/// the results in the lsp_servers table.
#[tauri::command]
pub async fn detect_lsp_servers(
    lsp_state: State<'_, LspState>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<LspServerStatus>>, String> {
    let detected = lsp_state.registry.detect_all();

    // Cache results in database if available
    let detected_clone = detected.clone();
    let _ = app_state
        .with_database(move |db| {
            let pool = db.pool().clone();
            let store = IndexStore::new(pool);
            for (language, server_name) in &detected_clone {
                let _ = store.upsert_lsp_server(language, "", server_name, None);
            }
            Ok(())
        })
        .await;

    // Build the full status list (detected + not detected)
    let statuses = build_server_statuses(&detected);

    Ok(CommandResponse {
        success: true,
        data: Some(statuses),
        error: None,
    })
}

/// Get the current LSP server status per language.
///
/// Returns cached detection results if available, otherwise runs detection.
#[tauri::command]
pub async fn get_lsp_status(
    lsp_state: State<'_, LspState>,
) -> Result<CommandResponse<Vec<LspServerStatus>>, String> {
    let detected = lsp_state.registry.detect_all();
    let statuses = build_server_statuses(&detected);

    Ok(CommandResponse {
        success: true,
        data: Some(statuses),
        error: None,
    })
}

/// Manually trigger an LSP enrichment pass for a project.
///
/// Creates an LspEnricher with the registry and runs the enrichment pass.
#[tauri::command]
pub async fn trigger_lsp_enrichment(
    project_path: String,
    lsp_state: State<'_, LspState>,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<EnrichmentReport>, String> {
    // Get the IndexStore from app state
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => {
            return Ok(CommandResponse {
                success: false,
                data: None,
                error: Some(format!("Database not available: {}", e)),
            });
        }
    };

    // Emit "enriching" status
    if let Some(mgr) = &*standalone_state.index_manager.read().await {
        mgr.set_lsp_enrichment_status(&project_path, "enriching").await;
    }

    let index_store = Arc::new(IndexStore::new(pool));
    let enricher = LspEnricher::new(Arc::clone(&lsp_state.registry), index_store);

    match enricher.enrich_project(&project_path).await {
        Ok(report) => {
            // Cache the report
            let mut last = lsp_state.last_report.write().await;
            *last = Some(report.clone());

            // Emit "enriched" status
            if let Some(mgr) = &*standalone_state.index_manager.read().await {
                mgr.set_lsp_enrichment_status(&project_path, "enriched").await;
            }

            Ok(CommandResponse {
                success: true,
                data: Some(report),
                error: None,
            })
        }
        Err(e) => {
            // Reset to "none" on failure
            if let Some(mgr) = &*standalone_state.index_manager.read().await {
                mgr.set_lsp_enrichment_status(&project_path, "none").await;
            }

            Ok(CommandResponse {
                success: false,
                data: None,
                error: Some(format!("Enrichment failed: {}", e)),
            })
        }
    }
}

/// Get the results of the last enrichment pass.
#[tauri::command]
pub async fn get_enrichment_report(
    lsp_state: State<'_, LspState>,
) -> Result<CommandResponse<Option<EnrichmentReport>>, String> {
    let report = lsp_state.last_report.read().await.clone();

    Ok(CommandResponse {
        success: true,
        data: Some(report),
        error: None,
    })
}

/// Build the full server status list for all 5 supported languages.
fn build_server_statuses(detected: &HashMap<String, String>) -> Vec<LspServerStatus> {
    let languages = [
        (
            "rust",
            "rust-analyzer",
            "Install via rustup: `rustup component add rust-analyzer`",
        ),
        (
            "python",
            "pyright",
            "Install: `npm i -g pyright` or `pip install pyright`",
        ),
        (
            "go",
            "gopls",
            "Install: `go install golang.org/x/tools/gopls@latest`",
        ),
        (
            "typescript",
            "vtsls",
            "Install: `npm i -g @vtsls/language-server` or `npm i -g typescript-language-server`",
        ),
        (
            "java",
            "jdtls",
            "Install: `brew install jdtls` (macOS) or download from Eclipse",
        ),
    ];

    languages
        .iter()
        .map(|(lang, default_name, install_hint)| {
            let is_detected = detected.contains_key(*lang);
            let server_name = detected
                .get(*lang)
                .cloned()
                .unwrap_or_else(|| default_name.to_string());

            LspServerStatus {
                language: lang.to_string(),
                server_name,
                detected: is_detected,
                binary_path: None, // Could be populated from cache
                version: None,
                install_hint: install_hint.to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_server_statuses_none_detected() {
        let detected = HashMap::new();
        let statuses = build_server_statuses(&detected);

        assert_eq!(statuses.len(), 5);
        for status in &statuses {
            assert!(!status.detected);
            assert!(!status.install_hint.is_empty());
        }
    }

    #[test]
    fn test_build_server_statuses_some_detected() {
        let mut detected = HashMap::new();
        detected.insert("rust".to_string(), "rust-analyzer".to_string());
        detected.insert("go".to_string(), "gopls".to_string());

        let statuses = build_server_statuses(&detected);
        assert_eq!(statuses.len(), 5);

        let rust_status = statuses.iter().find(|s| s.language == "rust").unwrap();
        assert!(rust_status.detected);

        let go_status = statuses.iter().find(|s| s.language == "go").unwrap();
        assert!(go_status.detected);

        let python_status = statuses.iter().find(|s| s.language == "python").unwrap();
        assert!(!python_status.detected);
    }

    #[test]
    fn test_lsp_state_new() {
        let state = LspState::new();
        // Should initialize with 5 adapters
        let languages = state.registry.supported_languages();
        assert_eq!(languages.len(), 5);
    }
}
