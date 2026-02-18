//! Git File Watcher
//!
//! Watches `.git/index` and `.git/HEAD` for changes and emits Tauri events
//! so the frontend can auto-refresh the git status/branch display.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use tauri::Emitter;
use tokio::sync::RwLock;

use super::types::GitWatchEvent;

/// Default debounce duration in milliseconds for git file watching.
const GIT_WATCH_DEBOUNCE_MS: u64 = 100;

/// Tauri event names emitted by the git watcher.
pub const EVENT_GIT_STATUS_CHANGED: &str = "git-status-changed";
pub const EVENT_GIT_HEAD_CHANGED: &str = "git-head-changed";

/// Watches git internal files for changes and emits Tauri events.
pub struct GitWatcher {
    /// Active debounced watcher (None if stopped).
    _watcher: Option<Debouncer<RecommendedWatcher>>,
    /// Repository path being watched.
    repo_path: PathBuf,
    /// Whether the watcher is currently active.
    active: Arc<RwLock<bool>>,
}

impl GitWatcher {
    /// Create a new GitWatcher (not yet started).
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            _watcher: None,
            repo_path,
            active: Arc::new(RwLock::new(false)),
        }
    }

    /// Start watching `.git/index` and `.git/HEAD`.
    ///
    /// Emits Tauri events:
    /// - `git-status-changed` when `.git/index` changes
    /// - `git-head-changed` when `.git/HEAD` changes
    pub fn start_watching<R: tauri::Runtime>(
        &mut self,
        app_handle: tauri::AppHandle<R>,
    ) -> Result<(), String> {
        let git_dir = self.resolve_git_dir()?;
        let repo_path_str = self.repo_path.to_string_lossy().to_string();
        let active = self.active.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(GIT_WATCH_DEBOUNCE_MS),
            move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let events = match events {
                    Ok(evts) => evts,
                    Err(_) => return,
                };

                for event in events {
                    if event.kind != DebouncedEventKind::Any {
                        continue;
                    }

                    let path = &event.path;
                    let file_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    let (event_name, change_kind) = if file_name == "index" {
                        (EVENT_GIT_STATUS_CHANGED, "index")
                    } else if file_name == "HEAD" {
                        (EVENT_GIT_HEAD_CHANGED, "head")
                    } else {
                        continue;
                    };

                    let payload = GitWatchEvent {
                        repo_path: repo_path_str.clone(),
                        change_kind: change_kind.to_string(),
                    };

                    // Emit the event to all frontend listeners
                    if let Ok(payload_json) = serde_json::to_string(&payload) {
                        let _ = app_handle.emit(event_name, payload_json);
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create git watcher: {}", e))?;

        // Watch the .git directory (non-recursive, only direct children)
        debouncer
            .watcher()
            .watch(&git_dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch git dir: {}", e))?;

        self._watcher = Some(debouncer);

        // Mark as active
        let active_clone = active;
        tokio::spawn(async move {
            let mut w = active_clone.write().await;
            *w = true;
        });

        Ok(())
    }

    /// Stop watching.
    pub fn stop_watching(&mut self) {
        self._watcher = None;
        let active = self.active.clone();
        tokio::spawn(async move {
            let mut w = active.write().await;
            *w = false;
        });
    }

    /// Check if the watcher is currently active.
    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }

    /// Get the repository path being watched.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Resolve the .git directory for this repo.
    fn resolve_git_dir(&self) -> Result<PathBuf, String> {
        let git_path = self.repo_path.join(".git");
        if git_path.is_dir() {
            Ok(git_path)
        } else if git_path.is_file() {
            // Worktree: .git is a file pointing to the real git dir
            let content = std::fs::read_to_string(&git_path)
                .map_err(|e| format!("Failed to read .git file: {}", e))?;
            let git_dir = content
                .trim()
                .strip_prefix("gitdir: ")
                .ok_or_else(|| "Invalid .git file format".to_string())?;
            let path = if Path::new(git_dir).is_absolute() {
                PathBuf::from(git_dir)
            } else {
                self.repo_path.join(git_dir)
            };
            Ok(path)
        } else {
            Err(format!(
                "No .git directory found at {}",
                self.repo_path.display()
            ))
        }
    }
}

impl std::fmt::Debug for GitWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitWatcher")
            .field("repo_path", &self.repo_path)
            .field("has_watcher", &self._watcher.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_watcher_creation() {
        let watcher = GitWatcher::new(PathBuf::from("/tmp/test-repo"));
        assert_eq!(watcher.repo_path(), Path::new("/tmp/test-repo"));
        assert!(watcher._watcher.is_none());
    }

    #[test]
    fn test_git_watcher_debug() {
        let watcher = GitWatcher::new(PathBuf::from("/tmp/test"));
        let debug = format!("{:?}", watcher);
        assert!(debug.contains("GitWatcher"));
        assert!(debug.contains("/tmp/test"));
    }

    #[tokio::test]
    async fn test_git_watcher_initially_inactive() {
        let watcher = GitWatcher::new(PathBuf::from("/tmp/test"));
        assert!(!watcher.is_active().await);
    }

    #[test]
    fn test_event_constants() {
        assert_eq!(EVENT_GIT_STATUS_CHANGED, "git-status-changed");
        assert_eq!(EVENT_GIT_HEAD_CHANGED, "git-head-changed");
    }

    #[test]
    fn test_resolve_git_dir_direct() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        std::fs::create_dir(repo.join(".git")).unwrap();

        let watcher = GitWatcher::new(repo.to_path_buf());
        let git_dir = watcher.resolve_git_dir().unwrap();
        assert_eq!(git_dir, repo.join(".git"));
    }

    #[test]
    fn test_resolve_git_dir_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();

        // Create a .git file (like in a worktree)
        let real_git_dir = tmp.path().join("real-git-dir");
        std::fs::create_dir(&real_git_dir).unwrap();
        std::fs::write(
            repo.join(".git"),
            format!("gitdir: {}", real_git_dir.display()),
        )
        .unwrap();

        let watcher = GitWatcher::new(repo.to_path_buf());
        let git_dir = watcher.resolve_git_dir().unwrap();
        assert_eq!(git_dir, real_git_dir);
    }

    #[test]
    fn test_resolve_git_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let watcher = GitWatcher::new(tmp.path().to_path_buf());
        assert!(watcher.resolve_git_dir().is_err());
    }

    #[tokio::test]
    async fn test_stop_watching_when_not_started() {
        let mut watcher = GitWatcher::new(PathBuf::from("/tmp/test"));
        watcher.stop_watching(); // Should not panic
        assert!(watcher._watcher.is_none());
    }
}
