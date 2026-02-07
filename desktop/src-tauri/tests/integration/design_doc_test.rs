//! Design Document Integration Tests
//!
//! Tests for the design document generator and importer services:
//! - Generation from PRD input (AC3)
//! - Import from Markdown sources (AC3)
//! - Import from JSON sources (AC3)
//! - Schema conformance of output (AC3)
//!
//! No LLM calls are made. Generator uses rule-based extraction.

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use plan_cascade_desktop::models::design_doc::{
    Component, Decision, DecisionStatus, DesignDoc, DesignDocError, DesignDocLevel,
    DesignDocMetadata, FeatureMapping, Pattern,
};
use plan_cascade_desktop::services::design::{
    DesignDocGenerator, DesignDocImporter, GenerateOptions, ImportFormat, ImportResult,
    PrdDocument, PrdStory, WarningSeverity,
};

// ============================================================================
// Helpers
// ============================================================================

fn sample_prd() -> PrdDocument {
    PrdDocument {
        title: "Integration Test Project".to_string(),
        description: "A project for testing design doc generation end-to-end".to_string(),
        stories: vec![
            PrdStory {
                id: "story-001".to_string(),
                title: "User Authentication".to_string(),
                description: "Implement user login and registration".to_string(),
                acceptance_criteria: vec![
                    "Users can register".to_string(),
                    "Users can log in".to_string(),
                ],
                dependencies: vec![],
                complexity: Some("medium".to_string()),
                story_points: Some(5),
            },
            PrdStory {
                id: "story-002".to_string(),
                title: "Dashboard Display".to_string(),
                description: "Show project metrics on dashboard".to_string(),
                acceptance_criteria: vec!["Dashboard renders metrics".to_string()],
                dependencies: vec!["story-001".to_string()],
                complexity: Some("low".to_string()),
                story_points: Some(3),
            },
            PrdStory {
                id: "story-003".to_string(),
                title: "Data Export".to_string(),
                description: "Export data to CSV and JSON formats".to_string(),
                acceptance_criteria: vec!["CSV export works".to_string(), "JSON export works".to_string()],
                dependencies: vec!["story-001".to_string(), "story-002".to_string()],
                complexity: Some("low".to_string()),
                story_points: Some(2),
            },
        ],
        tech_stack: vec![
            "Rust".to_string(),
            "React".to_string(),
            "Tauri".to_string(),
            "SQLite".to_string(),
        ],
        goals: vec!["Fast performance".to_string(), "Reliable execution".to_string()],
        non_goals: vec!["Mobile support".to_string()],
    }
}

// ============================================================================
// AC3: Generation from PRD Input
// ============================================================================

#[test]
fn test_generate_produces_complete_design_doc() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    let doc = &result.design_doc;

    // Overview populated from PRD
    assert_eq!(doc.overview.title, "Integration Test Project");
    assert_eq!(doc.overview.summary, prd.description);
    assert_eq!(doc.overview.goals.len(), 2);
    assert_eq!(doc.overview.non_goals.len(), 1);

    // Metadata
    assert!(doc.metadata.created_at.is_some());
    assert_eq!(doc.metadata.version, "1.0.0");
    assert_eq!(doc.metadata.source.as_deref(), Some("generated"));
    assert_eq!(doc.metadata.level, DesignDocLevel::Project);
}

#[test]
fn test_generate_extracts_components_from_stories() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // Should have at least 3 story-derived components plus infrastructure
    assert!(
        result.design_doc.architecture.components.len() >= 3,
        "Expected >= 3 components, got {}",
        result.design_doc.architecture.components.len()
    );
    assert_eq!(result.generation_info.stories_processed, 3);

    // Each story should map to a component with features
    let components_with_features: Vec<_> = result
        .design_doc
        .architecture
        .components
        .iter()
        .filter(|c| !c.features.is_empty())
        .collect();
    assert!(!components_with_features.is_empty());

    // SQLite in tech stack should produce DataLayer component
    let has_data_layer = result
        .design_doc
        .architecture
        .components
        .iter()
        .any(|c| c.name == "DataLayer");
    assert!(has_data_layer, "SQLite tech should produce DataLayer component");
}

