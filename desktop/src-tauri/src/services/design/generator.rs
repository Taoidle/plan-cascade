//! Design Document Generator
//!
//! Generates comprehensive design documents from PRD JSON structures.
//! Uses the LLM provider service to produce architecture overviews,
//! component definitions, API specifications, story-to-component mappings,
//! and architecture decision records (ADRs).

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::design_doc::{
    ApiStandards, Architecture, Component, Decision, DecisionStatus, DesignDoc, DesignDocError,
    DesignDocLevel, DesignDocMetadata, FeatureMapping, Infrastructure, Interfaces, Overview,
    Pattern,
};

/// PRD Story structure matching the frontend/CLI PRD format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdStory {
    /// Story identifier
    pub id: String,
    /// Story title
    pub title: String,
    /// Story description
    #[serde(default)]
    pub description: String,
    /// Acceptance criteria
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// Dependencies on other stories
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Story complexity estimate
    #[serde(default)]
    pub complexity: Option<String>,
    /// Story points estimate
    #[serde(default)]
    pub story_points: Option<u32>,
}

/// PRD structure matching the standard prd.json format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdDocument {
    /// Project title
    #[serde(default)]
    pub title: String,
    /// Project description
    #[serde(default)]
    pub description: String,
    /// List of stories
    #[serde(default)]
    pub stories: Vec<PrdStory>,
    /// Technical context
    #[serde(default)]
    pub tech_stack: Vec<String>,
    /// Goals
    #[serde(default)]
    pub goals: Vec<String>,
    /// Non-goals
    #[serde(default)]
    pub non_goals: Vec<String>,
}

/// Options for design document generation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateOptions {
    /// Document level to generate (project or feature)
    #[serde(default)]
    pub level: Option<DesignDocLevel>,
    /// Reference to a mega plan (for feature-level docs)
    #[serde(default)]
    pub mega_plan_reference: Option<String>,
    /// Additional context to include in generation
    #[serde(default)]
    pub additional_context: Option<String>,
}

/// Result of design document generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResult {
    /// The generated design document
    pub design_doc: DesignDoc,
    /// Path where the document was saved (if saved)
    pub saved_path: Option<String>,
    /// Generation metadata
    pub generation_info: GenerationInfo,
}

/// Metadata about the generation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationInfo {
    /// Number of stories processed
    pub stories_processed: usize,
    /// Number of components extracted
    pub components_generated: usize,
    /// Number of patterns identified
    pub patterns_identified: usize,
    /// Number of ADRs created
    pub decisions_created: usize,
    /// Number of feature mappings created
    pub feature_mappings_created: usize,
}

/// Design document generator service
pub struct DesignDocGenerator;

impl DesignDocGenerator {
    /// Generate a design document from a PRD structure.
    ///
    /// Analyzes the PRD stories, goals, and tech stack to produce:
    /// - Architecture overview with system description
    /// - Component definitions derived from story analysis
    /// - Design patterns based on tech stack and architecture
    /// - Architecture Decision Records (ADRs) for key choices
    /// - Story-to-component feature mappings
    ///
    /// # Arguments
    /// * `prd` - The PRD document to generate from
    /// * `options` - Optional generation configuration
    ///
    /// # Returns
    /// A `GenerateResult` containing the design document and generation metadata.
    pub fn generate(
        prd: &PrdDocument,
        options: Option<&GenerateOptions>,
    ) -> Result<GenerateResult, DesignDocError> {
        // Validate PRD input
        if prd.title.is_empty() && prd.stories.is_empty() {
            return Err(DesignDocError::ValidationError(
                "PRD must have a title or at least one story".to_string(),
            ));
        }

        let level = options
            .and_then(|o| o.level)
            .unwrap_or(DesignDocLevel::Project);

        // Build the design document
        let mut doc = DesignDoc::new();

        // Set metadata
        doc.metadata = DesignDocMetadata {
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            version: "1.0.0".to_string(),
            source: Some("generated".to_string()),
            level,
            mega_plan_reference: options.and_then(|o| o.mega_plan_reference.clone()),
        };

        // Set overview from PRD
        doc.overview = Overview {
            title: prd.title.clone(),
            summary: prd.description.clone(),
            goals: prd.goals.clone(),
            non_goals: prd.non_goals.clone(),
        };

        // Extract components from stories
        let components = Self::extract_components(&prd.stories, &prd.tech_stack);

        // Extract patterns from tech stack and stories
        let patterns = Self::extract_patterns(&prd.tech_stack, &prd.stories);

        // Build infrastructure from tech stack
        let infrastructure = Self::extract_infrastructure(&prd.tech_stack);

        // Build architecture section
        doc.architecture = Architecture {
            system_overview: Self::build_system_overview(prd),
            components: components.clone(),
            data_flow: Self::build_data_flow_description(&components),
            patterns: patterns.clone(),
            infrastructure,
        };

        // Build interfaces section
        doc.interfaces = Self::build_interfaces(&prd.tech_stack);

        // Extract decisions/ADRs from tech stack and architecture
        let decisions = Self::extract_decisions(&prd.tech_stack, &components);
        doc.decisions = decisions.clone();

        // Build feature mappings (story -> component mappings)
        let feature_mappings =
            Self::build_feature_mappings(&prd.stories, &components, &patterns, &decisions);
        doc.feature_mappings = feature_mappings.clone();

        let generation_info = GenerationInfo {
            stories_processed: prd.stories.len(),
            components_generated: components.len(),
            patterns_identified: patterns.len(),
            decisions_created: decisions.len(),
            feature_mappings_created: feature_mappings.len(),
        };

        Ok(GenerateResult {
            design_doc: doc,
            saved_path: None,
            generation_info,
        })
    }

