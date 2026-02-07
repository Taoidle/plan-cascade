//! Design Document Commands
//!
//! Tauri commands for design document generation, import, and retrieval.
//! Provides three commands: generate_design_doc, import_design_doc, get_design_doc.

use std::path::Path;

use crate::models::response::CommandResponse;
use crate::services::design::{DesignDocGenerator, GenerateOptions, GenerateResult};
use crate::services::design::{DesignDocImporter, ImportFormat, ImportResult};
use crate::models::design_doc::DesignDoc;

/// Generate a design document from a PRD file.
///
/// Reads the PRD from the given path, analyzes its stories, tech stack,
/// and goals, then produces a comprehensive design document with
/// architecture overview, component definitions, API specifications,
/// story-to-component mappings, and architecture decision records.
///
/// # Arguments
/// * `prd_path` - Path to the prd.json file
/// * `options` - Optional generation configuration (level, mega plan reference)
///
/// # Returns
/// `CommandResponse<GenerateResult>` with the generated design document
/// and generation metadata.
#[tauri::command]
pub async fn generate_design_doc(
    prd_path: String,
    options: Option<GenerateOptions>,
) -> CommandResponse<GenerateResult> {
    if prd_path.trim().is_empty() {
        return CommandResponse::err("PRD path cannot be empty");
    }

    let path = Path::new(&prd_path);
    if !path.exists() {
        return CommandResponse::err(format!("PRD file not found: {}", prd_path));
    }

    match DesignDocGenerator::generate_from_file(path, options.as_ref(), true) {
        Ok(result) => CommandResponse::ok(result),
        Err(e) => CommandResponse::err(e.to_string()),
    }
}

/// Import an external design document from Markdown or JSON format.
///
/// Parses the file at the given path, converts it to the standard
/// design_doc.json format, and returns validation warnings for
/// any non-fatal issues encountered during import.
///
/// # Arguments
/// * `file_path` - Path to the file to import (.md or .json)
/// * `format` - Format hint ("markdown" or "json"). If not provided,
///   auto-detected from file extension.
///
/// # Returns
/// `CommandResponse<ImportResult>` with the imported design document,
/// warnings, and source format.
#[tauri::command]
pub async fn import_design_doc(
    file_path: String,
    format: Option<String>,
) -> CommandResponse<ImportResult> {
    if file_path.trim().is_empty() {
        return CommandResponse::err("File path cannot be empty");
    }

    let path = Path::new(&file_path);
    if !path.exists() {
        return CommandResponse::err(format!("File not found: {}", file_path));
    }

    let import_format = match format.as_deref() {
        Some("markdown") | Some("md") => Some(ImportFormat::Markdown),
        Some("json") => Some(ImportFormat::Json),
        Some(other) => {
            return CommandResponse::err(format!(
                "Unsupported format '{}'. Use 'markdown' or 'json'.",
                other
            ));
        }
        None => None, // Auto-detect from extension
    };

    match DesignDocImporter::import(path, import_format) {
        Ok(result) => CommandResponse::ok(result),
        Err(e) => CommandResponse::err(e.to_string()),
    }
}