#[test]
fn test_generate_extracts_patterns_from_tech_stack() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    let pattern_names: Vec<&str> = result
        .design_doc
        .architecture
        .patterns
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // React -> Component-Based UI
    assert!(
        pattern_names.contains(&"Component-Based UI"),
        "React should trigger Component-Based UI pattern. Patterns: {:?}",
        pattern_names
    );

    // Tauri -> IPC Command Pattern
    assert!(
        pattern_names.contains(&"IPC Command Pattern"),
        "Tauri should trigger IPC Command Pattern. Patterns: {:?}",
        pattern_names
    );

    assert!(result.generation_info.patterns_identified > 0);
}

#[test]
fn test_generate_creates_adrs_for_tech_stack() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // One ADR per tech stack item
    assert!(
        result.design_doc.decisions.len() >= prd.tech_stack.len(),
        "Expected >= {} ADRs, got {}",
        prd.tech_stack.len(),
        result.design_doc.decisions.len()
    );

    for decision in &result.design_doc.decisions {
        assert!(decision.id.starts_with("ADR-"), "Decision ID should start with ADR-");
        assert_eq!(decision.status, DecisionStatus::Accepted);
        assert!(!decision.title.is_empty());
    }

    assert!(result.generation_info.decisions_created > 0);
}

#[test]
fn test_generate_creates_feature_mappings_for_all_stories() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // One mapping per story
    assert_eq!(
        result.design_doc.feature_mappings.len(),
        prd.stories.len(),
        "Should have one feature mapping per story"
    );

    for story in &prd.stories {
        let mapping = result.design_doc.feature_mappings.get(&story.id);
        assert!(
            mapping.is_some(),
            "Missing mapping for story {}",
            story.id
        );
        let mapping = mapping.unwrap();
        assert!(!mapping.components.is_empty(), "Mapping for {} should have components", story.id);
        assert!(!mapping.description.is_empty(), "Mapping for {} should have description", story.id);
    }

    assert_eq!(result.generation_info.feature_mappings_created, prd.stories.len());
}

#[test]
fn test_generate_builds_system_overview() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    let overview = &result.design_doc.architecture.system_overview;
    assert!(!overview.is_empty());
    assert!(overview.contains(&prd.title));
    assert!(overview.contains("Tech Stack"));
}

#[test]
fn test_generate_builds_data_flow() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    let data_flow = &result.design_doc.architecture.data_flow;
    assert!(!data_flow.is_empty());
}

#[test]
fn test_generate_builds_interfaces_from_tech() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // Tauri should set IPC Commands style
    assert_eq!(result.design_doc.interfaces.api_standards.style, "Tauri IPC Commands");
    // Rust should set async/await with tokio
    assert!(result.design_doc.interfaces.api_standards.async_pattern.contains("tokio"));
}

#[test]
fn test_generate_with_feature_level() {
    let prd = sample_prd();
    let options = GenerateOptions {
        level: Some(DesignDocLevel::Feature),
        mega_plan_reference: Some("mega-plan-001".to_string()),
        additional_context: None,
    };

    let result = DesignDocGenerator::generate(&prd, Some(&options)).unwrap();
    assert_eq!(result.design_doc.level(), DesignDocLevel::Feature);
    assert_eq!(
        result.design_doc.metadata.mega_plan_reference.as_deref(),
        Some("mega-plan-001")
    );
}

#[test]
fn test_generate_empty_prd_fails() {
    let empty = PrdDocument {
        title: String::new(),
        description: String::new(),
        stories: vec![],
        tech_stack: vec![],
        goals: vec![],
        non_goals: vec![],
    };

    let result = DesignDocGenerator::generate(&empty, None);
    assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
}

#[test]
fn test_generate_minimal_prd_with_title_only() {
    let minimal = PrdDocument {
        title: "Minimal".to_string(),
        description: String::new(),
        stories: vec![],
        tech_stack: vec![],
        goals: vec![],
        non_goals: vec![],
    };

    let result = DesignDocGenerator::generate(&minimal, None).unwrap();
    assert_eq!(result.design_doc.overview.title, "Minimal");
    // Should still have a default architecture ADR
    assert!(!result.design_doc.decisions.is_empty());
    // Should have default pattern
    assert!(!result.design_doc.architecture.patterns.is_empty());
}

