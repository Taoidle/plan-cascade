//! Markdown Service
//!
//! Scans and manages CLAUDE.md files across project directories.

use std::fs;
use std::path::{Path, PathBuf};

use crate::models::markdown::{ClaudeMdFile, ClaudeMdContent, FileMetadata};
use crate::utils::error::{AppError, AppResult};

/// Service for managing CLAUDE.md files
#[derive(Debug, Default)]
pub struct MarkdownService;

impl MarkdownService {
    /// Create a new markdown service
    pub fn new() -> Self {
        Self
    }

    /// Scan a directory tree recursively for all CLAUDE.md files
    pub fn scan_claude_md_files(&self, root_path: &str) -> AppResult<Vec<ClaudeMdFile>> {
        let root = PathBuf::from(root_path);

        if !root.exists() {
            return Err(AppError::not_found(format!(
                "Directory not found: {}",
                root_path
            )));
        }

        if !root.is_dir() {
            return Err(AppError::validation(format!(
                "Path is not a directory: {}",
                root_path
            )));
        }

        let mut files = Vec::new();
        self.scan_directory_recursive(&root, &root, &mut files)?;

        // Sort by modified time (most recent first)
        files.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

        Ok(files)
    }

    /// Recursively scan a directory for CLAUDE.md files
    fn scan_directory_recursive(
        &self,
        current_path: &Path,
        root_path: &Path,
        files: &mut Vec<ClaudeMdFile>,
    ) -> AppResult<()> {
        // Skip common directories that shouldn't be scanned
        if let Some(dir_name) = current_path.file_name().and_then(|n| n.to_str()) {
            if self.should_skip_directory(dir_name) {
                return Ok(());
            }
        }

        let entries = match fs::read_dir(current_path) {
            Ok(entries) => entries,
            Err(e) => {
                // Log but don't fail on permission errors
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    return Ok(());
                }
                return Err(AppError::Io(e));
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                self.scan_directory_recursive(&path, root_path, files)?;
            } else if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.eq_ignore_ascii_case("CLAUDE.md") {
                        if let Some(claude_md) = self.create_claude_md_file(&path, root_path) {
                            files.push(claude_md);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a directory should be skipped during scanning
    fn should_skip_directory(&self, dir_name: &str) -> bool {
        matches!(
            dir_name,
            "node_modules"
                | ".git"
                | ".hg"
                | ".svn"
                | "target"
                | "dist"
                | "build"
                | ".next"
                | ".nuxt"
                | "vendor"
                | "__pycache__"
                | ".venv"
                | "venv"
                | ".tox"
                | ".cache"
                | ".worktrees"
        )
    }

    /// Create a ClaudeMdFile from a path
    fn create_claude_md_file(&self, path: &Path, root_path: &Path) -> Option<ClaudeMdFile> {
        let metadata = fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?;
        let modified_at: chrono::DateTime<chrono::Utc> = modified.into();

        let relative_path = path
            .strip_prefix(root_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "CLAUDE.md".to_string());

        Some(ClaudeMdFile {
            path: path.to_string_lossy().to_string(),
            name,
            relative_path,
            modified_at: modified_at.to_rfc3339(),
            size: metadata.len(),
        })
    }

    /// Read the content of a CLAUDE.md file
    pub fn read_claude_md(&self, path: &str) -> AppResult<ClaudeMdContent> {
        let file_path = PathBuf::from(path);

        if !file_path.exists() {
            return Err(AppError::not_found(format!("File not found: {}", path)));
        }

        if !file_path.is_file() {
            return Err(AppError::validation(format!(
                "Path is not a file: {}",
                path
            )));
        }

        let content = fs::read_to_string(&file_path)?;

        let modified_at = fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            });

        Ok(ClaudeMdContent {
            path: path.to_string(),
            content,
            modified_at,
        })
    }

    /// Save content to a CLAUDE.md file atomically
    pub fn save_claude_md(&self, path: &str, content: &str) -> AppResult<()> {
        let file_path = PathBuf::from(path);

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Write atomically by writing to temp file then renaming
        let temp_path = file_path.with_extension("md.tmp");

        fs::write(&temp_path, content)?;

        // On Windows, we need to remove the destination first if it exists
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }

        fs::rename(&temp_path, &file_path)?;

        Ok(())
    }

