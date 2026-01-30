//! File System Watcher Service
//!
//! Implements real-time file system watching using the `notify` crate.
//! Supports watching multiple directories with debounced event handling.

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Runtime};
use tokio::sync::{mpsc, RwLock};

use super::events::{
    ChangeType, FileChangeEvent, PrdChangeEvent, ProgressChangeEvent,
    ProjectChangeEvent, SyncEventEmitter, WatchErrorEvent, WatchErrorKind,
    WatchStatus, WatchStatusEvent,
};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::claude_projects_dir;

/// Default debounce duration in milliseconds
const DEFAULT_DEBOUNCE_MS: u64 = 100;

/// Watch target types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WatchTarget {
    /// Watch the Claude projects directory for project changes
    ProjectsDirectory,
    /// Watch a specific project directory for file changes
    ProjectDirectory(PathBuf),
    /// Watch a specific file (e.g., prd.json, progress.txt)
    SpecificFile(PathBuf),
}

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce duration for rapid changes
    pub debounce_ms: u64,
    /// Whether to watch recursively
    pub recursive: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            recursive: true,
        }
    }
}

/// File system watcher service state
struct WatcherState {
    /// Active watchers by target
    watchers: HashMap<WatchTarget, Debouncer<RecommendedWatcher>>,
    /// Paths being watched
    watched_paths: HashMap<PathBuf, WatchTarget>,
}

impl WatcherState {
    fn new() -> Self {
        Self {
            watchers: HashMap::new(),
            watched_paths: HashMap::new(),
        }
    }
}

