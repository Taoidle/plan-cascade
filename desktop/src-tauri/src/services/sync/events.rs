//! File System Event Definitions
//!
//! Defines event types and payloads for file system change notifications.
//! Events are broadcast via Tauri's event system for multi-window sync.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Runtime};

/// Event channel names for file system sync events
pub mod channels {
    /// Project list changes (new/deleted projects)
    pub const PROJECT_CHANGE: &str = "sync:project_change";
    /// File changes within a project
    pub const FILE_CHANGE: &str = "sync:file_change";
    /// PRD file changes
    pub const PRD_CHANGE: &str = "sync:prd_change";
    /// Progress file changes
    pub const PROGRESS_CHANGE: &str = "sync:progress_change";
    /// Watch error events
    pub const WATCH_ERROR: &str = "sync:watch_error";
    /// Watch status events (started, stopped)
    pub const WATCH_STATUS: &str = "sync:watch_status";
}

/// Type of file system change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File or directory was created
    Created,
    /// File or directory was modified
    Modified,
    /// File or directory was deleted
    Deleted,
    /// File or directory was renamed
    Renamed,
}

impl From<notify::EventKind> for ChangeType {
    fn from(kind: notify::EventKind) -> Self {
        use notify::EventKind;
        match kind {
            EventKind::Create(_) => ChangeType::Created,
            EventKind::Modify(_) => ChangeType::Modified,
            EventKind::Remove(_) => ChangeType::Deleted,
            EventKind::Any | EventKind::Access(_) | EventKind::Other => ChangeType::Modified,
        }
    }
}

/// Project change event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChangeEvent {
    /// Type of change (created, deleted)
    pub change_type: ChangeType,
    /// Project directory path
    pub path: String,
    /// Project ID (derived from directory name)
    pub project_id: Option<String>,
    /// Timestamp of the event
    pub timestamp: String,
}

