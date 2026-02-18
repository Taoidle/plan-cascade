//! Proxy Configuration Commands
//!
//! Tauri commands for global and per-provider proxy configuration.
//! Proxy settings are stored in the SQLite settings table. Proxy
//! passwords are stored in the OS keyring.
//!
//! ## IPC Commands
//!
//! - `get_proxy_config` — Retrieve global proxy and all provider strategies
//! - `set_proxy_config` — Set/clear the global proxy configuration
//! - `get_provider_proxy_strategy` — Get proxy strategy for a specific provider
//! - `set_provider_proxy_strategy` — Set proxy strategy (and custom config) for a provider
//! - `test_proxy` — Test proxy connectivity

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::proxy::{ProxyConfig, ProxyStrategy};
use crate::state::AppState;
use crate::storage::KeyringService;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// All provider identifiers that support proxy configuration.
const PROVIDER_IDS: &[&str] = &[
    "anthropic",
    "openai",
    "deepseek",
    "qwen",
    "glm",
    "minimax",
    "ollama",
    "claude_code",
    "embedding_openai",
    "embedding_qwen",
    "embedding_glm",
    "embedding_ollama",
    "webhook_slack",
    "webhook_feishu",
    "webhook_telegram",
    "webhook_discord",
    "webhook_custom",
];

/// Settings DB key prefix for per-provider proxy strategy.
const STRATEGY_KEY_PREFIX: &str = "proxy_strategy_";

/// Settings DB key prefix for per-provider custom proxy config.
const CUSTOM_KEY_PREFIX: &str = "proxy_custom_";

/// Settings DB key for global proxy config.
const GLOBAL_PROXY_KEY: &str = "proxy_global";

/// Keyring key prefix for proxy passwords.
const KEYRING_PREFIX: &str = "proxy_";

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Full proxy settings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettingsResponse {
    /// Global proxy configuration (password excluded).
    pub global: Option<ProxyConfig>,
    /// Per-provider proxy strategy map.
    pub provider_strategies: HashMap<String, ProxyStrategy>,
    /// Per-provider custom proxy configs (only for providers with Custom strategy).
    pub provider_configs: HashMap<String, ProxyConfig>,
}

/// Request to set global proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProxyConfigRequest {
    /// The proxy configuration. null = disable global proxy.
    pub proxy: Option<ProxyConfig>,
    /// Password to store in keyring (not serialized with config).
    pub password: Option<String>,
}

/// Request to set per-provider proxy strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProviderProxyRequest {
    /// Provider identifier (e.g., "anthropic", "openai", "embedding_qwen").
    pub provider: String,
    /// Proxy strategy for this provider.
    pub strategy: ProxyStrategy,
    /// Custom proxy config (only when strategy is Custom).
    pub custom_proxy: Option<ProxyConfig>,
    /// Password for custom proxy (stored in keyring).
    pub custom_password: Option<String>,
}

/// Request to test proxy connectivity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProxyRequest {
    /// The proxy configuration to test.
    pub proxy: ProxyConfig,
    /// Password for authentication.
    pub password: Option<String>,
    /// URL to test against (default: https://httpbin.org/get).
    pub test_url: Option<String>,
}

/// Result of a proxy connectivity test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTestResult {
    pub success: bool,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// Retrieve the full proxy configuration (global + per-provider).