/// File System Watcher Service
///
/// Provides real-time file system watching with:
/// - Debounced event handling (100ms default)
/// - Multi-directory support
/// - Graceful error handling
/// - Tauri event broadcasting
pub struct FileWatcherService<R: Runtime> {
    /// Tauri app handle for event emission
    app_handle: AppHandle<R>,
    /// Event emitter
    emitter: SyncEventEmitter<R>,
    /// Watcher state (thread-safe)
    state: Arc<RwLock<WatcherState>>,
    /// Watcher configuration
    config: WatcherConfig,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl<R: Runtime> FileWatcherService<R> {
    /// Create a new file watcher service
    pub fn new(app_handle: AppHandle<R>) -> Self {
        let emitter = SyncEventEmitter::new(app_handle.clone());
        Self {
            app_handle,
            emitter,
            state: Arc::new(RwLock::new(WatcherState::new())),
            config: WatcherConfig::default(),
            shutdown_tx: None,
        }
    }

    /// Create with custom configuration
    pub fn with_config(app_handle: AppHandle<R>, config: WatcherConfig) -> Self {
        let emitter = SyncEventEmitter::new(app_handle.clone());
        Self {
            app_handle,
            emitter,
            state: Arc::new(RwLock::new(WatcherState::new())),
            config,
            shutdown_tx: None,
        }
    }

    /// Start watching the Claude projects directory
    pub async fn watch_projects_directory(&self) -> AppResult<()> {
        let projects_dir = claude_projects_dir()?;

        if !projects_dir.exists() {
            // Emit error but don't fail - directory might be created later
            self.emitter.emit_watch_error(WatchErrorEvent::new(
                format!("Projects directory does not exist: {:?}", projects_dir),
                Some(projects_dir.clone()),
                WatchErrorKind::PathNotFound,
            ));
            return Ok(());
        }

        self.add_watch(WatchTarget::ProjectsDirectory, &projects_dir, false).await
    }

    /// Start watching a specific project directory
    pub async fn watch_project(&self, project_path: PathBuf) -> AppResult<()> {
        if !project_path.exists() {
            return Err(AppError::not_found(format!(
                "Project directory not found: {:?}",
                project_path
            )));
        }

        self.add_watch(
            WatchTarget::ProjectDirectory(project_path.clone()),
            &project_path,
            true,
        ).await
    }

    /// Start watching a specific file (prd.json or progress.txt)
    pub async fn watch_file(&self, file_path: PathBuf) -> AppResult<()> {
        if !file_path.exists() {
            // Watch the parent directory instead, to catch creation
            if let Some(parent) = file_path.parent() {
                return self.add_watch(
                    WatchTarget::SpecificFile(file_path.clone()),
                    parent,
                    false,
                ).await;
            }
            return Err(AppError::not_found(format!(
                "File not found: {:?}",
                file_path
            )));
        }

        // Watch the parent directory to catch modifications
        if let Some(parent) = file_path.parent() {
            self.add_watch(
                WatchTarget::SpecificFile(file_path.clone()),
                parent,
                false,
            ).await
        } else {
            Err(AppError::validation("Cannot watch file without parent directory"))
        }
    }

    /// Stop watching a specific target
    pub async fn unwatch(&self, target: &WatchTarget) -> AppResult<()> {
        let mut state = self.state.write().await;

        if let Some(_watcher) = state.watchers.remove(target) {
            // Find and remove the watched path
            let path_to_remove = state
                .watched_paths
                .iter()
                .find(|(_, t)| *t == target)
                .map(|(p, _)| p.clone());

            if let Some(path) = path_to_remove {
                state.watched_paths.remove(&path);
                self.emitter.emit_watch_status(WatchStatusEvent::new(
                    WatchStatus::WatchRemoved,
                    Some(path),
                    None,
                ));
            }
        }

        Ok(())
    }

    /// Stop all watchers
    pub async fn stop_all(&self) -> AppResult<()> {
        let mut state = self.state.write().await;
        state.watchers.clear();
        state.watched_paths.clear();

        self.emitter.emit_watch_status(WatchStatusEvent::new(
            WatchStatus::Stopped,
            None,
            Some("All watchers stopped".to_string()),
        ));

        Ok(())
    }

    /// Get list of currently watched paths
    pub async fn get_watched_paths(&self) -> Vec<PathBuf> {
        let state = self.state.read().await;
        state.watched_paths.keys().cloned().collect()
    }

    /// Check if a path is being watched
    pub async fn is_watching(&self, path: &Path) -> bool {
        let state = self.state.read().await;
        state.watched_paths.contains_key(path)
    }

    /// Add a watch for a specific target
    async fn add_watch(
        &self,
        target: WatchTarget,
        path: &Path,
        recursive: bool,
    ) -> AppResult<()> {
        let mut state = self.state.write().await;

        // Check if already watching
        if state.watchers.contains_key(&target) {
            return Ok(());
        }

        let emitter = self.emitter.clone();
        let target_clone = target.clone();
        let path_buf = path.to_path_buf();

        // Create debounced watcher
        let debounce_duration = Duration::from_millis(self.config.debounce_ms);

        let mut debouncer = new_debouncer(
            debounce_duration,
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        for event in events {
                            Self::handle_event(&emitter, &target_clone, event);
                        }
                    }
                    Err(error) => {
                        emitter.emit_watch_error(WatchErrorEvent::from_notify_error(&error));
                    }
                }
            },
        ).map_err(|e| AppError::internal(format!("Failed to create watcher: {}", e)))?;

        // Start watching
        let mode = if recursive && self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        debouncer
            .watcher()
            .watch(path, mode)
            .map_err(|e| {
                let error_event = WatchErrorEvent::from_notify_error(&e);
                self.emitter.emit_watch_error(error_event);
                AppError::internal(format!("Failed to watch path {:?}: {}", path, e))
            })?;

        // Store the watcher
        state.watchers.insert(target.clone(), debouncer);
        state.watched_paths.insert(path_buf.clone(), target);

        // Emit status event
        self.emitter.emit_watch_status(WatchStatusEvent::new(
            WatchStatus::WatchAdded,
            Some(path_buf),
            None,
        ));

        Ok(())
    }

    /// Handle a debounced file system event
    fn handle_event(
        emitter: &SyncEventEmitter<R>,
        target: &WatchTarget,
        event: notify_debouncer_mini::DebouncedEvent,
    ) {
        let path = event.path;
        let change_type = match event.kind {
            DebouncedEventKind::Any => ChangeType::Modified,
            DebouncedEventKind::AnyContinuous => ChangeType::Modified,
            // Handle any future variants added to the non-exhaustive enum
            _ => ChangeType::Modified,
        };

        match target {
            WatchTarget::ProjectsDirectory => {
                // Only emit for direct children (project directories)
                if path.is_dir() {
                    emitter.emit_project_change(ProjectChangeEvent::new(
                        change_type,
                        path,
                    ));
                }
            }
            WatchTarget::ProjectDirectory(project_path) => {
                // Check for special files
                if let Some(file_name) = path.file_name() {
                    let name = file_name.to_string_lossy();
                    if name == "prd.json" {
                        emitter.emit_prd_change(PrdChangeEvent::new(
                            change_type.clone(),
                            path.clone(),
                        ));
                    } else if name == "progress.txt" {
                        emitter.emit_progress_change(ProgressChangeEvent::new(
                            change_type.clone(),
                            path.clone(),
                        ));
                    }
                }

                // Emit general file change
                emitter.emit_file_change(FileChangeEvent::new(
                    change_type,
                    path,
                    Some(project_path),
                ));
            }
            WatchTarget::SpecificFile(watched_file) => {
                // Only emit if it's the specific file we're watching
                if path == *watched_file {
                    if let Some(file_name) = path.file_name() {
                        let name = file_name.to_string_lossy();
                        if name == "prd.json" {
                            emitter.emit_prd_change(PrdChangeEvent::new(
                                change_type,
                                path,
                            ));
                        } else if name == "progress.txt" {
                            emitter.emit_progress_change(ProgressChangeEvent::new(
                                change_type,
                                path,
                            ));
                        } else {
                            emitter.emit_file_change(FileChangeEvent::new(
                                change_type,
                                path,
                                None,
                            ));
                        }
                    }
                }
            }
        }
    }
}

