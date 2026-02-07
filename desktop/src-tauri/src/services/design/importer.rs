//! Design Document Importer
//!
//! Imports external design documents from Markdown and JSON files,
//! converting them into the standard design_doc.json format.
//! Supports validation with non-fatal warnings.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::design_doc::{
    ApiStandards, Component, Decision, DecisionStatus, DesignDoc, DesignDocError,
    DesignDocLevel, DesignDocMetadata, FeatureMapping, Interfaces,
    Pattern,
};

/// Supported import formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportFormat {
    /// Markdown format (.md files)
    Markdown,
    /// JSON format (.json files)
    Json,
}

impl ImportFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => Some(ImportFormat::Markdown),
            Some("json") => Some(ImportFormat::Json),
            _ => None,
        }
    }
}

impl std::fmt::Display for ImportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportFormat::Markdown => write!(f, "markdown"),
            ImportFormat::Json => write!(f, "json"),
        }
    }
}

/// A validation warning produced during import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportWarning {
    /// Warning message
    pub message: String,
    /// Field or section that triggered the warning
    pub field: Option<String>,
    /// Severity level
    pub severity: WarningSeverity,
}

/// Severity level for import warnings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WarningSeverity {
    /// Information only, no action needed
    Info,
    /// Minor issue, may want to address
    Low,
    /// Should be addressed
    Medium,
    /// Important issue
    High,
}

/// Result of a design document import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// The imported design document
    pub design_doc: DesignDoc,
    /// Validation warnings (non-fatal issues)
    pub warnings: Vec<ImportWarning>,
    /// Source format that was imported
    pub source_format: ImportFormat,
    /// Whether the import was fully successful (no warnings)
    pub clean_import: bool,
}

/// Design document importer service
pub struct DesignDocImporter;

impl DesignDocImporter {
    /// Import a design document from a file.
    ///
    /// Auto-detects the format from the file extension, or uses the
    /// explicitly provided format.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to import
    /// * `format` - Optional format override (auto-detected from extension if None)
    ///
    /// # Returns
    /// An `ImportResult` with the design document and any validation warnings.
    pub fn import(
        file_path: &Path,
        format: Option<ImportFormat>,
    ) -> Result<ImportResult, DesignDocError> {
        if !file_path.exists() {
            return Err(DesignDocError::NotFound(
                format!("Import file not found: {}", file_path.display()),
            ));
        }

        let detected_format = format.or_else(|| ImportFormat::from_extension(file_path));
        let import_format = detected_format.ok_or_else(|| {
            DesignDocError::ValidationError(format!(
                "Cannot determine format for file: {}. Use .md or .json extension, or specify format explicitly.",
                file_path.display()
            ))
        })?;

        let content = std::fs::read_to_string(file_path)
            .map_err(|e| DesignDocError::IoError(e.to_string()))?;

        if content.trim().is_empty() {
            return Err(DesignDocError::ValidationError(
                "Import file is empty".to_string(),
            ));
        }

        match import_format {
            ImportFormat::Markdown => Self::import_markdown(&content),
            ImportFormat::Json => Self::import_json(&content),
        }
    }

