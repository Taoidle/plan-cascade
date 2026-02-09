//! PRD Generator
//!
//! Generates PRDs using LLM providers with design document context.
//! Supports multiple LLM backends and auto-infers story dependencies.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::models::design_doc::DesignDoc;
use crate::models::prd::{AcceptanceCriteria, Prd, Priority, Story, StoryType};
use crate::services::design::DesignDocLoader;

/// Errors from PRD generation
#[derive(Debug, Error)]
pub enum PrdGeneratorError {
    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Design document error: {0}")]
    DesignDocError(String),

    #[error("Generation cancelled")]
    Cancelled,
}

/// Configuration for PRD generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdGeneratorConfig {
    /// Maximum number of stories to generate
    #[serde(default = "default_max_stories")]
    pub max_stories: usize,
    /// Minimum number of stories to generate
    #[serde(default = "default_min_stories")]
    pub min_stories: usize,
    /// Whether to auto-infer dependencies
    #[serde(default = "default_auto_dependencies")]
    pub auto_infer_dependencies: bool,
    /// Whether to generate acceptance criteria
    #[serde(default = "default_generate_criteria")]
    pub generate_acceptance_criteria: bool,
    /// Whether to use design document context
    #[serde(default = "default_use_design_context")]
    pub use_design_context: bool,
    /// LLM model to use (e.g., "claude-3-sonnet", "gpt-4", "deepseek-coder")
    #[serde(default = "default_model")]
    pub model: String,
    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_stories() -> usize {
    7
}

fn default_min_stories() -> usize {
    3
}

fn default_auto_dependencies() -> bool {
    true
}

fn default_generate_criteria() -> bool {
    true
}

fn default_use_design_context() -> bool {
    true
}

fn default_model() -> String {
    "claude-3-sonnet".to_string()
}

fn default_temperature() -> f32 {
    0.3
}

impl Default for PrdGeneratorConfig {
    fn default() -> Self {
        Self {
            max_stories: default_max_stories(),
            min_stories: default_min_stories(),
            auto_infer_dependencies: default_auto_dependencies(),
            generate_acceptance_criteria: default_generate_criteria(),
            use_design_context: default_use_design_context(),
            model: default_model(),
            temperature: default_temperature(),
        }
    }
}

/// Request for PRD generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdGenerationRequest {
    /// Feature ID
    pub feature_id: String,
    /// Feature title
    pub feature_title: String,
    /// Feature description
    pub feature_description: String,
    /// Design document context (optional)
    #[serde(default)]
    pub design_context: Option<DesignDocContext>,
    /// Additional context/requirements
    #[serde(default)]
    pub additional_context: Option<String>,
}

/// Design document context for PRD generation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DesignDocContext {
    /// Relevant components
    pub components: Vec<String>,
    /// Relevant patterns
    pub patterns: Vec<String>,
    /// Relevant decisions
    pub decisions: Vec<String>,
    /// Feature mapping description
    pub mapping_description: Option<String>,
}

impl DesignDocContext {
    /// Create from a design document and feature ID
    pub fn from_design_doc(doc: &DesignDoc, feature_id: &str) -> Self {
        let mut ctx = Self::default();

        // Get feature mapping if exists
        if let Some(mapping) = doc.get_feature_mapping(feature_id) {
            ctx.mapping_description = Some(mapping.description.clone());

            // Get referenced components
            for comp_name in &mapping.components {
                if let Some(comp) = doc.get_component(comp_name) {
                    ctx.components
                        .push(format!("{}: {}", comp.name, comp.description));
                }
            }

            // Get referenced patterns
            for pattern_name in &mapping.patterns {
                if let Some(pattern) = doc.get_pattern(pattern_name) {
                    ctx.patterns
                        .push(format!("{}: {}", pattern.name, pattern.description));
                }
            }

            // Get referenced decisions
            for decision_id in &mapping.decisions {
                if let Some(decision) = doc.get_decision(decision_id) {
                    ctx.decisions.push(format!(
                        "{} - {}: {}",
                        decision.id, decision.title, decision.decision
                    ));
                }
            }
        }

        ctx
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
            && self.patterns.is_empty()
            && self.decisions.is_empty()
            && self.mapping_description.is_none()
    }
}