impl<R: Runtime> Clone for FileWatcherService<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            emitter: self.emitter.clone(),
            state: self.state.clone(),
            config: self.config.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

/// Builder for FileWatcherService
pub struct FileWatcherBuilder<R: Runtime> {
    app_handle: AppHandle<R>,
    config: WatcherConfig,
}

impl<R: Runtime> FileWatcherBuilder<R> {
    /// Create a new builder
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self {
            app_handle,
            config: WatcherConfig::default(),
        }
    }

    /// Set the debounce duration
    pub fn debounce_ms(mut self, ms: u64) -> Self {
        self.config.debounce_ms = ms;
        self
    }

    /// Set whether to watch recursively
    pub fn recursive(mut self, recursive: bool) -> Self {
        self.config.recursive = recursive;
        self
    }

    /// Build the watcher service
    pub fn build(self) -> FileWatcherService<R> {
        FileWatcherService::with_config(self.app_handle, self.config)
    }
}

/// Convenience function to create a watcher service and start watching common paths
pub async fn start_default_watches<R: Runtime>(
    app_handle: AppHandle<R>,
) -> AppResult<FileWatcherService<R>> {
    let service = FileWatcherService::new(app_handle);

    // Start watching the projects directory
    service.watch_projects_directory().await?;

    // Emit started status
    service.emitter.emit_watch_status(WatchStatusEvent::new(
        WatchStatus::Started,
        None,
        Some("Default watches started".to_string()),
    ));

    Ok(service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_ms, DEFAULT_DEBOUNCE_MS);
        assert!(config.recursive);
    }

    #[test]
    fn test_watch_target_equality() {
        let target1 = WatchTarget::ProjectsDirectory;
        let target2 = WatchTarget::ProjectsDirectory;
        assert_eq!(target1, target2);

        let path = PathBuf::from("/test/path");
        let target3 = WatchTarget::ProjectDirectory(path.clone());
        let target4 = WatchTarget::ProjectDirectory(path);
        assert_eq!(target3, target4);
    }

    #[test]
    fn test_watch_target_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(WatchTarget::ProjectsDirectory);
        set.insert(WatchTarget::ProjectDirectory(PathBuf::from("/test")));
        set.insert(WatchTarget::SpecificFile(PathBuf::from("/test/file.txt")));

        assert_eq!(set.len(), 3);
    }
}
