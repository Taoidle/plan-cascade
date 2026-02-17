//! AgentRegistry — named agent instance management and pipeline construction
//!
//! The registry stores `Arc<dyn Agent>` instances by name and provides a
//! `build_from_pipeline` method that recursively constructs agent compositions
//! from serializable `AgentPipeline` definitions.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::conditional::{ConditionalAgent, ConditionFn};
use super::llm_agent::LlmAgent;
use super::parallel::ParallelAgent;
use super::sequential::SequentialAgent;
use super::types::{Agent, AgentPipeline, AgentStep, LlmStepConfig};
use crate::utils::error::{AppError, AppResult};

/// Information about a registered agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent name.
    pub name: String,
    /// Agent description.
    pub description: String,
}

/// Registry for composable agent instances.
///
/// Stores `Arc<dyn Agent>` by name and provides construction from
/// serializable pipeline definitions.
pub struct ComposerRegistry {
    agents: HashMap<String, Arc<dyn Agent>>,
}

impl ComposerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register an agent by name.
    ///
    /// If an agent with this name already exists, it is replaced.
    pub fn register(&mut self, name: impl Into<String>, agent: Arc<dyn Agent>) {
        self.agents.insert(name.into(), agent);
    }

    /// Get an agent by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Agent>> {
        self.agents.get(name).cloned()
    }

    /// List all registered agents with their info.
    pub fn list(&self) -> Vec<AgentInfo> {
        self.agents
            .values()
            .map(|agent| AgentInfo {
                name: agent.name().to_string(),
                description: agent.description().to_string(),
            })
            .collect()
    }

    /// Remove an agent by name.
    pub fn remove(&mut self, name: &str) -> Option<Arc<dyn Agent>> {
        self.agents.remove(name)
    }

    /// Check if an agent is registered by name.
    pub fn contains(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    /// Get the number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Build an agent composition from a pipeline definition.
    ///
    /// Recursively constructs agents from the pipeline steps. For `LlmStep`,
    /// creates an `LlmAgent`. For `SequentialStep` and `ParallelStep`,
    /// recursively builds sub-steps. For `ConditionalStep`, creates a
    /// condition function based on the `condition_key`.
    pub fn build_from_pipeline(
        &self,
        pipeline: &AgentPipeline,
    ) -> AppResult<Arc<dyn Agent>> {
        if pipeline.steps.is_empty() {
            return Err(AppError::validation("Pipeline has no steps"));
        }

        // If there's only one step, build it directly
        if pipeline.steps.len() == 1 {
            return self.build_step(&pipeline.steps[0]);
        }

        // Multiple steps become a sequential agent
        let agents: Vec<Arc<dyn Agent>> = pipeline
            .steps
            .iter()
            .map(|step| self.build_step(step))
            .collect::<AppResult<Vec<_>>>()?;

        Ok(Arc::new(SequentialAgent::new(
            pipeline.name.clone(),
            agents,
        )))
    }

    /// Build a single agent step recursively.
    fn build_step(&self, step: &AgentStep) -> AppResult<Arc<dyn Agent>> {
        match step {
            AgentStep::LlmStep(config) => Ok(Arc::new(build_llm_agent(config))),
            AgentStep::SequentialStep { name, steps } => {
                let agents: Vec<Arc<dyn Agent>> = steps
                    .iter()
                    .map(|s| self.build_step(s))
                    .collect::<AppResult<Vec<_>>>()?;
                Ok(Arc::new(SequentialAgent::new(name.clone(), agents)))
            }
            AgentStep::ParallelStep { name, steps } => {
                let agents: Vec<Arc<dyn Agent>> = steps
                    .iter()
                    .map(|s| self.build_step(s))
                    .collect::<AppResult<Vec<_>>>()?;
                Ok(Arc::new(ParallelAgent::new(name.clone(), agents)))
            }
            AgentStep::ConditionalStep {
                name,
                condition_key,
                branches,
                default_branch,
            } => {
                let mut branch_agents: HashMap<String, Arc<dyn Agent>> = HashMap::new();
                for (key, branch_step) in branches {
                    branch_agents.insert(key.clone(), self.build_step(branch_step)?);
                }

                // Build the condition function from the condition_key
                let key = condition_key.clone();
                let condition: ConditionFn = Box::new(move |state| {
                    state
                        .get(&key)
                        .and_then(|v| {
                            // Try to extract a string; fall back to JSON serialization
                            v.as_str()
                                .map(|s| s.to_string())
                                .or_else(|| serde_json::to_string(v).ok())
                        })
                        .unwrap_or_default()
                });

                let mut cond_agent =
                    ConditionalAgent::new(name.clone(), condition, branch_agents);

                if let Some(default_step) = default_branch {
                    let default_agent = self.build_step(default_step)?;
                    cond_agent = cond_agent.with_default(default_agent);
                }

                Ok(Arc::new(cond_agent))
            }
        }
    }
}