// ============================================================================
// AC3: Generation from File
// ============================================================================

#[test]
fn test_generate_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let prd_path = temp_dir.path().join("prd.json");

    let prd = sample_prd();
    let json = serde_json::to_string_pretty(&prd).unwrap();
    fs::write(&prd_path, json).unwrap();

    let result = DesignDocGenerator::generate_from_file(&prd_path, None, false).unwrap();
    assert_eq!(result.design_doc.overview.title, "Integration Test Project");
    assert!(result.saved_path.is_none());
}

#[test]
fn test_generate_from_file_and_save() {
    let temp_dir = TempDir::new().unwrap();
    let prd_path = temp_dir.path().join("prd.json");

    let prd = sample_prd();
    let json = serde_json::to_string_pretty(&prd).unwrap();
    fs::write(&prd_path, json).unwrap();

    let result = DesignDocGenerator::generate_from_file(&prd_path, None, true).unwrap();
    assert!(result.saved_path.is_some());

    // Verify saved file
    let design_doc_path = temp_dir.path().join("design_doc.json");
    assert!(design_doc_path.exists());

    let loaded = DesignDoc::from_file(&design_doc_path).unwrap();
    assert_eq!(loaded.overview.title, "Integration Test Project");
    assert!(loaded.architecture.components.len() >= 3);
}

#[test]
fn test_generate_from_nonexistent_file() {
    let result = DesignDocGenerator::generate_from_file(
        Path::new("/nonexistent/prd.json"),
        None,
        false,
    );
    assert!(matches!(result, Err(DesignDocError::NotFound(_))));
}

// ============================================================================
// AC3: Import from Markdown Sources
// ============================================================================

#[test]
fn test_import_markdown_full_document() {
    let markdown = r#"# Architecture Design

## Overview
This system provides a comprehensive authentication and authorization framework.

- Secure login
- Token management
- Role-based access

## Architecture

### AuthService
Handles user authentication, token generation, and session management.

### UserStore
Manages user data persistence and profile operations.

### TokenManager
Handles JWT token lifecycle including creation, validation, and refresh.

## Patterns
### Repository Pattern
Data access abstraction for clean separation of concerns.

## Decisions
### ADR-001: Use JWT for Authentication
JWT provides stateless authentication that scales horizontally.

### ADR-002: Use SQLite for Local Storage
SQLite provides embedded database without external dependencies.

## API Interfaces
The system uses REST API for client communication.

## Feature Mappings
### feature-auth
- Component: AuthService
- Component: UserStore
- Pattern: Repository Pattern
- Decision: ADR-001
"#;

    let result = DesignDocImporter::import_markdown(markdown).unwrap();

    // Title from H1
    assert_eq!(result.design_doc.overview.title, "Architecture Design");
    assert_eq!(result.source_format, ImportFormat::Markdown);

    // Overview summary
    assert!(!result.design_doc.overview.summary.is_empty());

    // Components
    let comp_names: Vec<&str> = result
        .design_doc
        .architecture
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(comp_names.contains(&"AuthService"), "Missing AuthService. Got: {:?}", comp_names);
    assert!(comp_names.contains(&"UserStore"), "Missing UserStore. Got: {:?}", comp_names);
    assert!(comp_names.contains(&"TokenManager"), "Missing TokenManager. Got: {:?}", comp_names);

    // Decisions
    assert!(result.design_doc.decisions.len() >= 2);
    let decision_ids: Vec<&str> = result.design_doc.decisions.iter().map(|d| d.id.as_str()).collect();
    assert!(decision_ids.contains(&"ADR-001"));
    assert!(decision_ids.contains(&"ADR-002"));

    // Patterns
    assert!(!result.design_doc.architecture.patterns.is_empty());

    // Interfaces
    assert_eq!(result.design_doc.interfaces.api_standards.style, "REST");

    // Feature mappings
    assert!(result.design_doc.feature_mappings.contains_key("feature-auth"));
    let auth_mapping = &result.design_doc.feature_mappings["feature-auth"];
    assert_eq!(auth_mapping.components.len(), 2);
    assert_eq!(auth_mapping.patterns.len(), 1);
    assert_eq!(auth_mapping.decisions.len(), 1);
}

