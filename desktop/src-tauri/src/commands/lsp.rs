//! LSP Tauri Commands
//!
//! IPC commands for language server detection and enrichment management.
//! These commands are called from the frontend Settings UI.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;

use crate::commands::standalone::StandaloneState;
use crate::models::response::CommandResponse;
use crate::services::orchestrator::index_store::{IndexStore, LspServerInfo};
use crate::services::orchestrator::lsp_enricher::{EnrichmentReport, LspEnricher};
use crate::services::orchestrator::lsp_registry::{DetectedServer, LspServerRegistry};
use crate::state::AppState;
use crate::utils::configure_background_process;

const LSP_PREFERENCES_KEY: &str = "lsp_preferences_v1";
const DEFAULT_ENRICHMENT_DEBOUNCE_MS: u64 = 3000;
const MIN_ENRICHMENT_DEBOUNCE_MS: u64 = 500;
const MAX_ENRICHMENT_DEBOUNCE_MS: u64 = 60_000;

/// LSP state managed by Tauri.
pub struct LspState {
    pub registry: Arc<LspServerRegistry>,
    pub last_report: Arc<RwLock<Option<EnrichmentReport>>>,
    /// Guard against concurrent full enrichment + incremental enrichment.
    /// Shared with `IndexManager`'s debounce loop via `get_enrichment_lock`.
    pub enrichment_lock: Arc<tokio::sync::Mutex<()>>,
}

impl LspState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LspServerRegistry::new()),
            last_report: Arc::new(RwLock::new(None)),
            enrichment_lock: Arc::new(tokio::sync::Mutex::new(())),
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
    pub detected_at: Option<String>,
    pub install_hint: String,
}

/// LSP preferences persisted in the settings table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspPreferences {
    #[serde(default, alias = "auto_enrich")]
    pub auto_enrich: bool,
    #[serde(
        default = "default_incremental_debounce_ms",
        alias = "incremental_debounce_ms"
    )]
    pub incremental_debounce_ms: u64,
}

impl Default for LspPreferences {
    fn default() -> Self {
        Self {
            auto_enrich: false,
            incremental_debounce_ms: DEFAULT_ENRICHMENT_DEBOUNCE_MS,
        }
    }
}

fn default_incremental_debounce_ms() -> u64 {
    DEFAULT_ENRICHMENT_DEBOUNCE_MS
}

