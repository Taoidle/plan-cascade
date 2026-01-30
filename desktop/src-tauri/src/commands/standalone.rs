//! Standalone Mode Commands
//!
//! Tauri commands for standalone LLM execution without Claude Code CLI.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

use crate::models::CommandResponse;
use crate::services::llm::{ProviderConfig, ProviderType};
use crate::services::orchestrator::{ExecutionResult, OrchestratorConfig, OrchestratorService};
use crate::services::streaming::UnifiedStreamEvent;
use crate::state::AppState;
use crate::storage::KeyringService;

/// Stored provider configurations
#[derive(Default)]
pub struct StandaloneState {
    /// Active orchestrator (if any)
    pub orchestrator: Option<Arc<OrchestratorService>>,
}

/// Provider information returned to frontend
#[derive(serde::Serialize)]
pub struct ProviderInfo {
    pub provider_type: String,
    pub name: String,
    pub models: Vec<ModelInfo>,
    pub requires_api_key: bool,
    pub default_base_url: Option<String>,
}

/// Model information
#[derive(serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub supports_thinking: bool,
    pub supports_tools: bool,
    pub context_window: u32,
    pub pricing: Option<Pricing>,
}

/// Pricing information for cost tracking
#[derive(serde::Serialize, Clone)]
pub struct Pricing {
    /// Cost per million input tokens in USD
    pub input_per_million: f64,
    /// Cost per million output tokens in USD
    pub output_per_million: f64,
    /// Cost per million thinking tokens in USD (if separate)
    pub thinking_per_million: Option<f64>,
}

/// Usage statistics for a session
#[derive(serde::Serialize, Default)]
pub struct UsageStatistics {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_thinking_tokens: u32,
    pub total_cost_usd: f64,
    pub requests: u32,
}