    /// Generate a design document from a PRD file path.
    ///
    /// Reads the PRD JSON from disk, parses it, and generates the design document.
    /// Optionally saves the result to disk.
    ///
    /// # Arguments
    /// * `prd_path` - Path to the prd.json file
    /// * `options` - Optional generation configuration
    /// * `save` - Whether to save the generated document next to the PRD
    pub fn generate_from_file(
        prd_path: &Path,
        options: Option<&GenerateOptions>,
        save: bool,
    ) -> Result<GenerateResult, DesignDocError> {
        if !prd_path.exists() {
            return Err(DesignDocError::NotFound(format!(
                "PRD file not found: {}",
                prd_path.display()
            )));
        }

        let content = std::fs::read_to_string(prd_path)
            .map_err(|e| DesignDocError::IoError(e.to_string()))?;

        let prd: PrdDocument = serde_json::from_str(&content)
            .map_err(|e| DesignDocError::ParseError(format!("Failed to parse PRD: {}", e)))?;

        let mut result = Self::generate(&prd, options)?;

        if save {
            let output_path = prd_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("design_doc.json");

            result.design_doc.to_file(&output_path)?;
            result.saved_path = Some(output_path.display().to_string());
        }

        Ok(result)
    }

    /// Build a system overview string from the PRD
    fn build_system_overview(prd: &PrdDocument) -> String {
        let mut overview = String::new();

        if !prd.title.is_empty() {
            overview.push_str(&format!("{}", prd.title));
        }

        if !prd.description.is_empty() {
            if !overview.is_empty() {
                overview.push_str(": ");
            }
            overview.push_str(&prd.description);
        }

        if !prd.tech_stack.is_empty() {
            overview.push_str(&format!("\n\nTech Stack: {}", prd.tech_stack.join(", ")));
        }

        if overview.is_empty() {
            overview = "System architecture generated from PRD stories.".to_string();
        }

        overview
    }