/// Retrieve the current design document for a project.
///
/// Loads the design_doc.json from the project path (or current directory
/// if not specified). Returns the full design document structure.
///
/// # Arguments
/// * `project_path` - Optional path to the project root. If not provided,
///   uses the current working directory.
///
/// # Returns
/// `CommandResponse<DesignDoc>` with the loaded design document.
#[tauri::command]
pub async fn get_design_doc(
    project_path: Option<String>,
) -> CommandResponse<DesignDoc> {
    let base_path = match project_path {
        Some(p) if !p.trim().is_empty() => std::path::PathBuf::from(&p),
        _ => {
            match std::env::current_dir() {
                Ok(p) => p,
                Err(e) => {
                    return CommandResponse::err(format!(
                        "Cannot determine project path: {}",
                        e
                    ));
                }
            }
        }
    };

    let design_doc_path = base_path.join("design_doc.json");

    if !design_doc_path.exists() {
        return CommandResponse::err(format!(
            "No design_doc.json found at {}",
            design_doc_path.display()
        ));
    }

    match DesignDoc::from_file(&design_doc_path) {
        Ok(doc) => CommandResponse::ok(doc),
        Err(e) => CommandResponse::err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_design_doc_success() {
        let temp_dir = TempDir::new().unwrap();
        let prd_path = temp_dir.path().join("prd.json");

        let prd = r#"{
            "title": "Test Project",
            "description": "A test",
            "stories": [
                {
                    "id": "story-001",
                    "title": "User Login",
                    "description": "Implement login"
                }
            ],
            "tech_stack": ["Rust", "React"],
            "goals": ["Fast"],
            "non_goals": []
        }"#;
        fs::write(&prd_path, prd).unwrap();

        let result = generate_design_doc(
            prd_path.display().to_string(),
            None,
        )
        .await;

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data.design_doc.overview.title, "Test Project");
        assert!(data.saved_path.is_some());
        assert!(data.generation_info.stories_processed > 0);
    }

    #[tokio::test]
    async fn test_generate_design_doc_empty_path() {
        let result = generate_design_doc("".to_string(), None).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_generate_design_doc_not_found() {
        let result =
            generate_design_doc("/nonexistent/prd.json".to_string(), None).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_import_design_doc_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.md");

        let content = r#"# Imported Project

## Overview
A project imported from markdown.

## Architecture
### CoreService
The core service component.
"#;
        fs::write(&file_path, content).unwrap();

        let result = import_design_doc(
            file_path.display().to_string(),
            Some("markdown".to_string()),
        )
        .await;

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data.design_doc.overview.title, "Imported Project");
        assert_eq!(data.source_format, ImportFormat::Markdown);
    }

    #[tokio::test]
    async fn test_import_design_doc_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.json");

        let content = r#"{
            "overview": { "title": "JSON Imported", "summary": "Test" },
            "architecture": { "components": [{"name": "Comp", "description": "A comp"}] },
            "decisions": []
        }"#;
        fs::write(&file_path, content).unwrap();

        let result = import_design_doc(
            file_path.display().to_string(),
            None,
        )
        .await;

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data.design_doc.overview.title, "JSON Imported");
        assert_eq!(data.source_format, ImportFormat::Json);
    }

    #[tokio::test]
    async fn test_import_design_doc_empty_path() {
        let result = import_design_doc("".to_string(), None).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_import_design_doc_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.md");
        fs::write(&file_path, "# Test").unwrap();

        let result = import_design_doc(
            file_path.display().to_string(),
            Some("xml".to_string()),
        )
        .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unsupported format"));
    }

    #[tokio::test]
    async fn test_get_design_doc_success() {
        let temp_dir = TempDir::new().unwrap();
        let doc_path = temp_dir.path().join("design_doc.json");

        let content = r#"{
            "metadata": { "version": "1.0.0", "level": "project" },
            "overview": { "title": "Get Test", "summary": "Testing get" },
            "architecture": { "components": [] },
            "decisions": []
        }"#;
        fs::write(&doc_path, content).unwrap();

        let result = get_design_doc(
            Some(temp_dir.path().display().to_string()),
        )
        .await;

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data.overview.title, "Get Test");
    }

    #[tokio::test]
    async fn test_get_design_doc_not_found() {
        let temp_dir = TempDir::new().unwrap();

        let result = get_design_doc(
            Some(temp_dir.path().display().to_string()),
        )
        .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("No design_doc.json found"));
    }

    #[tokio::test]
    async fn test_generate_with_options() {
        let temp_dir = TempDir::new().unwrap();
        let prd_path = temp_dir.path().join("prd.json");

        let prd = r#"{
            "title": "Options Test",
            "description": "Testing with options",
            "stories": [{"id": "s1", "title": "Story One"}],
            "tech_stack": [],
            "goals": [],
            "non_goals": []
        }"#;
        fs::write(&prd_path, prd).unwrap();

        let options = GenerateOptions {
            level: Some(crate::models::design_doc::DesignDocLevel::Feature),
            mega_plan_reference: Some("mega-001".to_string()),
            additional_context: None,
        };

        let result = generate_design_doc(
            prd_path.display().to_string(),
            Some(options),
        )
        .await;

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(
            data.design_doc.level(),
            crate::models::design_doc::DesignDocLevel::Feature
        );
    }
}