#[test]
fn test_import_markdown_with_bold_list_components() {
    let markdown = r#"# Bold Components

## Components
- **AuthModule** - Handles authentication
- **DataModule** - Handles data persistence
- **UIModule** - Handles user interface rendering
"#;

    let result = DesignDocImporter::import_markdown(markdown).unwrap();
    assert!(result.design_doc.architecture.components.len() >= 3);

    let names: Vec<&str> = result
        .design_doc
        .architecture
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(names.contains(&"AuthModule"));
    assert!(names.contains(&"DataModule"));
    assert!(names.contains(&"UIModule"));
}

#[test]
fn test_import_markdown_graphql_api_detection() {
    let markdown = r#"# GraphQL Project

## API Interfaces
The system exposes a GraphQL API for frontend communication.
"#;

    let result = DesignDocImporter::import_markdown(markdown).unwrap();
    assert_eq!(result.design_doc.interfaces.api_standards.style, "GraphQL");
}

#[test]
fn test_import_markdown_empty_produces_warnings() {
    let result = DesignDocImporter::import_markdown("").unwrap();
    assert!(!result.clean_import);
    assert!(!result.warnings.is_empty());

    let high_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.severity == WarningSeverity::High)
        .collect();
    assert!(!high_warnings.is_empty());
}

#[test]
fn test_import_markdown_no_headers_produces_warnings() {
    let result = DesignDocImporter::import_markdown("Just plain text without any headers.").unwrap();
    assert!(!result.clean_import);
    assert!(!result.warnings.is_empty());
}

#[test]
fn test_import_markdown_unrecognized_sections_produce_warnings() {
    let markdown = r#"# Project

## Random Section
Some content here.

## Another Unknown Section
More content.
"#;

    let result = DesignDocImporter::import_markdown(markdown).unwrap();
    let low_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.severity == WarningSeverity::Low)
        .collect();
    assert!(low_warnings.len() >= 2, "Should have warnings for unrecognized sections");
}

// ============================================================================
// AC3: Import from JSON Sources
// ============================================================================

#[test]
fn test_import_json_standard_format() {
    let json_content = serde_json::to_string_pretty(&serde_json::json!({
        "metadata": {
            "version": "1.0.0",
            "level": "project"
        },
        "overview": {
            "title": "JSON Standard Import",
            "summary": "Testing standard JSON import"
        },
        "architecture": {
            "components": [
                {"name": "ServiceA", "description": "First service"},
                {"name": "ServiceB", "description": "Second service"}
            ],
            "patterns": [
                {"name": "CQRS", "description": "Command Query Responsibility Segregation"}
            ]
        },
        "decisions": [
            {
                "id": "ADR-001",
                "title": "Use CQRS",
                "status": "accepted",
                "context": "Need separation of read/write models"
            }
        ]
    }))
    .unwrap();

    let result = DesignDocImporter::import_json(&json_content).unwrap();

    assert_eq!(result.design_doc.overview.title, "JSON Standard Import");
    assert_eq!(result.design_doc.architecture.components.len(), 2);
    assert_eq!(result.design_doc.architecture.patterns.len(), 1);
    assert_eq!(result.design_doc.decisions.len(), 1);
    assert_eq!(result.source_format, ImportFormat::Json);
    assert_eq!(result.design_doc.metadata.source.as_deref(), Some("imported-json"));
}

#[test]
fn test_import_json_generic_format() {
    let json_content = r#"{
        "title": "Generic Format Project",
        "description": "A project using non-standard JSON format",
        "components": [
            {"name": "ModuleA", "description": "First module"},
            {"name": "ModuleB", "description": "Second module"}
        ],
        "decisions": [
            {"id": "ADR-001", "title": "Use microservices"}
        ]
    }"#;

    let result = DesignDocImporter::import_json(json_content).unwrap();

    assert_eq!(result.design_doc.overview.title, "Generic Format Project");
    assert_eq!(result.design_doc.architecture.components.len(), 2);
    assert_eq!(result.design_doc.decisions.len(), 1);
    // Should have warning about non-standard format
    assert!(!result.clean_import);
}

#[test]
fn test_import_json_invalid_fails() {
    let result = DesignDocImporter::import_json("not valid json");
    assert!(result.is_err());
    assert!(matches!(result, Err(DesignDocError::ParseError(_))));
}