    /// Extract components from PRD stories by analyzing story titles and descriptions
    fn extract_components(stories: &[PrdStory], tech_stack: &[String]) -> Vec<Component> {
        let mut components: Vec<Component> = Vec::new();
        let mut component_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Analyze stories to identify components
        for story in stories {
            let component_name = Self::story_to_component_name(story);

            if component_names.contains(&component_name) {
                // Add story as a feature reference to existing component
                if let Some(comp) = components.iter_mut().find(|c| c.name == component_name) {
                    comp.features.push(story.id.clone());
                    if !story.description.is_empty() {
                        comp.responsibilities.push(story.title.clone());
                    }
                }
            } else {
                component_names.insert(component_name.clone());
                let mut comp = Component::new(&component_name);
                comp.description = format!("Handles {}", story.title.to_lowercase());
                comp.features.push(story.id.clone());
                comp.responsibilities.push(story.title.clone());

                // Link dependencies between components
                for dep in &story.dependencies {
                    let dep_name = Self::id_to_component_name(dep);
                    if !comp.dependencies.contains(&dep_name) {
                        comp.dependencies.push(dep_name);
                    }
                }

                components.push(comp);
            }
        }

        // Add core infrastructure components based on tech stack
        for tech in tech_stack {
            let tech_lower = tech.to_lowercase();
            if (tech_lower.contains("database")
                || tech_lower.contains("sqlite")
                || tech_lower.contains("postgres"))
                && !component_names.contains("DataLayer")
            {
                let mut comp = Component::new("DataLayer");
                comp.description = format!("Data persistence layer using {}", tech);
                comp.responsibilities
                    .push("Data storage and retrieval".to_string());
                components.push(comp);
                component_names.insert("DataLayer".to_string());
            }

            if (tech_lower.contains("api")
                || tech_lower.contains("rest")
                || tech_lower.contains("graphql"))
                && !component_names.contains("ApiGateway")
            {
                let mut comp = Component::new("ApiGateway");
                comp.description = format!("API layer using {}", tech);
                comp.responsibilities
                    .push("Request routing and validation".to_string());
                components.push(comp);
                component_names.insert("ApiGateway".to_string());
            }
        }

        components
    }

