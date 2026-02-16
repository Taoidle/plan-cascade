//! Memory Commands
//!
//! Tauri commands for managing project memories (cross-session persistent knowledge).

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::memory::retrieval::search_memories;
use crate::services::memory::store::{
    MemoryCategory, MemoryEntry, MemorySearchRequest, MemorySearchResult, MemoryStats,
    MemoryUpdate, NewMemoryEntry,
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