#[tauri::command]
pub async fn get_proxy_config(
    state: State<'_, AppState>,
) -> Result<CommandResponse<ProxySettingsResponse>, String> {
    let result = state
        .with_database(|db| {
            // Read global proxy config
            let global: Option<ProxyConfig> = db
                .get_setting(GLOBAL_PROXY_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok());

            // Read per-provider strategies
            let mut provider_strategies = HashMap::new();
            let mut provider_configs = HashMap::new();

            for &id in PROVIDER_IDS {
                let strategy_key = format!("{}{}", STRATEGY_KEY_PREFIX, id);
                let strategy: ProxyStrategy = db
                    .get_setting(&strategy_key)?
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_else(|| default_strategy_for(id));

                provider_strategies.insert(id.to_string(), strategy.clone());

                // Read custom config if strategy is Custom
                if strategy == ProxyStrategy::Custom {
                    let custom_key = format!("{}{}", CUSTOM_KEY_PREFIX, id);
                    if let Some(json) = db.get_setting(&custom_key)? {
                        if let Ok(cfg) = serde_json::from_str::<ProxyConfig>(&json) {
                            provider_configs.insert(id.to_string(), cfg);
                        }
                    }
                }
            }

            Ok(ProxySettingsResponse {
                global,
                provider_strategies,
                provider_configs,
            })
        })
        .await;

    match result {
        Ok(response) => Ok(CommandResponse::ok(response)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Set or clear the global proxy configuration.
#[tauri::command]
pub async fn set_proxy_config(
    request: SetProxyConfigRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    // Store password in keyring if provided
    let keyring = KeyringService::new();
    if let Some(ref proxy) = request.proxy {
        if let Some(ref password) = request.password {
            if let Err(e) = keyring.set_api_key(&format!("{}global", KEYRING_PREFIX), password) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store proxy password in keyring: {}",
                    e
                )));
            }
        } else if proxy.username.is_some() {
            // Username provided without password — clear any stored password
            let _ = keyring.delete_api_key(&format!("{}global", KEYRING_PREFIX));
        }
    } else {
        // Clearing global proxy — remove password from keyring
        let _ = keyring.delete_api_key(&format!("{}global", KEYRING_PREFIX));
    }

    let result = state
        .with_database(|db| {
            match &request.proxy {
                Some(proxy) => {
                    let json = serde_json::to_string(proxy).map_err(|e| {
                        crate::utils::error::AppError::Internal(format!(
                            "Failed to serialize proxy config: {}",
                            e
                        ))
                    })?;
                    db.set_setting(GLOBAL_PROXY_KEY, &json)?;
                }
                None => {
                    db.delete_setting(GLOBAL_PROXY_KEY)?;
                }
            }
            Ok(true)
        })
        .await;

    match result {
        Ok(v) => Ok(CommandResponse::ok(v)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the proxy strategy for a specific provider.
#[tauri::command]
pub async fn get_provider_proxy_strategy(
    provider: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<ProxyStrategy>, String> {
    let result = state
        .with_database(|db| {
            let key = format!("{}{}", STRATEGY_KEY_PREFIX, provider);
            let strategy: ProxyStrategy = db
                .get_setting(&key)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_else(|| default_strategy_for(&provider));
            Ok(strategy)
        })
        .await;

    match result {
        Ok(strategy) => Ok(CommandResponse::ok(strategy)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Set the proxy strategy (and optional custom config) for a specific provider.
#[tauri::command]
pub async fn set_provider_proxy_strategy(
    request: SetProviderProxyRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    // Handle custom proxy password
    if request.strategy == ProxyStrategy::Custom {
        let keyring = KeyringService::new();
        if let Some(ref password) = request.custom_password {
            if let Err(e) = keyring.set_api_key(
                &format!("{}{}", KEYRING_PREFIX, request.provider),
                password,
            ) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store custom proxy password: {}",
                    e
                )));
            }
        }
    }

    let result = state
        .with_database(|db| {
            let strategy_key = format!("{}{}", STRATEGY_KEY_PREFIX, request.provider);
            let strategy_json = serde_json::to_string(&request.strategy).map_err(|e| {
                crate::utils::error::AppError::Internal(format!(
                    "Failed to serialize strategy: {}",
                    e
                ))
            })?;
            db.set_setting(&strategy_key, &strategy_json)?;

            // Store or clear custom proxy config
            let custom_key = format!("{}{}", CUSTOM_KEY_PREFIX, request.provider);
            if request.strategy == ProxyStrategy::Custom {
                if let Some(ref proxy) = request.custom_proxy {
                    let json = serde_json::to_string(proxy).map_err(|e| {
                        crate::utils::error::AppError::Internal(format!(
                            "Failed to serialize custom proxy config: {}",
                            e
                        ))
                    })?;
                    db.set_setting(&custom_key, &json)?;
                }
            } else {
                db.delete_setting(&custom_key)?;
            }

            Ok(true)
        })
        .await;

    match result {
        Ok(v) => Ok(CommandResponse::ok(v)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Test proxy connectivity by making a request through the proxy.
#[tauri::command]
pub async fn test_proxy(request: TestProxyRequest) -> Result<CommandResponse<ProxyTestResult>, String> {
    let mut proxy = request.proxy;
    proxy.password = request.password;

    let test_url = request
        .test_url
        .unwrap_or_else(|| "https://httpbin.org/get".to_string());

    let client = crate::services::proxy::build_http_client(Some(&proxy));

    let start = std::time::Instant::now();
    match client.get(&test_url).send().await {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u32;
            if response.status().is_success() || response.status().is_redirection() {
                Ok(CommandResponse::ok(ProxyTestResult {
                    success: true,
                    latency_ms: Some(latency),
                    error: None,
                }))
            } else {
                Ok(CommandResponse::ok(ProxyTestResult {
                    success: false,
                    latency_ms: Some(latency),
                    error: Some(format!("HTTP {}", response.status())),
                }))
            }
        }
        Err(e) => Ok(CommandResponse::ok(ProxyTestResult {
            success: false,
            latency_ms: None,
            error: Some(e.to_string()),
        })),
    }
}

// ---------------------------------------------------------------------------
// Proxy Resolution
// ---------------------------------------------------------------------------

/// Resolve the effective proxy configuration for a given provider.
///
/// Used by command handlers to inject proxy into `ProviderConfig` or
/// `EmbeddingProviderConfig` before provider construction.
///
/// Returns `Some(ProxyConfig)` if the provider should use a proxy,
/// or `None` for direct connection.
pub fn resolve_provider_proxy(
    keyring: &KeyringService,
    db: &crate::storage::Database,
    provider: &str,
) -> Option<ProxyConfig> {
    // 1. Read strategy
    let strategy_key = format!("{}{}", STRATEGY_KEY_PREFIX, provider);
    let strategy: ProxyStrategy = db
        .get_setting(&strategy_key)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(|| default_strategy_for(provider));

    match strategy {
        ProxyStrategy::NoProxy => None,
        ProxyStrategy::UseGlobal => {
            // Read global proxy config
            let mut proxy: ProxyConfig = db
                .get_setting(GLOBAL_PROXY_KEY)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())?;

            // Hydrate password from keyring
            if proxy.username.is_some() {
                proxy.password = keyring
                    .get_api_key(&format!("{}global", KEYRING_PREFIX))
                    .ok()
                    .flatten();
            }
            Some(proxy)
        }
        ProxyStrategy::Custom => {
            // Read custom proxy config
            let custom_key = format!("{}{}", CUSTOM_KEY_PREFIX, provider);
            let mut proxy: ProxyConfig = db
                .get_setting(&custom_key)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())?;

            // Hydrate password from keyring
            if proxy.username.is_some() {
                proxy.password = keyring
                    .get_api_key(&format!("{}{}", KEYRING_PREFIX, provider))
                    .ok()
                    .flatten();
            }
            Some(proxy)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the smart default proxy strategy for a given provider.
///
/// - International APIs (Anthropic, OpenAI): use_global
/// - Domestic APIs (Qwen, GLM, DeepSeek, Minimax): no_proxy
/// - Local (Ollama, TF-IDF): no_proxy
/// - Claude Code: use_global
fn default_strategy_for(provider: &str) -> ProxyStrategy {
    match provider {
        "anthropic" | "openai" | "claude_code" | "embedding_openai" => ProxyStrategy::UseGlobal,
        // Webhook channels: international services default to global proxy
        "webhook_slack" | "webhook_discord" | "webhook_telegram" | "webhook_custom" => {
            ProxyStrategy::UseGlobal
        }
        // Feishu is a domestic service; no proxy by default
        _ => ProxyStrategy::NoProxy,
    }
}
