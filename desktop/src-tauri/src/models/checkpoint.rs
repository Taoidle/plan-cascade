//! Checkpoint Models
//!
//! Data structures for timeline checkpoints and branching.

use serde::{Deserialize, Serialize};

/// A file snapshot entry within a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileSnapshot {
    /// Relative path from project root
    pub path: String,
    /// SHA-256 hash of file contents
    pub hash: String,
    /// File size in bytes
    pub size: u64,
    /// Whether this is a binary file
    pub is_binary: bool,
}

/// A checkpoint representing a snapshot of session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint identifier (UUID)
    pub id: String,
    /// Parent session identifier
    pub session_id: String,
    /// Creation timestamp (ISO 8601)
    pub timestamp: String,
    /// User-provided label for the checkpoint
    pub label: String,
    /// Optional parent checkpoint ID (for branching)
    pub parent_id: Option<String>,
    /// Optional branch ID this checkpoint belongs to
    pub branch_id: Option<String>,
    /// List of tracked files with hashes
    pub files_snapshot: Vec<FileSnapshot>,
    /// Optional description or notes
    pub description: Option<String>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(
        id: impl Into<String>,
        session_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            session_id: session_id.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            label: label.into(),
            parent_id: None,
            branch_id: None,
            files_snapshot: Vec::new(),
            description: None,
        }
    }

    /// Set the parent checkpoint ID
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Set the branch ID
    pub fn with_branch(mut self, branch_id: impl Into<String>) -> Self {
        self.branch_id = Some(branch_id.into());
        self
    }

    /// Set the files snapshot
    pub fn with_files(mut self, files: Vec<FileSnapshot>) -> Self {
        self.files_snapshot = files;
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A branch in the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointBranch {
    /// Unique branch identifier (UUID)
    pub id: String,
    /// User-provided branch name
    pub name: String,
    /// Checkpoint ID from which this branch was created
    pub parent_checkpoint_id: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Optional description
    pub description: Option<String>,
    /// Whether this is the main/default branch
    pub is_main: bool,
}

impl CheckpointBranch {
    /// Create a new branch
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        parent_checkpoint_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            parent_checkpoint_id: parent_checkpoint_id.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: None,
            is_main: false,
        }
    }

    /// Create the main branch
    pub fn main(id: impl Into<String>, parent_checkpoint_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: "main".to_string(),
            parent_checkpoint_id: parent_checkpoint_id.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: Some("Main timeline branch".to_string()),
            is_main: true,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Metadata file storing all checkpoints and branches for a session
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineMetadata {
    /// Session ID this timeline belongs to
    pub session_id: String,
    /// All checkpoints in this timeline
    pub checkpoints: Vec<Checkpoint>,
    /// All branches in this timeline
    pub branches: Vec<CheckpointBranch>,
    /// ID of the currently active checkpoint
    pub current_checkpoint_id: Option<String>,
    /// ID of the currently active branch
    pub current_branch_id: Option<String>,
}

impl TimelineMetadata {
    /// Create new timeline metadata for a session
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            checkpoints: Vec::new(),
            branches: Vec::new(),
            current_checkpoint_id: None,
            current_branch_id: None,
        }
    }
}

// ========== Diff Models (Story-003) ==========

/// Change type for a file between checkpoints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
}

/// Individual file diff between two checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// Relative path to the file
    pub path: String,
    /// Type of change (added, modified, deleted)
    pub change_type: FileChangeType,
    /// Whether this is a binary file
    pub is_binary: bool,
    /// Unified diff content (only for text files)
    pub diff_content: Option<String>,
    /// Old file hash (for modified/deleted)
    pub old_hash: Option<String>,
    /// New file hash (for added/modified)
    pub new_hash: Option<String>,
    /// Old file size in bytes
    pub old_size: Option<u64>,
    /// New file size in bytes
    pub new_size: Option<u64>,
    /// Number of lines added
    pub lines_added: u32,
    /// Number of lines removed
    pub lines_removed: u32,
}

