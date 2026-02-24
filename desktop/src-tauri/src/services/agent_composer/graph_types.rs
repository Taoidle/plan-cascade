//! Graph Workflow Types
//!
//! Defines the data structures for graph-based workflow execution:
//! - `GraphWorkflow`: A directed graph of agent nodes connected by edges
//! - `GraphNode`: A node containing an AgentStep with optional UI position
//! - `Edge`: Direct or conditional connections between nodes
//! - `StateSchema`: Channel configuration and reducers for graph state
//! - `Reducer`: Operations for combining state updates (Overwrite, Append, Sum)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::AgentStep;

// ============================================================================
// Graph Workflow
// ============================================================================

/// A graph-based workflow definition.
///
/// Contains a set of nodes connected by edges, with an entry point and
/// a state schema defining how state channels are managed during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphWorkflow {
    /// Workflow name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Map of node IDs to graph nodes.
    pub nodes: HashMap<String, GraphNode>,
    /// Edges connecting nodes.
    pub edges: Vec<Edge>,
    /// ID of the entry node where execution starts.
    pub entry_node: String,
    /// State schema defining channels and reducers.
    #[serde(default)]
    pub state_schema: StateSchema,
}

// ============================================================================
// Graph Node
// ============================================================================

/// A node in a graph workflow.
///
/// Each node wraps an `AgentStep` (LLM, Sequential, Parallel, Conditional)
/// and optionally includes a position for UI layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique node identifier.
    pub id: String,
    /// The agent step to execute at this node.
    pub agent_step: AgentStep,
    /// Optional UI position for visual layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<NodePosition>,
    /// If true, execution will pause before this node (creating a checkpoint).
    #[serde(default)]
    pub interrupt_before: bool,
    /// If true, execution will pause after this node (creating a checkpoint).
    #[serde(default)]
    pub interrupt_after: bool,
}

/// UI position for a graph node (x, y coordinates).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

// ============================================================================
// Edge
// ============================================================================

/// An edge connecting two nodes in a graph workflow.
///
/// Edges can be direct (unconditional) or conditional (routing based on
/// shared state values).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "edge_type", rename_all = "snake_case")]
pub enum Edge {
    /// Unconditional edge from one node to another.
    Direct { from: String, to: String },
    /// Conditional edge that routes based on a state key's value.
    Conditional {
        from: String,
        /// Configuration for the condition evaluation.
        condition: ConditionConfig,
        /// Map of condition values to target node IDs.
        branches: HashMap<String, String>,
        /// Optional default target if no branch matches.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_branch: Option<String>,
    },
}

/// Configuration for a conditional edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionConfig {
    /// The shared state key to look up for condition evaluation.
    pub condition_key: String,
}

// ============================================================================
// State Schema
// ============================================================================

/// Schema defining how graph state is managed during execution.
///
/// Channels define the state keys and their types/defaults.
/// Reducers define how state updates are combined.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSchema {
    /// Channel configurations keyed by channel name.
    #[serde(default)]
    pub channels: HashMap<String, ChannelConfig>,
    /// Reducer configurations keyed by channel name.
    #[serde(default)]
    pub reducers: HashMap<String, Reducer>,
}

/// Configuration for a state channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Type of the channel (e.g., "string", "number", "array").
    pub channel_type: String,
    /// Optional default value for the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
}

/// Reducer operations for combining state updates.
///
/// When a node emits a `StateUpdate` event, the reducer determines
/// how the new value is merged with the existing channel value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Reducer {
    /// Replace the existing value entirely.
    Overwrite,
    /// Append to an array (creates array if not present).
    Append,
    /// Add numeric values together.
    Sum,
}

// ============================================================================
// Persistence types
// ============================================================================

