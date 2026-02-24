//! Prompt Commands
//!
//! Tauri command handlers for prompt template management.

use crate::models::prompt::{PromptCreateRequest, PromptTemplate, PromptUpdateRequest};
use crate::models::response::CommandResponse;
use crate::services::prompt::PromptService;
use crate::state::AppState;

/// List all prompts with optional category filter and search
#[tauri::command]
pub async fn list_prompts(
    category: Option<String>,
    search: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<Vec<PromptTemplate>>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            // Seed built-in prompts on first access
            service.seed_builtins()?;
            service.list_prompts(category.as_deref(), search.as_deref())
        })
        .await;

    match result {
        Ok(prompts) => Ok(CommandResponse::ok(prompts)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new prompt template
#[tauri::command]
pub async fn create_prompt(
    request: PromptCreateRequest,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<PromptTemplate>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            service.create_prompt(request)
        })
        .await;

    match result {
        Ok(prompt) => Ok(CommandResponse::ok(prompt)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing prompt template
#[tauri::command]
pub async fn update_prompt(
    id: String,
    request: PromptUpdateRequest,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<PromptTemplate>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            service.update_prompt(&id, request)
        })
        .await;

    match result {
        Ok(prompt) => Ok(CommandResponse::ok(prompt)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a prompt template (refuses to delete built-in prompts)
#[tauri::command]
pub async fn delete_prompt(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            service.delete_prompt(&id)
        })
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Record usage of a prompt template
#[tauri::command]
pub async fn record_prompt_use(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            service.record_use(&id)
        })
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Toggle pin status of a prompt template
#[tauri::command]
pub async fn toggle_prompt_pin(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<PromptTemplate>, String> {
    let result = state
        .with_database(|db| {
            let service = PromptService::from_database(db);
            service.toggle_pin(&id)
        })
        .await;

    match result {
        Ok(prompt) => Ok(CommandResponse::ok(prompt)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}
