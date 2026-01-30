//! Markdown Models
//!
//! Data structures for CLAUDE.md file management.

use serde::{Deserialize, Serialize};

/// A discovered CLAUDE.md file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMdFile {
    /// Full absolute path to the file
    pub path: String,
    /// Display name (parent directory name or project name)
    pub name: String,
    /// Relative path from the scanned root
    pub relative_path: String,
    /// Last modification timestamp (ISO 8601)
    pub modified_at: String,
    /// File size in bytes
    pub size: u64,
}

/// Content of a CLAUDE.md file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMdContent {
    /// Full path to the file
    pub path: String,
    /// File content as a string
    pub content: String,
    /// Last modification timestamp (ISO 8601)
    pub modified_at: Option<String>,
}

/// Metadata for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Full path to the file
    pub path: String,
    /// File size in bytes
    pub size: u64,
    /// Last modification timestamp (ISO 8601)
    pub modified_at: Option<String>,
    /// Creation timestamp (ISO 8601)
    pub created_at: Option<String>,
}

/// Result of a save operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveResult {
    /// Whether the save was successful
    pub success: bool,
    /// Path that was saved
    pub path: String,
    /// Error message if save failed
    pub error: Option<String>,
}

impl SaveResult {
    /// Create a successful save result
    pub fn ok(path: impl Into<String>) -> Self {
        Self {
            success: true,
            path: path.into(),
            error: None,
        }
    }

    /// Create a failed save result
    pub fn err(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            path: path.into(),
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_md_file_serialization() {
        let file = ClaudeMdFile {
            path: "/path/to/CLAUDE.md".to_string(),
            name: "my-project".to_string(),
            relative_path: "my-project/CLAUDE.md".to_string(),
            modified_at: "2024-01-01T00:00:00Z".to_string(),
            size: 1024,
        };

        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("\"path\":\"/path/to/CLAUDE.md\""));

        let parsed: ClaudeMdFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, file.path);
    }

    #[test]
    fn test_claude_md_content_serialization() {
        let content = ClaudeMdContent {
            path: "/path/to/CLAUDE.md".to_string(),
            content: "# My Project\n\nDocumentation.".to_string(),
            modified_at: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("# My Project"));

        let parsed: ClaudeMdContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, content.content);
    }

    #[test]
    fn test_save_result_ok() {
        let result = SaveResult::ok("/path/to/file");
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_save_result_err() {
        let result = SaveResult::err("/path/to/file", "Permission denied");
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), "Permission denied");
    }
}