/// List all supported providers and their models
#[tauri::command]
pub async fn list_providers() -> CommandResponse<Vec<ProviderInfo>> {
    let providers = vec![
        ProviderInfo {
            provider_type: "anthropic".to_string(),
            name: "Anthropic Claude".to_string(),
            models: vec![
                ModelInfo {
                    id: "claude-3-5-sonnet-20241022".to_string(),
                    name: "Claude 3.5 Sonnet".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 3.0,
                        output_per_million: 15.0,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "claude-3-opus-20240229".to_string(),
                    name: "Claude 3 Opus".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 15.0,
                        output_per_million: 75.0,
                        thinking_per_million: None,
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.anthropic.com/v1/messages".to_string()),
        },
        ProviderInfo {
            provider_type: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec![
                ModelInfo {
                    id: "gpt-4-turbo".to_string(),
                    name: "GPT-4 Turbo".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: Some(Pricing {
                        input_per_million: 10.0,
                        output_per_million: 30.0,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "o1-preview".to_string(),
                    name: "o1 Preview".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: Some(Pricing {
                        input_per_million: 15.0,
                        output_per_million: 60.0,
                        thinking_per_million: Some(15.0),
                    }),
                },
                ModelInfo {
                    id: "o3-mini".to_string(),
                    name: "o3 Mini".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 1.1,
                        output_per_million: 4.4,
                        thinking_per_million: Some(1.1),
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.openai.com/v1/chat/completions".to_string()),
        },
        ProviderInfo {
            provider_type: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            models: vec![
                ModelInfo {
                    id: "deepseek-chat".to_string(),
                    name: "DeepSeek Chat".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 64_000,
                    pricing: Some(Pricing {
                        input_per_million: 0.14,
                        output_per_million: 0.28,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "deepseek-r1".to_string(),
                    name: "DeepSeek R1".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 64_000,
                    pricing: Some(Pricing {
                        input_per_million: 0.55,
                        output_per_million: 2.19,
                        thinking_per_million: Some(0.55),
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.deepseek.com/v1/chat/completions".to_string()),
        },
        ProviderInfo {
            provider_type: "ollama".to_string(),
            name: "Ollama (Local)".to_string(),
            models: vec![
                ModelInfo {
                    id: "llama3.2".to_string(),
                    name: "Llama 3.2".to_string(),
                    supports_thinking: false,
                    supports_tools: false,
                    context_window: 128_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "deepseek-r1:14b".to_string(),
                    name: "DeepSeek R1 14B".to_string(),
                    supports_thinking: true,
                    supports_tools: false,
                    context_window: 64_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "qwq:32b".to_string(),
                    name: "QwQ 32B".to_string(),
                    supports_thinking: true,
                    supports_tools: false,
                    context_window: 32_000,
                    pricing: None,
                },
            ],
            requires_api_key: false,
            default_base_url: Some("http://localhost:11434".to_string()),
        },
    ];

    CommandResponse::ok(providers)
}

/// Configure a provider (store API key securely)
#[tauri::command]
pub async fn configure_provider(
    provider: String,
    api_key: Option<String>,
    base_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    // Store API key if provided
    if let Some(key) = api_key {
        if let Err(e) = state.set_api_key(&provider, &key).await {
            return Ok(CommandResponse::err(format!("Failed to store API key: {}", e)));
        }
    }

    // Store base URL in settings if provided
    if let Some(url) = base_url {
        let db_result = state.with_database(|db| {
            let key = format!("provider_{}_base_url", provider);
            db.set_setting(&key, &url)
        }).await;

        if let Err(e) = db_result {
            return Ok(CommandResponse::err(format!("Failed to store base URL: {}", e)));
        }
    }

    Ok(CommandResponse::ok(true))
}

/// Check provider health (validate API key and connectivity)
#[tauri::command]
pub async fn check_provider_health(
    provider: String,
    model: String,
    base_url: Option<String>,
) -> CommandResponse<HealthCheckResult> {
    let keyring = KeyringService::new();

    // Get API key
    let api_key = match keyring.get_api_key(&provider) {
        Ok(key) => key,
        Err(e) => {
            return CommandResponse::ok(HealthCheckResult {
                healthy: false,
                error: Some(format!("Failed to get API key: {}", e)),
                latency_ms: None,
            });
        }
    };

    // Ollama doesn't need API key
    let provider_type = match provider.as_str() {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "ollama" => ProviderType::Ollama,
        _ => {
            return CommandResponse::err(format!("Unknown provider: {}", provider));
        }
    };

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return CommandResponse::ok(HealthCheckResult {
            healthy: false,
            error: Some("API key not configured".to_string()),
            latency_ms: None,
        });
    }

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url,
        model,
        ..Default::default()
    };

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1000,
        project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        streaming: false,
    };

    let orchestrator = OrchestratorService::new(orchestrator_config);

    let start = std::time::Instant::now();
    match orchestrator.health_check().await {
        Ok(_) => CommandResponse::ok(HealthCheckResult {
            healthy: true,
            error: None,
            latency_ms: Some(start.elapsed().as_millis() as u32),
        }),
        Err(e) => CommandResponse::ok(HealthCheckResult {
            healthy: false,
            error: Some(e.to_string()),
            latency_ms: Some(start.elapsed().as_millis() as u32),
        }),
    }
}

/// Health check result
#[derive(serde::Serialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub error: Option<String>,
    pub latency_ms: Option<u32>,
}

/// Execute a message in standalone mode
#[tauri::command]
pub async fn execute_standalone(
    message: String,
    provider: String,
    model: String,
    project_path: String,
    system_prompt: Option<String>,
    enable_tools: bool,
    app: AppHandle,
) -> CommandResponse<ExecutionResult> {
    let keyring = KeyringService::new();

    // Get API key
    let api_key = match keyring.get_api_key(&provider) {
        Ok(key) => key,
        Err(e) => {
            return CommandResponse::err(format!("Failed to get API key: {}", e));
        }
    };

    // Parse provider type
    let provider_type = match provider.as_str() {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "ollama" => ProviderType::Ollama,
        _ => {
            return CommandResponse::err(format!("Unknown provider: {}", provider));
        }
    };

    // Validate API key for non-Ollama providers
    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return CommandResponse::err("API key not configured for this provider");
    }

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: None,
        model,
        ..Default::default()
    };

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt,
        max_iterations: 50,
        max_total_tokens: 100_000,
        project_root: PathBuf::from(&project_path),
        streaming: true,
    };

    let orchestrator = OrchestratorService::new(orchestrator_config);

    // Create channel for streaming events
    let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(100);

    // Spawn task to forward events to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app_clone.emit("standalone-event", &event);
        }
    });

    // Execute the message
    let result = if enable_tools {
        orchestrator.execute(message, tx).await
    } else {
        orchestrator.execute_single(message, tx).await
    };

    CommandResponse::ok(result)
}

/// Get usage statistics from the database
#[tauri::command]
pub async fn get_usage_stats(
    provider: Option<String>,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<UsageStatistics>, String> {
    let provider_clone = provider.clone();
    let model_clone = model.clone();

    let result = state.with_database(|db| {
        let conn = db.get_connection()?;

        // Build query based on filters
        let mut query = String::from(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cost), 0), COUNT(*) FROM analytics WHERE event_type = 'llm_request'"
        );

        if let Some(p) = &provider_clone {
            query.push_str(&format!(" AND provider = '{}'", p));
        }
        if let Some(m) = &model_clone {
            query.push_str(&format!(" AND model = '{}'", m));
        }

        let stats = conn.query_row(&query, [], |row| {
            Ok(UsageStatistics {
                total_input_tokens: row.get::<_, i64>(0).unwrap_or(0) as u32,
                total_output_tokens: row.get::<_, i64>(1).unwrap_or(0) as u32,
                total_thinking_tokens: 0, // Not tracked separately yet
                total_cost_usd: row.get::<_, f64>(2).unwrap_or(0.0),
                requests: row.get::<_, i64>(3).unwrap_or(0) as u32,
            })
        })?;

        Ok(stats)
    }).await;

    match result {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(format!("Failed to get usage stats: {}", e))),
    }
}