/// Summary information about a graph workflow (for list views).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphWorkflowInfo {
    /// Workflow identifier.
    pub id: String,
    /// Workflow name.
    pub name: String,
    /// Number of nodes.
    pub node_count: usize,
    /// Number of edges.
    pub edge_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::{AgentConfig, AgentStep, LlmStepConfig};

    fn sample_llm_step(name: &str) -> AgentStep {
        AgentStep::LlmStep(LlmStepConfig {
            name: name.to_string(),
            instruction: None,
            model: None,
            tools: None,
            config: AgentConfig::default(),
        })
    }

    #[test]
    fn test_graph_workflow_serialization_roundtrip() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "node-a".to_string(),
            GraphNode {
                id: "node-a".to_string(),
                agent_step: sample_llm_step("agent-a"),
                position: Some(NodePosition { x: 100.0, y: 200.0 }),
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "node-b".to_string(),
            GraphNode {
                id: "node-b".to_string(),
                agent_step: sample_llm_step("agent-b"),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        let workflow = GraphWorkflow {
            name: "Test Workflow".to_string(),
            description: Some("A test graph".to_string()),
            nodes,
            edges: vec![Edge::Direct {
                from: "node-a".to_string(),
                to: "node-b".to_string(),
            }],
            entry_node: "node-a".to_string(),
            state_schema: StateSchema::default(),
        };

        let json = serde_json::to_string_pretty(&workflow).unwrap();
        let parsed: GraphWorkflow = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "Test Workflow");
        assert_eq!(parsed.description, Some("A test graph".to_string()));
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.entry_node, "node-a");
    }

    #[test]
    fn test_graph_node_serialization() {
        let node = GraphNode {
            id: "n1".to_string(),
            agent_step: sample_llm_step("my-agent"),
            position: Some(NodePosition { x: 50.0, y: 75.0 }),
            interrupt_before: false,
            interrupt_after: false,
        };

        let json = serde_json::to_string(&node).unwrap();
        let parsed: GraphNode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "n1");
        assert!(parsed.position.is_some());
        let pos = parsed.position.unwrap();
        assert!((pos.x - 50.0).abs() < f64::EPSILON);
        assert!((pos.y - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_edge_direct_serialization() {
        let edge = Edge::Direct {
            from: "a".to_string(),
            to: "b".to_string(),
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"edge_type\":\"direct\""));

        let parsed: Edge = serde_json::from_str(&json).unwrap();
        match parsed {
            Edge::Direct { from, to } => {
                assert_eq!(from, "a");
                assert_eq!(to, "b");
            }
            _ => panic!("Expected Direct edge"),
        }
    }

    #[test]
    fn test_edge_conditional_serialization() {
        let mut branches = HashMap::new();
        branches.insert("yes".to_string(), "node-yes".to_string());
        branches.insert("no".to_string(), "node-no".to_string());

        let edge = Edge::Conditional {
            from: "router".to_string(),
            condition: ConditionConfig {
                condition_key: "decision".to_string(),
            },
            branches,
            default_branch: Some("node-default".to_string()),
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"edge_type\":\"conditional\""));

        let parsed: Edge = serde_json::from_str(&json).unwrap();
        match parsed {
            Edge::Conditional {
                from,
                condition,
                branches,
                default_branch,
            } => {
                assert_eq!(from, "router");
                assert_eq!(condition.condition_key, "decision");
                assert_eq!(branches.len(), 2);
                assert_eq!(default_branch, Some("node-default".to_string()));
            }
            _ => panic!("Expected Conditional edge"),
        }
    }

    #[test]
    fn test_state_schema_serialization() {
        let mut channels = HashMap::new();
        channels.insert(
            "counter".to_string(),
            ChannelConfig {
                channel_type: "number".to_string(),
                default_value: Some(serde_json::json!(0)),
            },
        );
        channels.insert(
            "items".to_string(),
            ChannelConfig {
                channel_type: "array".to_string(),
                default_value: Some(serde_json::json!([])),
            },
        );

        let mut reducers = HashMap::new();
        reducers.insert("counter".to_string(), Reducer::Sum);
        reducers.insert("items".to_string(), Reducer::Append);

        let schema = StateSchema { channels, reducers };

        let json = serde_json::to_string(&schema).unwrap();
        let parsed: StateSchema = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.channels.len(), 2);
        assert_eq!(parsed.reducers.len(), 2);
        assert_eq!(parsed.reducers.get("counter"), Some(&Reducer::Sum));
        assert_eq!(parsed.reducers.get("items"), Some(&Reducer::Append));
    }

    #[test]
    fn test_reducer_serialization() {
        let overwrite = Reducer::Overwrite;
        let append = Reducer::Append;
        let sum = Reducer::Sum;

        assert_eq!(serde_json::to_string(&overwrite).unwrap(), "\"overwrite\"");
        assert_eq!(serde_json::to_string(&append).unwrap(), "\"append\"");
        assert_eq!(serde_json::to_string(&sum).unwrap(), "\"sum\"");

        // Round-trip
        let parsed: Reducer = serde_json::from_str("\"overwrite\"").unwrap();
        assert_eq!(parsed, Reducer::Overwrite);
        let parsed: Reducer = serde_json::from_str("\"append\"").unwrap();
        assert_eq!(parsed, Reducer::Append);
        let parsed: Reducer = serde_json::from_str("\"sum\"").unwrap();
        assert_eq!(parsed, Reducer::Sum);
    }

    #[test]
    fn test_channel_config_without_default() {
        let config = ChannelConfig {
            channel_type: "string".to_string(),
            default_value: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("default_value"));

        let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.channel_type, "string");
        assert!(parsed.default_value.is_none());
    }

    #[test]
    fn test_graph_workflow_info_serialization() {
        let info = GraphWorkflowInfo {
            id: "wf-1".to_string(),
            name: "My Workflow".to_string(),
            node_count: 5,
            edge_count: 4,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: GraphWorkflowInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "wf-1");
        assert_eq!(parsed.name, "My Workflow");
        assert_eq!(parsed.node_count, 5);
        assert_eq!(parsed.edge_count, 4);
    }

    #[test]
    fn test_state_schema_default() {
        let schema = StateSchema::default();
        assert!(schema.channels.is_empty());
        assert!(schema.reducers.is_empty());
    }

    #[test]
    fn test_condition_config_serialization() {
        let config = ConditionConfig {
            condition_key: "status".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ConditionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.condition_key, "status");
    }

    #[test]
    fn test_node_position_serialization() {
        let pos = NodePosition {
            x: 123.45,
            y: 678.9,
        };
        let json = serde_json::to_string(&pos).unwrap();
        let parsed: NodePosition = serde_json::from_str(&json).unwrap();
        assert!((parsed.x - 123.45).abs() < f64::EPSILON);
        assert!((parsed.y - 678.9).abs() < f64::EPSILON);
    }
}