    /// Convert a story to a component name
    fn story_to_component_name(story: &PrdStory) -> String {
        // Extract meaningful words from the story title
        let title = &story.title;
        let words: Vec<&str> = title
            .split_whitespace()
            .filter(|w| {
                let lower = w.to_lowercase();
                ![
                    "the", "a", "an", "and", "or", "for", "to", "in", "of", "with", "as",
                ]
                .contains(&lower.as_str())
            })
            .take(3)
            .collect();

        if words.is_empty() {
            return Self::id_to_component_name(&story.id);
        }

        // PascalCase the significant words
        words
            .iter()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let first: String = c.to_uppercase().collect();
                        first + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Convert a story ID to a component name
    fn id_to_component_name(id: &str) -> String {
        id.split('-')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let first: String = c.to_uppercase().collect();
                        first + chars.as_str()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Extract design patterns from tech stack and story analysis
    fn extract_patterns(tech_stack: &[String], stories: &[PrdStory]) -> Vec<Pattern> {
        let mut patterns: Vec<Pattern> = Vec::new();
        let mut pattern_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Tech stack derived patterns
        for tech in tech_stack {
            let tech_lower = tech.to_lowercase();

            if tech_lower.contains("react") && !pattern_names.contains("Component-Based UI") {
                let mut p = Pattern::new("Component-Based UI");
                p.description =
                    "React component architecture with composable UI elements".to_string();
                p.rationale = "Enables reusable, testable UI components".to_string();
                patterns.push(p);
                pattern_names.insert("Component-Based UI".to_string());
            }

            if (tech_lower.contains("rest") || tech_lower.contains("api"))
                && !pattern_names.contains("RESTful API")
            {
                let mut p = Pattern::new("RESTful API");
                p.description = "REST API design with standard HTTP methods".to_string();
                p.rationale = "Standard, well-understood API pattern".to_string();
                patterns.push(p);
                pattern_names.insert("RESTful API".to_string());
            }

            if (tech_lower.contains("tauri") || tech_lower.contains("electron"))
                && !pattern_names.contains("IPC Command Pattern")
            {
                let mut p = Pattern::new("IPC Command Pattern");
                p.description = "Command-based IPC between frontend and backend".to_string();
                p.rationale = "Type-safe communication across process boundaries".to_string();
                patterns.push(p);
                pattern_names.insert("IPC Command Pattern".to_string());
            }
        }

        // Story-derived patterns
        let has_parallel = stories
            .iter()
            .any(|s| s.title.to_lowercase().contains("parallel") || s.dependencies.len() > 1);
        if has_parallel && !pattern_names.contains("Parallel Execution") {
            let mut p = Pattern::new("Parallel Execution");
            p.description = "Concurrent task execution with dependency resolution".to_string();
            p.rationale = "Maximize throughput for independent tasks".to_string();
            patterns.push(p);
            pattern_names.insert("Parallel Execution".to_string());
        }

        // If no patterns found, add a generic one
        if patterns.is_empty() {
            let mut p = Pattern::new("Modular Architecture");
            p.description = "Separation of concerns through modular design".to_string();
            p.rationale = "Maintainability and testability".to_string();
            patterns.push(p);
        }

        patterns
    }

    /// Extract infrastructure information from tech stack
    fn extract_infrastructure(tech_stack: &[String]) -> Infrastructure {
        let mut existing = Vec::new();
        let mut new_services = Vec::new();

        for tech in tech_stack {
            let tech_lower = tech.to_lowercase();
            if tech_lower.contains("existing") || tech_lower.contains("legacy") {
                existing.push(tech.clone());
            } else {
                new_services.push(tech.clone());
            }
        }

        // If nothing was marked as existing, all are new
        if existing.is_empty() && new_services.is_empty() {
            new_services = tech_stack.to_vec();
        }

        Infrastructure {
            existing_services: existing,
            new_services,
        }
    }

    /// Build interfaces section from tech stack
    fn build_interfaces(tech_stack: &[String]) -> Interfaces {
        let tech_str = tech_stack.join(", ").to_lowercase();

        let style = if tech_str.contains("graphql") {
            "GraphQL"
        } else if tech_str.contains("grpc") {
            "gRPC"
        } else if tech_str.contains("tauri") {
            "Tauri IPC Commands"
        } else {
            "REST"
        }
        .to_string();

        let async_pattern = if tech_str.contains("rust") || tech_str.contains("tauri") {
            "async/await with tokio"
        } else if tech_str.contains("node") || tech_str.contains("typescript") {
            "Promise-based async/await"
        } else {
            "async/await"
        }
        .to_string();

        Interfaces {
            api_standards: ApiStandards {
                style,
                error_handling: "Structured error responses with error codes".to_string(),
                async_pattern,
            },
            shared_data_models: Vec::new(),
        }
    }

    /// Build a data flow description from components
    fn build_data_flow_description(components: &[Component]) -> String {
        if components.is_empty() {
            return "No data flow identified.".to_string();
        }

        let component_names: Vec<&str> = components.iter().map(|c| c.name.as_str()).collect();
        format!(
            "Data flows between {} components: {}",
            components.len(),
            component_names.join(" -> ")
        )
    }

    /// Extract architecture decision records from tech choices
    fn extract_decisions(tech_stack: &[String], components: &[Component]) -> Vec<Decision> {
        let mut decisions = Vec::new();
        let mut adr_counter = 1;

        // Create an ADR for each significant tech stack choice
        for tech in tech_stack {
            let id = format!("ADR-{:03}", adr_counter);
            let mut decision = Decision::new(&id, format!("Use {}", tech));
            decision.context = format!("Technology selection for the project architecture");
            decision.decision = format!("Adopt {} as part of the technology stack", tech);
            decision.rationale = format!("{} was selected based on project requirements", tech);
            decision.status = DecisionStatus::Accepted;

            // Associate with components that might use this tech
            let tech_lower = tech.to_lowercase();
            for comp in components {
                let comp_lower = comp.name.to_lowercase();
                if comp_lower.contains(&tech_lower)
                    || tech_lower.contains(&comp_lower)
                    || comp.description.to_lowercase().contains(&tech_lower)
                {
                    decision.applies_to.push(comp.name.clone());
                }
            }

            decisions.push(decision);
            adr_counter += 1;
        }

        // If no tech stack, create a general architecture ADR
        if decisions.is_empty() {
            let mut decision = Decision::new("ADR-001", "Modular Architecture");
            decision.context = "Need a maintainable and extensible architecture".to_string();
            decision.decision =
                "Use modular architecture with clear separation of concerns".to_string();
            decision.rationale =
                "Enables independent development and testing of components".to_string();
            decision.status = DecisionStatus::Accepted;
            decisions.push(decision);
        }

        decisions
    }

    /// Build story-to-component feature mappings
    fn build_feature_mappings(
        stories: &[PrdStory],
        components: &[Component],
        patterns: &[Pattern],
        decisions: &[Decision],
    ) -> HashMap<String, FeatureMapping> {
        let mut mappings = HashMap::new();

        for story in stories {
            let component_name = Self::story_to_component_name(story);

            // Find matching components
            let mut mapping_components: Vec<String> = Vec::new();
            for comp in components {
                if comp.name == component_name || comp.features.contains(&story.id) {
                    mapping_components.push(comp.name.clone());
                }
                // Also include components that are dependencies
                for dep in &story.dependencies {
                    let dep_comp = Self::id_to_component_name(dep);
                    if comp.name == dep_comp && !mapping_components.contains(&comp.name) {
                        mapping_components.push(comp.name.clone());
                    }
                }
            }

            // Find applicable patterns (first pattern if any)
            let mapping_patterns: Vec<String> = patterns
                .iter()
                .filter(|p| p.applies_to.contains(&story.id) || p.applies_to.is_empty())
                .map(|p| p.name.clone())
                .take(1)
                .collect();

            // Find applicable decisions
            let mapping_decisions: Vec<String> = decisions
                .iter()
                .filter(|d| {
                    d.applies_to.iter().any(|a| mapping_components.contains(a))
                        || d.applies_to.is_empty()
                })
                .map(|d| d.id.clone())
                .collect();

            let mapping = FeatureMapping {
                components: mapping_components,
                patterns: mapping_patterns,
                decisions: mapping_decisions,
                description: story.title.clone(),
            };

            mappings.insert(story.id.clone(), mapping);
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
    use tempfile::TempDir;

    fn sample_prd() -> PrdDocument {
        PrdDocument {
            title: "Test Project".to_string(),
            description: "A test project for design doc generation".to_string(),
            stories: vec![
                PrdStory {
                    id: "story-001".to_string(),
                    title: "User Authentication".to_string(),
                    description: "Implement user login and registration".to_string(),
                    acceptance_criteria: vec!["Users can log in".to_string()],
                    dependencies: vec![],
                    complexity: Some("medium".to_string()),
                    story_points: Some(5),
                },
                PrdStory {
                    id: "story-002".to_string(),
                    title: "Dashboard Display".to_string(),
                    description: "Show project metrics on dashboard".to_string(),
                    acceptance_criteria: vec!["Dashboard shows metrics".to_string()],
                    dependencies: vec!["story-001".to_string()],
                    complexity: Some("low".to_string()),
                    story_points: Some(3),
                },
                PrdStory {
                    id: "story-003".to_string(),
                    title: "Data Export".to_string(),
                    description: "Export data to CSV and JSON".to_string(),
                    acceptance_criteria: vec!["Data can be exported".to_string()],
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
            goals: vec!["Fast and reliable".to_string()],
            non_goals: vec!["Mobile support".to_string()],
        }
    }

    #[test]
    fn test_generate_from_prd() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        assert_eq!(result.design_doc.overview.title, "Test Project");
        assert_eq!(result.design_doc.overview.summary, prd.description);
        assert_eq!(result.design_doc.overview.goals.len(), 1);
        assert_eq!(result.design_doc.overview.non_goals.len(), 1);
        assert_eq!(result.design_doc.level(), DesignDocLevel::Project);
        assert_eq!(
            result.design_doc.metadata.source.as_deref(),
            Some("generated")
        );
    }

    #[test]
    fn test_generate_produces_components() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        // Should have components for each story plus infrastructure
        assert!(result.design_doc.architecture.components.len() >= 3);
        assert!(result.generation_info.components_generated >= 3);

        // Each story-derived component should have at least one feature reference
        let story_components: Vec<_> = result
            .design_doc
            .architecture
            .components
            .iter()
            .filter(|c| !c.features.is_empty())
            .collect();
        assert!(!story_components.is_empty());
    }

    #[test]
    fn test_generate_produces_patterns() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        // Should identify React and Tauri patterns from tech stack
        assert!(!result.design_doc.architecture.patterns.is_empty());
        assert!(result.generation_info.patterns_identified > 0);

        let pattern_names: Vec<&str> = result
            .design_doc
            .architecture
            .patterns
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        // React should trigger Component-Based UI pattern
        assert!(pattern_names.contains(&"Component-Based UI"));
        // Tauri should trigger IPC Command Pattern
        assert!(pattern_names.contains(&"IPC Command Pattern"));
    }

    #[test]
    fn test_generate_produces_decisions() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        // Should have at least one ADR per tech stack item
        assert!(result.design_doc.decisions.len() >= prd.tech_stack.len());
        assert!(result.generation_info.decisions_created > 0);

        // All decisions should have IDs starting with ADR-
        for decision in &result.design_doc.decisions {
            assert!(decision.id.starts_with("ADR-"));
            assert_eq!(decision.status, DecisionStatus::Accepted);
        }
    }

    #[test]
    fn test_generate_produces_feature_mappings() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        // Should have one mapping per story
        assert_eq!(result.design_doc.feature_mappings.len(), prd.stories.len());
        assert_eq!(
            result.generation_info.feature_mappings_created,
            prd.stories.len()
        );

        // Each mapping should reference at least one component
        for (story_id, mapping) in &result.design_doc.feature_mappings {
            assert!(
                !mapping.components.is_empty(),
                "Story {} should have component mappings",
                story_id
            );
            assert!(!mapping.description.is_empty());
        }
    }

    #[test]
    fn test_generate_with_feature_level() {
        let prd = sample_prd();
        let options = GenerateOptions {
            level: Some(DesignDocLevel::Feature),
            mega_plan_reference: Some("mega-001".to_string()),
            additional_context: None,
        };

        let result = DesignDocGenerator::generate(&prd, Some(&options)).unwrap();
        assert_eq!(result.design_doc.level(), DesignDocLevel::Feature);
        assert_eq!(
            result.design_doc.metadata.mega_plan_reference.as_deref(),
            Some("mega-001")
        );
    }

    #[test]
    fn test_generate_empty_prd_fails() {
        let empty_prd = PrdDocument {
            title: String::new(),
            description: String::new(),
            stories: vec![],
            tech_stack: vec![],
            goals: vec![],
            non_goals: vec![],
        };

        let result = DesignDocGenerator::generate(&empty_prd, None);
        assert!(matches!(result, Err(DesignDocError::ValidationError(_))));
    }

    #[test]
    fn test_generate_minimal_prd() {
        let minimal_prd = PrdDocument {
            title: "Minimal".to_string(),
            description: String::new(),
            stories: vec![],
            tech_stack: vec![],
            goals: vec![],
            non_goals: vec![],
        };

        let result = DesignDocGenerator::generate(&minimal_prd, None).unwrap();
        assert_eq!(result.design_doc.overview.title, "Minimal");
        // Should still have at least one decision (the default architecture ADR)
        assert!(!result.design_doc.decisions.is_empty());
    }

    #[test]
    fn test_generate_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let prd_path = temp_dir.path().join("prd.json");

        let prd = sample_prd();
        let json = serde_json::to_string_pretty(&prd).unwrap();
        std::fs::write(&prd_path, json).unwrap();

        let result = DesignDocGenerator::generate_from_file(&prd_path, None, false).unwrap();
        assert_eq!(result.design_doc.overview.title, "Test Project");
        assert!(result.saved_path.is_none());
    }