    /// Import a design document from a Markdown string.
    ///
    /// Parses the Markdown structure:
    /// - H1 headers: Document title
    /// - H2 headers: Top-level sections (architecture, components, etc.)
    /// - H3 headers: Sub-sections
    /// - Code blocks: Specifications, schemas, or configuration
    /// - Bullet lists: Items within sections
    pub fn import_markdown(content: &str) -> Result<ImportResult, DesignDocError> {
        let mut warnings: Vec<ImportWarning> = Vec::new();
        let mut doc = DesignDoc::new();

        // Set import metadata
        doc.metadata = DesignDocMetadata {
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            version: "1.0.0".to_string(),
            source: Some("imported-markdown".to_string()),
            level: DesignDocLevel::Project,
            mega_plan_reference: None,
        };

        // Parse sections from markdown
        let sections = Self::parse_markdown_sections(content);

        if sections.is_empty() {
            warnings.push(ImportWarning {
                message: "No sections found in Markdown document".to_string(),
                field: None,
                severity: WarningSeverity::High,
            });
        }

        // Process each section
        for (header, body) in &sections {
            let header_lower = header.to_lowercase();

            if header.starts_with("# ") && !header.starts_with("## ") {
                // H1: Document title
                doc.overview.title = header.trim_start_matches("# ").trim().to_string();
            } else if header_lower.contains("overview") || header_lower.contains("summary") {
                doc.overview.summary = Self::extract_text_content(body);
                doc.overview.goals = Self::extract_list_items(body, "goal");
                doc.overview.non_goals = Self::extract_list_items(body, "non-goal");
            } else if header_lower.contains("architecture") {
                doc.architecture.system_overview = Self::extract_text_content(body);
                doc.architecture.components = Self::extract_components_from_markdown(body);
                doc.architecture.patterns = Self::extract_patterns_from_markdown(body);
            } else if header_lower.contains("component") {
                let components = Self::extract_components_from_markdown(body);
                doc.architecture.components.extend(components);
            } else if header_lower.contains("pattern") {
                let patterns = Self::extract_patterns_from_markdown(body);
                doc.architecture.patterns.extend(patterns);
            } else if header_lower.contains("decision") || header_lower.contains("adr") {
                let decisions = Self::extract_decisions_from_markdown(body);
                doc.decisions.extend(decisions);
            } else if header_lower.contains("api") || header_lower.contains("interface") {
                doc.interfaces = Self::extract_interfaces_from_markdown(body);
            } else if header_lower.contains("mapping") || header_lower.contains("feature") {
                let mappings = Self::extract_mappings_from_markdown(body);
                doc.feature_mappings.extend(mappings);
            } else {
                warnings.push(ImportWarning {
                    message: format!("Unrecognized section '{}', skipping", header),
                    field: Some(header.clone()),
                    severity: WarningSeverity::Low,
                });
            }
        }

        // Validate minimal content
        if doc.overview.title.is_empty() {
            warnings.push(ImportWarning {
                message: "No document title found (expected H1 header)".to_string(),
                field: Some("overview.title".to_string()),
                severity: WarningSeverity::Medium,
            });
            doc.overview.title = "Imported Design Document".to_string();
        }

        if doc.architecture.components.is_empty() {
            warnings.push(ImportWarning {
                message: "No components found in the document".to_string(),
                field: Some("architecture.components".to_string()),
                severity: WarningSeverity::Medium,
            });
        }

        let clean_import = warnings.is_empty();

        Ok(ImportResult {
            design_doc: doc,
            warnings,
            source_format: ImportFormat::Markdown,
            clean_import,
        })
    }

    /// Import a design document from a JSON string.
    ///
    /// Validates the JSON against the design_doc schema and maps fields.
    /// Supports both the standard design_doc.json format and simplified formats.
    pub fn import_json(content: &str) -> Result<ImportResult, DesignDocError> {
        let mut warnings: Vec<ImportWarning> = Vec::new();

        // Try to parse as standard DesignDoc first
        match serde_json::from_str::<DesignDoc>(content) {
            Ok(mut doc) => {
                // Set import metadata
                doc.metadata.source = Some("imported-json".to_string());
                if doc.metadata.created_at.is_none() {
                    doc.metadata.created_at = Some(chrono::Utc::now().to_rfc3339());
                }

                // Validate and warn about missing fields
                if doc.overview.title.is_empty() {
                    warnings.push(ImportWarning {
                        message: "Document has no title".to_string(),
                        field: Some("overview.title".to_string()),
                        severity: WarningSeverity::Medium,
                    });
                }

                if doc.architecture.components.is_empty() {
                    warnings.push(ImportWarning {
                        message: "No components defined".to_string(),
                        field: Some("architecture.components".to_string()),
                        severity: WarningSeverity::Low,
                    });
                }

                if doc.decisions.is_empty() {
                    warnings.push(ImportWarning {
                        message: "No architecture decisions (ADRs) defined".to_string(),
                        field: Some("decisions".to_string()),
                        severity: WarningSeverity::Low,
                    });
                }

                let clean_import = warnings.is_empty();

                Ok(ImportResult {
                    design_doc: doc,
                    warnings,
                    source_format: ImportFormat::Json,
                    clean_import,
                })
            }
            Err(parse_err) => {
                // Try to parse as a generic JSON object and map fields
                match serde_json::from_str::<serde_json::Value>(content) {
                    Ok(value) => Self::import_json_generic(&value, &mut warnings),
                    Err(_) => Err(DesignDocError::ParseError(format!(
                        "Failed to parse JSON: {}",
                        parse_err
                    ))),
                }
            }
        }
    }