impl Default for ComposerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build an LlmAgent from a step configuration.
fn build_llm_agent(config: &LlmStepConfig) -> LlmAgent {
    let mut agent = LlmAgent::new(config.name.clone()).with_config(config.config.clone());

    if let Some(ref instruction) = config.instruction {
        agent = agent.with_instruction(instruction.clone());
    }
    if let Some(ref model) = config.model {
        agent = agent.with_model(model.clone());
    }
    if let Some(ref tools) = config.tools {
        agent = agent.with_tools(tools.clone());
    }

    agent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::*;
    use async_trait::async_trait;
    use futures_util::StreamExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Simple mock agent for registry tests.
    struct MockNamedAgent {
        name: String,
        desc: String,
    }

    impl MockNamedAgent {
        fn new(name: &str, desc: &str) -> Self {
            Self {
                name: name.to_string(),
                desc: desc.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for MockNamedAgent {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            &self.desc
        }
        async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
            let output = self.name.clone();
            let stream = futures_util::stream::iter(vec![Ok(AgentEvent::Done {
                output: Some(output),
            })]);
            Ok(Box::pin(stream))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ComposerRegistry::new();
        let agent = Arc::new(MockNamedAgent::new("test-agent", "A test agent"));

        registry.register("test-agent", agent.clone());

        assert!(registry.contains("test-agent"));
        assert_eq!(registry.len(), 1);

        let fetched = registry.get("test-agent").unwrap();
        assert_eq!(fetched.name(), "test-agent");
    }

    #[test]
    fn test_registry_list() {
        let mut registry = ComposerRegistry::new();
        registry.register(
            "agent-1",
            Arc::new(MockNamedAgent::new("agent-1", "First")),
        );
        registry.register(
            "agent-2",
            Arc::new(MockNamedAgent::new("agent-2", "Second")),
        );

        let list = registry.list();
        assert_eq!(list.len(), 2);

        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"agent-1"));
        assert!(names.contains(&"agent-2"));
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = ComposerRegistry::new();
        registry.register(
            "removable",
            Arc::new(MockNamedAgent::new("removable", "To remove")),
        );

        assert!(registry.contains("removable"));
        let removed = registry.remove("removable");
        assert!(removed.is_some());
        assert!(!registry.contains("removable"));
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = ComposerRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_build_from_pipeline_single_llm_step() {
        let registry = ComposerRegistry::new();
        let pipeline = AgentPipeline {
            pipeline_id: "p-1".to_string(),
            name: "Test Pipeline".to_string(),
            description: None,
            steps: vec![AgentStep::LlmStep(LlmStepConfig {
                name: "llm-step".to_string(),
                instruction: Some("Be helpful".to_string()),
                model: None,
                tools: None,
                config: AgentConfig::default(),
            })],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let agent = registry.build_from_pipeline(&pipeline).unwrap();
        assert_eq!(agent.name(), "llm-step");
    }

    #[test]
    fn test_build_from_pipeline_sequential_steps() {
        let registry = ComposerRegistry::new();
        let pipeline = AgentPipeline {
            pipeline_id: "p-2".to_string(),
            name: "Sequential Pipeline".to_string(),
            description: None,
            steps: vec![
                AgentStep::LlmStep(LlmStepConfig {
                    name: "step-1".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                AgentStep::LlmStep(LlmStepConfig {
                    name: "step-2".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
            ],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let agent = registry.build_from_pipeline(&pipeline).unwrap();
        // Multiple steps should produce a SequentialAgent named after the pipeline
        assert_eq!(agent.name(), "Sequential Pipeline");
    }

    #[test]
    fn test_build_from_pipeline_nested_parallel() {
        let registry = ComposerRegistry::new();
        let pipeline = AgentPipeline {
            pipeline_id: "p-3".to_string(),
            name: "Nested Pipeline".to_string(),
            description: None,
            steps: vec![AgentStep::ParallelStep {
                name: "parallel-step".to_string(),
                steps: vec![
                    AgentStep::LlmStep(LlmStepConfig {
                        name: "worker-a".to_string(),
                        instruction: None,
                        model: None,
                        tools: None,
                        config: AgentConfig::default(),
                    }),
                    AgentStep::LlmStep(LlmStepConfig {
                        name: "worker-b".to_string(),
                        instruction: None,
                        model: None,
                        tools: None,
                        config: AgentConfig::default(),
                    }),
                ],
            }],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let agent = registry.build_from_pipeline(&pipeline).unwrap();
        assert_eq!(agent.name(), "parallel-step");
    }

    #[test]
    fn test_build_from_pipeline_conditional() {
        let registry = ComposerRegistry::new();
        let mut branches = HashMap::new();
        branches.insert(
            "fast".to_string(),
            AgentStep::LlmStep(LlmStepConfig {
                name: "fast-agent".to_string(),
                instruction: None,
                model: None,
                tools: None,
                config: AgentConfig::default(),
            }),
        );

        let pipeline = AgentPipeline {
            pipeline_id: "p-4".to_string(),
            name: "Conditional Pipeline".to_string(),
            description: None,
            steps: vec![AgentStep::ConditionalStep {
                name: "router".to_string(),
                condition_key: "mode".to_string(),
                branches,
                default_branch: Some(Box::new(AgentStep::LlmStep(LlmStepConfig {
                    name: "default-agent".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }))),
            }],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let agent = registry.build_from_pipeline(&pipeline).unwrap();
        assert_eq!(agent.name(), "router");
    }

    #[test]
    fn test_build_from_empty_pipeline_returns_error() {
        let registry = ComposerRegistry::new();
        let pipeline = AgentPipeline {
            pipeline_id: "p-empty".to_string(),
            name: "Empty".to_string(),
            description: None,
            steps: vec![],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let result = registry.build_from_pipeline(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_llm_agent_with_all_options() {
        let config = LlmStepConfig {
            name: "full-agent".to_string(),
            instruction: Some("System prompt".to_string()),
            model: Some("claude-3-opus".to_string()),
            tools: Some(vec!["read_file".to_string()]),
            config: AgentConfig {
                max_iterations: 10,
                ..Default::default()
            },
        };

        let agent = build_llm_agent(&config);
        assert_eq!(agent.name(), "full-agent");
    }
}
