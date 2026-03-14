//! File Change Tracker with Content-Addressable Storage (CAS)
//!
//! Tracks all file modifications made by LLM tools (Write/Edit), storing
//! before/after content snapshots in a CAS directory. Supports rollback
//! to any previous conversation turn.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

use crate::utils::paths::plan_cascade_dir;

/// Maximum file size to store in CAS (10 MB).
const MAX_CAS_FILE_SIZE: usize = 10 * 1024 * 1024;

// ── Data Models ─────────────────────────────────────────────────────────

/// A single file modification record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub id: String,
    pub session_id: String,
    pub turn_index: u32,
    pub tool_call_id: String,
    pub tool_name: String,
    pub file_path: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub timestamp: i64,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mode: Option<FileChangeSourceMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_kind: Option<FileChangeActorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_agent_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeSourceMode {
    Chat,
    Plan,
    Task,
    Debug,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeActorKind {
    RootAgent,
    SubAgent,
    DebugPatch,
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileChangeMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mode: Option<FileChangeSourceMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_kind: Option<FileChangeActorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_agent_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_session_id: Option<String>,
}

/// Changes grouped by conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnChanges {
    pub turn_index: u32,
    pub changes: Vec<FileChange>,
    pub timestamp: i64,
}

/// Result of restoring a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoredFile {
    pub path: String,
    pub action: String, // "restored" or "deleted"
}

/// Preview item for restoring to a turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreviewItem {
    pub path: String,
    pub action: String, // "restore" or "delete"
    pub source_turn: u32,
}

/// Result payload for v2 restore command with optional undo handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreExecutionResult {
    pub operation_id: Option<String>,
    pub restored: Vec<RestoredFile>,
}

/// A content-addressed snapshot of workspace files at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceChangeSnapshot {
    pub files: HashMap<String, String>,
}

