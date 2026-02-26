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
    pub after_hash: String,
    pub timestamp: i64,
    pub description: String,
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
    after_hash: String,
    description: String,
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

    // ── Change Recording ────────────────────────────────────────────────

    /// Record a file modification.
    pub fn record_change(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        file_path: &str,
        before_hash: Option<String>,
        after_hash: &str,
        description: &str,
    ) {
        let change = FileChange {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            turn_index: self.current_turn_index,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            file_path: file_path.to_string(),
            before_hash,
            after_hash: after_hash.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            description: description.to_string(),
        };
        // Capture event fields before moving change into the vec
        let event = FileChangeEvent {
            session_id: self.session_id.clone(),
            turn_index: self.current_turn_index,
            file_path: change.file_path.clone(),
            tool_name: change.tool_name.clone(),
            change_id: change.id.clone(),
            before_hash: change.before_hash.clone(),
            after_hash: change.after_hash.clone(),
            description: change.description.clone(),
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
        after_hash: &str,
    ) -> Result<String, String> {
        let before_content = match before_hash {
            Some(h) => {
                let bytes = self.get_content(h)?;
                String::from_utf8_lossy(&bytes).to_string()
            }
            None => String::new(),
        };

        let after_bytes = self.get_content(after_hash)?;
        let after_content = String::from_utf8_lossy(&after_bytes).to_string();

        Ok(unified_diff(&before_content, &after_content))
    }

    // ── Restore ─────────────────────────────────────────────────────────

    /// Restore all files to their state before the given turn index.
    ///
    /// For each file modified in `turn_index` or later:
    /// - Find the state of that file just before `turn_index`
    /// - If the file didn't exist before (before_hash is None for its first
    ///   change in/after the target turn), delete it
    /// - Otherwise, restore from CAS
    pub fn restore_to_before_turn(&self, turn_index: u32) -> Result<Vec<RestoredFile>, String> {
        // Collect all changes at or after the target turn
        let affected_changes: Vec<&FileChange> = self
            .changes
            .iter()
            .filter(|c| c.turn_index >= turn_index)
            .collect();

        if affected_changes.is_empty() {
            return Ok(Vec::new());
        }

        // For each affected file, find the state just before the target turn.
        // This is the `before_hash` of the earliest change at/after `turn_index`.
        let mut file_restore_target: HashMap<&str, Option<&str>> = HashMap::new();
        for change in &affected_changes {
            file_restore_target
                .entry(&change.file_path)
                .or_insert(change.before_hash.as_deref());
        }

        let mut restored = Vec::new();
        for (file_path, target_hash) in &file_restore_target {
            let full_path = self.project_root.join(file_path);

            match target_hash {
                None => {
                    // File was created by LLM — delete it
                    if full_path.exists() {
                        fs::remove_file(&full_path)
                            .map_err(|e| format!("Failed to delete {file_path}: {e}"))?;
                    }
                    restored.push(RestoredFile {
                        path: file_path.to_string(),
                        action: "deleted".to_string(),
                    });
                }
                Some(hash) => {
                    let content = self.get_content(hash)?;
                    // Ensure parent directory exists
                    if let Some(parent) = full_path.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create dirs for {file_path}: {e}"))?;
                    }
                    fs::write(&full_path, &content)
                        .map_err(|e| format!("Failed to restore {file_path}: {e}"))?;
                    restored.push(RestoredFile {
                        path: file_path.to_string(),
                        action: "restored".to_string(),
                    });
                }
            }
        }

        Ok(restored)
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
        tracker.record_change("tc1", "Write", "src/a.rs", None, "hash_a", "Wrote 10 lines");
        tracker.set_turn_index(1);
        tracker.record_change(
            "tc2",
            "Edit",
            "src/b.rs",
            Some("hash_b0".to_string()),
            "hash_b1",
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
        tracker.record_change("tc1", "Write", "new.txt", None, &after_hash, "Wrote file");

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
            &edit_after,
            "Edited",
        );

        // Restore to before turn 1 — should restore existing.txt, keep new.txt
        let restored = tracker.restore_to_before_turn(1).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].path, "existing.txt");
        assert_eq!(restored[0].action, "restored");
        let content = fs::read_to_string(&existing).unwrap();
        assert_eq!(content, "original");

        // Restore to before turn 0 — should delete new.txt
        let restored = tracker.restore_to_before_turn(0).unwrap();
        assert_eq!(restored.len(), 2); // new.txt + existing.txt
        let new_file_restore = restored.iter().find(|r| r.path == "new.txt").unwrap();
        assert_eq!(new_file_restore.action, "deleted");
        assert!(!file_path.exists());
    }

    #[test]
    fn test_persist_and_reload() {
        let dir = TempDir::new().unwrap();
        {
            let mut tracker = make_tracker(dir.path());
            tracker.set_turn_index(2);
            tracker.record_change("tc1", "Write", "a.txt", None, "hash1", "Wrote");
        }
        // New tracker should reload persisted changes
        let tracker = make_tracker(dir.path());
        assert_eq!(tracker.change_count(), 1);
        assert_eq!(tracker.turn_index(), 2);
    }

    #[test]
    fn test_diff() {
        let dir = TempDir::new().unwrap();
        let tracker = make_tracker(dir.path());
        let h1 = tracker.store_content(b"line 1\nline 2\nline 3").unwrap();
        let h2 = tracker.store_content(b"line 1\nmodified\nline 3").unwrap();
        let diff = tracker.get_file_diff(Some(&h1), &h2).unwrap();
        assert!(diff.contains("+modified"));
        assert!(diff.contains("-line 2"));
    }

    #[test]
    fn test_truncate_from_turn() {
        let dir = TempDir::new().unwrap();
        let mut tracker = make_tracker(dir.path());
        tracker.set_turn_index(0);
        tracker.record_change("tc1", "Write", "a.txt", None, "h1", "Wrote");
        tracker.set_turn_index(1);
        tracker.record_change("tc2", "Write", "b.txt", None, "h2", "Wrote");
        tracker.set_turn_index(2);
        tracker.record_change("tc3", "Write", "c.txt", None, "h3", "Wrote");

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
}
