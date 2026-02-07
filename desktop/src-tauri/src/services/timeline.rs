//! Timeline Service
//!
//! Manages checkpoint creation, storage, and timeline operations.
//! Stores checkpoints as JSON files in the project's checkpoints directory.

use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;

use sha2::{Sha256, Digest};
use uuid::Uuid;

use similar::{ChangeTag, TextDiff};

use crate::models::checkpoint::{
    Checkpoint, CheckpointBranch, CheckpointDiff, FileDiff, FileSnapshot, RestoreResult, TimelineMetadata,
};
use crate::utils::error::{AppError, AppResult};

/// Checkpoints directory name within project
const CHECKPOINTS_DIR: &str = "checkpoints";
/// Metadata file name
const METADATA_FILE: &str = "timeline.json";

/// Service for managing timeline checkpoints
#[derive(Debug, Default)]
pub struct TimelineService;

impl TimelineService {
    /// Create a new timeline service
    pub fn new() -> Self {
        Self
    }

    /// Get the checkpoints directory for a project
    fn checkpoints_dir(&self, project_path: &str) -> PathBuf {
        PathBuf::from(project_path).join(CHECKPOINTS_DIR)
    }

    /// Get the metadata file path for a session
    fn metadata_path(&self, project_path: &str, session_id: &str) -> PathBuf {
        self.checkpoints_dir(project_path)
            .join(session_id)
            .join(METADATA_FILE)
    }