    /// Create a new CLAUDE.md file from a template
    pub fn create_claude_md(&self, path: &str, template_content: &str) -> AppResult<()> {
        let file_path = PathBuf::from(path);

        if file_path.exists() {
            return Err(AppError::validation(format!(
                "File already exists: {}",
                path
            )));
        }

        self.save_claude_md(path, template_content)
    }

    /// Get file metadata for a CLAUDE.md file
    pub fn get_file_metadata(&self, path: &str) -> AppResult<FileMetadata> {
        let file_path = PathBuf::from(path);

        if !file_path.exists() {
            return Err(AppError::not_found(format!("File not found: {}", path)));
        }

        let metadata = fs::metadata(&file_path)?;

        let modified_at = metadata.modified().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.to_rfc3339()
        });

        let created_at = metadata.created().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.to_rfc3339()
        });

        Ok(FileMetadata {
            path: path.to_string(),
            size: metadata.len(),
            modified_at,
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_service_creation() {
        let service = MarkdownService::new();
        let _ = service;
    }

    #[test]
    fn test_should_skip_directory() {
        let service = MarkdownService::new();
        assert!(service.should_skip_directory("node_modules"));
        assert!(service.should_skip_directory(".git"));
        assert!(service.should_skip_directory("target"));
        assert!(!service.should_skip_directory("src"));
        assert!(!service.should_skip_directory("docs"));
    }

    #[test]
    fn test_scan_claude_md_files() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create a CLAUDE.md file
        let claude_md_path = root_path.join("CLAUDE.md");
        let mut file = File::create(&claude_md_path).unwrap();
        writeln!(file, "# Test Project").unwrap();

        // Create a nested CLAUDE.md file
        let sub_dir = root_path.join("subproject");
        fs::create_dir(&sub_dir).unwrap();
        let nested_claude_md = sub_dir.join("CLAUDE.md");
        let mut file = File::create(&nested_claude_md).unwrap();
        writeln!(file, "# Nested Project").unwrap();

        let service = MarkdownService::new();
        let files = service.scan_claude_md_files(root_path.to_str().unwrap()).unwrap();

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_read_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        let content = "# Test Project\n\nThis is a test.";
        fs::write(&claude_md_path, content).unwrap();

        let service = MarkdownService::new();
        let result = service.read_claude_md(claude_md_path.to_str().unwrap()).unwrap();

        assert_eq!(result.content, content);
        assert!(result.modified_at.is_some());
    }

    #[test]
    fn test_save_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        let service = MarkdownService::new();
        let content = "# New Project\n\nNew content here.";

        service.save_claude_md(claude_md_path.to_str().unwrap(), content).unwrap();

        let saved_content = fs::read_to_string(&claude_md_path).unwrap();
        assert_eq!(saved_content, content);
    }

    #[test]
    fn test_create_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("new_dir").join("CLAUDE.md");

        let service = MarkdownService::new();
        let template = "# New Project\n\nProject documentation.";

        service.create_claude_md(claude_md_path.to_str().unwrap(), template).unwrap();

        assert!(claude_md_path.exists());
        let content = fs::read_to_string(&claude_md_path).unwrap();
        assert_eq!(content, template);
    }

    #[test]
    fn test_create_claude_md_fails_if_exists() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        fs::write(&claude_md_path, "existing content").unwrap();

        let service = MarkdownService::new();
        let result = service.create_claude_md(claude_md_path.to_str().unwrap(), "new content");

        assert!(result.is_err());
    }

    #[test]
    fn test_get_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        let content = "# Test\n\nContent";
        fs::write(&claude_md_path, content).unwrap();

        let service = MarkdownService::new();
        let metadata = service.get_file_metadata(claude_md_path.to_str().unwrap()).unwrap();

        assert_eq!(metadata.size, content.len() as u64);
        assert!(metadata.modified_at.is_some());
    }
}