/// Result of PRD generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdGenerationResult {
    /// Generated PRD
    pub prd: Prd,
    /// Generation metadata
    pub metadata: GenerationMetadata,
}

/// Metadata about the generation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationMetadata {
    /// Model used for generation
    pub model: String,
    /// Number of stories generated
    pub story_count: usize,
    /// Whether design context was used
    pub used_design_context: bool,
    /// Generation duration in milliseconds
    pub duration_ms: u64,
    /// Token usage (if available)
    pub tokens_used: Option<u32>,
}

/// PRD Generator Service
///
/// Generates PRDs using LLM providers with design document context.
pub struct PrdGenerator {
    /// Configuration
    config: PrdGeneratorConfig,
    /// Design document loader
    design_loader: Arc<DesignDocLoader>,
    /// Custom generator function (for testing/flexibility)
    custom_generator: Option<GeneratorFn>,
}

/// Generator function type
pub type GeneratorFn = Arc<
    dyn Fn(
            PrdGenerationRequest,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Prd, String>> + Send>>
        + Send
        + Sync,
>;

impl PrdGenerator {
    /// Create a new PRD generator
    pub fn new(config: PrdGeneratorConfig) -> Self {
        Self {
            config,
            design_loader: Arc::new(DesignDocLoader::new()),
            custom_generator: None,
        }
    }

    /// Set a custom design loader
    pub fn with_design_loader(mut self, loader: Arc<DesignDocLoader>) -> Self {
        self.design_loader = loader;
        self
    }

    /// Set a custom generator function (useful for testing)
    pub fn with_generator(mut self, generator: GeneratorFn) -> Self {
        self.custom_generator = Some(generator);
        self
    }

    /// Generate a PRD for a feature
    pub async fn generate(
        &self,
        request: PrdGenerationRequest,
    ) -> Result<PrdGenerationResult, PrdGeneratorError> {
        let start_time = std::time::Instant::now();

        info!("Generating PRD for feature: {}", request.feature_id);

        // Use custom generator if provided
        let prd = if let Some(generator) = &self.custom_generator {
            generator(request.clone())
                .await
                .map_err(|e| PrdGeneratorError::LlmError(e))?
        } else {
            // Default implementation: create structured PRD based on description
            self.generate_default_prd(&request).await?
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        let metadata = GenerationMetadata {
            model: self.config.model.clone(),
            story_count: prd.stories.len(),
            used_design_context: request.design_context.is_some(),
            duration_ms,
            tokens_used: None,
        };

        info!(
            "Generated PRD for {} with {} stories in {}ms",
            request.feature_id,
            prd.stories.len(),
            duration_ms
        );

        Ok(PrdGenerationResult { prd, metadata })
    }

    /// Generate PRD with design document context
    pub async fn generate_with_context(
        &self,
        feature_id: &str,
        feature_title: &str,
        feature_description: &str,
        project_root: &Path,
    ) -> Result<PrdGenerationResult, PrdGeneratorError> {
        // Load design document context
        let design_context = if self.config.use_design_context {
            self.load_design_context(feature_id, project_root)
                .await
                .ok()
        } else {
            None
        };

        let request = PrdGenerationRequest {
            feature_id: feature_id.to_string(),
            feature_title: feature_title.to_string(),
            feature_description: feature_description.to_string(),
            design_context,
            additional_context: None,
        };

        self.generate(request).await
    }

    /// Load design document context for a feature
    async fn load_design_context(
        &self,
        feature_id: &str,
        project_root: &Path,
    ) -> Result<DesignDocContext, PrdGeneratorError> {
        // Try to load project design doc
        self.design_loader
            .load_project_doc(project_root)
            .await
            .map_err(|e| PrdGeneratorError::DesignDocError(e.to_string()))?;

        // Get the design doc and extract context
        if let Some(doc) = self.design_loader.get_project_doc().await {
            Ok(DesignDocContext::from_design_doc(&doc, feature_id))
        } else {
            Ok(DesignDocContext::default())
        }
    }

    /// Default PRD generation (structured breakdown based on description)
    async fn generate_default_prd(
        &self,
        request: &PrdGenerationRequest,
    ) -> Result<Prd, PrdGeneratorError> {
        let mut prd = Prd::new(&request.feature_title);
        prd.name = request.feature_id.clone();
        prd.description = request.feature_description.clone();

        // Parse the description to extract stories
        let stories = self.extract_stories_from_description(
            &request.feature_description,
            &request.design_context,
        );

        for story in stories {
            prd.add_story(story);
        }

        // Auto-infer dependencies if enabled
        if self.config.auto_infer_dependencies && prd.stories.len() > 1 {
            self.infer_dependencies(&mut prd);
        }

        Ok(prd)
    }

    /// Extract stories from a feature description
    fn extract_stories_from_description(
        &self,
        description: &str,
        design_context: &Option<DesignDocContext>,
    ) -> Vec<Story> {
        let mut stories = Vec::new();
        let lines: Vec<&str> = description.lines().collect();

        // Look for numbered items or key phrases
        let mut story_num = 1;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Check for numbered items (1. 2. etc)
            if let Some(stripped) = line
                .strip_prefix(|c: char| c.is_ascii_digit())
                .and_then(|s| s.strip_prefix('.'))
                .or_else(|| line.strip_prefix('-'))
                .or_else(|| line.strip_prefix('*'))
            {
                let stripped = stripped.trim();
                if !stripped.is_empty() {
                    let story =
                        self.create_story_from_line(&format!("S{:03}", story_num), stripped);
                    stories.push(story);
                    story_num += 1;
                }
            }
        }

        // If no explicit items found, create stories based on key concepts
        if stories.is_empty() {
            stories = self.create_stories_from_concepts(description, design_context);
        }

        // Ensure minimum number of stories
        if stories.len() < self.config.min_stories {
            // Add a setup story
            let setup = Story::new(
                format!("S{:03}", stories.len() + 1),
                "Setup and Configuration",
            );
            stories.insert(0, setup);
        }

        // Limit maximum stories
        stories.truncate(self.config.max_stories);

        stories
    }

    /// Create a story from a line of text
    fn create_story_from_line(&self, id: &str, text: &str) -> Story {
        let mut story = Story::new(id.to_string(), text.to_string());

        // Infer story type from text
        let text_lower = text.to_lowercase();
        if text_lower.contains("test") || text_lower.contains("测试") {
            story.story_type = Some(StoryType::Test);
        } else if text_lower.contains("refactor") || text_lower.contains("重构") {
            story.story_type = Some(StoryType::Refactor);
        } else if text_lower.contains("fix")
            || text_lower.contains("bug")
            || text_lower.contains("修复")
        {
            story.story_type = Some(StoryType::Bugfix);
        } else {
            story.story_type = Some(StoryType::Feature);
        }

        // Infer priority from keywords
        if text_lower.contains("critical")
            || text_lower.contains("核心")
            || text_lower.contains("关键")
        {
            story.priority = Priority::Critical;
        } else if text_lower.contains("important")
            || text_lower.contains("high")
            || text_lower.contains("重要")
        {
            story.priority = Priority::High;
        } else if text_lower.contains("low")
            || text_lower.contains("optional")
            || text_lower.contains("可选")
        {
            story.priority = Priority::Low;
        } else {
            story.priority = Priority::Medium;
        }

        // Generate acceptance criteria if enabled
        if self.config.generate_acceptance_criteria {
            story.acceptance_criteria.push(AcceptanceCriteria {
                id: format!("{}-AC1", id),
                description: format!("Implementation of '{}' is complete", text),
                met: false,
            });
            story.acceptance_criteria.push(AcceptanceCriteria {
                id: format!("{}-AC2", id),
                description: "Unit tests pass".to_string(),
                met: false,
            });
        }

        story
    }

    /// Create stories from key concepts in description
    fn create_stories_from_concepts(
        &self,
        description: &str,
        design_context: &Option<DesignDocContext>,
    ) -> Vec<Story> {
        let mut stories = Vec::new();
        let mut story_num = 1;

        // Extract key concepts from design context
        if let Some(ctx) = design_context {
            // Create stories for each component
            for comp in &ctx.components {
                if story_num > self.config.max_stories {
                    break;
                }
                let title = format!(
                    "Implement {}",
                    comp.split(':').next().unwrap_or(comp).trim()
                );
                let story = self.create_story_from_line(&format!("S{:03}", story_num), &title);
                stories.push(story);
                story_num += 1;
            }
        }

        // If still no stories, create generic ones based on common patterns
        if stories.is_empty() {
            // Setup story
            let setup = self.create_story_from_line("S001", "Setup core structures and types");
            stories.push(setup);

            // Main implementation story
            let main_impl = self.create_story_from_line("S002", "Implement main functionality");
            stories.push(main_impl);

            // Test story
            let tests = self.create_story_from_line("S003", "Add unit tests");
            stories.push(tests);
        }

        stories
    }

    /// Infer dependencies between stories
    fn infer_dependencies(&self, prd: &mut Prd) {
        let story_ids: Vec<String> = prd.stories.iter().map(|s| s.id.clone()).collect();

        // Simple heuristic: sequential dependencies
        // Setup/core stories first, then implementation, then tests
        for i in 1..prd.stories.len() {
            let story = &prd.stories[i];

            // Tests depend on implementation
            if story.story_type == Some(StoryType::Test) {
                // Find non-test stories to depend on
                let deps: Vec<String> = prd.stories[..i]
                    .iter()
                    .filter(|s| s.story_type != Some(StoryType::Test))
                    .map(|s| s.id.clone())
                    .collect();

                if !deps.is_empty() {
                    prd.stories[i].dependencies = deps;
                }
            } else {
                // Regular stories depend on setup stories
                if i == 1 || prd.stories[i - 1].title.to_lowercase().contains("setup") {
                    prd.stories[i].dependencies = vec![story_ids[i - 1].clone()];
                }
            }
        }
    }

    /// Build a prompt for LLM-based PRD generation
    pub fn build_generation_prompt(&self, request: &PrdGenerationRequest) -> String {
        let mut prompt = String::new();

        prompt.push_str("Generate a Product Requirements Document (PRD) in JSON format.\n\n");
        prompt.push_str(&format!("## Feature: {}\n\n", request.feature_title));
        prompt.push_str(&format!(
            "### Description:\n{}\n\n",
            request.feature_description
        ));

        if let Some(ctx) = &request.design_context {
            if !ctx.is_empty() {
                prompt.push_str("### Architectural Context:\n\n");

                if !ctx.components.is_empty() {
                    prompt.push_str("**Components:**\n");
                    for comp in &ctx.components {
                        prompt.push_str(&format!("- {}\n", comp));
                    }
                    prompt.push('\n');
                }

                if !ctx.patterns.is_empty() {
                    prompt.push_str("**Design Patterns:**\n");
                    for pattern in &ctx.patterns {
                        prompt.push_str(&format!("- {}\n", pattern));
                    }
                    prompt.push('\n');
                }

                if !ctx.decisions.is_empty() {
                    prompt.push_str("**Architectural Decisions:**\n");
                    for decision in &ctx.decisions {
                        prompt.push_str(&format!("- {}\n", decision));
                    }
                    prompt.push('\n');
                }
            }
        }

        if let Some(additional) = &request.additional_context {
            prompt.push_str("### Additional Context:\n");
            prompt.push_str(additional);
            prompt.push_str("\n\n");
        }

        prompt.push_str(&format!(
            "Generate {} to {} user stories with:\n",
            self.config.min_stories, self.config.max_stories
        ));
        prompt.push_str("- Clear, actionable titles\n");
        prompt.push_str("- Appropriate priorities (critical, high, medium, low)\n");
        prompt.push_str("- Story types (feature, bugfix, refactor, test)\n");
        prompt.push_str("- Dependencies between stories where applicable\n");
        if self.config.generate_acceptance_criteria {
            prompt.push_str("- Acceptance criteria for each story\n");
        }

        prompt.push_str("\nOutput valid JSON matching the PRD schema.\n");

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prd_generator_config_defaults() {
        let config = PrdGeneratorConfig::default();
        assert_eq!(config.max_stories, 7);
        assert_eq!(config.min_stories, 3);
        assert!(config.auto_infer_dependencies);
        assert!(config.generate_acceptance_criteria);
    }

    #[test]
    fn test_design_doc_context_empty() {
        let ctx = DesignDocContext::default();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_create_story_from_line() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config);

        let story = generator.create_story_from_line("S001", "Implement core module");
        assert_eq!(story.id, "S001");
        assert_eq!(story.title, "Implement core module");
        assert_eq!(story.story_type, Some(StoryType::Feature));

        let test_story = generator.create_story_from_line("S002", "Add unit tests");
        assert_eq!(test_story.story_type, Some(StoryType::Test));

        let critical_story = generator.create_story_from_line("S003", "Critical security fix");
        assert_eq!(critical_story.priority, Priority::Critical);
    }

    #[test]
    fn test_extract_stories_numbered() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config);

        let description = r#"
1. Setup core types
2. Implement main logic
3. Add validation
4. Write tests
"#;

        let stories = generator.extract_stories_from_description(description, &None);
        assert_eq!(stories.len(), 4);
        assert_eq!(stories[0].title, "Setup core types");
        assert_eq!(stories[1].title, "Implement main logic");
    }

    #[test]
    fn test_extract_stories_bullets() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config);

        let description = r#"