impl FileDiff {
    /// Create a new file diff for an added file
    pub fn added(path: impl Into<String>, hash: impl Into<String>, size: u64, is_binary: bool) -> Self {
        Self {
            path: path.into(),
            change_type: FileChangeType::Added,
            is_binary,
            diff_content: None,
            old_hash: None,
            new_hash: Some(hash.into()),
            old_size: None,
            new_size: Some(size),
            lines_added: 0,
            lines_removed: 0,
        }
    }

    /// Create a new file diff for a deleted file
    pub fn deleted(path: impl Into<String>, hash: impl Into<String>, size: u64, is_binary: bool) -> Self {
        Self {
            path: path.into(),
            change_type: FileChangeType::Deleted,
            is_binary,
            diff_content: None,
            old_hash: Some(hash.into()),
            new_hash: None,
            old_size: Some(size),
            new_size: None,
            lines_added: 0,
            lines_removed: 0,
        }
    }

    /// Create a new file diff for a modified file
    pub fn modified(
        path: impl Into<String>,
        old_hash: impl Into<String>,
        new_hash: impl Into<String>,
        old_size: u64,
        new_size: u64,
        is_binary: bool,
    ) -> Self {
        Self {
            path: path.into(),
            change_type: FileChangeType::Modified,
            is_binary,
            diff_content: None,
            old_hash: Some(old_hash.into()),
            new_hash: Some(new_hash.into()),
            old_size: Some(old_size),
            new_size: Some(new_size),
            lines_added: 0,
            lines_removed: 0,
        }
    }

    /// Set the unified diff content
    pub fn with_diff_content(mut self, content: impl Into<String>, added: u32, removed: u32) -> Self {
        self.diff_content = Some(content.into());
        self.lines_added = added;
        self.lines_removed = removed;
        self
    }
}

/// Diff result between two checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointDiff {
    /// ID of the source (older) checkpoint
    pub from_checkpoint_id: String,
    /// ID of the target (newer) checkpoint
    pub to_checkpoint_id: String,
    /// List of added files
    pub added_files: Vec<FileDiff>,
    /// List of modified files
    pub modified_files: Vec<FileDiff>,
    /// List of deleted files
    pub deleted_files: Vec<FileDiff>,
    /// Total number of files changed
    pub total_files_changed: u32,
    /// Summary statistics
    pub summary: DiffSummary,
}

impl CheckpointDiff {
    /// Create a new checkpoint diff
    pub fn new(from_checkpoint_id: impl Into<String>, to_checkpoint_id: impl Into<String>) -> Self {
        Self {
            from_checkpoint_id: from_checkpoint_id.into(),
            to_checkpoint_id: to_checkpoint_id.into(),
            added_files: Vec::new(),
            modified_files: Vec::new(),
            deleted_files: Vec::new(),
            total_files_changed: 0,
            summary: DiffSummary::default(),
        }
    }

    /// Calculate summary after all diffs are added
    pub fn calculate_summary(&mut self) {
        self.total_files_changed = (self.added_files.len() + self.modified_files.len() + self.deleted_files.len()) as u32;

        let mut lines_added = 0u32;
        let mut lines_removed = 0u32;

        for diff in &self.added_files {
            lines_added += diff.lines_added;
        }
        for diff in &self.modified_files {
            lines_added += diff.lines_added;
            lines_removed += diff.lines_removed;
        }
        for diff in &self.deleted_files {
            lines_removed += diff.lines_removed;
        }

        self.summary = DiffSummary {
            files_added: self.added_files.len() as u32,
            files_modified: self.modified_files.len() as u32,
            files_deleted: self.deleted_files.len() as u32,
            lines_added,
            lines_removed,
        };
    }
}

/// Summary statistics for a diff
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiffSummary {
    /// Number of files added
    pub files_added: u32,
    /// Number of files modified
    pub files_modified: u32,
    /// Number of files deleted
    pub files_deleted: u32,
    /// Total lines added across all files
    pub lines_added: u32,
    /// Total lines removed across all files
    pub lines_removed: u32,
}

// ========== Restore Models (Story-004) ==========

