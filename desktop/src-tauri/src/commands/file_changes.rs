//! Tauri commands for LLM file change tracking and rollback.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::file_change_tracker::{FileChangeTracker, RestoredFile, TurnChanges};

/// Tauri-managed state holding file change trackers keyed by session ID.
pub struct FileChangesState {
    trackers: RwLock<HashMap<String, Arc<Mutex<FileChangeTracker>>>>,
}

impl FileChangesState {
    pub fn new() -> Self {
        Self {
            trackers: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a tracker for the given session + project.
    pub async fn get_or_create(
        &self,
        session_id: &str,
        project_root: &str,
    ) -> Arc<Mutex<FileChangeTracker>> {
        // Fast path: read lock
        {
            let trackers = self.trackers.read().await;
            if let Some(tracker) = trackers.get(session_id) {
                return Arc::clone(tracker);
            }
        }
        // Slow path: write lock and create
        let mut trackers = self.trackers.write().await;
        Arc::clone(trackers.entry(session_id.to_string()).or_insert_with(|| {
            Arc::new(Mutex::new(FileChangeTracker::new(
                session_id,
                PathBuf::from(project_root),
            )))
        }))
    }

    /// Get an existing tracker (returns None if not found).
    pub async fn get(&self, session_id: &str) -> Option<Arc<Mutex<FileChangeTracker>>> {
        let trackers = self.trackers.read().await;
        trackers.get(session_id).cloned()
    }
}

impl Default for FileChangesState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

/// Initialize a file change tracker for a session.
#[tauri::command]
pub async fn init_file_change_tracker(
    session_id: String,
    project_root: String,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<u32>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(t) => CommandResponse::ok(t.turn_index()),
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}

/// Advance the turn index for a session tracker.
#[tauri::command]
pub async fn advance_turn_index(
    session_id: String,
    turn_index: u32,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<bool>, String> {
    let result = match state.get(&session_id).await {
        Some(tracker) => match tracker.lock() {
            Ok(mut t) => {
                t.set_turn_index(turn_index);
                CommandResponse::ok(true)
            }
            Err(_) => CommandResponse::err("Failed to lock tracker"),
        },
        None => CommandResponse::err("No tracker for session"),
    };
    Ok(result)
}

/// Get all file changes grouped by turn for a session.
#[tauri::command]
pub async fn get_file_changes_by_turn(
    session_id: String,
    project_root: String,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<Vec<TurnChanges>>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(t) => CommandResponse::ok(t.get_changes_by_turn()),
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}

/// Get a unified diff between two content versions.
#[tauri::command]
pub async fn get_file_change_diff(
    session_id: String,
    project_root: String,
    before_hash: Option<String>,
    after_hash: String,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<String>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(t) => match t.get_file_diff(before_hash.as_deref(), &after_hash) {
            Ok(diff) => CommandResponse::ok(diff),
            Err(e) => CommandResponse::err(e),
        },
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}

/// Restore all files to the state before the given turn index.
#[tauri::command]
pub async fn restore_files_to_turn(
    session_id: String,
    project_root: String,
    turn_index: u32,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<Vec<RestoredFile>>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(t) => match t.restore_to_before_turn(turn_index) {
            Ok(restored) => CommandResponse::ok(restored),
            Err(e) => CommandResponse::err(e),
        },
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}

/// Truncate change records from a turn onward (used after rollback).
#[tauri::command]
pub async fn truncate_changes_from_turn(
    session_id: String,
    project_root: String,
    turn_index: u32,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<bool>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(mut t) => {
            t.truncate_from_turn(turn_index);
            CommandResponse::ok(true)
        }
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}

/// Restore a single file to a specific CAS version.
#[tauri::command]
pub async fn restore_single_file(
    session_id: String,
    project_root: String,
    file_path: String,
    target_hash: String,
    state: tauri::State<'_, FileChangesState>,
) -> Result<CommandResponse<bool>, String> {
    let tracker = state.get_or_create(&session_id, &project_root).await;
    let result = match tracker.lock() {
        Ok(t) => match t.restore_single_file(&file_path, &target_hash) {
            Ok(_) => CommandResponse::ok(true),
            Err(e) => CommandResponse::err(e),
        },
        Err(_) => CommandResponse::err("Failed to lock tracker"),
    };
    Ok(result)
}