/// A detected workspace delta between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetectedWorkspaceChange {
    pub path: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub change_type: WorkspaceChangeType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceChangeType {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RestoreSnapshotEntry {
    path: String,
    before_restore_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RestoreOperation {
    operation_id: String,
    session_id: String,
    turn_index: u32,
    timestamp: i64,
    files: Vec<RestoreSnapshotEntry>,
}

#[derive(Debug, Clone)]
struct RestorePreviewTarget {
    path: String,
    target_hash: Option<String>,
    source_turn: u32,
}

// ── FileChangeTracker ───────────────────────────────────────────────────

/// Payload emitted on the `file-change-recorded` Tauri event.
#[derive(Debug, Clone, Serialize)]
struct FileChangeEvent {
    session_id: String,
    turn_index: u32,
    file_path: String,
    tool_name: String,
    change_id: String,
    before_hash: Option<String>,
    after_hash: Option<String>,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_mode: Option<FileChangeSourceMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_kind: Option<FileChangeActorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub_agent_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin_session_id: Option<String>,
}

/// Tracks LLM file modifications per session with CAS backing.
pub struct FileChangeTracker {
    session_id: String,
    project_root: PathBuf,
    /// Root directory for CAS blobs and change records.
    /// Stored under the app data directory (`~/.plan-cascade/file-changes/<project-hash>/`),
    /// NOT inside the user's project directory.
    data_dir: PathBuf,
    cas_dir: PathBuf,
    changes: Vec<FileChange>,
    current_turn_index: u32,
    /// Optional Tauri app handle for emitting events to the frontend.
    app_handle: Option<AppHandle>,
}

/// Compute a short (8-char) hex hash of a project path for use as a directory name.
fn project_path_hash(project_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_root.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..8].to_string()
}

/// Resolve the data directory for file change tracking.
///
/// Uses `~/.plan-cascade/file-changes/<project-hash>/` to keep data out of the
/// user's project directory. Falls back to `<project-root>/.plan-cascade/` only
/// if the home directory cannot be determined (should never happen in practice).
fn resolve_data_dir(project_root: &Path) -> PathBuf {
    let hash = project_path_hash(project_root);
    match plan_cascade_dir() {
        Ok(base) => base.join("file-changes").join(hash),
        Err(_) => {
            // Fallback: store in project root (legacy behavior)
            eprintln!(
                "[FileChangeTracker] WARNING: Could not resolve ~/.plan-cascade, \
                 falling back to project-local storage"
            );
            project_root.join(".plan-cascade")
        }
    }
}

impl FileChangeTracker {
    /// Create a new tracker for a session.
    ///
    /// Data (CAS blobs, change records) is stored under
    /// `~/.plan-cascade/file-changes/<project-hash>/`, keeping the user's
    /// project directory clean.
    pub fn new(session_id: impl Into<String>, project_root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = project_root.into();
        let data_dir = resolve_data_dir(&root);
        let cas_dir = data_dir.join("cas");
        let sid = session_id.into();
        let mut tracker = Self {
            session_id: sid.clone(),
            project_root: root,
            data_dir,
            cas_dir,
            changes: Vec::new(),
            current_turn_index: 0,
            app_handle: None,
        };
        // Attempt to load persisted changes
        tracker.load_silent();
        tracker
    }

    /// Create a new tracker with an explicit data directory.
    ///
    /// Used primarily by tests that need to control storage location.
    #[cfg(test)]
    pub fn new_with_data_dir(
        session_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        data_dir: impl Into<PathBuf>,
    ) -> Self {
        let root: PathBuf = project_root.into();
        let dd: PathBuf = data_dir.into();
        let cas_dir = dd.join("cas");
        let sid = session_id.into();
        let mut tracker = Self {
            session_id: sid.clone(),
            project_root: root,
            data_dir: dd,
            cas_dir,
            changes: Vec::new(),
            current_turn_index: 0,
            app_handle: None,
        };
        tracker.load_silent();
        tracker
    }

    /// Update the current turn index.
    pub fn set_turn_index(&mut self, idx: u32) {
        self.current_turn_index = idx;
    }

    /// Get the current turn index.
    pub fn turn_index(&self) -> u32 {
        self.current_turn_index
    }

    /// Set the Tauri app handle for event emission.
    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self.app_handle = Some(handle);
    }

    // ── CAS Storage ─────────────────────────────────────────────────────

    /// Store content in CAS; returns the SHA-256 hex hash.
    pub fn store_content(&self, content: &[u8]) -> Result<String, String> {
        if content.len() > MAX_CAS_FILE_SIZE {
            return Err(format!(
                "File too large for CAS ({} bytes, max {})",
                content.len(),
                MAX_CAS_FILE_SIZE
            ));
        }

        let hash = sha256_hex(content);
        let (prefix, _rest) = hash.split_at(2);
        let dir = self.cas_dir.join(prefix);

        // Skip if already stored
        let blob_path = dir.join(&hash);
        if blob_path.exists() {
            return Ok(hash);
        }

        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create CAS dir: {e}"))?;
        fs::write(&blob_path, content).map_err(|e| format!("Failed to write CAS blob: {e}"))?;
        Ok(hash)
    }

    /// Retrieve content from CAS by hash.
    pub fn get_content(&self, hash: &str) -> Result<Vec<u8>, String> {
        if hash.len() < 3 {
            return Err("Invalid hash".to_string());
        }
        let (prefix, _) = hash.split_at(2);
        let blob_path = self.cas_dir.join(prefix).join(hash);
        fs::read(&blob_path).map_err(|e| format!("CAS blob not found ({hash}): {e}"))
    }

    // ── Before/After Capture ────────────────────────────────────────────

    /// Capture the current content of a file before modification.
    /// Returns the CAS hash, or None if the file doesn't exist (new file).
    pub fn capture_before(&self, file_path: &Path) -> Option<String> {
        if !file_path.exists() {
            return None;
        }
        let bytes = fs::read(file_path).ok()?;
        self.store_content(&bytes).ok()
    }

    /// Capture a workspace snapshot for later delta calculation.
    pub fn capture_workspace_snapshot(&self) -> Result<WorkspaceChangeSnapshot, String> {
        let mut snapshot = WorkspaceChangeSnapshot::default();
        self.capture_workspace_snapshot_dir(&self.project_root, &mut snapshot)?;
        Ok(snapshot)
    }

    fn capture_workspace_snapshot_dir(
        &self,
        dir: &Path,
        snapshot: &mut WorkspaceChangeSnapshot,
    ) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| {
            format!(
                "Failed to read workspace directory '{}': {}",
                dir.display(),
                e
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!(
                    "Failed to inspect workspace entry in '{}': {}",
                    dir.display(),
                    e
                )
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Failed to inspect file type '{}': {}", path.display(), e))?;

            if should_skip_workspace_path(
                &self.project_root,
                &self.data_dir,
                &path,
                file_type.is_dir(),
            ) {
                continue;
            }

            if file_type.is_dir() {
                self.capture_workspace_snapshot_dir(&path, snapshot)?;
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let rel_path = path
                .strip_prefix(&self.project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path)
                .map_err(|e| format!("Failed to read workspace file '{}': {}", rel_path, e))?;
            let hash = self.store_content(&bytes)?;
            snapshot.files.insert(rel_path, hash);
        }
        Ok(())
    }

    pub fn detect_workspace_changes(
        &self,
        before: &WorkspaceChangeSnapshot,
        after: &WorkspaceChangeSnapshot,
    ) -> Vec<DetectedWorkspaceChange> {
        let mut paths: Vec<String> = before
            .files
            .keys()
            .chain(after.files.keys())
            .cloned()
            .collect();
        paths.sort();
        paths.dedup();

        let mut changes = Vec::new();
        for path in paths {
            let before_hash = before.files.get(&path).cloned();
            let after_hash = after.files.get(&path).cloned();
            let change_type = match (&before_hash, &after_hash) {
                (None, Some(_)) => Some(WorkspaceChangeType::Created),
                (Some(_), None) => Some(WorkspaceChangeType::Deleted),
                (Some(left), Some(right)) if left != right => Some(WorkspaceChangeType::Modified),
                _ => None,
            };

            if let Some(change_type) = change_type {
                changes.push(DetectedWorkspaceChange {
                    path,
                    before_hash,
                    after_hash,
                    change_type,
                });
            }
        }

        changes
    }

    pub fn record_workspace_delta_between_at(
        &mut self,
        turn_index: u32,
        tool_call_id_prefix: &str,
        tool_name: &str,
        before: &WorkspaceChangeSnapshot,
        after: &WorkspaceChangeSnapshot,
        description_prefix: &str,
    ) -> usize {
        self.record_workspace_delta_between_at_with_metadata(
            turn_index,
            tool_call_id_prefix,
            tool_name,
            before,
            after,
            description_prefix,
            None,
        )
    }

    pub fn record_workspace_delta_between_at_with_metadata(
        &mut self,
        turn_index: u32,
        tool_call_id_prefix: &str,
        tool_name: &str,
        before: &WorkspaceChangeSnapshot,
        after: &WorkspaceChangeSnapshot,
        description_prefix: &str,
        metadata: Option<&FileChangeMetadata>,
    ) -> usize {
        let changes = self.detect_workspace_changes(before, after);
        for (idx, change) in changes.iter().enumerate() {
            let action = match change.change_type {
                WorkspaceChangeType::Created => "created",
                WorkspaceChangeType::Modified => "modified",
                WorkspaceChangeType::Deleted => "deleted",
            };
            let description = if description_prefix.is_empty() {
                action.to_string()
            } else {
                format!("{description_prefix} {action}")
            };
            self.record_change_at_with_metadata(
                turn_index,
                &format!("{tool_call_id_prefix}-{idx}"),
                tool_name,
                &change.path,
                change.before_hash.clone(),
                change.after_hash.as_deref(),
                &description,
                metadata,
            );
        }
        changes.len()
    }

    // ── Change Recording ────────────────────────────────────────────────

    /// Record a file modification.
    pub fn record_change(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        file_path: &str,
        before_hash: Option<String>,
        after_hash: Option<&str>,
        description: &str,
    ) {
        self.record_change_at_with_metadata(
            self.current_turn_index,
            tool_call_id,
            tool_name,
            file_path,
            before_hash,
            after_hash,
            description,
            None,
        );
    }

    pub fn record_change_at(
        &mut self,
        turn_index: u32,
        tool_call_id: &str,
        tool_name: &str,
        file_path: &str,
        before_hash: Option<String>,
        after_hash: Option<&str>,
        description: &str,
    ) {
        self.record_change_at_with_metadata(
            turn_index,
            tool_call_id,
            tool_name,
            file_path,
            before_hash,
            after_hash,
            description,
            None,
        );
    }

    pub fn record_change_at_with_metadata(
        &mut self,
        turn_index: u32,
        tool_call_id: &str,
        tool_name: &str,
        file_path: &str,
        before_hash: Option<String>,
        after_hash: Option<&str>,
        description: &str,
        metadata: Option<&FileChangeMetadata>,
    ) {
        let change = FileChange {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            turn_index,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            file_path: file_path.to_string(),
            before_hash,
            after_hash: after_hash.map(|value| value.to_string()),
            timestamp: chrono::Utc::now().timestamp_millis(),
            description: description.to_string(),
            source_mode: metadata.and_then(|value| value.source_mode),
            actor_kind: metadata.and_then(|value| value.actor_kind),
            actor_id: metadata.and_then(|value| value.actor_id.clone()),
            actor_label: metadata.and_then(|value| value.actor_label.clone()),
            sub_agent_depth: metadata.and_then(|value| value.sub_agent_depth),
            origin_session_id: metadata.and_then(|value| value.origin_session_id.clone()),
        };
        // Capture event fields before moving change into the vec
        let event = FileChangeEvent {
            session_id: self.session_id.clone(),
            turn_index,
            file_path: change.file_path.clone(),
            tool_name: change.tool_name.clone(),
            change_id: change.id.clone(),
            before_hash: change.before_hash.clone(),
            after_hash: change.after_hash.clone(),
            description: change.description.clone(),
            source_mode: change.source_mode,
            actor_kind: change.actor_kind,
            actor_id: change.actor_id.clone(),
            actor_label: change.actor_label.clone(),
            sub_agent_depth: change.sub_agent_depth,
            origin_session_id: change.origin_session_id.clone(),
        };
        self.changes.push(change);
        // Persist after each change (fire-and-forget)
        let _ = self.persist();
        // Notify the frontend about the new change
        if let Some(ref handle) = self.app_handle {
            let _ = handle.emit("file-change-recorded", event);
        }
    }

    // ── Query ───────────────────────────────────────────────────────────

    /// Get all changes grouped by turn.
    pub fn get_changes_by_turn(&self) -> Vec<TurnChanges> {
        let mut by_turn: HashMap<u32, Vec<FileChange>> = HashMap::new();
        for change in &self.changes {
            by_turn
                .entry(change.turn_index)
                .or_default()
                .push(change.clone());
        }

        let mut result: Vec<TurnChanges> = by_turn
            .into_iter()
            .map(|(turn_index, changes)| {
                let timestamp = changes.iter().map(|c| c.timestamp).min().unwrap_or(0);
                TurnChanges {
                    turn_index,
                    changes,
                    timestamp,
                }
            })
            .collect();

        result.sort_by_key(|t| t.turn_index);
        result
    }

    /// Get the total number of recorded changes.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    // ── Diff ────────────────────────────────────────────────────────────

    /// Compute a unified diff between two CAS blobs.
    pub fn get_file_diff(
        &self,
        before_hash: Option<&str>,
        after_hash: Option<&str>,
    ) -> Result<String, String> {
        let before_content = match before_hash {
            Some(h) => {
                let bytes = self.get_content(h)?;
                String::from_utf8_lossy(&bytes).to_string()
            }
            None => String::new(),
        };

        let after_content = match after_hash {
            Some(h) => {
                let after_bytes = self.get_content(h)?;
                String::from_utf8_lossy(&after_bytes).to_string()
            }
            None => String::new(),
        };

        Ok(unified_diff(&before_content, &after_content))
    }

    // ── Restore ─────────────────────────────────────────────────────────

    fn compute_restore_targets(&self, turn_index: u32) -> Vec<RestorePreviewTarget> {
        // Keep the earliest change at/after target turn for each file.
        let mut target_map: HashMap<String, RestorePreviewTarget> = HashMap::new();
        for change in self.changes.iter().filter(|c| c.turn_index >= turn_index) {
            target_map
                .entry(change.file_path.clone())
                .or_insert_with(|| RestorePreviewTarget {
                    path: change.file_path.clone(),
                    target_hash: change.before_hash.clone(),
                    source_turn: change.turn_index,
                });
        }
        let mut targets: Vec<RestorePreviewTarget> = target_map.into_values().collect();
        targets.sort_by(|a, b| a.path.cmp(&b.path));
        targets
    }

    /// Preview all files that would be affected by restoring to before a turn.
    pub fn preview_restore_to_before_turn(&self, turn_index: u32) -> Vec<RestorePreviewItem> {
        self.compute_restore_targets(turn_index)
            .into_iter()
            .map(|t| RestorePreviewItem {
                path: t.path,
                action: if t.target_hash.is_some() {
                    "restore".to_string()
                } else {
                    "delete".to_string()
                },
                source_turn: t.source_turn,
            })
            .collect()
    }

    fn restore_target_to_disk(
        &self,
        target: &RestorePreviewTarget,
    ) -> Result<RestoredFile, String> {
        let full_path = self.project_root.join(&target.path);
        match target.target_hash.as_deref() {
            None => {
                if full_path.exists() {
                    fs::remove_file(&full_path)
                        .map_err(|e| format!("Failed to delete {}: {}", target.path, e))?;
                }
                Ok(RestoredFile {
                    path: target.path.clone(),
                    action: "deleted".to_string(),
                })
            }
            Some(hash) => {
                let content = self.get_content(hash)?;
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create dirs for {}: {}", target.path, e))?;
                }
                fs::write(&full_path, &content)
                    .map_err(|e| format!("Failed to restore {}: {}", target.path, e))?;
                Ok(RestoredFile {
                    path: target.path.clone(),
                    action: "restored".to_string(),
                })
            }
        }
    }

    fn capture_restore_snapshot(
        &self,
        targets: &[RestorePreviewTarget],
    ) -> Result<Vec<RestoreSnapshotEntry>, String> {
        let mut snapshots = Vec::with_capacity(targets.len());
        for target in targets {
            let full_path = self.project_root.join(&target.path);
            let before_restore_hash = if full_path.exists() {
                let bytes = fs::read(&full_path)
                    .map_err(|e| format!("Failed to read {} for snapshot: {}", target.path, e))?;
                Some(self.store_content(&bytes)?)
            } else {
                None
            };
            snapshots.push(RestoreSnapshotEntry {
                path: target.path.clone(),
                before_restore_hash,
            });
        }
        Ok(snapshots)
    }

    fn apply_snapshot_entries(
        &self,
        entries: &[RestoreSnapshotEntry],
    ) -> Result<Vec<RestoredFile>, String> {
        let mut reverted = Vec::with_capacity(entries.len());
        for entry in entries {
            let full_path = self.project_root.join(&entry.path);
            match entry.before_restore_hash.as_deref() {
                Some(hash) => {
                    let content = self.get_content(hash)?;
                    if let Some(parent) = full_path.parent() {
                        fs::create_dir_all(parent).map_err(|e| {
                            format!("Failed to create dirs for {}: {}", entry.path, e)
                        })?;
                    }
                    fs::write(&full_path, &content)
                        .map_err(|e| format!("Failed to undo restore for {}: {}", entry.path, e))?;
                    reverted.push(RestoredFile {
                        path: entry.path.clone(),
                        action: "restored".to_string(),
                    });
                }
                None => {
                    if full_path.exists() {
                        fs::remove_file(&full_path).map_err(|e| {
                            format!("Failed to remove {} during undo: {}", entry.path, e)
                        })?;
                    }
                    reverted.push(RestoredFile {
                        path: entry.path.clone(),
                        action: "deleted".to_string(),
                    });
                }
            }
        }
        Ok(reverted)
    }

    /// Restore all files to their state before the given turn index.
    ///
    /// For each file modified in `turn_index` or later:
    /// - Find the state of that file just before `turn_index`
    /// - If the file didn't exist before, delete it
    /// - Otherwise, restore from CAS
    ///
    /// Optionally creates an undo snapshot and returns an operation ID.
    pub fn restore_to_before_turn_v2(
        &mut self,
        turn_index: u32,
        create_snapshot: bool,
    ) -> Result<RestoreExecutionResult, String> {
        let targets = self.compute_restore_targets(turn_index);
        if targets.is_empty() {
            return Ok(RestoreExecutionResult {
                operation_id: None,
                restored: Vec::new(),
            });
        }

        let operation = if create_snapshot {
            let snapshots = self.capture_restore_snapshot(&targets)?;
            let op = RestoreOperation {
                operation_id: uuid::Uuid::new_v4().to_string(),
                session_id: self.session_id.clone(),
                turn_index,
                timestamp: chrono::Utc::now().timestamp_millis(),
                files: snapshots,
            };
            Some(op)
        } else {
            None
        };

        let mut restored = Vec::with_capacity(targets.len());
        let rollback_entries = operation.as_ref().map(|op| op.files.clone());
        for target in &targets {
            match self.restore_target_to_disk(target) {
                Ok(file) => restored.push(file),
                Err(e) => {
                    if let Some(entries) = &rollback_entries {
                        let _ = self.apply_snapshot_entries(entries);
                    }
                    return Err(e);
                }
            }
        }

        let operation_id = if let Some(op) = operation {
            self.persist_restore_operation(&op)?;
            Some(op.operation_id)
        } else {
            None
        };

        Ok(RestoreExecutionResult {
            operation_id,
            restored,
        })
    }

    /// Undo a previous restore operation by operation ID.
    pub fn undo_restore(&mut self, operation_id: &str) -> Result<Vec<RestoredFile>, String> {
        let operation = self.load_restore_operation(operation_id)?;
        self.apply_snapshot_entries(&operation.files)
    }

    /// Restore a single file to a specific CAS version.
    pub fn restore_single_file(&self, file_path: &str, target_hash: &str) -> Result<bool, String> {
        let content = self.get_content(target_hash)?;
        let full_path = self.project_root.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create dirs: {e}"))?;
        }
        fs::write(&full_path, &content).map_err(|e| format!("Failed to restore file: {e}"))?;
        Ok(true)
    }

    // ── Persistence ─────────────────────────────────────────────────────

    fn changes_file_path(&self) -> PathBuf {
        self.data_dir
            .join("changes")
            .join(format!("{}.json", self.session_id))
    }

    fn restore_ops_dir(&self) -> PathBuf {
        self.data_dir.join("restore-ops").join(&self.session_id)
    }

    fn restore_operation_file_path(&self, operation_id: &str) -> PathBuf {
        self.restore_ops_dir()
            .join(format!("{}.json", operation_id))
    }

    fn restore_last_file_path(&self) -> PathBuf {
        self.restore_ops_dir().join("last.json")
    }

    fn persist_restore_operation(&self, operation: &RestoreOperation) -> Result<(), String> {
        let ops_dir = self.restore_ops_dir();
        fs::create_dir_all(&ops_dir)
            .map_err(|e| format!("Failed to create restore ops dir: {e}"))?;
        let op_json = serde_json::to_string_pretty(operation)
            .map_err(|e| format!("Failed to serialize restore op: {e}"))?;
        let op_path = self.restore_operation_file_path(&operation.operation_id);
        fs::write(&op_path, &op_json)
            .map_err(|e| format!("Failed to write restore op file: {e}"))?;
        let last_path = self.restore_last_file_path();
        fs::write(last_path, op_json)
            .map_err(|e| format!("Failed to write last restore op file: {e}"))?;
        Ok(())
    }

    fn load_restore_operation(&self, operation_id: &str) -> Result<RestoreOperation, String> {
        let path = self.restore_operation_file_path(operation_id);
        let data = fs::read_to_string(path)
            .map_err(|e| format!("Restore operation not found ({operation_id}): {e}"))?;
        serde_json::from_str::<RestoreOperation>(&data)
            .map_err(|e| format!("Failed to parse restore operation ({operation_id}): {e}"))
    }

    /// Persist change records to disk.
    pub fn persist(&self) -> Result<(), String> {
        let path = self.changes_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create changes dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(&self.changes)
            .map_err(|e| format!("Failed to serialize changes: {e}"))?;
        fs::write(&path, json).map_err(|e| format!("Failed to write changes file: {e}"))?;
        Ok(())
    }

    /// Load change records from disk (no error on missing file).
    fn load_silent(&mut self) {
        let path = self.changes_file_path();
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(changes) = serde_json::from_str::<Vec<FileChange>>(&data) {
                self.changes = changes;
                // Restore turn_index to the maximum recorded
                if let Some(max_turn) = self.changes.iter().map(|c| c.turn_index).max() {
                    self.current_turn_index = max_turn;
                }
            }
        }
    }

    /// Remove all changes at or after the given turn (for rollback cleanup).
    pub fn truncate_from_turn(&mut self, turn_index: u32) {
        self.changes.retain(|c| c.turn_index < turn_index);
        let _ = self.persist();
    }
}

