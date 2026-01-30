//! Markdown Commands
//!
//! Tauri commands for CLAUDE.md file operations.

use crate::models::markdown::{ClaudeMdContent, ClaudeMdFile, FileMetadata, SaveResult};
use crate::models::response::CommandResponse;
use crate::services::markdown::MarkdownService;

/// Scan a directory for all CLAUDE.md files
#[tauri::command]
pub fn scan_claude_md(root_path: String) -> Result<CommandResponse<Vec<ClaudeMdFile>>, String> {
    let service = MarkdownService::new();

    match service.scan_claude_md_files(&root_path) {
        Ok(files) => Ok(CommandResponse::ok(files)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Read the content of a CLAUDE.md file
#[tauri::command]
pub fn read_claude_md(path: String) -> Result<CommandResponse<ClaudeMdContent>, String> {
    let service = MarkdownService::new();

    match service.read_claude_md(&path) {
        Ok(content) => Ok(CommandResponse::ok(content)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Save content to a CLAUDE.md file
#[tauri::command]
pub fn save_claude_md(path: String, content: String) -> Result<CommandResponse<SaveResult>, String> {
    let service = MarkdownService::new();

    match service.save_claude_md(&path, &content) {
        Ok(()) => Ok(CommandResponse::ok(SaveResult::ok(&path))),
        Err(e) => Ok(CommandResponse::ok(SaveResult::err(&path, e.to_string()))),
    }
}

/// Create a new CLAUDE.md file from a template
#[tauri::command]
pub fn create_claude_md(
    path: String,
    template_content: String,
) -> Result<CommandResponse<SaveResult>, String> {
    let service = MarkdownService::new();

    match service.create_claude_md(&path, &template_content) {
        Ok(()) => Ok(CommandResponse::ok(SaveResult::ok(&path))),
        Err(e) => Ok(CommandResponse::ok(SaveResult::err(&path, e.to_string()))),
    }
}

/// Get file metadata for a CLAUDE.md file
#[tauri::command]
pub fn get_claude_md_metadata(path: String) -> Result<CommandResponse<FileMetadata>, String> {
    let service = MarkdownService::new();

    match service.get_file_metadata(&path) {
        Ok(metadata) => Ok(CommandResponse::ok(metadata)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_scan_claude_md_command() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_md_path, "# Test").unwrap();

        let result = scan_claude_md(temp_dir.path().to_str().unwrap().to_string()).unwrap();
        assert!(result.success);
        assert_eq!(result.data.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_read_claude_md_command() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        let content = "# Test Project\n\nContent here.";
        fs::write(&claude_md_path, content).unwrap();

        let result = read_claude_md(claude_md_path.to_str().unwrap().to_string()).unwrap();
        assert!(result.success);
        assert_eq!(result.data.as_ref().unwrap().content, content);
    }

    #[test]
    fn test_save_claude_md_command() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        let content = "# New Content".to_string();
        let result = save_claude_md(
            claude_md_path.to_str().unwrap().to_string(),
            content.clone(),
        )
        .unwrap();

        assert!(result.success);
        assert!(result.data.as_ref().unwrap().success);

        let saved = fs::read_to_string(&claude_md_path).unwrap();
        assert_eq!(saved, content);
    }

    #[test]
    fn test_create_claude_md_command() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("new_project").join("CLAUDE.md");

        let template = "# New Project\n\nTemplate content.".to_string();
        let result = create_claude_md(
            claude_md_path.to_str().unwrap().to_string(),
            template.clone(),
        )
        .unwrap();

        assert!(result.success);
        assert!(result.data.as_ref().unwrap().success);

        let saved = fs::read_to_string(&claude_md_path).unwrap();
        assert_eq!(saved, template);
    }

    #[test]
    fn test_create_claude_md_fails_if_exists() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_md_path, "existing").unwrap();

        let result = create_claude_md(
            claude_md_path.to_str().unwrap().to_string(),
            "new content".to_string(),
        )
        .unwrap();

        assert!(result.success); // CommandResponse is success
        assert!(!result.data.as_ref().unwrap().success); // But SaveResult indicates failure
        assert!(result.data.as_ref().unwrap().error.is_some());
    }

    #[test]
    fn test_get_claude_md_metadata_command() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_md_path, "# Test").unwrap();

        let result = get_claude_md_metadata(claude_md_path.to_str().unwrap().to_string()).unwrap();
        assert!(result.success);
        assert!(result.data.as_ref().unwrap().size > 0);
    }
}