/// Result of restoring to a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// Whether the restore was successful
    pub success: bool,
    /// ID of the checkpoint that was restored
    pub restored_checkpoint_id: String,
    /// ID of the backup checkpoint created before restore (if requested)
    pub backup_checkpoint_id: Option<String>,
    /// List of files that were restored
    pub restored_files: Vec<String>,
    /// List of files that were removed
    pub removed_files: Vec<String>,
    /// Error message if restore failed
    pub error: Option<String>,
}

impl RestoreResult {
    /// Create a successful restore result
    pub fn success(checkpoint_id: impl Into<String>) -> Self {
        Self {
            success: true,
            restored_checkpoint_id: checkpoint_id.into(),
            backup_checkpoint_id: None,
            restored_files: Vec::new(),
            removed_files: Vec::new(),
            error: None,
        }
    }

    /// Create a failed restore result
    pub fn failure(checkpoint_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            restored_checkpoint_id: checkpoint_id.into(),
            backup_checkpoint_id: None,
            restored_files: Vec::new(),
            removed_files: Vec::new(),
            error: Some(error.into()),
        }
    }

    /// Set the backup checkpoint ID
    pub fn with_backup(mut self, backup_id: impl Into<String>) -> Self {
        self.backup_checkpoint_id = Some(backup_id.into());
        self
    }

    /// Set the restored files
    pub fn with_restored_files(mut self, files: Vec<String>) -> Self {
        self.restored_files = files;
        self
    }

    /// Set the removed files
    pub fn with_removed_files(mut self, files: Vec<String>) -> Self {
        self.removed_files = files;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = Checkpoint::new("cp1", "sess1", "Initial state");
        assert_eq!(checkpoint.id, "cp1");
        assert_eq!(checkpoint.session_id, "sess1");
        assert_eq!(checkpoint.label, "Initial state");
        assert!(checkpoint.parent_id.is_none());
        assert!(checkpoint.files_snapshot.is_empty());
    }

    #[test]
    fn test_checkpoint_with_parent() {
        let checkpoint = Checkpoint::new("cp2", "sess1", "Second checkpoint")
            .with_parent("cp1")
            .with_description("After first change");

        assert_eq!(checkpoint.parent_id, Some("cp1".to_string()));
        assert_eq!(checkpoint.description, Some("After first change".to_string()));
    }

    #[test]
    fn test_checkpoint_with_files() {
        let files = vec![
            FileSnapshot {
                path: "src/main.rs".to_string(),
                hash: "abc123".to_string(),
                size: 1024,
                is_binary: false,
            },
        ];

        let checkpoint = Checkpoint::new("cp1", "sess1", "Test")
            .with_files(files.clone());

        assert_eq!(checkpoint.files_snapshot.len(), 1);
        assert_eq!(checkpoint.files_snapshot[0].path, "src/main.rs");
    }

    #[test]
    fn test_branch_creation() {
        let branch = CheckpointBranch::new("br1", "feature-x", "cp1");
        assert_eq!(branch.id, "br1");
        assert_eq!(branch.name, "feature-x");
        assert_eq!(branch.parent_checkpoint_id, "cp1");
        assert!(!branch.is_main);
    }

    #[test]
    fn test_main_branch() {
        let branch = CheckpointBranch::main("br1", "cp1");
        assert_eq!(branch.name, "main");
        assert!(branch.is_main);
    }

    #[test]
    fn test_file_snapshot() {
        let snapshot = FileSnapshot {
            path: "test.txt".to_string(),
            hash: "sha256hash".to_string(),
            size: 100,
            is_binary: false,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: FileSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, snapshot);
    }

    #[test]
    fn test_timeline_metadata() {
        let mut metadata = TimelineMetadata::new("sess1");
        assert_eq!(metadata.session_id, "sess1");
        assert!(metadata.checkpoints.is_empty());
        assert!(metadata.branches.is_empty());

        metadata.checkpoints.push(Checkpoint::new("cp1", "sess1", "First"));
        assert_eq!(metadata.checkpoints.len(), 1);
    }

    #[test]
    fn test_checkpoint_serialization() {
        let checkpoint = Checkpoint::new("cp1", "sess1", "Test")
            .with_parent("cp0")
            .with_branch("br1");

        let json = serde_json::to_string(&checkpoint).unwrap();
        assert!(json.contains("\"id\":\"cp1\""));
        assert!(json.contains("\"parent_id\":\"cp0\""));

        let parsed: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, checkpoint.id);
        assert_eq!(parsed.parent_id, checkpoint.parent_id);
    }

    #[test]
    fn test_branch_serialization() {
        let branch = CheckpointBranch::new("br1", "feature", "cp1")
            .with_description("Test branch");

        let json = serde_json::to_string(&branch).unwrap();
        let parsed: CheckpointBranch = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, branch.id);
        assert_eq!(parsed.description, branch.description);
    }

    // ========== Diff Model Tests (Story-003) ==========

    #[test]
    fn test_file_diff_added() {
        let diff = FileDiff::added("new_file.txt", "hash123", 1024, false);
        assert_eq!(diff.change_type, FileChangeType::Added);
        assert!(diff.old_hash.is_none());
        assert_eq!(diff.new_hash, Some("hash123".to_string()));
        assert!(!diff.is_binary);
    }

    #[test]
    fn test_file_diff_deleted() {
        let diff = FileDiff::deleted("old_file.txt", "hash456", 512, false);
        assert_eq!(diff.change_type, FileChangeType::Deleted);
        assert_eq!(diff.old_hash, Some("hash456".to_string()));
        assert!(diff.new_hash.is_none());
    }

    #[test]
    fn test_file_diff_modified() {
        let diff = FileDiff::modified("changed.txt", "old_hash", "new_hash", 100, 150, false);
        assert_eq!(diff.change_type, FileChangeType::Modified);
        assert_eq!(diff.old_hash, Some("old_hash".to_string()));
        assert_eq!(diff.new_hash, Some("new_hash".to_string()));
        assert_eq!(diff.old_size, Some(100));
        assert_eq!(diff.new_size, Some(150));
    }

    #[test]
    fn test_file_diff_with_content() {
        let diff = FileDiff::modified("file.txt", "h1", "h2", 10, 20, false)
            .with_diff_content("@@ -1 +1 @@\n-old\n+new", 1, 1);

        assert!(diff.diff_content.is_some());
        assert_eq!(diff.lines_added, 1);
        assert_eq!(diff.lines_removed, 1);
    }

    #[test]
    fn test_checkpoint_diff_summary() {
        let mut diff = CheckpointDiff::new("cp1", "cp2");
        diff.added_files.push(FileDiff::added("a.txt", "h1", 100, false)
            .with_diff_content("+line1\n+line2", 2, 0));
        diff.modified_files.push(FileDiff::modified("b.txt", "h1", "h2", 50, 60, false)
            .with_diff_content("-old\n+new", 1, 1));
        diff.deleted_files.push(FileDiff::deleted("c.txt", "h3", 30, false)
            .with_diff_content("-removed", 0, 1));

        diff.calculate_summary();

        assert_eq!(diff.total_files_changed, 3);
        assert_eq!(diff.summary.files_added, 1);
        assert_eq!(diff.summary.files_modified, 1);
        assert_eq!(diff.summary.files_deleted, 1);
        assert_eq!(diff.summary.lines_added, 3);
        assert_eq!(diff.summary.lines_removed, 2);
    }

    #[test]
    fn test_file_diff_serialization() {
        let diff = FileDiff::added("test.txt", "abc", 100, false);
        let json = serde_json::to_string(&diff).unwrap();
        assert!(json.contains("\"change_type\":\"added\""));

        let parsed: FileDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.change_type, FileChangeType::Added);
    }

    #[test]
    fn test_checkpoint_diff_serialization() {
        let mut diff = CheckpointDiff::new("cp1", "cp2");
        diff.added_files.push(FileDiff::added("new.txt", "hash", 50, false));
        diff.calculate_summary();

        let json = serde_json::to_string(&diff).unwrap();
        let parsed: CheckpointDiff = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.from_checkpoint_id, "cp1");
        assert_eq!(parsed.to_checkpoint_id, "cp2");
        assert_eq!(parsed.added_files.len(), 1);
    }
}