    /// Import from a generic JSON value that doesn't match the standard schema
    fn import_json_generic(
        value: &serde_json::Value,
        warnings: &mut Vec<ImportWarning>,
    ) -> Result<ImportResult, DesignDocError> {
        let obj = value.as_object().ok_or_else(|| {
            DesignDocError::ValidationError("JSON root must be an object".to_string())
        })?;

        warnings.push(ImportWarning {
            message: "JSON does not match standard design_doc schema, performing best-effort import".to_string(),
            field: None,
            severity: WarningSeverity::Medium,
        });

        let mut doc = DesignDoc::new();
        doc.metadata.source = Some("imported-json-generic".to_string());
        doc.metadata.created_at = Some(chrono::Utc::now().to_rfc3339());

        // Try to extract title
        if let Some(title) = obj
            .get("title")
            .or_else(|| obj.get("name"))
            .or_else(|| obj.get("project"))
            .and_then(|v| v.as_str())
        {
            doc.overview.title = title.to_string();
        }

        // Try to extract description/summary
        if let Some(desc) = obj
            .get("description")
            .or_else(|| obj.get("summary"))
            .or_else(|| obj.get("overview"))
            .and_then(|v| v.as_str())
        {
            doc.overview.summary = desc.to_string();
        }

        // Try to extract components
        if let Some(components) = obj
            .get("components")
            .or_else(|| obj.get("modules"))
            .and_then(|v| v.as_array())
        {
            for comp_val in components {
                if let Some(name) = comp_val
                    .get("name")
                    .or_else(|| comp_val.get("id"))
                    .and_then(|v| v.as_str())
                {
                    let mut comp = Component::new(name);
                    if let Some(desc) = comp_val.get("description").and_then(|v| v.as_str()) {
                        comp.description = desc.to_string();
                    }
                    doc.architecture.components.push(comp);
                }
            }
        }

        // Try to extract decisions
        if let Some(decisions) = obj
            .get("decisions")
            .or_else(|| obj.get("adrs"))
            .and_then(|v| v.as_array())
        {
            for (idx, dec_val) in decisions.iter().enumerate() {
                let id = dec_val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| "")
                    .to_string();
                let id = if id.is_empty() {
                    format!("ADR-{:03}", idx + 1)
                } else {
                    id
                };

                let title = dec_val
                    .get("title")
                    .or_else(|| dec_val.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Decision")
                    .to_string();

                let mut decision = Decision::new(&id, &title);
                if let Some(context) = dec_val.get("context").and_then(|v| v.as_str()) {
                    decision.context = context.to_string();
                }
                if let Some(dec_text) = dec_val.get("decision").and_then(|v| v.as_str()) {
                    decision.decision = dec_text.to_string();
                }
                doc.decisions.push(decision);
            }
        }

        let clean_import = warnings.is_empty();

        Ok(ImportResult {
            design_doc: doc,
            warnings: warnings.clone(),
            source_format: ImportFormat::Json,
            clean_import,
        })
    }

    // ========================================================================
    // Markdown Parsing Helpers
    // ========================================================================

    /// Parse markdown into (header, body) sections
    fn parse_markdown_sections(content: &str) -> Vec<(String, String)> {
        let mut sections: Vec<(String, String)> = Vec::new();
        let mut current_header: Option<String> = None;
        let mut current_body = String::new();

        for line in content.lines() {
            if line.starts_with('#') {
                // Save previous section
                if let Some(header) = current_header.take() {
                    sections.push((header, current_body.trim().to_string()));
                    current_body = String::new();
                }
                current_header = Some(line.to_string());
            } else if current_header.is_some() {
                current_body.push_str(line);
                current_body.push('\n');
            }
        }

        // Save last section
        if let Some(header) = current_header {
            sections.push((header, current_body.trim().to_string()));
        }

        sections
    }