    /// Ensure the checkpoints directory exists
    fn ensure_checkpoints_dir(&self, project_path: &str, session_id: &str) -> AppResult<PathBuf> {
        let dir = self.checkpoints_dir(project_path).join(session_id);
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(AppError::Io)?;
        }
        Ok(dir)
    }

    /// Load timeline metadata for a session
    fn load_metadata(&self, project_path: &str, session_id: &str) -> AppResult<TimelineMetadata> {
        let path = self.metadata_path(project_path, session_id);

        if !path.exists() {
            return Ok(TimelineMetadata::new(session_id));
        }

        let file = File::open(&path).map_err(AppError::Io)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(AppError::Serialization)
    }

    /// Save timeline metadata for a session
    fn save_metadata(&self, project_path: &str, metadata: &TimelineMetadata) -> AppResult<()> {
        self.ensure_checkpoints_dir(project_path, &metadata.session_id)?;
        let path = self.metadata_path(project_path, &metadata.session_id);

        let json = serde_json::to_string_pretty(metadata).map_err(AppError::Serialization)?;
        let mut file = File::create(&path).map_err(AppError::Io)?;
        file.write_all(json.as_bytes()).map_err(AppError::Io)?;

        Ok(())
    }

    /// Calculate SHA-256 hash of file contents
    fn calculate_file_hash(&self, path: &PathBuf) -> AppResult<String> {
        let content = fs::read(path).map_err(AppError::Io)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Check if a file is binary
    fn is_binary_file(&self, path: &PathBuf) -> bool {
        // Check by extension first
        let binary_extensions = [
            "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp",
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
            "zip", "tar", "gz", "rar", "7z",
            "exe", "dll", "so", "dylib",
            "mp3", "mp4", "avi", "mov", "wav",
            "woff", "woff2", "ttf", "otf", "eot",
        ];

        if let Some(ext) = path.extension() {
            if binary_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                return true;
            }
        }

        // Check content for null bytes (simple binary detection)
        if let Ok(content) = fs::read(path) {
            let sample_size = content.len().min(8192);
            return content[..sample_size].contains(&0);
        }

        false
    }

    /// Create a snapshot of tracked files in the project
    fn create_files_snapshot(&self, project_path: &str, tracked_paths: &[String]) -> AppResult<Vec<FileSnapshot>> {
        let project_root = PathBuf::from(project_path);
        let mut snapshots = Vec::new();

        for relative_path in tracked_paths {
            let full_path = project_root.join(relative_path);

            if !full_path.exists() {
                continue;
            }

            let metadata = fs::metadata(&full_path).map_err(AppError::Io)?;

            if metadata.is_file() {
                let hash = self.calculate_file_hash(&full_path)?;
                let is_binary = self.is_binary_file(&full_path);

                snapshots.push(FileSnapshot {
                    path: relative_path.clone(),
                    hash,
                    size: metadata.len(),
                    is_binary,
                });
            }
        }

        Ok(snapshots)
    }

    /// Create a new checkpoint
    pub fn create_checkpoint(
        &self,
        project_path: &str,
        session_id: &str,
        label: &str,
        tracked_files: &[String],
    ) -> AppResult<Checkpoint> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Generate new checkpoint ID
        let checkpoint_id = Uuid::new_v4().to_string();

        // Create files snapshot
        let files_snapshot = self.create_files_snapshot(project_path, tracked_files)?;

        // Determine parent checkpoint
        let parent_id = metadata.current_checkpoint_id.clone();

        // Create checkpoint
        let mut checkpoint = Checkpoint::new(&checkpoint_id, session_id, label)
            .with_files(files_snapshot);

        if let Some(parent) = parent_id {
            checkpoint = checkpoint.with_parent(parent);
        }

        if let Some(branch_id) = &metadata.current_branch_id {
            checkpoint = checkpoint.with_branch(branch_id.clone());
        }

        // Update metadata
        metadata.checkpoints.push(checkpoint.clone());
        metadata.current_checkpoint_id = Some(checkpoint_id);

        // Initialize main branch if this is the first checkpoint
        if metadata.branches.is_empty() {
            let main_branch = CheckpointBranch::main(
                Uuid::new_v4().to_string(),
                &checkpoint.id,
            );
            metadata.current_branch_id = Some(main_branch.id.clone());
            metadata.branches.push(main_branch);
        }

        self.save_metadata(project_path, &metadata)?;

        Ok(checkpoint)
    }

    /// List all checkpoints for a session
    pub fn list_checkpoints(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: Option<&str>,
    ) -> AppResult<Vec<Checkpoint>> {
        let metadata = self.load_metadata(project_path, session_id)?;

        let checkpoints = if let Some(bid) = branch_id {
            metadata
                .checkpoints
                .into_iter()
                .filter(|cp| cp.branch_id.as_deref() == Some(bid))
                .collect()
        } else {
            metadata.checkpoints
        };

        Ok(checkpoints)
    }

    /// Get a single checkpoint by ID
    pub fn get_checkpoint(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
    ) -> AppResult<Checkpoint> {
        let metadata = self.load_metadata(project_path, session_id)?;

        metadata
            .checkpoints
            .into_iter()
            .find(|cp| cp.id == checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)))
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
    ) -> AppResult<()> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Check if checkpoint exists
        let index = metadata
            .checkpoints
            .iter()
            .position(|cp| cp.id == checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)))?;

        // Don't allow deleting if other checkpoints depend on this one
        let has_children = metadata
            .checkpoints
            .iter()
            .any(|cp| cp.parent_id.as_deref() == Some(checkpoint_id));

        if has_children {
            return Err(AppError::validation(
                "Cannot delete checkpoint that has child checkpoints",
            ));
        }

        // Remove checkpoint
        metadata.checkpoints.remove(index);

        // Update current checkpoint if needed
        if metadata.current_checkpoint_id.as_deref() == Some(checkpoint_id) {
            metadata.current_checkpoint_id = None;
        }

        self.save_metadata(project_path, &metadata)?;

        Ok(())
    }

    /// Get the timeline metadata for a session
    pub fn get_timeline(&self, project_path: &str, session_id: &str) -> AppResult<TimelineMetadata> {
        self.load_metadata(project_path, session_id)
    }

    /// Update the current checkpoint
    pub fn set_current_checkpoint(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
    ) -> AppResult<()> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Verify checkpoint exists
        if !metadata.checkpoints.iter().any(|cp| cp.id == checkpoint_id) {
            return Err(AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)));
        }

        metadata.current_checkpoint_id = Some(checkpoint_id.to_string());
        self.save_metadata(project_path, &metadata)?;

        Ok(())
    }

    // ========== Branch Management Methods (Story-002) ==========

    /// Fork a new branch from a checkpoint
    pub fn fork_branch(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
        branch_name: &str,
    ) -> AppResult<CheckpointBranch> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Verify checkpoint exists
        if !metadata.checkpoints.iter().any(|cp| cp.id == checkpoint_id) {
            return Err(AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)));
        }

        // Check branch name is unique
        if metadata.branches.iter().any(|b| b.name == branch_name) {
            return Err(AppError::validation(format!(
                "Branch with name '{}' already exists",
                branch_name
            )));
        }

        // Create new branch
        let branch = CheckpointBranch::new(
            Uuid::new_v4().to_string(),
            branch_name,
            checkpoint_id,
        );

        metadata.branches.push(branch.clone());
        metadata.current_branch_id = Some(branch.id.clone());
        metadata.current_checkpoint_id = Some(checkpoint_id.to_string());

        self.save_metadata(project_path, &metadata)?;

        Ok(branch)
    }

    /// List all branches for a session
    pub fn list_branches(
        &self,
        project_path: &str,
        session_id: &str,
    ) -> AppResult<Vec<CheckpointBranch>> {
        let metadata = self.load_metadata(project_path, session_id)?;
        Ok(metadata.branches)
    }

    /// Get a single branch by ID
    pub fn get_branch(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: &str,
    ) -> AppResult<CheckpointBranch> {
        let metadata = self.load_metadata(project_path, session_id)?;

        metadata
            .branches
            .into_iter()
            .find(|b| b.id == branch_id)
            .ok_or_else(|| AppError::not_found(format!("Branch not found: {}", branch_id)))
    }

    /// Get all checkpoints in a specific branch (including ancestors)
    pub fn get_branch_checkpoints(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: &str,
    ) -> AppResult<Vec<Checkpoint>> {
        let metadata = self.load_metadata(project_path, session_id)?;

        // Find the branch
        let branch = metadata
            .branches
            .iter()
            .find(|b| b.id == branch_id)
            .ok_or_else(|| AppError::not_found(format!("Branch not found: {}", branch_id)))?;

        // Get checkpoints directly on this branch
        let mut branch_checkpoints: Vec<Checkpoint> = metadata
            .checkpoints
            .iter()
            .filter(|cp| cp.branch_id.as_deref() == Some(branch_id))
            .cloned()
            .collect();

        // Get the ancestor chain from the branch point
        let mut ancestor_chain = self.get_checkpoint_ancestors(
            &metadata.checkpoints,
            &branch.parent_checkpoint_id,
        );

        // Combine: ancestors first, then branch checkpoints
        ancestor_chain.append(&mut branch_checkpoints);

        Ok(ancestor_chain)
    }

    /// Get ancestor checkpoints from a given checkpoint (including the checkpoint itself)
    fn get_checkpoint_ancestors(
        &self,
        all_checkpoints: &[Checkpoint],
        checkpoint_id: &str,
    ) -> Vec<Checkpoint> {
        let mut ancestors = Vec::new();
        let mut current_id = Some(checkpoint_id.to_string());

        while let Some(id) = current_id {
            if let Some(cp) = all_checkpoints.iter().find(|c| c.id == id) {
                ancestors.push(cp.clone());
                current_id = cp.parent_id.clone();
            } else {
                break;
            }
        }

        // Reverse to get chronological order (oldest first)
        ancestors.reverse();
        ancestors
    }

    /// Switch to a different branch
    pub fn switch_branch(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: &str,
    ) -> AppResult<CheckpointBranch> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Find the branch
        let branch = metadata
            .branches
            .iter()
            .find(|b| b.id == branch_id)
            .ok_or_else(|| AppError::not_found(format!("Branch not found: {}", branch_id)))?
            .clone();

        // Get the latest checkpoint on this branch
        let latest_checkpoint = metadata
            .checkpoints
            .iter()
            .filter(|cp| cp.branch_id.as_deref() == Some(branch_id))
            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
            .map(|cp| cp.id.clone())
            .unwrap_or_else(|| branch.parent_checkpoint_id.clone());

        metadata.current_branch_id = Some(branch_id.to_string());
        metadata.current_checkpoint_id = Some(latest_checkpoint);

        self.save_metadata(project_path, &metadata)?;

        Ok(branch)
    }

    /// Delete a branch (cannot delete main branch)
    pub fn delete_branch(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: &str,
    ) -> AppResult<()> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Find the branch
        let branch_index = metadata
            .branches
            .iter()
            .position(|b| b.id == branch_id)
            .ok_or_else(|| AppError::not_found(format!("Branch not found: {}", branch_id)))?;

        // Cannot delete main branch
        if metadata.branches[branch_index].is_main {
            return Err(AppError::validation("Cannot delete the main branch"));
        }

        // Remove the branch
        metadata.branches.remove(branch_index);

        // Remove all checkpoints that are exclusively on this branch
        metadata.checkpoints.retain(|cp| cp.branch_id.as_deref() != Some(branch_id));

        // If current branch was deleted, switch to main
        if metadata.current_branch_id.as_deref() == Some(branch_id) {
            let main_branch = metadata.branches.iter().find(|b| b.is_main);
            if let Some(main) = main_branch {
                metadata.current_branch_id = Some(main.id.clone());
                metadata.current_checkpoint_id = Some(main.parent_checkpoint_id.clone());
            }
        }

        self.save_metadata(project_path, &metadata)?;

        Ok(())
    }

    /// Rename a branch
    pub fn rename_branch(
        &self,
        project_path: &str,
        session_id: &str,
        branch_id: &str,
        new_name: &str,
    ) -> AppResult<CheckpointBranch> {
        let mut metadata = self.load_metadata(project_path, session_id)?;

        // Check new name is unique
        if metadata.branches.iter().any(|b| b.name == new_name && b.id != branch_id) {
            return Err(AppError::validation(format!(
                "Branch with name '{}' already exists",
                new_name
            )));
        }

        // Find and update the branch
        let branch = metadata
            .branches
            .iter_mut()
            .find(|b| b.id == branch_id)
            .ok_or_else(|| AppError::not_found(format!("Branch not found: {}", branch_id)))?;

        branch.name = new_name.to_string();
        let updated_branch = branch.clone();

        self.save_metadata(project_path, &metadata)?;

        Ok(updated_branch)
    }

    /// Merge branch placeholder (for future implementation)
    pub fn merge_branch(
        &self,
        _project_path: &str,
        _session_id: &str,
        _source_branch_id: &str,
        _target_branch_id: &str,
    ) -> AppResult<()> {
        // TODO: Implement merge logic
        // This would require:
        // 1. Finding common ancestor
        // 2. Applying changes from source to target
        // 3. Creating a merge checkpoint
        Err(AppError::internal("Branch merging is not yet implemented"))
    }

    // ========== Diff Calculation Methods (Story-003) ==========

    /// Calculate diff between two checkpoints
    pub fn calculate_diff(
        &self,
        project_path: &str,
        session_id: &str,
        from_checkpoint_id: &str,
        to_checkpoint_id: &str,
    ) -> AppResult<CheckpointDiff> {
        let metadata = self.load_metadata(project_path, session_id)?;

        // Get both checkpoints
        let from_checkpoint = metadata
            .checkpoints
            .iter()
            .find(|cp| cp.id == from_checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", from_checkpoint_id)))?;

        let to_checkpoint = metadata
            .checkpoints
            .iter()
            .find(|cp| cp.id == to_checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", to_checkpoint_id)))?;

        self.compute_checkpoint_diff(project_path, from_checkpoint, to_checkpoint)
    }

    /// Compute the diff between two checkpoints
    fn compute_checkpoint_diff(
        &self,
        project_path: &str,
        from: &Checkpoint,
        to: &Checkpoint,
    ) -> AppResult<CheckpointDiff> {
        let mut diff = CheckpointDiff::new(&from.id, &to.id);

        // Create maps for quick lookup
        let from_files: std::collections::HashMap<&str, &FileSnapshot> = from
            .files_snapshot
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();

        let to_files: std::collections::HashMap<&str, &FileSnapshot> = to
            .files_snapshot
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();

        // Find added and modified files
        for (path, to_file) in &to_files {
            if let Some(from_file) = from_files.get(path) {
                // File exists in both - check if modified
                if from_file.hash != to_file.hash {
                    let file_diff = self.create_file_diff(
                        project_path,
                        *path,
                        Some(from_file),
                        Some(to_file),
                    )?;
                    diff.modified_files.push(file_diff);
                }
            } else {
                // File only in to - added
                let file_diff = self.create_file_diff(
                    project_path,
                    *path,
                    None,
                    Some(to_file),
                )?;
                diff.added_files.push(file_diff);
            }
        }

        // Find deleted files
        for (path, from_file) in &from_files {
            if !to_files.contains_key(path) {
                let file_diff = self.create_file_diff(
                    project_path,
                    *path,
                    Some(from_file),
                    None,
                )?;
                diff.deleted_files.push(file_diff);
            }
        }

        diff.calculate_summary();
        Ok(diff)
    }

    /// Create a FileDiff for a single file change
    fn create_file_diff(
        &self,
        project_path: &str,
        path: &str,
        from_file: Option<&FileSnapshot>,
        to_file: Option<&FileSnapshot>,
    ) -> AppResult<FileDiff> {
        match (from_file, to_file) {
            // Added
            (None, Some(to)) => {
                let mut diff = FileDiff::added(&to.path, &to.hash, to.size, to.is_binary);

                if !to.is_binary {
                    let content = self.read_file_content(project_path, path).unwrap_or_default();
                    let lines: Vec<&str> = content.lines().collect();
                    let line_count = lines.len() as u32;

                    // For added files, generate a diff showing all lines as additions
                    let diff_content = self.generate_unified_diff("", &content, path);
                    diff = diff.with_diff_content(diff_content, line_count, 0);
                }

                Ok(diff)
            }

            // Deleted
            (Some(from), None) => {
                let mut diff = FileDiff::deleted(&from.path, &from.hash, from.size, from.is_binary);

                if !from.is_binary {
                    // For deleted files, we estimate lines based on size
                    let estimated_lines = (from.size / 40).max(1) as u32; // rough estimate
                    diff.lines_removed = estimated_lines;
                }

                Ok(diff)
            }

            // Modified
            (Some(from), Some(to)) => {
                let mut diff = FileDiff::modified(
                    &to.path,
                    &from.hash,
                    &to.hash,
                    from.size,
                    to.size,
                    to.is_binary,
                );

                if !from.is_binary && !to.is_binary {
                    // Try to get current file content for diff
                    let new_content = self.read_file_content(project_path, path).unwrap_or_default();

                    // For old content, we'd need to have stored it or reconstruct
                    // For now, we'll show a simplified diff
                    let diff_content = format!(
                        "@@ File modified: {} @@\n--- a/{}\n+++ b/{}\n(binary diff or content not available)",
                        path, path, path
                    );

                    // Count lines in new content
                    let new_lines = new_content.lines().count() as u32;
                    let old_lines = (from.size / 40).max(1) as u32; // estimate

                    // Rough estimate of changes
                    let added = new_lines.saturating_sub(old_lines);
                    let removed = old_lines.saturating_sub(new_lines);

                    diff = diff.with_diff_content(diff_content, added.max(1), removed.max(1));
                }

                Ok(diff)
            }

            // Shouldn't happen
            (None, None) => Err(AppError::internal("Invalid diff state")),
        }
    }

    /// Read file content as string
    fn read_file_content(&self, project_path: &str, relative_path: &str) -> AppResult<String> {
        let full_path = PathBuf::from(project_path).join(relative_path);
        fs::read_to_string(&full_path).map_err(AppError::Io)
    }

    /// Generate unified diff format between two strings
    fn generate_unified_diff(&self, old_content: &str, new_content: &str, path: &str) -> String {
        let diff = TextDiff::from_lines(old_content, new_content);

        let mut output = format!("--- a/{}\n+++ b/{}\n", path, path);

        for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
            if idx > 0 {
                output.push('\n');
            }

            // Add hunk header
            let (old_start, old_count, new_start, new_count) = group.iter().fold(
                (usize::MAX, 0usize, usize::MAX, 0usize),
                |(os, oc, ns, nc), op| {
                    let old_range = op.old_range();
                    let new_range = op.new_range();
                    (
                        os.min(old_range.start),
                        oc + old_range.len(),
                        ns.min(new_range.start),
                        nc + new_range.len(),
                    )
                },
            );

            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                old_start + 1,
                old_count,
                new_start + 1,
                new_count
            ));

            for op in group {
                for change in diff.iter_changes(op) {
                    let prefix = match change.tag() {
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
                        ChangeTag::Equal => " ",
                    };

                    output.push_str(prefix);
                    output.push_str(change.value());
                }
            }
        }

        output
    }

    /// Get diff between a checkpoint and current state
    pub fn get_diff_from_current(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
        tracked_files: &[String],
    ) -> AppResult<CheckpointDiff> {
        let metadata = self.load_metadata(project_path, session_id)?;

        let checkpoint = metadata
            .checkpoints
            .iter()
            .find(|cp| cp.id == checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)))?;

        // Create a virtual "current state" checkpoint
        let current_snapshot = self.create_files_snapshot(project_path, tracked_files)?;
        let current = Checkpoint::new("current", session_id, "Current state")
            .with_files(current_snapshot);

        self.compute_checkpoint_diff(project_path, checkpoint, &current)
    }

    // ========== Restore Methods (Story-004) ==========

    /// Restore session state to a checkpoint
    ///
    /// This restores the tracked files to their state at the given checkpoint.
    /// Optionally creates a backup checkpoint of the current state before restoring.
    pub fn restore_checkpoint(
        &self,
        project_path: &str,
        session_id: &str,
        checkpoint_id: &str,
        create_backup: bool,
        current_tracked_files: &[String],
    ) -> AppResult<RestoreResult> {
        let metadata = self.load_metadata(project_path, session_id)?;

        // Find the checkpoint to restore
        let checkpoint = metadata
            .checkpoints
            .iter()
            .find(|cp| cp.id == checkpoint_id)
            .ok_or_else(|| AppError::not_found(format!("Checkpoint not found: {}", checkpoint_id)))?
            .clone();

        let mut result = RestoreResult::success(checkpoint_id);

        // Optionally create backup of current state
        if create_backup {
            let backup = self.create_checkpoint(
                project_path,
                session_id,
                &format!("Backup before restore to: {}", checkpoint.label),
                current_tracked_files,
            )?;
            result = result.with_backup(&backup.id);
        }

        // Restore files from checkpoint
        let _project_root = PathBuf::from(project_path);
        let mut restored_files = Vec::new();
        let mut removed_files = Vec::new();

        // Note: In a real implementation, we would need to store file contents
        // along with the checkpoint. For now, we just update the metadata
        // to point to this checkpoint as the current state.

        // Get list of files in checkpoint
        for file_snapshot in &checkpoint.files_snapshot {
            restored_files.push(file_snapshot.path.clone());
        }

        // Find files that exist now but not in checkpoint (to be removed conceptually)
        let checkpoint_paths: std::collections::HashSet<&str> = checkpoint
            .files_snapshot
            .iter()
            .map(|f| f.path.as_str())
            .collect();

        for path in current_tracked_files {
            if !checkpoint_paths.contains(path.as_str()) {
                removed_files.push(path.clone());
            }
        }

        // Update current checkpoint pointer
        let mut updated_metadata = self.load_metadata(project_path, session_id)?;
        updated_metadata.current_checkpoint_id = Some(checkpoint_id.to_string());

        // Update branch if checkpoint is on a different branch
        if let Some(branch_id) = &checkpoint.branch_id {
            updated_metadata.current_branch_id = Some(branch_id.clone());
        }

        self.save_metadata(project_path, &updated_metadata)?;

        result = result
            .with_restored_files(restored_files)
            .with_removed_files(removed_files);

        Ok(result)
    }

    /// Get file content from a checkpoint
    /// Note: This is a placeholder - in a full implementation,
    /// file contents would be stored alongside the checkpoint
    pub fn get_checkpoint_file_content(
        &self,
        _project_path: &str,
        _session_id: &str,
        _checkpoint_id: &str,
        _file_path: &str,
    ) -> AppResult<Vec<u8>> {
        // In a full implementation, we would:
        // 1. Store file contents in a content-addressable storage (by hash)
        // 2. Retrieve the content for the given file at the given checkpoint
        Err(AppError::internal(
            "File content restoration not yet implemented. Checkpoint metadata only stores file hashes.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_temp_project() -> PathBuf {
        let temp_dir = env::temp_dir().join(format!("timeline_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        temp_dir
    }

    fn cleanup_temp_project(path: &PathBuf) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_service_creation() {
        let service = TimelineService::new();
        let _ = service;
    }

    #[test]
    fn test_create_checkpoint() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create a test file
        let test_file = temp_dir.join("test.txt");
        fs::write(&test_file, "Hello, World!").unwrap();

        let checkpoint = service
            .create_checkpoint(&project_path, "sess1", "Initial", &["test.txt".to_string()])
            .unwrap();

        assert_eq!(checkpoint.session_id, "sess1");
        assert_eq!(checkpoint.label, "Initial");
        assert!(checkpoint.parent_id.is_none());
        assert_eq!(checkpoint.files_snapshot.len(), 1);
        assert_eq!(checkpoint.files_snapshot[0].path, "test.txt");

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_list_checkpoints() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create checkpoints
        service
            .create_checkpoint(&project_path, "sess1", "First", &[])
            .unwrap();
        service
            .create_checkpoint(&project_path, "sess1", "Second", &[])
            .unwrap();

        let checkpoints = service
            .list_checkpoints(&project_path, "sess1", None)
            .unwrap();

        assert_eq!(checkpoints.len(), 2);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_get_checkpoint() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let created = service
            .create_checkpoint(&project_path, "sess1", "Test", &[])
            .unwrap();

        let retrieved = service
            .get_checkpoint(&project_path, "sess1", &created.id)
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.label, "Test");

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_delete_checkpoint() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "ToDelete", &[])
            .unwrap();

        // Create a child checkpoint
        let _child = service
            .create_checkpoint(&project_path, "sess1", "Child", &[])
            .unwrap();

        // Should fail to delete parent (has child)
        let result = service.delete_checkpoint(&project_path, "sess1", &cp.id);
        assert!(result.is_err());

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_checkpoint_parent_chain() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let first = service
            .create_checkpoint(&project_path, "sess1", "First", &[])
            .unwrap();

        let second = service
            .create_checkpoint(&project_path, "sess1", "Second", &[])
            .unwrap();

        assert!(first.parent_id.is_none());
        assert_eq!(second.parent_id, Some(first.id));

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_file_snapshot_with_hash() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create test files
        let file1 = temp_dir.join("file1.txt");
        let file2 = temp_dir.join("file2.txt");
        fs::write(&file1, "Content 1").unwrap();
        fs::write(&file2, "Content 2").unwrap();

        let checkpoint = service
            .create_checkpoint(
                &project_path,
                "sess1",
                "With files",
                &["file1.txt".to_string(), "file2.txt".to_string()],
            )
            .unwrap();

        assert_eq!(checkpoint.files_snapshot.len(), 2);

        // Verify hashes are different for different content
        let hash1 = &checkpoint.files_snapshot.iter().find(|f| f.path == "file1.txt").unwrap().hash;
        let hash2 = &checkpoint.files_snapshot.iter().find(|f| f.path == "file2.txt").unwrap().hash;
        assert_ne!(hash1, hash2);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_binary_file_detection() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();

        // Create a text file
        let text_file = temp_dir.join("text.txt");
        fs::write(&text_file, "Hello").unwrap();
        assert!(!service.is_binary_file(&text_file));

        // Create a file with binary extension
        let binary_file = temp_dir.join("image.png");
        fs::write(&binary_file, &[0x89, 0x50, 0x4E, 0x47]).unwrap();
        assert!(service.is_binary_file(&binary_file));

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_get_timeline() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Initially empty
        let timeline = service.get_timeline(&project_path, "sess1").unwrap();
        assert!(timeline.checkpoints.is_empty());
        assert!(timeline.branches.is_empty());

        // Create checkpoint
        service
            .create_checkpoint(&project_path, "sess1", "Test", &[])
            .unwrap();

        let timeline = service.get_timeline(&project_path, "sess1").unwrap();
        assert_eq!(timeline.checkpoints.len(), 1);
        assert_eq!(timeline.branches.len(), 1); // Main branch auto-created

        cleanup_temp_project(&temp_dir);
    }

    // ========== Branch Management Tests (Story-002) ==========

    #[test]
    fn test_fork_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create initial checkpoint
        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        // Fork a new branch
        let branch = service
            .fork_branch(&project_path, "sess1", &cp.id, "feature-x")
            .unwrap();

        assert_eq!(branch.name, "feature-x");
        assert_eq!(branch.parent_checkpoint_id, cp.id);
        assert!(!branch.is_main);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_fork_branch_duplicate_name() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        // Create first branch
        service
            .fork_branch(&project_path, "sess1", &cp.id, "feature-x")
            .unwrap();

        // Try to create duplicate
        let result = service.fork_branch(&project_path, "sess1", &cp.id, "feature-x");
        assert!(result.is_err());

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_list_branches() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        // Should have main branch
        let branches = service.list_branches(&project_path, "sess1").unwrap();
        assert_eq!(branches.len(), 1);
        assert!(branches[0].is_main);

        // Add another branch
        service
            .fork_branch(&project_path, "sess1", &cp.id, "feature-y")
            .unwrap();

        let branches = service.list_branches(&project_path, "sess1").unwrap();
        assert_eq!(branches.len(), 2);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_get_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        let created = service
            .fork_branch(&project_path, "sess1", &cp.id, "test-branch")
            .unwrap();

        let retrieved = service
            .get_branch(&project_path, "sess1", &created.id)
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.name, "test-branch");

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_get_branch_checkpoints() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create checkpoints on main branch
        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "First", &[])
            .unwrap();
        let _cp2 = service
            .create_checkpoint(&project_path, "sess1", "Second", &[])
            .unwrap();

        // Fork from first checkpoint
        let branch = service
            .fork_branch(&project_path, "sess1", &cp1.id, "feature")
            .unwrap();

        // Create checkpoint on new branch
        let _cp3 = service
            .create_checkpoint(&project_path, "sess1", "On feature", &[])
            .unwrap();

        // Get branch checkpoints
        let branch_cps = service
            .get_branch_checkpoints(&project_path, "sess1", &branch.id)
            .unwrap();

        // Should include cp1 (ancestor) and cp3 (on branch)
        assert_eq!(branch_cps.len(), 2);
        assert_eq!(branch_cps[0].label, "First");
        assert_eq!(branch_cps[1].label, "On feature");

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_switch_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        // Get main branch
        let branches = service.list_branches(&project_path, "sess1").unwrap();
        let main_branch = &branches[0];

        // Create new branch
        let feature_branch = service
            .fork_branch(&project_path, "sess1", &cp.id, "feature")
            .unwrap();

        // Timeline should be on feature branch now
        let timeline = service.get_timeline(&project_path, "sess1").unwrap();
        assert_eq!(timeline.current_branch_id, Some(feature_branch.id.clone()));

        // Switch back to main
        service
            .switch_branch(&project_path, "sess1", &main_branch.id)
            .unwrap();

        let timeline = service.get_timeline(&project_path, "sess1").unwrap();
        assert_eq!(timeline.current_branch_id, Some(main_branch.id.clone()));

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_delete_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        let branch = service
            .fork_branch(&project_path, "sess1", &cp.id, "to-delete")
            .unwrap();

        // Delete the branch
        service
            .delete_branch(&project_path, "sess1", &branch.id)
            .unwrap();

        let branches = service.list_branches(&project_path, "sess1").unwrap();
        assert_eq!(branches.len(), 1);
        assert!(branches[0].is_main);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_cannot_delete_main_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        let branches = service.list_branches(&project_path, "sess1").unwrap();
        let main_branch = &branches[0];

        let result = service.delete_branch(&project_path, "sess1", &main_branch.id);
        assert!(result.is_err());

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_rename_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        let cp = service
            .create_checkpoint(&project_path, "sess1", "Initial", &[])
            .unwrap();

        let branch = service
            .fork_branch(&project_path, "sess1", &cp.id, "old-name")
            .unwrap();

        let renamed = service
            .rename_branch(&project_path, "sess1", &branch.id, "new-name")
            .unwrap();

        assert_eq!(renamed.name, "new-name");
        assert_eq!(renamed.id, branch.id);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_checkpoints_on_branch() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create checkpoint on main
        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "Main cp1", &[])
            .unwrap();

        // Fork and create on branch
        let branch = service
            .fork_branch(&project_path, "sess1", &cp1.id, "feature")
            .unwrap();

        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "Feature cp1", &[])
            .unwrap();

        // Verify cp2 is on the feature branch
        assert_eq!(cp2.branch_id, Some(branch.id.clone()));

        // List checkpoints filtered by branch
        let branch_cps = service
            .list_checkpoints(&project_path, "sess1", Some(&branch.id))
            .unwrap();

        // Only cp2 should be directly on this branch
        assert_eq!(branch_cps.len(), 1);
        assert_eq!(branch_cps[0].label, "Feature cp1");

        cleanup_temp_project(&temp_dir);
    }

    // ========== Diff Calculation Tests (Story-003) ==========

    #[test]
    fn test_diff_added_files() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create first checkpoint with no files
        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "Empty", &[])
            .unwrap();

        // Add a file
        let test_file = temp_dir.join("new_file.txt");
        fs::write(&test_file, "Hello, World!").unwrap();

        // Create second checkpoint with file
        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "With file", &["new_file.txt".to_string()])
            .unwrap();

        // Calculate diff
        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.added_files.len(), 1);
        assert_eq!(diff.modified_files.len(), 0);
        assert_eq!(diff.deleted_files.len(), 0);
        assert_eq!(diff.added_files[0].path, "new_file.txt");
        assert_eq!(diff.summary.files_added, 1);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_deleted_files() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create file and checkpoint
        let test_file = temp_dir.join("to_delete.txt");
        fs::write(&test_file, "Content").unwrap();

        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "With file", &["to_delete.txt".to_string()])
            .unwrap();

        // Create checkpoint without file
        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "File gone", &[])
            .unwrap();

        // Calculate diff
        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.added_files.len(), 0);
        assert_eq!(diff.modified_files.len(), 0);
        assert_eq!(diff.deleted_files.len(), 1);
        assert_eq!(diff.deleted_files[0].path, "to_delete.txt");
        assert_eq!(diff.summary.files_deleted, 1);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_modified_files() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create file and checkpoint
        let test_file = temp_dir.join("modify.txt");
        fs::write(&test_file, "Original content").unwrap();

        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "Original", &["modify.txt".to_string()])
            .unwrap();

        // Modify file
        fs::write(&test_file, "Modified content with more text").unwrap();

        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "Modified", &["modify.txt".to_string()])
            .unwrap();

        // Calculate diff
        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.added_files.len(), 0);
        assert_eq!(diff.modified_files.len(), 1);
        assert_eq!(diff.deleted_files.len(), 0);
        assert_eq!(diff.modified_files[0].path, "modify.txt");
        assert_eq!(diff.summary.files_modified, 1);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_no_changes() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create file
        let test_file = temp_dir.join("same.txt");
        fs::write(&test_file, "Unchanged").unwrap();

        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "First", &["same.txt".to_string()])
            .unwrap();

        // Same file content
        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "Second", &["same.txt".to_string()])
            .unwrap();

        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.total_files_changed, 0);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_multiple_changes() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create initial files
        fs::write(temp_dir.join("keep.txt"), "Keep this").unwrap();
        fs::write(temp_dir.join("modify.txt"), "Will modify").unwrap();
        fs::write(temp_dir.join("delete.txt"), "Will delete").unwrap();

        let cp1 = service
            .create_checkpoint(
                &project_path,
                "sess1",
                "Initial",
                &["keep.txt".to_string(), "modify.txt".to_string(), "delete.txt".to_string()],
            )
            .unwrap();

        // Modify files
        fs::write(temp_dir.join("modify.txt"), "Modified!").unwrap();
        fs::write(temp_dir.join("new.txt"), "New file").unwrap();
        // delete.txt removed from tracked files

        let cp2 = service
            .create_checkpoint(
                &project_path,
                "sess1",
                "Changed",
                &["keep.txt".to_string(), "modify.txt".to_string(), "new.txt".to_string()],
            )
            .unwrap();

        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.added_files.len(), 1);
        assert_eq!(diff.modified_files.len(), 1);
        assert_eq!(diff.deleted_files.len(), 1);
        assert_eq!(diff.total_files_changed, 3);

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_from_current() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create file and checkpoint
        let test_file = temp_dir.join("evolving.txt");
        fs::write(&test_file, "Version 1").unwrap();

        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "V1", &["evolving.txt".to_string()])
            .unwrap();

        // Modify without checkpointing
        fs::write(&test_file, "Version 2 - not checkpointed yet").unwrap();

        // Get diff from checkpoint to current state
        let diff = service
            .get_diff_from_current(&project_path, "sess1", &cp1.id, &["evolving.txt".to_string()])
            .unwrap();

        assert_eq!(diff.modified_files.len(), 1);
        assert_eq!(diff.from_checkpoint_id, cp1.id);
        assert_eq!(diff.to_checkpoint_id, "current");

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_diff_binary_file() {
        let service = TimelineService::new();
        let temp_dir = create_temp_project();
        let project_path = temp_dir.to_string_lossy().to_string();

        // Create binary file (PNG header)
        let binary_file = temp_dir.join("image.png");
        fs::write(&binary_file, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();

        let cp1 = service
            .create_checkpoint(&project_path, "sess1", "Empty", &[])
            .unwrap();

        let cp2 = service
            .create_checkpoint(&project_path, "sess1", "With binary", &["image.png".to_string()])
            .unwrap();

        let diff = service
            .calculate_diff(&project_path, "sess1", &cp1.id, &cp2.id)
            .unwrap();

        assert_eq!(diff.added_files.len(), 1);
        assert!(diff.added_files[0].is_binary);
        assert!(diff.added_files[0].diff_content.is_none());

        cleanup_temp_project(&temp_dir);
    }

    #[test]
    fn test_generate_unified_diff() {
        let service = TimelineService::new();

        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nmodified line 2\nline 3\nnew line 4\n";

        let diff = service.generate_unified_diff(old, new, "test.txt");

        // Should contain unified diff markers
        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(diff.contains("@@"));
    }
}
