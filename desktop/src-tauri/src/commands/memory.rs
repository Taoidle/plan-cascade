//! Memory Commands
//!
//! Tauri commands for managing project memories (cross-session persistent knowledge).

use serde::Serialize;
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::memory::extraction::MemoryExtractor;
use crate::services::memory::retrieval::search_memories;
use crate::services::memory::store::{
    MemoryCategory, MemoryEntry, MemorySearchRequest, MemorySearchResult, MemoryStats,
    MemoryUpdate, NewMemoryEntry, UpsertResult,
};
use crate::state::AppState;

/// Search project memories by semantic similarity and keyword match
#[tauri::command]
pub async fn search_project_memories(
    project_path: String,
    query: String,
    categories: Option<Vec<String>>,
    top_k: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemorySearchResult>>, String> {
    let parsed_categories = categories
        .as_ref()
        .map(|cats| {
            cats.iter()
                .filter_map(|c| MemoryCategory::from_str(c).ok())
                .collect::<Vec<_>>()
        });

    let request = MemorySearchRequest {
        project_path,
        query,
        categories: parsed_categories,
        top_k: top_k.unwrap_or(10),
        min_importance: 0.1,
    };

    match state
        .with_memory_store(|store| {
            search_memories(store, &request)
        })
        .await
    {
        Ok(results) => Ok(CommandResponse::ok(results)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List project memories with optional category filter and pagination
#[tauri::command]
pub async fn list_project_memories(
    project_path: String,
    category: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemoryEntry>>, String> {
    let parsed_category = category
        .as_ref()
        .and_then(|c| MemoryCategory::from_str(c).ok());

    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    match state
        .with_memory_store(|store| {
            store.list_memories(&project_path, parsed_category, offset, limit)
        })
        .await
    {
        Ok(memories) => Ok(CommandResponse::ok(memories)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Add a new project memory
#[tauri::command]
pub async fn add_project_memory(
    project_path: String,
    category: String,
    content: String,
    keywords: Vec<String>,
    importance: Option<f32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String> {
    let parsed_category = match MemoryCategory::from_str(&category) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let entry = NewMemoryEntry {
        project_path,
        category: parsed_category,
        content,
        keywords,
        importance: importance.unwrap_or(0.5),
        source_session_id: None,
        source_context: None,
    };

    match state
        .with_memory_store(|store| store.add_memory(entry))
        .await
    {
        Ok(memory) => Ok(CommandResponse::ok(memory)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing project memory
#[tauri::command]
pub async fn update_project_memory(
    id: String,
    content: Option<String>,
    category: Option<String>,
    importance: Option<f32>,
    keywords: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String> {
    let parsed_category = category
        .as_ref()
        .map(|c| MemoryCategory::from_str(c))
        .transpose()
        .map_err(|e| e.to_string());

    let parsed_category = match parsed_category {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let updates = MemoryUpdate {
        content,
        category: parsed_category,
        importance,
        keywords,
    };

    match state
        .with_memory_store(|store| store.update_memory(&id, updates))
        .await
    {
        Ok(memory) => Ok(CommandResponse::ok(memory)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a specific project memory
#[tauri::command]
pub async fn delete_project_memory(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    match state
        .with_memory_store(|store| store.delete_memory(&id))
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Clear all memories for a project
#[tauri::command]
pub async fn clear_project_memories(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    match state
        .with_memory_store(|store| store.clear_project_memories(&project_path))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get memory statistics for a project
#[tauri::command]
pub async fn get_memory_stats(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryStats>, String> {
    match state
        .with_memory_store(|store| store.get_stats(&project_path))
        .await
    {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Result of automatic memory extraction from a session
#[derive(Debug, Clone, Serialize)]
pub struct MemoryExtractionResult {
    pub extracted_count: usize,
    pub inserted_count: usize,
    pub merged_count: usize,
    pub skipped_count: usize,
}

/// Extract memories from a completed session using LLM analysis.
///
/// Called by the frontend after a session completes. Uses the configured
/// LLM provider to analyze the conversation and extract persistent memories.
/// Silently returns zero results if no provider is configured or on any error.
#[tauri::command]
pub async fn extract_session_memories(
    project_path: String,
    task_description: String,
    conversation_summary: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryExtractionResult>, String> {
    let zero_result = MemoryExtractionResult {
        extracted_count: 0,
        inserted_count: 0,
        merged_count: 0,
        skipped_count: 0,
    };

    // Skip short conversations
    if conversation_summary.len() < 50 {
        return Ok(CommandResponse::ok(zero_result));
    }

    // Resolve LLM provider from app config + keyring
    let provider = match resolve_extraction_provider(&state).await {
        Ok(p) => p,
        Err(_) => return Ok(CommandResponse::ok(zero_result)),
    };

    use crate::services::llm::types::{LlmRequestOptions, Message};

    // Stage 1: If conversation is long, use LLM to create a focused summary
    // that emphasizes user preferences, tech stack, patterns, etc.
    let effective_summary = if conversation_summary.len() > MemoryExtractor::SUMMARIZE_THRESHOLD {
        let summarize_prompt = MemoryExtractor::build_summarization_prompt(
            &task_description,
            &conversation_summary,
        );
        let messages = vec![Message::user(summarize_prompt)];
        match provider
            .send_message(messages, None, vec![], LlmRequestOptions::default())
            .await
        {
            Ok(resp) => resp.content.unwrap_or(conversation_summary.clone()),
            Err(_) => {
                // Summarization failed — fall back to truncated raw content
                conversation_summary
                    .chars()
                    .take(MemoryExtractor::SUMMARIZE_THRESHOLD)
                    .collect::<String>()
            }
        }
    } else {
        conversation_summary.clone()
    };

    // Load existing memories to avoid duplicates
    let existing_memories = state
        .with_memory_store(|store| store.list_memories(&project_path, None, 0, 200))
        .await
        .unwrap_or_default();

    // Stage 2: Build extraction prompt using the (possibly summarized) conversation
    let prompt = MemoryExtractor::build_extraction_prompt(
        &task_description,
        &[], // files_read not available from frontend trigger
        &[], // key_findings not available from frontend trigger
        &effective_summary,
        &existing_memories,
    );

    // Call LLM for memory extraction
    let messages = vec![Message::user(prompt)];
    let response = provider
        .send_message(messages, None, vec![], LlmRequestOptions::default())
        .await;

    let response_text = match response {
        Ok(resp) => match resp.content {
            Some(text) => text,
            None => return Ok(CommandResponse::ok(zero_result)),
        },
        Err(_) => return Ok(CommandResponse::ok(zero_result)),
    };

    // Parse extraction response
    let entries = MemoryExtractor::parse_extraction_response(
        &response_text,
        &project_path,
        session_id.as_deref(),
    );

    let extracted_count = entries.len();
    if extracted_count == 0 {
        return Ok(CommandResponse::ok(zero_result));
    }

    // Upsert each memory entry
    let mut inserted_count = 0usize;
    let mut merged_count = 0usize;
    let mut skipped_count = 0usize;

    for entry in entries {
        match state
            .with_memory_store(|store| store.upsert_memory(entry.clone()))
            .await
        {
            Ok(UpsertResult::Inserted(_)) => inserted_count += 1,
            Ok(UpsertResult::Merged { .. }) => merged_count += 1,
            Ok(UpsertResult::Skipped { .. }) => skipped_count += 1,
            Err(_) => skipped_count += 1,
        }
    }

    Ok(CommandResponse::ok(MemoryExtractionResult {
        extracted_count,
        inserted_count,
        merged_count,
        skipped_count,
    }))
}

/// Resolve the LLM provider for memory extraction from app settings.
///
/// Uses the default_provider/default_model from AppConfig and retrieves
/// the API key from the OS keyring. Returns an error if no provider is
/// configured or no API key is found (except for Ollama).
async fn resolve_extraction_provider(
    state: &AppState,
) -> Result<Box<dyn crate::services::llm::provider::LlmProvider>, String> {
    use crate::commands::standalone::{get_api_key_with_aliases, normalize_provider_name};
    use crate::services::llm::types::{ProviderConfig, ProviderType};
    use crate::storage::KeyringService;

    let app_config = state
        .get_config()
        .await
        .map_err(|e| format!("Config not initialized: {}", e))?;

    let canonical = normalize_provider_name(&app_config.default_provider)
        .ok_or_else(|| format!("Unknown provider: {}", app_config.default_provider))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    let keyring = KeyringService::new();
    let api_key = get_api_key_with_aliases(&keyring, canonical)
        .map_err(|e| format!("Keyring error: {}", e))?;

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err("No API key configured".into());
    }

    // Resolve proxy settings
    let proxy = state
        .with_database(|db| {
            Ok(crate::commands::proxy::resolve_provider_proxy(
                &keyring, db, canonical,
            ))
        })
        .await
        .ok()
        .flatten();

    let config = ProviderConfig {
        provider: provider_type.clone(),
        api_key,
        base_url: None,
        model: app_config.default_model.clone(),
        max_tokens: 2048,
        temperature: 0.3,
        proxy,
        ..Default::default()
    };

    Ok(create_extraction_provider(config))
}

/// Create an LLM provider instance from a ProviderConfig.
fn create_extraction_provider(
    config: crate::services::llm::types::ProviderConfig,
) -> Box<dyn crate::services::llm::provider::LlmProvider> {
    use crate::services::llm::types::ProviderType;
    use crate::services::llm::*;

    match config.provider {
        ProviderType::Anthropic => Box::new(AnthropicProvider::new(config)),
        ProviderType::OpenAI => Box::new(OpenAIProvider::new(config)),
        ProviderType::DeepSeek => Box::new(DeepSeekProvider::new(config)),
        ProviderType::Glm => Box::new(GlmProvider::new(config)),
        ProviderType::Qwen => Box::new(QwenProvider::new(config)),
        ProviderType::Minimax => Box::new(MinimaxProvider::new(config)),
        ProviderType::Ollama => Box::new(OllamaProvider::new(config)),
    }
}