    /// Extract plain text content from a markdown body (skip lists and code blocks)
    fn extract_text_content(body: &str) -> String {
        let mut text = String::new();
        let mut in_code_block = false;

        for line in body.lines() {
            if line.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }
            if line.starts_with("- ") || line.starts_with("* ") || line.starts_with('#') {
                continue;
            }
            if !line.trim().is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(line.trim());
            }
        }

        text
    }

    /// Extract list items from a markdown body, optionally filtering by keyword
    fn extract_list_items(body: &str, _keyword: &str) -> Vec<String> {
        let mut items = Vec::new();
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let item = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim()
                    .to_string();
                if !item.is_empty() {
                    items.push(item);
                }
            }
        }
        items
    }

    /// Extract components from a markdown body
    fn extract_components_from_markdown(body: &str) -> Vec<Component> {
        let mut components = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_desc = String::new();

        for line in body.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("### ") {
                // Save previous component
                if let Some(name) = current_name.take() {
                    let mut comp = Component::new(&name);
                    comp.description = current_desc.trim().to_string();
                    components.push(comp);
                    current_desc = String::new();
                }
                current_name = Some(trimmed.trim_start_matches("### ").trim().to_string());
            } else if trimmed.starts_with("- **") || trimmed.starts_with("* **") {
                // Bold list item is likely a component name
                let item = trimmed
                    .trim_start_matches("- **")
                    .trim_start_matches("* **")
                    .trim_end_matches("**")
                    .split("**")
                    .next()
                    .unwrap_or("")
                    .trim();
                if !item.is_empty() && current_name.is_none() {
                    let desc = trimmed
                        .split("**")
                        .nth(2)
                        .unwrap_or("")
                        .trim_start_matches(": ")
                        .trim_start_matches(" - ")
                        .trim();
                    let mut comp = Component::new(item);
                    comp.description = desc.to_string();
                    components.push(comp);
                }
            } else if current_name.is_some() && !trimmed.is_empty() {
                current_desc.push_str(trimmed);
                current_desc.push('\n');
            }
        }

        // Save last component
        if let Some(name) = current_name {
            let mut comp = Component::new(&name);
            comp.description = current_desc.trim().to_string();
            components.push(comp);
        }

        components
    }

    /// Extract patterns from a markdown body
    fn extract_patterns_from_markdown(body: &str) -> Vec<Pattern> {
        let mut patterns = Vec::new();

        for line in body.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("### ") {
                let name = trimmed.trim_start_matches("### ").trim().to_string();
                patterns.push(Pattern::new(name));
            } else if trimmed.starts_with("- **") || trimmed.starts_with("* **") {
                let item = trimmed
                    .trim_start_matches("- **")
                    .trim_start_matches("* **")
                    .split("**")
                    .next()
                    .unwrap_or("")
                    .trim();
                if !item.is_empty() {
                    let desc = trimmed
                        .split("**")
                        .nth(2)
                        .unwrap_or("")
                        .trim_start_matches(": ")
                        .trim_start_matches(" - ")
                        .trim();
                    let mut p = Pattern::new(item);
                    p.description = desc.to_string();
                    patterns.push(p);
                }
            }
        }

        patterns
    }

    /// Extract decisions from a markdown body
    fn extract_decisions_from_markdown(body: &str) -> Vec<Decision> {
        let mut decisions = Vec::new();
        let mut current_id: Option<String> = None;
        let mut current_title = String::new();
        let mut current_context = String::new();
        let mut adr_counter = 1;

        for line in body.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("### ") {
                // Save previous decision
                if current_id.is_some() || !current_title.is_empty() {
                    let id = current_id
                        .take()
                        .unwrap_or_else(|| format!("ADR-{:03}", adr_counter));
                    let mut decision = Decision::new(&id, &current_title);
                    decision.context = current_context.trim().to_string();
                    decision.status = DecisionStatus::Accepted;
                    decisions.push(decision);
                    adr_counter += 1;
                    current_title = String::new();
                    current_context = String::new();
                }

                let header = trimmed.trim_start_matches("### ").trim();
                // Check if header contains an ADR ID
                if header.starts_with("ADR-") || header.starts_with("adr-") {
                    let parts: Vec<&str> = header.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        current_id = Some(parts[0].trim().to_string());
                        current_title = parts[1].trim().to_string();
                    } else {
                        let parts: Vec<&str> = header.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            current_id = Some(parts[0].trim().to_string());
                            current_title = parts[1].trim().to_string();
                        } else {
                            current_title = header.to_string();
                        }
                    }
                } else {
                    current_title = header.to_string();
                }
            } else if trimmed.starts_with("- **") || trimmed.starts_with("* **") {
                let item = trimmed
                    .trim_start_matches("- **")
                    .trim_start_matches("* **")
                    .split("**")
                    .next()
                    .unwrap_or("")
                    .trim();
                if !item.is_empty() {
                    let desc = trimmed
                        .split("**")
                        .nth(2)
                        .unwrap_or("")
                        .trim_start_matches(": ")
                        .trim_start_matches(" - ")
                        .trim();
                    let id = format!("ADR-{:03}", adr_counter);
                    let mut decision = Decision::new(&id, item);
                    decision.context = desc.to_string();
                    decision.status = DecisionStatus::Accepted;
                    decisions.push(decision);
                    adr_counter += 1;
                }
            } else if !current_title.is_empty() && !trimmed.is_empty() {
                current_context.push_str(trimmed);
                current_context.push('\n');
            }
        }

        // Save last decision
        if !current_title.is_empty() {
            let id = current_id.unwrap_or_else(|| format!("ADR-{:03}", adr_counter));
            let mut decision = Decision::new(&id, &current_title);
            decision.context = current_context.trim().to_string();
            decision.status = DecisionStatus::Accepted;
            decisions.push(decision);
        }

        decisions
    }

    /// Extract interfaces from a markdown body
    fn extract_interfaces_from_markdown(body: &str) -> Interfaces {
        let text = Self::extract_text_content(body);

        let style = if text.to_lowercase().contains("graphql") {
            "GraphQL"
        } else if text.to_lowercase().contains("grpc") {
            "gRPC"
        } else {
            "REST"
        }
        .to_string();

        Interfaces {
            api_standards: ApiStandards {
                style,
                error_handling: "Structured error responses".to_string(),
                async_pattern: "async/await".to_string(),
            },
            shared_data_models: Vec::new(),
        }
    }

    /// Extract feature mappings from a markdown body
    fn extract_mappings_from_markdown(body: &str) -> HashMap<String, FeatureMapping> {
        let mut mappings = HashMap::new();
        let mut current_feature: Option<String> = None;
        let mut current_mapping = FeatureMapping::default();

        for line in body.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("### ") {
                // Save previous mapping
                if let Some(feature) = current_feature.take() {
                    mappings.insert(feature, current_mapping);
                    current_mapping = FeatureMapping::default();
                }
                current_feature = Some(trimmed.trim_start_matches("### ").trim().to_string());
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let item = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim();
                if current_feature.is_some() {
                    // Try to categorize the list item
                    let lower = item.to_lowercase();
                    if lower.starts_with("component:") || lower.starts_with("comp:") {
                        let value = item.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
                        current_mapping.components.push(value);
                    } else if lower.starts_with("pattern:") {
                        let value = item.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
                        current_mapping.patterns.push(value);
                    } else if lower.starts_with("decision:") || lower.starts_with("adr:") {
                        let value = item.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
                        current_mapping.decisions.push(value);
                    } else {
                        // Default: treat as component
                        current_mapping.components.push(item.to_string());
                    }
                }
            }
        }

        // Save last mapping
        if let Some(feature) = current_feature {
            mappings.insert(feature, current_mapping);
        }

        mappings
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_import_format_from_extension() {
        assert_eq!(
            ImportFormat::from_extension(Path::new("doc.md")),
            Some(ImportFormat::Markdown)
        );
        assert_eq!(
            ImportFormat::from_extension(Path::new("doc.json")),
            Some(ImportFormat::Json)
        );
        assert_eq!(
            ImportFormat::from_extension(Path::new("doc.txt")),
            None
        );
        assert_eq!(
            ImportFormat::from_extension(Path::new("doc.markdown")),
            Some(ImportFormat::Markdown)
        );
    }

    #[test]
    fn test_import_markdown_basic() {
        let markdown = r#"# My Project

## Overview
This is a test project for demonstrating import functionality.

- Fast performance
- Easy to use

## Architecture
The system uses a modular architecture.

### AuthService
Handles user authentication and authorization.

### DataStore
Manages persistent data storage.

## Decisions
### ADR-001: Use Rust
Rust was chosen for its memory safety and performance.

### ADR-002: Use React
React provides a component-based UI framework.
"#;

        let result = DesignDocImporter::import_markdown(markdown).unwrap();

        assert_eq!(result.design_doc.overview.title, "My Project");
        assert!(!result.design_doc.overview.summary.is_empty());
        assert_eq!(result.source_format, ImportFormat::Markdown);

        // Check components were extracted
        assert!(!result.design_doc.architecture.components.is_empty());
        let comp_names: Vec<&str> = result
            .design_doc
            .architecture
            .components
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(comp_names.contains(&"AuthService"));
        assert!(comp_names.contains(&"DataStore"));

        // Check decisions were extracted
        assert!(result.design_doc.decisions.len() >= 2);
        let decision_ids: Vec<&str> = result
            .design_doc
            .decisions
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert!(decision_ids.contains(&"ADR-001"));
        assert!(decision_ids.contains(&"ADR-002"));
    }

    #[test]
    fn test_import_markdown_with_bold_list_components() {
        let markdown = r#"# Components

## Components
- **AuthModule** - Handles authentication
- **DataModule** - Handles data persistence
- **UIModule** - Handles user interface
"#;

        let result = DesignDocImporter::import_markdown(markdown).unwrap();
        assert!(result.design_doc.architecture.components.len() >= 3);
    }

    #[test]
    fn test_import_markdown_empty() {
        let result = DesignDocImporter::import_markdown("");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.clean_import);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_import_json_standard_format() {
        let json = r#"{
            "metadata": {
                "version": "1.0.0",
                "level": "project"
            },
            "overview": {
                "title": "JSON Import Test",
                "summary": "Testing JSON import"
            },
            "architecture": {
                "components": [
                    {"name": "CompA", "description": "Component A"},
                    {"name": "CompB", "description": "Component B"}
                ],
                "patterns": [
                    {"name": "MVC", "description": "Model-View-Controller"}
                ]
            },
            "decisions": [
                {
                    "id": "ADR-001",
                    "title": "Use JSON",
                    "status": "accepted"
                }
            ]
        }"#;

        let result = DesignDocImporter::import_json(json).unwrap();

        assert_eq!(result.design_doc.overview.title, "JSON Import Test");
        assert_eq!(result.design_doc.architecture.components.len(), 2);
        assert_eq!(result.design_doc.architecture.patterns.len(), 1);
        assert_eq!(result.design_doc.decisions.len(), 1);
        assert_eq!(result.source_format, ImportFormat::Json);
        assert_eq!(
            result.design_doc.metadata.source.as_deref(),
            Some("imported-json")
        );
    }

    #[test]
    fn test_import_json_generic_format() {
        let json = r#"{
            "title": "Generic Project",
            "description": "A project with non-standard format",
            "components": [
                {"name": "ServiceA", "description": "First service"},
                {"name": "ServiceB", "description": "Second service"}
            ],
            "decisions": [
                {"id": "ADR-001", "title": "Use microservices"}
            ]
        }"#;

        let result = DesignDocImporter::import_json(json).unwrap();

        assert_eq!(result.design_doc.overview.title, "Generic Project");
        assert_eq!(result.design_doc.architecture.components.len(), 2);
        assert_eq!(result.design_doc.decisions.len(), 1);
        // Should have warnings about non-standard format
        assert!(!result.clean_import);
    }

    #[test]
    fn test_import_json_invalid() {
        let result = DesignDocImporter::import_json("not valid json at all");
        assert!(result.is_err());
        assert!(matches!(result, Err(DesignDocError::ParseError(_))));
    }

    #[test]
    fn test_import_json_empty_object() {
        let result = DesignDocImporter::import_json("{}").unwrap();
        // Should succeed but with warnings about missing fields
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_import_from_file_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.md");

        let content = r#"# File Import Test

## Overview
Testing file-based import.

## Architecture
### MainComponent
The main application component.
"#;
        fs::write(&file_path, content).unwrap();

        let result = DesignDocImporter::import(&file_path, None).unwrap();
        assert_eq!(result.design_doc.overview.title, "File Import Test");
        assert_eq!(result.source_format, ImportFormat::Markdown);
    }

    #[test]
    fn test_import_from_file_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.json");

        let content = r#"{
            "overview": { "title": "JSON File Test", "summary": "Testing" },
            "architecture": { "components": [{"name": "Test", "description": "A test"}] },
            "decisions": []
        }"#;
        fs::write(&file_path, content).unwrap();

        let result = DesignDocImporter::import(&file_path, None).unwrap();
        assert_eq!(result.design_doc.overview.title, "JSON File Test");
        assert_eq!(result.source_format, ImportFormat::Json);
    }

    #[test]
    fn test_import_from_file_not_found() {
        let result =
            DesignDocImporter::import(Path::new("/nonexistent/file.md"), None);
        assert!(matches!(result, Err(DesignDocError::NotFound(_))));
    }

    #[test]
    fn test_import_from_file_unknown_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.txt");
        fs::write(&file_path, "some content").unwrap();

        let result = DesignDocImporter::import(&file_path, None);
        assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
    }

    #[test]
    fn test_import_from_file_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.md");
        fs::write(&file_path, "").unwrap();

        let result = DesignDocImporter::import(&file_path, None);
        assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
    }

    #[test]
    fn test_import_with_format_override() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("design.txt");

        let content = r#"# Override Test