impl ProjectChangeEvent {
    /// Create a new project change event
    pub fn new(change_type: ChangeType, path: PathBuf) -> Self {
        let project_id = path.file_name().map(|n| n.to_string_lossy().to_string());

        Self {
            change_type,
            path: path.to_string_lossy().to_string(),
            project_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// File change event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeEvent {
    /// Type of change
    pub change_type: ChangeType,
    /// Full path to the changed file
    pub path: String,
    /// Relative path within the project
    pub relative_path: Option<String>,
    /// Project ID this file belongs to
    pub project_id: Option<String>,
    /// File extension (if any)
    pub extension: Option<String>,
    /// Timestamp of the event
    pub timestamp: String,
}

impl FileChangeEvent {
    /// Create a new file change event
    pub fn new(change_type: ChangeType, path: PathBuf, project_path: Option<&PathBuf>) -> Self {
        let relative_path = project_path.and_then(|proj| {
            path.strip_prefix(proj)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        });

        let project_id =
            project_path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

        let extension = path.extension().map(|e| e.to_string_lossy().to_string());

        Self {
            change_type,
            path: path.to_string_lossy().to_string(),
            relative_path,
            project_id,
            extension,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// PRD change event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdChangeEvent {
    /// Type of change
    pub change_type: ChangeType,
    /// Full path to the PRD file
    pub path: String,
    /// Project ID this PRD belongs to
    pub project_id: Option<String>,
    /// Timestamp of the event
    pub timestamp: String,
}

impl PrdChangeEvent {
    /// Create a new PRD change event
    pub fn new(change_type: ChangeType, path: PathBuf) -> Self {
        // Try to extract project ID from path
        let project_id = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        Self {
            change_type,
            path: path.to_string_lossy().to_string(),
            project_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Progress file change event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressChangeEvent {
    /// Type of change
    pub change_type: ChangeType,
    /// Full path to the progress file
    pub path: String,
    /// Project ID this progress file belongs to
    pub project_id: Option<String>,
    /// Timestamp of the event
    pub timestamp: String,
}

impl ProgressChangeEvent {
    /// Create a new progress change event
    pub fn new(change_type: ChangeType, path: PathBuf) -> Self {
        // Try to extract project ID from path
        let project_id = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        Self {
            change_type,
            path: path.to_string_lossy().to_string(),
            project_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Watch error event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchErrorEvent {
    /// Error message
    pub message: String,
    /// Path that caused the error (if applicable)
    pub path: Option<String>,
    /// Error kind (permission_denied, path_not_found, etc.)
    pub kind: WatchErrorKind,
    /// Timestamp of the event
    pub timestamp: String,
}

/// Types of watch errors
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchErrorKind {
    /// Permission denied to watch path
    PermissionDenied,
    /// Path does not exist
    PathNotFound,
    /// Too many files to watch
    MaxFilesReached,
    /// Generic watch error
    Generic,
}

impl WatchErrorEvent {
    /// Create a new watch error event
    pub fn new(message: String, path: Option<PathBuf>, kind: WatchErrorKind) -> Self {
        Self {
            message,
            path: path.map(|p| p.to_string_lossy().to_string()),
            kind,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create from a notify error
    pub fn from_notify_error(error: &notify::Error) -> Self {
        let kind = match &error.kind {
            notify::ErrorKind::PathNotFound => WatchErrorKind::PathNotFound,
            notify::ErrorKind::MaxFilesWatch => WatchErrorKind::MaxFilesReached,
            notify::ErrorKind::Io(io_err)
                if io_err.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                WatchErrorKind::PermissionDenied
            }
            _ => WatchErrorKind::Generic,
        };

        let path = error.paths.first().map(|p| p.to_path_buf());

        Self::new(error.to_string(), path, kind)
    }
}

/// Watch status event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatusEvent {
    /// Status of the watcher
    pub status: WatchStatus,
    /// Path being watched (if applicable)
    pub path: Option<String>,
    /// Message with additional details
    pub message: Option<String>,
    /// Timestamp of the event
    pub timestamp: String,
}

/// Status of the file watcher
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    /// Watcher started
    Started,
    /// Watcher stopped
    Stopped,
    /// Watch added for a path
    WatchAdded,
    /// Watch removed for a path
    WatchRemoved,
}

impl WatchStatusEvent {
    /// Create a new watch status event
    pub fn new(status: WatchStatus, path: Option<PathBuf>, message: Option<String>) -> Self {
        Self {
            status,
            path: path.map(|p| p.to_string_lossy().to_string()),
            message,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Event emitter for file sync events
///
/// Wraps Tauri's AppHandle to provide typed event emission
/// with proper error handling (log failures, don't crash).
pub struct SyncEventEmitter<R: Runtime> {
    app_handle: AppHandle<R>,
}

impl<R: Runtime> SyncEventEmitter<R> {
    /// Create a new event emitter
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self { app_handle }
    }

    /// Emit a project change event
    pub fn emit_project_change(&self, event: ProjectChangeEvent) {
        if let Err(e) = self.app_handle.emit(channels::PROJECT_CHANGE, &event) {
            eprintln!("[WARN] Failed to emit project change event: {}", e);
        }
    }

    /// Emit a file change event
    pub fn emit_file_change(&self, event: FileChangeEvent) {
        if let Err(e) = self.app_handle.emit(channels::FILE_CHANGE, &event) {
            eprintln!("[WARN] Failed to emit file change event: {}", e);
        }
    }

    /// Emit a PRD change event
    pub fn emit_prd_change(&self, event: PrdChangeEvent) {
        if let Err(e) = self.app_handle.emit(channels::PRD_CHANGE, &event) {
            eprintln!("[WARN] Failed to emit PRD change event: {}", e);
        }
    }

    /// Emit a progress change event
    pub fn emit_progress_change(&self, event: ProgressChangeEvent) {
        if let Err(e) = self.app_handle.emit(channels::PROGRESS_CHANGE, &event) {
            eprintln!("[WARN] Failed to emit progress change event: {}", e);
        }
    }

    /// Emit a watch error event
    pub fn emit_watch_error(&self, event: WatchErrorEvent) {
        if let Err(e) = self.app_handle.emit(channels::WATCH_ERROR, &event) {
            eprintln!("[WARN] Failed to emit watch error event: {}", e);
        }
    }

    /// Emit a watch status event
    pub fn emit_watch_status(&self, event: WatchStatusEvent) {
        if let Err(e) = self.app_handle.emit(channels::WATCH_STATUS, &event) {
            eprintln!("[WARN] Failed to emit watch status event: {}", e);
        }
    }
}

impl<R: Runtime> Clone for SyncEventEmitter<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_names() {
        assert_eq!(channels::PROJECT_CHANGE, "sync:project_change");
        assert_eq!(channels::FILE_CHANGE, "sync:file_change");
        assert_eq!(channels::PRD_CHANGE, "sync:prd_change");
        assert_eq!(channels::PROGRESS_CHANGE, "sync:progress_change");
        assert_eq!(channels::WATCH_ERROR, "sync:watch_error");
        assert_eq!(channels::WATCH_STATUS, "sync:watch_status");
    }

    #[test]
    fn test_project_change_event_creation() {
        let path = PathBuf::from("/home/user/.claude/projects/abc123-myproject");
        let event = ProjectChangeEvent::new(ChangeType::Created, path);

        assert_eq!(event.change_type, ChangeType::Created);
        assert!(event.path.contains("abc123-myproject"));
        assert_eq!(event.project_id, Some("abc123-myproject".to_string()));
    }

    #[test]
    fn test_file_change_event_creation() {
        let path = PathBuf::from("/home/user/.claude/projects/abc123/src/main.rs");
        let project_path = PathBuf::from("/home/user/.claude/projects/abc123");
        let event = FileChangeEvent::new(ChangeType::Modified, path, Some(&project_path));

        assert_eq!(event.change_type, ChangeType::Modified);
        assert_eq!(event.extension, Some("rs".to_string()));
        assert_eq!(event.project_id, Some("abc123".to_string()));
    }

    #[test]
    fn test_prd_change_event_creation() {
        let path = PathBuf::from("/home/user/project/prd.json");
        let event = PrdChangeEvent::new(ChangeType::Modified, path);

        assert_eq!(event.change_type, ChangeType::Modified);
        assert!(event.path.contains("prd.json"));
    }

    #[test]
    fn test_progress_change_event_creation() {
        let path = PathBuf::from("/home/user/project/progress.txt");
        let event = ProgressChangeEvent::new(ChangeType::Modified, path);

        assert_eq!(event.change_type, ChangeType::Modified);
        assert!(event.path.contains("progress.txt"));
    }

    #[test]
    fn test_watch_error_event_creation() {
        let event = WatchErrorEvent::new(
            "Permission denied".to_string(),
            Some(PathBuf::from("/restricted/path")),
            WatchErrorKind::PermissionDenied,
        );

        assert_eq!(event.kind, WatchErrorKind::PermissionDenied);
        assert!(event.path.is_some());
    }

    #[test]
    fn test_watch_status_event_creation() {
        let event = WatchStatusEvent::new(
            WatchStatus::Started,
            Some(PathBuf::from("/watch/path")),
            Some("Watching started".to_string()),
        );

        assert_eq!(event.status, WatchStatus::Started);
        assert!(event.path.is_some());
        assert!(event.message.is_some());
    }

    #[test]
    fn test_change_type_serialization() {
        let json = serde_json::to_string(&ChangeType::Created).unwrap();
        assert_eq!(json, "\"created\"");

        let json = serde_json::to_string(&ChangeType::Modified).unwrap();
        assert_eq!(json, "\"modified\"");

        let json = serde_json::to_string(&ChangeType::Deleted).unwrap();
        assert_eq!(json, "\"deleted\"");
    }
}