#[test]
fn test_import_json_empty_object_has_warnings() {
    let result = DesignDocImporter::import_json("{}").unwrap();
    assert!(!result.warnings.is_empty());
}

// ============================================================================
// AC3: Import from File
// ============================================================================

#[test]
fn test_import_from_markdown_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("design.md");

    let content = r#"# File Import Test

## Overview
Testing file-based markdown import.

## Architecture
### MainComponent
The main application component handles routing and business logic.

## Decisions
### ADR-001: Use Rust
Chosen for memory safety and performance.
"#;
    fs::write(&file_path, content).unwrap();

    let result = DesignDocImporter::import(&file_path, None).unwrap();
    assert_eq!(result.design_doc.overview.title, "File Import Test");
    assert_eq!(result.source_format, ImportFormat::Markdown);
    assert!(!result.design_doc.architecture.components.is_empty());
}

#[test]
fn test_import_from_json_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("design.json");

    let content = serde_json::to_string_pretty(&serde_json::json!({
        "overview": {"title": "JSON File Test", "summary": "Testing"},
        "architecture": {
            "components": [{"name": "TestComp", "description": "A test component"}]
        },
        "decisions": []
    }))
    .unwrap();
    fs::write(&file_path, &content).unwrap();

    let result = DesignDocImporter::import(&file_path, None).unwrap();
    assert_eq!(result.design_doc.overview.title, "JSON File Test");
    assert_eq!(result.source_format, ImportFormat::Json);
}

#[test]
fn test_import_file_not_found() {
    let result = DesignDocImporter::import(Path::new("/nonexistent/file.md"), None);
    assert!(matches!(result, Err(DesignDocError::NotFound(_))));
}

#[test]
fn test_import_file_unknown_extension() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("design.txt");
    fs::write(&file_path, "content").unwrap();

    let result = DesignDocImporter::import(&file_path, None);
    assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
}

#[test]
fn test_import_file_empty() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("empty.md");
    fs::write(&file_path, "").unwrap();

    let result = DesignDocImporter::import(&file_path, None);
    assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
}

#[test]
fn test_import_with_format_override() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("design.txt"); // .txt extension

    let content = r#"# Override Test
## Overview
Format override testing.
"#;
    fs::write(&file_path, content).unwrap();

    // Without override: fails
    assert!(DesignDocImporter::import(&file_path, None).is_err());

    // With override: succeeds
    let result = DesignDocImporter::import(&file_path, Some(ImportFormat::Markdown)).unwrap();
    assert_eq!(result.design_doc.overview.title, "Override Test");
}

// ============================================================================
// AC3: Schema Conformance
// ============================================================================

#[test]
fn test_generated_doc_serialization_roundtrip() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&result.design_doc).unwrap();

    // Deserialize back
    let parsed: DesignDoc = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.overview.title, result.design_doc.overview.title);
    assert_eq!(parsed.overview.summary, result.design_doc.overview.summary);
    assert_eq!(
        parsed.architecture.components.len(),
        result.design_doc.architecture.components.len()
    );
    assert_eq!(
        parsed.architecture.patterns.len(),
        result.design_doc.architecture.patterns.len()
    );
    assert_eq!(parsed.decisions.len(), result.design_doc.decisions.len());
    assert_eq!(
        parsed.feature_mappings.len(),
        result.design_doc.feature_mappings.len()
    );
}

#[test]
fn test_generated_doc_can_be_saved_and_loaded() {
    let temp_dir = TempDir::new().unwrap();
    let save_path = temp_dir.path().join("design_doc.json");

    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // Save
    result.design_doc.to_file(&save_path).unwrap();

    // Load
    let loaded = DesignDoc::from_file(&save_path).unwrap();

    assert_eq!(loaded.overview.title, result.design_doc.overview.title);
    assert_eq!(loaded.level(), result.design_doc.level());
    assert!(!loaded.architecture.components.is_empty());
    assert!(!loaded.decisions.is_empty());
}