fn should_skip_workspace_path(
    project_root: &Path,
    tracker_data_dir: &Path,
    path: &Path,
    is_dir: bool,
) -> bool {
    if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
        if name == ".git" || name == ".plan-cascade" {
            return true;
        }
    }

    if path.starts_with(tracker_data_dir) {
        return true;
    }

    if let Ok(global_plan_cascade_dir) = plan_cascade_dir() {
        if path.starts_with(global_plan_cascade_dir) {
            return true;
        }
    }

    if is_dir {
        return path == project_root.join(".git") || path == project_root.join(".plan-cascade");
    }

    false
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Simple unified diff implementation.
fn unified_diff(before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    // Use a simple LCS-based diff
    let mut output = String::new();
    let (m, n) = (before_lines.len(), after_lines.len());

    // Build LCS table
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if before_lines[i - 1] == after_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to produce diff
    let mut i = m;
    let mut j = n;
    let mut diff_lines: Vec<String> = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && before_lines[i - 1] == after_lines[j - 1] {
            diff_lines.push(format!(" {}", before_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            diff_lines.push(format!("+{}", after_lines[j - 1]));
            j -= 1;
        } else if i > 0 {
            diff_lines.push(format!("-{}", before_lines[i - 1]));
            i -= 1;
        }
    }

    diff_lines.reverse();
    for line in &diff_lines {
        output.push_str(line);
        output.push('\n');
    }

    output
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tracker(dir: &Path) -> FileChangeTracker {
        // Use the project dir as both project root and data dir in tests
        // to keep everything inside the temp directory.
        FileChangeTracker::new_with_data_dir("test-session", dir, dir)
    }

    #[test]
    fn test_store_and_retrieve_content() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let content = b"hello world";
        let hash = tracker.store_content(content).unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 hex
        let retrieved = tracker.get_content(&hash).unwrap();
        assert_eq!(retrieved, content);
    }

    #[test]
    fn test_store_deduplicates() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let h1 = tracker.store_content(b"same content").unwrap();
        let h2 = tracker.store_content(b"same content").unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_capture_before_existing_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "original").unwrap();
        let tracker = make_tracker(dir.path());
        let hash = tracker.capture_before(&file);
        assert!(hash.is_some());
        let content = tracker.get_content(&hash.unwrap()).unwrap();
        assert_eq!(content, b"original");
    }

    #[test]
    fn test_capture_before_nonexistent() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let hash = tracker.capture_before(&dir.path().join("nope.txt"));
        assert!(hash.is_none());
    }

    #[test]
    fn test_record_and_query_changes() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());

        tracker.set_turn_index(0);
        tracker.record_change(
            "tc1",
            "Write",
            "src/a.rs",
            None,
            Some("hash_a"),
            "Wrote 10 lines",
        );
        tracker.set_turn_index(1);
        tracker.record_change(
            "tc2",
            "Edit",
            "src/b.rs",
            Some("hash_b0".to_string()),
            Some("hash_b1"),
            "Edited 1 occurrence",
        );

        let turns = tracker.get_changes_by_turn();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].turn_index, 0);
        assert_eq!(turns[0].changes.len(), 1);
        assert_eq!(turns[1].turn_index, 1);
        assert_eq!(turns[1].changes.len(), 1);
    }

    #[test]
    fn test_restore_to_before_turn() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());

        // Simulate: Turn 0 creates a new file
        let file_path = dir.path().join("new.txt");
        tracker.set_turn_index(0);
        let after_hash = tracker.store_content(b"new content").unwrap();
        fs::write(&file_path, "new content").unwrap();
        tracker.record_change(
            "tc1",
            "Write",
            "new.txt",
            None,
            Some(&after_hash),
            "Wrote file",
        );

        // Simulate: Turn 1 edits an existing file
        let existing = dir.path().join("existing.txt");
        fs::write(&existing, "original").unwrap();
        let before_hash = tracker.store_content(b"original").unwrap();
        tracker.set_turn_index(1);
        let edit_after = tracker.store_content(b"modified").unwrap();
        fs::write(&existing, "modified").unwrap();
        tracker.record_change(
            "tc2",
            "Edit",
            "existing.txt",
            Some(before_hash),
            Some(&edit_after),
            "Edited",
        );

        // Restore to before turn 1 — should restore existing.txt, keep new.txt
        let restored = tracker
            .restore_to_before_turn_v2(1, false)
            .unwrap()
            .restored;
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].path, "existing.txt");
        assert_eq!(restored[0].action, "restored");
        let content = fs::read_to_string(&existing).unwrap();
        assert_eq!(content, "original");

        // Restore to before turn 0 — should delete new.txt
        let restored = tracker
            .restore_to_before_turn_v2(0, false)
            .unwrap()
            .restored;
        assert_eq!(restored.len(), 2); // new.txt + existing.txt
        let new_file_restore = restored.iter().find(|r| r.path == "new.txt").unwrap();
        assert_eq!(new_file_restore.action, "deleted");
        assert!(!file_path.exists());
    }

    #[test]
    fn test_preview_restore_to_before_turn() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());

        // Turn 0: create new file
        let new_file = dir.path().join("new.txt");
        fs::write(&new_file, "new content").unwrap();
        let new_after = tracker.store_content(b"new content").unwrap();
        tracker.set_turn_index(0);
        tracker.record_change(
            "tc-new",
            "Write",
            "new.txt",
            None,
            Some(&new_after),
            "Created new file",
        );

        // Turn 1: edit existing file
        let existing = dir.path().join("existing.txt");
        fs::write(&existing, "original").unwrap();
        let existing_before = tracker.store_content(b"original").unwrap();
        fs::write(&existing, "modified").unwrap();
        let existing_after = tracker.store_content(b"modified").unwrap();
        tracker.set_turn_index(1);
        tracker.record_change(
            "tc-edit",
            "Edit",
            "existing.txt",
            Some(existing_before),
            Some(&existing_after),
            "Edited existing file",
        );

        let preview_turn_0 = tracker.preview_restore_to_before_turn(0);
        assert_eq!(preview_turn_0.len(), 2);
        let new_preview = preview_turn_0.iter().find(|p| p.path == "new.txt").unwrap();
        assert_eq!(new_preview.action, "delete");
        let existing_preview = preview_turn_0
            .iter()
            .find(|p| p.path == "existing.txt")
            .unwrap();
        assert_eq!(existing_preview.action, "restore");
        assert_eq!(existing_preview.source_turn, 1);

        let preview_turn_1 = tracker.preview_restore_to_before_turn(1);
        assert_eq!(preview_turn_1.len(), 1);
        assert_eq!(preview_turn_1[0].path, "existing.txt");
        assert_eq!(preview_turn_1[0].action, "restore");
    }

    #[test]
    fn test_restore_v2_and_undo_restore() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());

        // Existing file edited by AI in turn 1
        let existing = dir.path().join("existing.txt");
        fs::write(&existing, "original").unwrap();
        let existing_before = tracker.store_content(b"original").unwrap();
        fs::write(&existing, "modified").unwrap();
        let existing_after = tracker.store_content(b"modified").unwrap();
        tracker.set_turn_index(1);
        tracker.record_change(
            "tc-edit",
            "Edit",
            "existing.txt",
            Some(existing_before),
            Some(&existing_after),
            "Edited existing file",
        );

        // New file created by AI in same turn
        let new_file = dir.path().join("new.txt");
        fs::write(&new_file, "new content").unwrap();
        let new_after = tracker.store_content(b"new content").unwrap();
        tracker.record_change(
            "tc-new",
            "Write",
            "new.txt",
            None,
            Some(&new_after),
            "Created new file",
        );

        // Restore to before turn 1
        let restore = tracker.restore_to_before_turn_v2(1, true).unwrap();
        assert!(restore.operation_id.is_some());
        assert_eq!(fs::read_to_string(&existing).unwrap(), "original");
        assert!(!new_file.exists());

        // Undo restore
        let op_id = restore.operation_id.unwrap();
        let undone = tracker.undo_restore(&op_id).unwrap();
        assert_eq!(undone.len(), 2);
        assert_eq!(fs::read_to_string(&existing).unwrap(), "modified");
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "new content");
    }

    #[test]
    fn test_persist_and_reload() {
        let dir = TempDir::new().unwrap();
        {
            let mut tracker = make_tracker(dir.path());
            tracker.set_turn_index(2);
            tracker.record_change("tc1", "Write", "a.txt", None, Some("hash1"), "Wrote");
        }
        // New tracker should reload persisted changes
        let tracker = make_tracker(dir.path());
        assert_eq!(tracker.change_count(), 1);
        assert_eq!(tracker.turn_index(), 2);
    }

    #[test]
    fn test_persist_and_reload_with_metadata() {
        let dir = TempDir::new().unwrap();
        {
            let mut tracker = make_tracker(dir.path());
            tracker.record_change_at_with_metadata(
                3,
                "tc-meta",
                "Write",
                "a.txt",
                None,
                Some("hash1"),
                "Wrote",
                Some(&FileChangeMetadata {
                    source_mode: Some(FileChangeSourceMode::Task),
                    actor_kind: Some(FileChangeActorKind::SubAgent),
                    actor_id: Some("story-agent".to_string()),
                    actor_label: Some("Story Agent".to_string()),
                    sub_agent_depth: Some(1),
                    origin_session_id: Some("root-session".to_string()),
                }),
            );
        }

        let tracker = make_tracker(dir.path());
        let turns = tracker.get_changes_by_turn();
        assert_eq!(turns.len(), 1);
        let change = &turns[0].changes[0];
        assert_eq!(change.source_mode, Some(FileChangeSourceMode::Task));
        assert_eq!(change.actor_kind, Some(FileChangeActorKind::SubAgent));
        assert_eq!(change.actor_label.as_deref(), Some("Story Agent"));
        assert_eq!(change.origin_session_id.as_deref(), Some("root-session"));
    }

    #[test]
    fn test_load_legacy_change_file_without_metadata() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("tracker");
        fs::create_dir_all(data_dir.join("changes")).unwrap();
        fs::write(
            data_dir.join("changes").join("session-1.json"),
            r#"[{
                "id":"legacy-1",
                "session_id":"session-1",
                "turn_index":2,
                "tool_call_id":"tool-1",
                "tool_name":"Write",
                "file_path":"a.txt",
                "before_hash":null,
                "after_hash":"hash1",
                "timestamp":123,
                "description":"legacy write"
            }]"#,
        )
        .unwrap();

        let tracker = FileChangeTracker::new_with_data_dir("session-1", dir.path(), &data_dir);
        let turns = tracker.get_changes_by_turn();
        assert_eq!(turns.len(), 1);
        let change = &turns[0].changes[0];
        assert_eq!(change.file_path, "a.txt");
        assert!(change.source_mode.is_none());
        assert!(change.actor_kind.is_none());
    }

    #[test]
    fn test_diff() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let h1 = tracker.store_content(b"line 1\nline 2\nline 3").unwrap();
        let h2 = tracker.store_content(b"line 1\nmodified\nline 3").unwrap();
        let diff = tracker.get_file_diff(Some(&h1), Some(&h2)).unwrap();
        assert!(diff.contains("+modified"));
        assert!(diff.contains("-line 2"));
    }

    #[test]
    fn test_truncate_from_turn() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());
        tracker.set_turn_index(0);
        tracker.record_change("tc1", "Write", "a.txt", None, Some("h1"), "Wrote");
        tracker.set_turn_index(1);
        tracker.record_change("tc2", "Write", "b.txt", None, Some("h2"), "Wrote");
        tracker.set_turn_index(2);
        tracker.record_change("tc3", "Write", "c.txt", None, Some("h3"), "Wrote");

        tracker.truncate_from_turn(1);
        assert_eq!(tracker.change_count(), 1);
        let turns = tracker.get_changes_by_turn();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].turn_index, 0);
    }

    #[test]
    fn test_rejects_oversized_content() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let big = vec![0u8; MAX_CAS_FILE_SIZE + 1];
        let result = tracker.store_content(&big);
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_snapshot_detects_create_modify_delete() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());

        fs::write(dir.path().join("old.txt"), "remove me").unwrap();
        fs::write(dir.path().join("keep.txt"), "before").unwrap();
        let before = tracker.capture_workspace_snapshot().unwrap();

        fs::remove_file(dir.path().join("old.txt")).unwrap();
        fs::write(dir.path().join("keep.txt"), "after").unwrap();
        fs::write(dir.path().join("new.txt"), "created").unwrap();
        let after = tracker.capture_workspace_snapshot().unwrap();

        let changes = tracker.detect_workspace_changes(&before, &after);
        assert_eq!(changes.len(), 3);
        assert!(changes.iter().any(|change| {
            change.path == "old.txt" && change.change_type == WorkspaceChangeType::Deleted
        }));
        assert!(changes.iter().any(|change| {
            change.path == "keep.txt" && change.change_type == WorkspaceChangeType::Modified
        }));
        assert!(changes.iter().any(|change| {
            change.path == "new.txt" && change.change_type == WorkspaceChangeType::Created
        }));

        tracker.record_workspace_delta_between_at(
            7,
            "workspace",
            "Bash",
            &before,
            &after,
            "workspace delta",
        );
        let turns = tracker.get_changes_by_turn();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].turn_index, 7);
        assert_eq!(turns[0].changes.len(), 3);
        let deleted = turns[0]
            .changes
            .iter()
            .find(|change| change.file_path == "old.txt")
            .unwrap();
        assert!(deleted.after_hash.is_none());
    }
}