/// Detect installed language servers.
///
/// Runs PATH + fallback detection for all 5 supported languages and caches
/// the results in the lsp_servers table.
#[tauri::command]
pub async fn detect_lsp_servers(
    lsp_state: State<'_, LspState>,
    app_state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<CommandResponse<Vec<LspServerStatus>>, String> {
    let statuses = detect_and_cache_servers(
        lsp_state.inner(),
        app_state.inner(),
        force_refresh.unwrap_or(false),
    )
    .await;

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
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<LspServerStatus>>, String> {
    let cached_rows = app_state
        .with_database(|db| {
            let store = IndexStore::new(db.pool().clone());
            store.get_lsp_servers()
        })
        .await
        .unwrap_or_default();

    if !cached_rows.is_empty() {
        let statuses = build_server_statuses(None, &cached_rows);
        return Ok(CommandResponse {
            success: true,
            data: Some(statuses),
            error: None,
        });
    }

    let statuses = detect_and_cache_servers(lsp_state.inner(), app_state.inner(), false).await;

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
                error: Some(format!("LSP_DB_UNAVAILABLE: {}", e)),
            });
        }
    };

    // Acquire enrichment lock from IndexManager (shared with incremental debounce loop).
    // Fall back to the local lock if IndexManager is not available yet.
    let lock = if let Some(mgr) = &*standalone_state.index_manager.read().await {
        mgr.get_enrichment_lock(&project_path).await
    } else {
        Arc::clone(&lsp_state.enrichment_lock)
    };
    let _guard = lock.lock().await;

    // Emit "enriching" status
    if let Some(mgr) = &*standalone_state.index_manager.read().await {
        mgr.set_lsp_enrichment_status(&project_path, "enriching")
            .await;
    }

    let index_store = Arc::new(IndexStore::new(pool));
    let enricher = LspEnricher::new(Arc::clone(&lsp_state.registry), index_store);

    match enricher.enrich_project(&project_path).await {
        Ok(report) => {
            if report.languages_enriched.is_empty() {
                if let Some(mgr) = &*standalone_state.index_manager.read().await {
                    mgr.set_lsp_enrichment_status(&project_path, "none").await;
                    mgr.set_enrichment_enabled(&project_path, false).await;
                }
                return Ok(CommandResponse {
                    success: false,
                    data: None,
                    error: Some(
                        "LSP_NO_LIVE_CLIENTS: No language server clients are available for enrichment"
                            .to_string(),
                    ),
                });
            }

            // Cache the report
            let mut last = lsp_state.last_report.write().await;
            *last = Some(report.clone());

            // Emit "enriched" status and update cached enrichment flag
            if let Some(mgr) = &*standalone_state.index_manager.read().await {
                mgr.set_lsp_enrichment_status(&project_path, "enriched")
                    .await;
                mgr.set_enrichment_enabled(&project_path, true).await;
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
                mgr.set_enrichment_enabled(&project_path, false).await;
            }

            let error_text = e.to_string();
            let code = if error_text.contains("LSP_NO_SERVERS_DETECTED") {
                "LSP_NO_SERVERS_DETECTED"
            } else if error_text.contains("LSP_NO_LIVE_CLIENTS") {
                "LSP_NO_LIVE_CLIENTS"
            } else {
                "LSP_ENRICHMENT_FAILED"
            };

            Ok(CommandResponse {
                success: false,
                data: None,
                error: Some(format!("{}: {}", code, error_text)),
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

/// Get LSP preferences from persisted settings.
#[tauri::command]
pub async fn get_lsp_preferences(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<LspPreferences>, String> {
    let prefs = app_state
        .with_database(|db| {
            let parsed = db
                .get_setting(LSP_PREFERENCES_KEY)?
                .and_then(|json| serde_json::from_str::<LspPreferences>(&json).ok())
                .unwrap_or_default();
            Ok(normalize_lsp_preferences(parsed))
        })
        .await
        .unwrap_or_default();

    Ok(CommandResponse::ok(prefs))
}

/// Persist LSP preferences.
#[tauri::command]
pub async fn set_lsp_preferences(
    preferences: LspPreferences,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<LspPreferences>, String> {
    if let Err(message) = validate_lsp_preferences(&preferences) {
        return Ok(CommandResponse::err(message));
    }
    let normalized = normalize_lsp_preferences(preferences);

    let serialized = match serde_json::to_string(&normalized) {
        Ok(value) => value,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to serialize LSP preferences: {}",
                e
            )))
        }
    };

    let persist = app_state
        .with_database(|db| db.set_setting(LSP_PREFERENCES_KEY, &serialized))
        .await;

    if let Err(e) = persist {
        return Ok(CommandResponse::err(format!(
            "Failed to persist LSP preferences: {}",
            e
        )));
    }

    if let Some(mgr) = &*standalone_state.index_manager.read().await {
        mgr.set_lsp_incremental_debounce_ms(normalized.incremental_debounce_ms)
            .await;
    }

    Ok(CommandResponse::ok(normalized))
}

/// Get the current enrichment debounce time in milliseconds.
///
/// Compatibility wrapper for older frontend callers.
#[tauri::command]
pub async fn get_enrichment_debounce(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<u64>, String> {
    let prefs = app_state
        .with_database(|db| {
            let parsed = db
                .get_setting(LSP_PREFERENCES_KEY)?
                .and_then(|json| serde_json::from_str::<LspPreferences>(&json).ok())
                .unwrap_or_default();
            Ok(normalize_lsp_preferences(parsed))
        })
        .await
        .unwrap_or_default();

    Ok(CommandResponse {
        success: true,
        data: Some(prefs.incremental_debounce_ms),
        error: None,
    })
}

async fn detect_and_cache_servers(
    lsp_state: &LspState,
    app_state: &AppState,
    force_refresh: bool,
) -> Vec<LspServerStatus> {
    let mut detected = lsp_state.registry.detect_all_with_options(force_refresh);

    // Best-effort version probing with timeout; do not fail detection flow.
    for server in detected.values_mut() {
        server.version = probe_server_version(server).await;
    }

    let detected_for_db = detected.clone();
    let _ = app_state
        .with_database(move |db| {
            let store = IndexStore::new(db.pool().clone());
            for language in LspServerRegistry::new().supported_languages() {
                if let Some(server) = detected_for_db.get(language) {
                    let _ = store.upsert_lsp_server(
                        language,
                        &server.binary_path.to_string_lossy(),
                        &server.server_name,
                        server.version.as_deref(),
                    );
                } else {
                    let _ = store.delete_lsp_server(language);
                }
            }
            Ok(())
        })
        .await;

    let cached_rows = app_state
        .with_database(|db| {
            let store = IndexStore::new(db.pool().clone());
            store.get_lsp_servers()
        })
        .await
        .unwrap_or_default();

    build_server_statuses(Some(&detected), &cached_rows)
}

async fn probe_server_version(server: &DetectedServer) -> Option<String> {
    let mut candidates: Vec<Vec<&str>> = vec![vec!["--version"], vec!["-version"], vec!["version"]];
    if server.binary_name.contains("jdtls") {
        candidates.push(vec!["-v"]);
    }

    for args in candidates {
        let mut cmd = tokio::process::Command::new(&server.command);
        cmd.args(args);
        configure_background_process(&mut cmd);

        let output = match tokio::time::timeout(Duration::from_secs(2), cmd.output()).await {
            Ok(Ok(output)) => output,
            _ => continue,
        };

        if !output.status.success() {
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            return Some(stdout.lines().next().unwrap_or_default().trim().to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Some(stderr.lines().next().unwrap_or_default().trim().to_string());
        }
    }

    None
}

fn normalize_lsp_preferences(preferences: LspPreferences) -> LspPreferences {
    LspPreferences {
        auto_enrich: preferences.auto_enrich,
        incremental_debounce_ms: if preferences.incremental_debounce_ms == 0 {
            DEFAULT_ENRICHMENT_DEBOUNCE_MS
        } else {
            preferences
                .incremental_debounce_ms
                .clamp(MIN_ENRICHMENT_DEBOUNCE_MS, MAX_ENRICHMENT_DEBOUNCE_MS)
        },
    }
}

fn validate_lsp_preferences(preferences: &LspPreferences) -> Result<(), String> {
    if preferences.incremental_debounce_ms < MIN_ENRICHMENT_DEBOUNCE_MS
        || preferences.incremental_debounce_ms > MAX_ENRICHMENT_DEBOUNCE_MS
    {
        return Err(format!(
            "incrementalDebounceMs must be between {} and {} ms",
            MIN_ENRICHMENT_DEBOUNCE_MS, MAX_ENRICHMENT_DEBOUNCE_MS
        ));
    }

    Ok(())
}

/// Build the full server status list for all supported languages.
fn build_server_statuses(
    detected: Option<&HashMap<String, DetectedServer>>,
    cached: &[LspServerInfo],
) -> Vec<LspServerStatus> {
    let cache_map: HashMap<&str, &LspServerInfo> = cached
        .iter()
        .map(|entry| (entry.language.as_str(), entry))
        .collect();

    supported_languages_meta()
        .iter()
        .map(|(lang, default_name, install_hint)| {
            let cached_entry = cache_map.get(*lang).copied();

            if let Some(detected_map) = detected {
                if let Some(server) = detected_map.get(*lang) {
                    return LspServerStatus {
                        language: (*lang).to_string(),
                        server_name: server.server_name.clone(),
                        detected: true,
                        binary_path: Some(server.binary_path.to_string_lossy().to_string()),
                        version: server.version.clone(),
                        detected_at: cached_entry.and_then(|e| e.detected_at.clone()),
                        install_hint: (*install_hint).to_string(),
                    };
                }

                return LspServerStatus {
                    language: (*lang).to_string(),
                    server_name: (*default_name).to_string(),
                    detected: false,
                    binary_path: None,
                    version: None,
                    detected_at: None,
                    install_hint: (*install_hint).to_string(),
                };
            }

            if let Some(entry) = cached_entry {
                return LspServerStatus {
                    language: (*lang).to_string(),
                    server_name: entry.server_name.clone(),
                    detected: true,
                    binary_path: Some(entry.binary_path.clone()),
                    version: entry.version.clone(),
                    detected_at: entry.detected_at.clone(),
                    install_hint: (*install_hint).to_string(),
                };
            }

            LspServerStatus {
                language: (*lang).to_string(),
                server_name: (*default_name).to_string(),
                detected: false,
                binary_path: None,
                version: None,
                detected_at: None,
                install_hint: (*install_hint).to_string(),
            }
        })
        .collect()
}

fn supported_languages_meta() -> [(&'static str, &'static str, &'static str); 5] {
    [
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_server_statuses_none_detected() {
        let detected = HashMap::new();
        let statuses = build_server_statuses(Some(&detected), &[]);

        assert_eq!(statuses.len(), 5);
        for status in &statuses {
            assert!(!status.detected);
            assert!(!status.install_hint.is_empty());
            assert!(status.binary_path.is_none());
            assert!(status.detected_at.is_none());
        }
    }

    #[test]
    fn test_build_server_statuses_some_detected() {
        let mut detected = HashMap::new();
        detected.insert(
            "rust".to_string(),
            DetectedServer {
                language: "rust".to_string(),
                server_name: "rust-analyzer".to_string(),
                binary_name: "rust-analyzer".to_string(),
                binary_path: PathBuf::from("/usr/bin/rust-analyzer"),
                command: "/usr/bin/rust-analyzer".to_string(),
                args: vec![],
                version: Some("1.0".to_string()),
            },
        );
        detected.insert(
            "go".to_string(),
            DetectedServer {
                language: "go".to_string(),
                server_name: "gopls".to_string(),
                binary_name: "gopls".to_string(),
                binary_path: PathBuf::from("/usr/bin/gopls"),
                command: "/usr/bin/gopls".to_string(),
                args: vec!["serve".to_string()],
                version: None,
            },
        );

        let statuses = build_server_statuses(Some(&detected), &[]);
        assert_eq!(statuses.len(), 5);

        let rust_status = statuses.iter().find(|s| s.language == "rust").unwrap();
        assert!(rust_status.detected);
        assert_eq!(
            rust_status.binary_path.as_deref(),
            Some("/usr/bin/rust-analyzer")
        );
        assert_eq!(rust_status.version.as_deref(), Some("1.0"));

        let go_status = statuses.iter().find(|s| s.language == "go").unwrap();
        assert!(go_status.detected);

        let python_status = statuses.iter().find(|s| s.language == "python").unwrap();
        assert!(!python_status.detected);
    }

    #[test]
    fn test_build_server_statuses_from_cache() {
        let cached = vec![LspServerInfo {
            language: "rust".to_string(),
            binary_path: "/cache/rust-analyzer".to_string(),
            server_name: "rust-analyzer".to_string(),
            version: Some("cached-version".to_string()),
            detected_at: Some("2026-03-03T10:00:00Z".to_string()),
        }];

        let statuses = build_server_statuses(None, &cached);
        let rust = statuses.iter().find(|s| s.language == "rust").unwrap();
        assert!(rust.detected);
        assert_eq!(rust.version.as_deref(), Some("cached-version"));
        assert_eq!(rust.detected_at.as_deref(), Some("2026-03-03T10:00:00Z"));
    }

    #[test]
    fn test_lsp_state_new() {
        let state = LspState::new();
        // Should initialize with 5 adapters
        let languages = state.registry.supported_languages();
        assert_eq!(languages.len(), 5);
    }

    #[test]
    fn test_validate_lsp_preferences() {
        let ok = LspPreferences {
            auto_enrich: true,
            incremental_debounce_ms: 3000,
        };
        assert!(validate_lsp_preferences(&ok).is_ok());

        let min_ok = LspPreferences {
            auto_enrich: false,
            incremental_debounce_ms: MIN_ENRICHMENT_DEBOUNCE_MS,
        };
        assert!(validate_lsp_preferences(&min_ok).is_ok());

        let max_ok = LspPreferences {
            auto_enrich: false,
            incremental_debounce_ms: MAX_ENRICHMENT_DEBOUNCE_MS,
        };
        assert!(validate_lsp_preferences(&max_ok).is_ok());

        let bad = LspPreferences {
            auto_enrich: true,
            incremental_debounce_ms: 100,
        };
        assert!(validate_lsp_preferences(&bad).is_err());
    }

    #[test]
    fn test_normalize_lsp_preferences_default_on_zero() {
        let normalized = normalize_lsp_preferences(LspPreferences {
            auto_enrich: true,
            incremental_debounce_ms: 0,
        });
        assert_eq!(
            normalized.incremental_debounce_ms,
            DEFAULT_ENRICHMENT_DEBOUNCE_MS
        );
    }
}