## Overview
Testing format override.
"#;
        fs::write(&file_path, content).unwrap();

        // Should fail without format override
        assert!(DesignDocImporter::import(&file_path, None).is_err());

        // Should succeed with format override
        let result =
            DesignDocImporter::import(&file_path, Some(ImportFormat::Markdown)).unwrap();
        assert_eq!(result.design_doc.overview.title, "Override Test");
    }

    #[test]
    fn test_warning_severity() {
        let markdown = "No headers here, just plain text.";
        let result = DesignDocImporter::import_markdown(markdown).unwrap();

        // Should have warnings
        assert!(!result.warnings.is_empty());
        assert!(!result.clean_import);

        // At least one high severity warning about no sections
        let high_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.severity == WarningSeverity::High)
            .collect();
        assert!(!high_warnings.is_empty());
    }

    #[test]
    fn test_import_markdown_with_api_section() {
        let markdown = r#"# API Project

## API Interfaces
The system exposes a GraphQL API for client communication.
"#;

        let result = DesignDocImporter::import_markdown(markdown).unwrap();
        assert_eq!(result.design_doc.interfaces.api_standards.style, "GraphQL");
    }

    #[test]
    fn test_import_markdown_with_feature_mappings() {
        let markdown = r#"# Mapped Project

## Feature Mappings
### feature-auth
- Component: AuthService
- Component: UserStore
- Pattern: JWT
- Decision: ADR-001

### feature-data
- Component: DataService
"#;

        let result = DesignDocImporter::import_markdown(markdown).unwrap();

        assert!(result.design_doc.feature_mappings.contains_key("feature-auth"));
        let auth_mapping = &result.design_doc.feature_mappings["feature-auth"];
        assert_eq!(auth_mapping.components.len(), 2);
        assert_eq!(auth_mapping.patterns.len(), 1);
        assert_eq!(auth_mapping.decisions.len(), 1);

        assert!(result.design_doc.feature_mappings.contains_key("feature-data"));
    }

    #[test]
    fn test_parse_markdown_sections() {
        let content = r#"# Title
Body 1

## Section A
Content A

## Section B
Content B
"#;

        let sections = DesignDocImporter::parse_markdown_sections(content);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].0, "# Title");
        assert_eq!(sections[1].0, "## Section A");
        assert_eq!(sections[2].0, "## Section B");
    }
}