/// Calculate cost for token usage
pub fn calculate_cost(
    input_tokens: u32,
    output_tokens: u32,
    thinking_tokens: u32,
    pricing: &Pricing,
) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_per_million;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_per_million;
    let thinking_cost = if let Some(thinking_price) = pricing.thinking_per_million {
        (thinking_tokens as f64 / 1_000_000.0) * thinking_price
    } else {
        0.0
    };

    input_cost + output_cost + thinking_cost
}

/// Record usage in the database
pub async fn record_usage(
    state: &AppState,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cost: f64,
) -> Result<(), String> {
    let provider = provider.to_string();
    let model = model.to_string();

    state.with_database(move |db| {
        let conn = db.get_connection()?;
        conn.execute(
            "INSERT INTO analytics (event_type, provider, model, input_tokens, output_tokens, cost) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["llm_request", provider, model, input_tokens, output_tokens, cost],
        )?;
        Ok(())
    }).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost() {
        let pricing = Pricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            thinking_per_million: None,
        };

        let cost = calculate_cost(1_000_000, 1_000_000, 0, &pricing);
        assert!((cost - 18.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_cost_with_thinking() {
        let pricing = Pricing {
            input_per_million: 15.0,
            output_per_million: 60.0,
            thinking_per_million: Some(15.0),
        };

        let cost = calculate_cost(100_000, 50_000, 200_000, &pricing);
        // 0.1 * 15 + 0.05 * 60 + 0.2 * 15 = 1.5 + 3.0 + 3.0 = 7.5
        assert!((cost - 7.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_list_providers() {
        let result = list_providers().await;
        assert!(result.success);
        let providers = result.data.unwrap();
        assert!(!providers.is_empty());

        // Check that all expected providers are present
        let names: Vec<&str> = providers.iter().map(|p| p.provider_type.as_str()).collect();
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"ollama"));
    }
}