- Create data models
- Implement API endpoints
- Add authentication
"#;

        let stories = generator.extract_stories_from_description(description, &None);
        assert_eq!(stories.len(), 3);
    }

    #[test]
    fn test_infer_dependencies() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config);

        let mut prd = Prd::new("Test");
        prd.add_story(Story::new("S001", "Setup core structures"));
        prd.add_story(Story::new("S002", "Implement features"));

        let mut test_story = Story::new("S003", "Add tests");
        test_story.story_type = Some(StoryType::Test);
        prd.add_story(test_story);

        generator.infer_dependencies(&mut prd);

        // Test story should depend on non-test stories
        assert!(!prd.stories[2].dependencies.is_empty());
    }

    #[test]
    fn test_build_generation_prompt() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config);

        let request = PrdGenerationRequest {
            feature_id: "feature-001".to_string(),
            feature_title: "User Authentication".to_string(),
            feature_description: "Implement user login and registration".to_string(),
            design_context: Some(DesignDocContext {
                components: vec!["AuthService: Handles authentication".to_string()],
                patterns: vec!["Repository: Data access pattern".to_string()],
                decisions: vec![],
                mapping_description: None,
            }),
            additional_context: None,
        };

        let prompt = generator.build_generation_prompt(&request);
        assert!(prompt.contains("User Authentication"));
        assert!(prompt.contains("AuthService"));
        assert!(prompt.contains("Repository"));
    }

    #[tokio::test]
    async fn test_generate_default_prd() {
        let config = PrdGeneratorConfig {
            min_stories: 2,
            max_stories: 5,
            ..Default::default()
        };
        let generator = PrdGenerator::new(config);

        let request = PrdGenerationRequest {
            feature_id: "feature-test".to_string(),
            feature_title: "Test Feature".to_string(),
            feature_description: r#"
1. Create models
2. Implement logic
3. Add tests
"#
            .to_string(),
            design_context: None,
            additional_context: None,
        };

        let result = generator.generate(request).await.unwrap();
        assert_eq!(result.prd.stories.len(), 3);
        assert!(result.metadata.story_count >= 2);
    }

    #[tokio::test]
    async fn test_custom_generator() {
        let config = PrdGeneratorConfig::default();
        let generator = PrdGenerator::new(config).with_generator(Arc::new(|req| {
            Box::pin(async move {
                let mut prd = Prd::new(&req.feature_title);
                prd.add_story(Story::new("S001", "Custom generated story"));
                Ok(prd)
            })
        }));

        let request = PrdGenerationRequest {
            feature_id: "feature-test".to_string(),
            feature_title: "Test".to_string(),
            feature_description: "Test".to_string(),
            design_context: None,
            additional_context: None,
        };

        let result = generator.generate(request).await.unwrap();
        assert_eq!(result.prd.stories.len(), 1);
        assert_eq!(result.prd.stories[0].title, "Custom generated story");
    }
}