    #[test]
    fn test_generate_from_file_and_save() {
        let temp_dir = TempDir::new().unwrap();
        let prd_path = temp_dir.path().join("prd.json");

        let prd = sample_prd();
        let json = serde_json::to_string_pretty(&prd).unwrap();
        std::fs::write(&prd_path, json).unwrap();

        let result = DesignDocGenerator::generate_from_file(&prd_path, None, true).unwrap();
        assert!(result.saved_path.is_some());

        // Verify the file was created
        let design_doc_path = temp_dir.path().join("design_doc.json");
        assert!(design_doc_path.exists());

        // Verify it can be loaded back
        let loaded = DesignDoc::from_file(&design_doc_path).unwrap();
        assert_eq!(loaded.overview.title, "Test Project");
    }

    #[test]
    fn test_generate_from_nonexistent_file() {
        let result =
            DesignDocGenerator::generate_from_file(Path::new("/nonexistent/prd.json"), None, false);
        assert!(matches!(result, Err(DesignDocError::NotFound(_))));
    }

    #[test]
    fn test_story_to_component_name() {
        let story = PrdStory {
            id: "story-001".to_string(),
            title: "User Authentication System".to_string(),
            description: String::new(),
            acceptance_criteria: vec![],
            dependencies: vec![],
            complexity: None,
            story_points: None,
        };

        let name = DesignDocGenerator::story_to_component_name(&story);
        assert_eq!(name, "UserAuthenticationSystem");
    }

    #[test]
    fn test_id_to_component_name() {
        assert_eq!(
            DesignDocGenerator::id_to_component_name("story-001"),
            "Story001"
        );
        assert_eq!(
            DesignDocGenerator::id_to_component_name("feature-auth"),
            "FeatureAuth"
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let prd = sample_prd();
        let result = DesignDocGenerator::generate(&prd, None).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&result.design_doc).unwrap();

        // Deserialize back
        let parsed: DesignDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.overview.title, result.design_doc.overview.title);
        assert_eq!(
            parsed.architecture.components.len(),
            result.design_doc.architecture.components.len()
        );
    }
}