#[test]
fn test_import_format_detection() {
    assert_eq!(ImportFormat::from_extension(Path::new("doc.md")), Some(ImportFormat::Markdown));
    assert_eq!(ImportFormat::from_extension(Path::new("doc.markdown")), Some(ImportFormat::Markdown));
    assert_eq!(ImportFormat::from_extension(Path::new("doc.json")), Some(ImportFormat::Json));
    assert_eq!(ImportFormat::from_extension(Path::new("doc.txt")), None);
    assert_eq!(ImportFormat::from_extension(Path::new("doc.yaml")), None);
}

#[test]
fn test_import_format_display() {
    assert_eq!(format!("{}", ImportFormat::Markdown), "markdown");
    assert_eq!(format!("{}", ImportFormat::Json), "json");
}

// ============================================================================
// Infrastructure Extraction
// ============================================================================

#[test]
fn test_infrastructure_from_tech_stack() {
    let prd = sample_prd();
    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    let infra = &result.design_doc.architecture.infrastructure;
    // All items should be in new_services since none are marked as existing
    assert!(!infra.new_services.is_empty());
}

// ============================================================================
// Component Name Derivation
// ============================================================================

#[test]
fn test_component_names_are_pascal_case() {
    let prd = PrdDocument {
        title: "Name Test".to_string(),
        description: String::new(),
        stories: vec![
            PrdStory {
                id: "story-001".to_string(),
                title: "User Authentication System".to_string(),
                description: String::new(),
                acceptance_criteria: vec![],
                dependencies: vec![],
                complexity: None,
                story_points: None,
            },
        ],
        tech_stack: vec![],
        goals: vec![],
        non_goals: vec![],
    };

    let result = DesignDocGenerator::generate(&prd, None).unwrap();
    let comp = result
        .design_doc
        .architecture
        .components
        .iter()
        .find(|c| c.features.contains(&"story-001".to_string()));

    assert!(comp.is_some());
    let name = &comp.unwrap().name;
    // Should be PascalCase: UserAuthenticationSystem
    assert!(name.chars().next().unwrap().is_uppercase());
    assert!(!name.contains(' '));
}

// ============================================================================
// Dependency Tracking
// ============================================================================

#[test]
fn test_component_dependencies_from_story_dependencies() {
    let prd = PrdDocument {
        title: "Deps Test".to_string(),
        description: String::new(),
        stories: vec![
            PrdStory {
                id: "story-001".to_string(),
                title: "Base Setup".to_string(),
                description: "Foundation".to_string(),
                acceptance_criteria: vec![],
                dependencies: vec![],
                complexity: None,
                story_points: None,
            },
            PrdStory {
                id: "story-002".to_string(),
                title: "Feature Build".to_string(),
                description: "Depends on setup".to_string(),
                acceptance_criteria: vec![],
                dependencies: vec!["story-001".to_string()],
                complexity: None,
                story_points: None,
            },
        ],
        tech_stack: vec![],
        goals: vec![],
        non_goals: vec![],
    };

    let result = DesignDocGenerator::generate(&prd, None).unwrap();

    // The FeatureBuild component should depend on the BaseSetup component
    let feature_comp = result
        .design_doc
        .architecture
        .components
        .iter()
        .find(|c| c.features.contains(&"story-002".to_string()));

    assert!(feature_comp.is_some());
    assert!(
        !feature_comp.unwrap().dependencies.is_empty(),
        "Feature component should have dependencies"
    );
}

// ============================================================================
// Parallel Execution Pattern Detection
// ============================================================================

#[test]
fn test_parallel_execution_pattern_detected() {
    let prd = PrdDocument {
        title: "Parallel Test".to_string(),
        description: String::new(),
        stories: vec![
            PrdStory {
                id: "s1".to_string(),
                title: "Parallel Task A".to_string(),
                description: "Independent A".to_string(),
                acceptance_criteria: vec![],
                dependencies: vec!["dep-1".to_string(), "dep-2".to_string()], // multiple deps
                complexity: None,
                story_points: None,
            },
        ],
        tech_stack: vec![],
        goals: vec![],
        non_goals: vec![],
    };

    let result = DesignDocGenerator::generate(&prd, None).unwrap();
    let pattern_names: Vec<&str> = result
        .design_doc
        .architecture
        .patterns
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    assert!(
        pattern_names.contains(&"Parallel Execution"),
        "Should detect Parallel Execution pattern. Got: {:?}",
        pattern_names
    );
}
