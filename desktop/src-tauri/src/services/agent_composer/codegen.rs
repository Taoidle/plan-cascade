//! Code Generation for Graph Workflows
//!
//! Generates TypeScript and Rust source code from a `GraphWorkflow` definition.
//! Both generators sort nodes by ID for deterministic, snapshot-testable output.
//!
//! - `generate_typescript()` — produces a self-contained TypeScript module with
//!   agent definitions, graph construction, conditional routing, and invocation.
//! - `generate_rust()` — produces Rust source with `plan_cascade_core` types,
//!   HashMap construction, Vec edges, and interrupt-point comments.

use std::fmt::Write as FmtWrite;

use super::graph_types::{ChannelConfig, Edge, GraphWorkflow, Reducer, StateSchema};
use super::types::AgentStep;

// ============================================================================
// TypeScript Code Generation
// ============================================================================

/// Generate TypeScript source code from a `GraphWorkflow`.
///
/// The output includes:
/// - An import statement for `plan-cascade` types
/// - Agent definitions (const declarations) for each node, sorted by node ID
/// - Graph construction with `addNode` calls
/// - Edge wiring with `addEdge` or `addConditionalEdge`
/// - Conditional routing logic via switch statements
/// - State schema channel and reducer declarations
/// - An `invoke()` call on the compiled graph
pub fn generate_typescript(workflow: &GraphWorkflow) -> String {
    let mut out = String::new();

    // Import statement
    writeln!(
        out,
        "import {{ GraphWorkflow, AgentNode, Edge, StateSchema }} from \"plan-cascade\";\n"
    )
    .unwrap();

    // Workflow description comment
    writeln!(out, "// Workflow: {}", workflow.name).unwrap();
    if let Some(ref desc) = workflow.description {
        writeln!(out, "// {}", desc).unwrap();
    }
    writeln!(out).unwrap();

    // State schema
    if !workflow.state_schema.channels.is_empty() || !workflow.state_schema.reducers.is_empty() {
        write_typescript_state_schema(&mut out, &workflow.state_schema);
        writeln!(out).unwrap();
    }

    // Agent definitions — sorted by node ID for deterministic output
    let mut node_ids: Vec<&String> = workflow.nodes.keys().collect();
    node_ids.sort();

    writeln!(out, "// --- Agent Definitions ---").unwrap();
    for node_id in &node_ids {
        if let Some(node) = workflow.nodes.get(*node_id) {
            write_typescript_agent_definition(&mut out, node_id, &node.agent_step);
        }
    }
    writeln!(out).unwrap();

    // Graph construction
    writeln!(out, "// --- Graph Construction ---").unwrap();
    writeln!(
        out,
        "const graph = new GraphWorkflow(\"{}\");",
        workflow.name
    )
    .unwrap();
    writeln!(out).unwrap();

    // Add nodes
    for node_id in &node_ids {
        if let Some(node) = workflow.nodes.get(*node_id) {
            let interrupt_comment = if node.interrupt_before || node.interrupt_after {
                let mut parts = Vec::new();
                if node.interrupt_before {
                    parts.push("before");
                }
                if node.interrupt_after {
                    parts.push("after");
                }
                format!(" // interrupt: {}", parts.join(", "))
            } else {
                String::new()
            };
            writeln!(
                out,
                "graph.addNode(\"{}\", {});{}",
                node_id,
                agent_step_var_name(node_id),
                interrupt_comment
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // Set entry node
    writeln!(out, "graph.setEntryNode(\"{}\");", workflow.entry_node).unwrap();
    writeln!(out).unwrap();

    // Add edges
    writeln!(out, "// --- Edges ---").unwrap();
    for edge in &workflow.edges {
        match edge {
            Edge::Direct { from, to } => {
                writeln!(out, "graph.addEdge(\"{}\", \"{}\");", from, to).unwrap();
            }
            Edge::Conditional {
                from,
                condition,
                branches,
                default_branch,
            } => {
                writeln!(
                    out,
                    "graph.addConditionalEdge(\"{}\", (state) => {{",
                    from
                )
                .unwrap();
                writeln!(
                    out,
                    "  switch (state[\"{}\"])",
                    condition.condition_key
                )
                .unwrap();
                writeln!(out, "  {{").unwrap();

                // Sort branch keys for deterministic output
                let mut branch_keys: Vec<&String> = branches.keys().collect();
                branch_keys.sort();
                for key in branch_keys {
                    if let Some(target) = branches.get(key) {
                        writeln!(out, "    case \"{}\": return \"{}\";", key, target).unwrap();
                    }
                }
                if let Some(ref default) = default_branch {
                    writeln!(out, "    default: return \"{}\";", default).unwrap();
                } else {
                    writeln!(out, "    default: return undefined;").unwrap();
                }
                writeln!(out, "  }}").unwrap();
                writeln!(out, "}});").unwrap();
            }
        }
    }
    writeln!(out).unwrap();

    // Invocation
    writeln!(out, "// --- Invoke ---").unwrap();
    writeln!(out, "const app = graph.compile();").unwrap();
    writeln!(out, "const result = await app.invoke({{}});").unwrap();

    out
}

/// Write a TypeScript agent definition for a node.
fn write_typescript_agent_definition(out: &mut String, node_id: &str, step: &AgentStep) {
    let var_name = agent_step_var_name(node_id);
    match step {
        AgentStep::LlmStep(config) => {
            writeln!(out, "const {} = new AgentNode({{", var_name).unwrap();
            writeln!(out, "  name: \"{}\",", config.name).unwrap();
            writeln!(out, "  type: \"llm\",").unwrap();
            if let Some(ref instruction) = config.instruction {
                writeln!(out, "  instruction: \"{}\",", escape_ts_string(instruction)).unwrap();
            }
            if let Some(ref model) = config.model {
                writeln!(out, "  model: \"{}\",", model).unwrap();
            }
            if let Some(ref tools) = config.tools {
                let tools_str: Vec<String> = tools.iter().map(|t| format!("\"{}\"", t)).collect();
                writeln!(out, "  tools: [{}],", tools_str.join(", ")).unwrap();
            }
            writeln!(out, "}});").unwrap();
        }
        AgentStep::SequentialStep { name, steps } => {
            writeln!(out, "const {} = new AgentNode({{", var_name).unwrap();
            writeln!(out, "  name: \"{}\",", name).unwrap();
            writeln!(out, "  type: \"sequential\",").unwrap();
            writeln!(out, "  steps: [{}],", steps_summary(steps)).unwrap();
            writeln!(out, "}});").unwrap();
        }
        AgentStep::ParallelStep { name, steps } => {
            writeln!(out, "const {} = new AgentNode({{", var_name).unwrap();
            writeln!(out, "  name: \"{}\",", name).unwrap();
            writeln!(out, "  type: \"parallel\",").unwrap();
            writeln!(out, "  steps: [{}],", steps_summary(steps)).unwrap();
            writeln!(out, "}});").unwrap();
        }
        AgentStep::ConditionalStep {
            name,
            condition_key,
            branches,
            default_branch: _,
        } => {
            writeln!(out, "const {} = new AgentNode({{", var_name).unwrap();
            writeln!(out, "  name: \"{}\",", name).unwrap();
            writeln!(out, "  type: \"conditional\",").unwrap();
            writeln!(out, "  conditionKey: \"{}\",", condition_key).unwrap();
            let mut branch_keys: Vec<&String> = branches.keys().collect();
            branch_keys.sort();
            let branch_strs: Vec<String> = branch_keys
                .iter()
                .map(|k| format!("\"{}\"", k))
                .collect();
            writeln!(out, "  branches: [{}],", branch_strs.join(", ")).unwrap();
            writeln!(out, "}});").unwrap();
        }
        AgentStep::LoopStep {
            name,
            condition_key,
            max_iterations,
            step: _,
        } => {
            writeln!(out, "const {} = new AgentNode({{", var_name).unwrap();
            writeln!(out, "  name: \"{}\",", name).unwrap();
            writeln!(out, "  type: \"loop\",").unwrap();
            writeln!(out, "  conditionKey: \"{}\",", condition_key).unwrap();
            writeln!(out, "  maxIterations: {},", max_iterations).unwrap();
            writeln!(out, "}});").unwrap();
        }
    }
}

/// Write TypeScript state schema declarations.
fn write_typescript_state_schema(out: &mut String, schema: &StateSchema) {
    writeln!(out, "// --- State Schema ---").unwrap();
    writeln!(out, "const stateSchema: StateSchema = {{").unwrap();

    if !schema.channels.is_empty() {
        writeln!(out, "  channels: {{").unwrap();
        let mut channel_keys: Vec<&String> = schema.channels.keys().collect();
        channel_keys.sort();
        for key in channel_keys {
            if let Some(channel) = schema.channels.get(key) {
                write_typescript_channel(out, key, channel);
            }
        }
        writeln!(out, "  }},").unwrap();
    }

    if !schema.reducers.is_empty() {
        writeln!(out, "  reducers: {{").unwrap();
        let mut reducer_keys: Vec<&String> = schema.reducers.keys().collect();
        reducer_keys.sort();
        for key in reducer_keys {
            if let Some(reducer) = schema.reducers.get(key) {
                let reducer_str = match reducer {
                    Reducer::Overwrite => "\"overwrite\"",
                    Reducer::Append => "\"append\"",
                    Reducer::Sum => "\"sum\"",
                };
                writeln!(out, "    \"{}\": {},", key, reducer_str).unwrap();
            }
        }
        writeln!(out, "  }},").unwrap();
    }

    writeln!(out, "}};").unwrap();
}

/// Write a single TypeScript channel config.
fn write_typescript_channel(out: &mut String, key: &str, channel: &ChannelConfig) {
    if let Some(ref default_value) = channel.default_value {
        writeln!(
            out,
            "    \"{}\": {{ type: \"{}\", default: {} }},",
            key,
            channel.channel_type,
            serde_json::to_string(default_value).unwrap_or_default()
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "    \"{}\": {{ type: \"{}\" }},",
            key, channel.channel_type
        )
        .unwrap();
    }
}

// ============================================================================
// Rust Code Generation
// ============================================================================

/// Generate Rust source code from a `GraphWorkflow`.
///
/// The output includes:
/// - `use` statements for `plan_cascade_core` types
/// - `HashMap` construction for nodes, sorted by node ID
/// - `Vec<Edge>` construction for edges
/// - `StateSchema` with channels and reducers
/// - `GraphWorkflow` struct initialization
/// - Comments marking interrupt points (interrupt_before / interrupt_after)
pub fn generate_rust(workflow: &GraphWorkflow) -> String {
    let mut out = String::new();

    // Use statements
    writeln!(out, "use std::collections::HashMap;").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "use plan_cascade_core::graph::{{GraphWorkflow, GraphNode, Edge, ConditionConfig}};"
    )
    .unwrap();
    writeln!(
        out,
        "use plan_cascade_core::graph::{{StateSchema, ChannelConfig, Reducer}};"
    )
    .unwrap();
    writeln!(out, "use plan_cascade_core::agent::AgentStep;").unwrap();
    writeln!(out).unwrap();

    // Workflow description comment
    writeln!(out, "/// Workflow: {}", workflow.name).unwrap();
    if let Some(ref desc) = workflow.description {
        writeln!(out, "/// {}", desc).unwrap();
    }
    writeln!(out, "pub fn build_workflow() -> GraphWorkflow {{").unwrap();

    // Build nodes HashMap — sorted by ID for deterministic output
    let mut node_ids: Vec<&String> = workflow.nodes.keys().collect();
    node_ids.sort();

    writeln!(out, "    let mut nodes = HashMap::new();").unwrap();
    for node_id in &node_ids {
        if let Some(node) = workflow.nodes.get(*node_id) {
            writeln!(out).unwrap();

            // Interrupt-point comments
            if node.interrupt_before {
                writeln!(
                    out,
                    "    // INTERRUPT: pause before node \"{}\"",
                    node_id
                )
                .unwrap();
            }

            writeln!(out, "    nodes.insert(\"{}\".to_string(), GraphNode {{", node_id).unwrap();
            writeln!(out, "        id: \"{}\".to_string(),", node_id).unwrap();
            write_rust_agent_step(&mut out, &node.agent_step, "        ");
            writeln!(out, "        position: None,").unwrap();
            writeln!(
                out,
                "        interrupt_before: {},",
                node.interrupt_before
            )
            .unwrap();
            writeln!(out, "        interrupt_after: {},", node.interrupt_after).unwrap();
            writeln!(out, "    }});").unwrap();

            if node.interrupt_after {
                writeln!(
                    out,
                    "    // INTERRUPT: pause after node \"{}\"",
                    node_id
                )
                .unwrap();
            }
        }
    }
    writeln!(out).unwrap();

    // Build edges Vec
    writeln!(out, "    let edges = vec![").unwrap();
    for edge in &workflow.edges {
        match edge {
            Edge::Direct { from, to } => {
                writeln!(out, "        Edge::Direct {{").unwrap();
                writeln!(out, "            from: \"{}\".to_string(),", from).unwrap();
                writeln!(out, "            to: \"{}\".to_string(),", to).unwrap();
                writeln!(out, "        }},").unwrap();
            }
            Edge::Conditional {
                from,
                condition,
                branches,
                default_branch,
            } => {
                writeln!(out, "        Edge::Conditional {{").unwrap();
                writeln!(out, "            from: \"{}\".to_string(),", from).unwrap();
                writeln!(out, "            condition: ConditionConfig {{").unwrap();
                writeln!(
                    out,
                    "                condition_key: \"{}\".to_string(),",
                    condition.condition_key
                )
                .unwrap();
                writeln!(out, "            }},").unwrap();
                writeln!(out, "            branches: {{").unwrap();
                writeln!(out, "                let mut m = HashMap::new();").unwrap();

                // Sort branch keys for deterministic output
                let mut branch_keys: Vec<&String> = branches.keys().collect();
                branch_keys.sort();
                for key in branch_keys {
                    if let Some(target) = branches.get(key) {
                        writeln!(
                            out,
                            "                m.insert(\"{}\".to_string(), \"{}\".to_string());",
                            key, target
                        )
                        .unwrap();
                    }
                }
                writeln!(out, "                m").unwrap();
                writeln!(out, "            }},").unwrap();
                match default_branch {
                    Some(default) => {
                        writeln!(
                            out,
                            "            default_branch: Some(\"{}\".to_string()),",
                            default
                        )
                        .unwrap();
                    }
                    None => {
                        writeln!(out, "            default_branch: None,").unwrap();
                    }
                }
                writeln!(out, "        }},").unwrap();
            }
        }
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out).unwrap();

    // State schema
    write_rust_state_schema(&mut out, &workflow.state_schema);

    // GraphWorkflow struct initialization
    writeln!(out).unwrap();
    writeln!(out, "    GraphWorkflow {{").unwrap();
    writeln!(out, "        name: \"{}\".to_string(),", workflow.name).unwrap();
    match &workflow.description {
        Some(desc) => {
            writeln!(
                out,
                "        description: Some(\"{}\".to_string()),",
                desc
            )
            .unwrap();
        }
        None => {
            writeln!(out, "        description: None,").unwrap();
        }
    }
    writeln!(out, "        nodes,").unwrap();
    writeln!(out, "        edges,").unwrap();
    writeln!(
        out,
        "        entry_node: \"{}\".to_string(),",
        workflow.entry_node
    )
    .unwrap();
    writeln!(out, "        state_schema,").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    out
}

/// Write a Rust `AgentStep` expression.
fn write_rust_agent_step(out: &mut String, step: &AgentStep, indent: &str) {
    match step {
        AgentStep::LlmStep(config) => {
            writeln!(out, "{}agent_step: AgentStep::LlmStep(LlmStepConfig {{", indent).unwrap();
            writeln!(out, "{}    name: \"{}\".to_string(),", indent, config.name).unwrap();
            match &config.instruction {
                Some(instruction) => {
                    writeln!(
                        out,
                        "{}    instruction: Some(\"{}\".to_string()),",
                        indent, instruction
                    )
                    .unwrap();
                }
                None => {
                    writeln!(out, "{}    instruction: None,", indent).unwrap();
                }
            }
            match &config.model {
                Some(model) => {
                    writeln!(
                        out,
                        "{}    model: Some(\"{}\".to_string()),",
                        indent, model
                    )
                    .unwrap();
                }
                None => {
                    writeln!(out, "{}    model: None,", indent).unwrap();
                }
            }
            writeln!(out, "{}    tools: None,", indent).unwrap();
            writeln!(out, "{}    config: AgentConfig::default(),", indent).unwrap();
            writeln!(out, "{}}}),", indent).unwrap();
        }
        AgentStep::SequentialStep { name, steps } => {
            writeln!(
                out,
                "{}agent_step: AgentStep::SequentialStep {{",
                indent
            )
            .unwrap();
            writeln!(out, "{}    name: \"{}\".to_string(),", indent, name).unwrap();
            writeln!(out, "{}    steps: vec![/* {} sub-steps */],", indent, steps.len()).unwrap();
            writeln!(out, "{}}},", indent).unwrap();
        }
        AgentStep::ParallelStep { name, steps } => {
            writeln!(out, "{}agent_step: AgentStep::ParallelStep {{", indent).unwrap();
            writeln!(out, "{}    name: \"{}\".to_string(),", indent, name).unwrap();
            writeln!(out, "{}    steps: vec![/* {} sub-steps */],", indent, steps.len()).unwrap();
            writeln!(out, "{}}},", indent).unwrap();
        }
        AgentStep::ConditionalStep {
            name,
            condition_key,
            branches,
            default_branch: _,
        } => {
            writeln!(
                out,
                "{}agent_step: AgentStep::ConditionalStep {{",
                indent
            )
            .unwrap();
            writeln!(out, "{}    name: \"{}\".to_string(),", indent, name).unwrap();
            writeln!(
                out,
                "{}    condition_key: \"{}\".to_string(),",
                indent, condition_key
            )
            .unwrap();
            writeln!(out, "{}    branches: HashMap::new(), // {} branches", indent, branches.len())
                .unwrap();
            writeln!(out, "{}    default_branch: None,", indent).unwrap();
            writeln!(out, "{}}},", indent).unwrap();
        }
        AgentStep::LoopStep {
            name,
            condition_key,
            max_iterations,
            step: _,
        } => {
            writeln!(out, "{}agent_step: AgentStep::LoopStep {{", indent).unwrap();
            writeln!(out, "{}    name: \"{}\".to_string(),", indent, name).unwrap();
            writeln!(
                out,
                "{}    condition_key: \"{}\".to_string(),",
                indent, condition_key
            )
            .unwrap();
            writeln!(out, "{}    max_iterations: {},", indent, max_iterations).unwrap();
            writeln!(out, "{}    step: Box::new(/* sub-step */),", indent).unwrap();
            writeln!(out, "{}}},", indent).unwrap();
        }
    }
}

/// Write Rust state schema construction.
fn write_rust_state_schema(out: &mut String, schema: &StateSchema) {
    if schema.channels.is_empty() && schema.reducers.is_empty() {
        writeln!(out, "    let state_schema = StateSchema::default();").unwrap();
        return;
    }

    writeln!(out, "    let state_schema = StateSchema {{").unwrap();

    // Channels
    if schema.channels.is_empty() {
        writeln!(out, "        channels: HashMap::new(),").unwrap();
    } else {
        writeln!(out, "        channels: {{").unwrap();
        writeln!(out, "            let mut ch = HashMap::new();").unwrap();
        let mut channel_keys: Vec<&String> = schema.channels.keys().collect();
        channel_keys.sort();
        for key in channel_keys {
            if let Some(channel) = schema.channels.get(key) {
                let default_str = match &channel.default_value {
                    Some(v) => format!(
                        "Some(serde_json::json!({}))",
                        serde_json::to_string(v).unwrap_or_default()
                    ),
                    None => "None".to_string(),
                };
                writeln!(
                    out,
                    "            ch.insert(\"{}\".to_string(), ChannelConfig {{",
                    key
                )
                .unwrap();
                writeln!(
                    out,
                    "                channel_type: \"{}\".to_string(),",
                    channel.channel_type
                )
                .unwrap();
                writeln!(out, "                default_value: {},", default_str).unwrap();
                writeln!(out, "            }});").unwrap();
            }
        }
        writeln!(out, "            ch").unwrap();
        writeln!(out, "        }},").unwrap();
    }

    // Reducers
    if schema.reducers.is_empty() {
        writeln!(out, "        reducers: HashMap::new(),").unwrap();
    } else {
        writeln!(out, "        reducers: {{").unwrap();
        writeln!(out, "            let mut rd = HashMap::new();").unwrap();
        let mut reducer_keys: Vec<&String> = schema.reducers.keys().collect();
        reducer_keys.sort();
        for key in reducer_keys {
            if let Some(reducer) = schema.reducers.get(key) {
                let reducer_str = match reducer {
                    Reducer::Overwrite => "Reducer::Overwrite",
                    Reducer::Append => "Reducer::Append",
                    Reducer::Sum => "Reducer::Sum",
                };
                writeln!(
                    out,
                    "            rd.insert(\"{}\".to_string(), {});",
                    key, reducer_str
                )
                .unwrap();
            }
        }
        writeln!(out, "            rd").unwrap();
        writeln!(out, "        }},").unwrap();
    }

    writeln!(out, "    }};").unwrap();
}

// ============================================================================
// Helpers
// ============================================================================

/// Convert a node ID into a valid variable name.
fn agent_step_var_name(node_id: &str) -> String {
    let sanitized: String = node_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("agent_{}", sanitized)
}

/// Escape a string for TypeScript string literals.
fn escape_ts_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Summarize sub-steps for TypeScript inline arrays.
fn steps_summary(steps: &[AgentStep]) -> String {
    let names: Vec<String> = steps
        .iter()
        .map(|s| {
            let name = match s {
                AgentStep::LlmStep(c) => c.name.clone(),
                AgentStep::SequentialStep { name, .. } => name.clone(),
                AgentStep::ParallelStep { name, .. } => name.clone(),
                AgentStep::ConditionalStep { name, .. } => name.clone(),
                AgentStep::LoopStep { name, .. } => name.clone(),
            };
            format!("\"{}\"", name)
        })
        .collect();
    names.join(", ")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::graph_types::*;
    use crate::services::agent_composer::types::{AgentConfig, AgentStep, LlmStepConfig};
    use std::collections::HashMap;

    // ========================================================================
    // Test helpers
    // ========================================================================

    fn sample_llm_step(name: &str) -> AgentStep {
        AgentStep::LlmStep(LlmStepConfig {
            name: name.to_string(),
            instruction: None,
            model: None,
            tools: None,
            config: AgentConfig::default(),
        })
    }

    fn sample_direct_workflow() -> GraphWorkflow {
        let mut nodes = HashMap::new();
        nodes.insert(
            "a".to_string(),
            GraphNode {
                id: "a".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "agent-a".to_string(),
                    instruction: Some("Do task A".to_string()),
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "b".to_string(),
            GraphNode {
                id: "b".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "agent-b".to_string(),
                    instruction: None,
                    model: Some("gpt-4".to_string()),
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: true,
            },
        );

        let mut channels = HashMap::new();
        channels.insert(
            "messages".to_string(),
            ChannelConfig {
                channel_type: "array".to_string(),
                default_value: Some(serde_json::json!([])),
            },
        );
        let mut reducers = HashMap::new();
        reducers.insert("messages".to_string(), Reducer::Append);

        GraphWorkflow {
            name: "Direct Flow".to_string(),
            description: Some("A simple two-node flow".to_string()),
            nodes,
            edges: vec![Edge::Direct {
                from: "a".to_string(),
                to: "b".to_string(),
            }],
            entry_node: "a".to_string(),
            state_schema: StateSchema { channels, reducers },
        }
    }

    fn sample_conditional_workflow() -> GraphWorkflow {
        let mut nodes = HashMap::new();
        nodes.insert(
            "router".to_string(),
            GraphNode {
                id: "router".to_string(),
                agent_step: sample_llm_step("Router Agent"),
                position: None,
                interrupt_before: true,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "handler_a".to_string(),
            GraphNode {
                id: "handler_a".to_string(),
                agent_step: AgentStep::SequentialStep {
                    name: "Sequential Handler A".to_string(),
                    steps: vec![sample_llm_step("sub-a1"), sample_llm_step("sub-a2")],
                },
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "handler_b".to_string(),
            GraphNode {
                id: "handler_b".to_string(),
                agent_step: AgentStep::ParallelStep {
                    name: "Parallel Handler B".to_string(),
                    steps: vec![sample_llm_step("sub-b1")],
                },
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        let mut branches = HashMap::new();
        branches.insert("option_a".to_string(), "handler_a".to_string());
        branches.insert("option_b".to_string(), "handler_b".to_string());

        GraphWorkflow {
            name: "Conditional Flow".to_string(),
            description: None,
            nodes,
            edges: vec![Edge::Conditional {
                from: "router".to_string(),
                condition: ConditionConfig {
                    condition_key: "route".to_string(),
                },
                branches,
                default_branch: Some("handler_a".to_string()),
            }],
            entry_node: "router".to_string(),
            state_schema: StateSchema::default(),
        }
    }

    // ========================================================================
    // TypeScript tests
    // ========================================================================

    #[test]
    fn test_generate_typescript_direct_edges() {
        let workflow = sample_direct_workflow();
        let ts = generate_typescript(&workflow);

        // Import statement
        assert!(
            ts.contains("import { GraphWorkflow, AgentNode, Edge, StateSchema } from \"plan-cascade\";"),
            "Missing import statement"
        );

        // Workflow comment
        assert!(ts.contains("// Workflow: Direct Flow"), "Missing workflow name comment");
        assert!(
            ts.contains("// A simple two-node flow"),
            "Missing workflow description comment"
        );

        // Agent definitions — sorted by ID: a before b
        let pos_a = ts.find("const agent_a =").expect("Missing agent_a definition");
        let pos_b = ts.find("const agent_b =").expect("Missing agent_b definition");
        assert!(pos_a < pos_b, "Nodes not sorted by ID: a should come before b");

        // Agent a properties
        assert!(ts.contains("name: \"agent-a\""), "Missing agent-a name");
        assert!(ts.contains("type: \"llm\""), "Missing type llm");
        assert!(
            ts.contains("instruction: \"Do task A\""),
            "Missing instruction for agent-a"
        );

        // Agent b properties
        assert!(ts.contains("name: \"agent-b\""), "Missing agent-b name");
        assert!(ts.contains("model: \"gpt-4\""), "Missing model for agent-b");

        // Graph construction
        assert!(
            ts.contains("const graph = new GraphWorkflow(\"Direct Flow\");"),
            "Missing graph construction"
        );

        // Node additions
        assert!(
            ts.contains("graph.addNode(\"a\", agent_a);"),
            "Missing addNode for a"
        );
        assert!(
            ts.contains("graph.addNode(\"b\", agent_b);"),
            "Missing addNode for b"
        );

        // Interrupt comment on node b
        assert!(
            ts.contains("// interrupt: after"),
            "Missing interrupt comment for node b"
        );

        // Entry node
        assert!(
            ts.contains("graph.setEntryNode(\"a\");"),
            "Missing setEntryNode"
        );

        // Direct edge
        assert!(
            ts.contains("graph.addEdge(\"a\", \"b\");"),
            "Missing direct edge"
        );

        // State schema
        assert!(ts.contains("const stateSchema: StateSchema = {"), "Missing state schema");
        assert!(ts.contains("\"messages\": { type: \"array\", default: [] }"), "Missing channel");
        assert!(ts.contains("\"messages\": \"append\""), "Missing reducer");

        // Invocation
        assert!(ts.contains("const app = graph.compile();"), "Missing compile");
        assert!(ts.contains("await app.invoke({});"), "Missing invoke");
    }

    #[test]
    fn test_generate_typescript_conditional_edges() {
        let workflow = sample_conditional_workflow();
        let ts = generate_typescript(&workflow);

        // Import
        assert!(ts.contains("import { GraphWorkflow"));

        // Sorted nodes: handler_a, handler_b, router
        let pos_handler_a = ts
            .find("const agent_handler_a =")
            .expect("Missing handler_a definition");
        let pos_handler_b = ts
            .find("const agent_handler_b =")
            .expect("Missing handler_b definition");
        let pos_router = ts
            .find("const agent_router =")
            .expect("Missing router definition");
        assert!(
            pos_handler_a < pos_handler_b,
            "handler_a should come before handler_b"
        );
        assert!(
            pos_handler_b < pos_router,
            "handler_b should come before router"
        );

        // Sequential step type
        assert!(
            ts.contains("type: \"sequential\""),
            "Missing sequential step type"
        );

        // Parallel step type
        assert!(
            ts.contains("type: \"parallel\""),
            "Missing parallel step type"
        );

        // Interrupt before router
        assert!(
            ts.contains("// interrupt: before"),
            "Missing interrupt_before comment for router"
        );

        // Conditional edge with switch
        assert!(
            ts.contains("graph.addConditionalEdge(\"router\","),
            "Missing addConditionalEdge"
        );
        assert!(
            ts.contains("switch (state[\"route\"])"),
            "Missing switch on condition key"
        );

        // Branch cases — sorted: option_a before option_b
        let pos_case_a = ts
            .find("case \"option_a\": return \"handler_a\";")
            .expect("Missing case option_a");
        let pos_case_b = ts
            .find("case \"option_b\": return \"handler_b\";")
            .expect("Missing case option_b");
        assert!(
            pos_case_a < pos_case_b,
            "Branch cases not sorted: option_a should come before option_b"
        );

        // Default branch
        assert!(
            ts.contains("default: return \"handler_a\";"),
            "Missing default branch"
        );

        // Entry node
        assert!(ts.contains("graph.setEntryNode(\"router\");"));
    }

    // ========================================================================
    // Rust tests
    // ========================================================================

    #[test]
    fn test_generate_rust_direct_edges() {
        let workflow = sample_direct_workflow();
        let rs = generate_rust(&workflow);

        // Use statements
        assert!(
            rs.contains("use std::collections::HashMap;"),
            "Missing HashMap use"
        );
        assert!(
            rs.contains("use plan_cascade_core::graph::{GraphWorkflow, GraphNode, Edge, ConditionConfig};"),
            "Missing graph use"
        );
        assert!(
            rs.contains("use plan_cascade_core::graph::{StateSchema, ChannelConfig, Reducer};"),
            "Missing schema use"
        );
        assert!(
            rs.contains("use plan_cascade_core::agent::AgentStep;"),
            "Missing AgentStep use"
        );

        // Function signature
        assert!(
            rs.contains("pub fn build_workflow() -> GraphWorkflow {"),
            "Missing function signature"
        );

        // Workflow doc comment
        assert!(
            rs.contains("/// Workflow: Direct Flow"),
            "Missing workflow doc comment"
        );
        assert!(
            rs.contains("/// A simple two-node flow"),
            "Missing description doc comment"
        );

        // Nodes sorted: a before b
        let pos_a = rs
            .find("nodes.insert(\"a\".to_string()")
            .expect("Missing node a insertion");
        let pos_b = rs
            .find("nodes.insert(\"b\".to_string()")
            .expect("Missing node b insertion");
        assert!(pos_a < pos_b, "Nodes not sorted by ID: a should come before b");

        // Agent step for node a
        assert!(
            rs.contains("name: \"agent-a\".to_string()"),
            "Missing agent-a name"
        );
        assert!(
            rs.contains("instruction: Some(\"Do task A\".to_string())"),
            "Missing instruction for agent-a"
        );

        // Agent step for node b
        assert!(
            rs.contains("name: \"agent-b\".to_string()"),
            "Missing agent-b name"
        );
        assert!(
            rs.contains("model: Some(\"gpt-4\".to_string())"),
            "Missing model for agent-b"
        );

        // Interrupt comments
        assert!(
            rs.contains("// INTERRUPT: pause after node \"b\""),
            "Missing interrupt_after comment for node b"
        );
        // Node a has no interrupt
        assert!(
            !rs.contains("// INTERRUPT: pause before node \"a\""),
            "Unexpected interrupt comment for node a"
        );

        // Direct edge
        assert!(
            rs.contains("Edge::Direct {"),
            "Missing Direct edge variant"
        );
        assert!(
            rs.contains("from: \"a\".to_string()"),
            "Missing edge from field"
        );
        assert!(
            rs.contains("to: \"b\".to_string()"),
            "Missing edge to field"
        );

        // State schema channels and reducers
        assert!(
            rs.contains("channel_type: \"array\".to_string()"),
            "Missing channel type"
        );
        assert!(
            rs.contains("Reducer::Append"),
            "Missing Append reducer"
        );

        // GraphWorkflow struct
        assert!(
            rs.contains("name: \"Direct Flow\".to_string()"),
            "Missing workflow name in struct"
        );
        assert!(
            rs.contains("description: Some(\"A simple two-node flow\".to_string())"),
            "Missing description in struct"
        );
        assert!(
            rs.contains("entry_node: \"a\".to_string()"),
            "Missing entry_node in struct"
        );
    }

    #[test]
    fn test_generate_rust_conditional_edges() {
        let workflow = sample_conditional_workflow();
        let rs = generate_rust(&workflow);

        // Use statements present
        assert!(rs.contains("use std::collections::HashMap;"));
        assert!(rs.contains("use plan_cascade_core::agent::AgentStep;"));

        // Sorted nodes: handler_a, handler_b, router
        let pos_handler_a = rs
            .find("nodes.insert(\"handler_a\"")
            .expect("Missing handler_a insertion");
        let pos_handler_b = rs
            .find("nodes.insert(\"handler_b\"")
            .expect("Missing handler_b insertion");
        let pos_router = rs
            .find("nodes.insert(\"router\"")
            .expect("Missing router insertion");
        assert!(
            pos_handler_a < pos_handler_b,
            "handler_a should come before handler_b"
        );
        assert!(
            pos_handler_b < pos_router,
            "handler_b should come before router"
        );

        // Sequential step
        assert!(
            rs.contains("AgentStep::SequentialStep"),
            "Missing SequentialStep"
        );
        assert!(
            rs.contains("/* 2 sub-steps */"),
            "Missing sub-steps count for sequential"
        );

        // Parallel step
        assert!(
            rs.contains("AgentStep::ParallelStep"),
            "Missing ParallelStep"
        );
        assert!(
            rs.contains("/* 1 sub-steps */"),
            "Missing sub-steps count for parallel"
        );

        // Interrupt before router
        assert!(
            rs.contains("// INTERRUPT: pause before node \"router\""),
            "Missing interrupt_before comment"
        );

        // Conditional edge
        assert!(
            rs.contains("Edge::Conditional {"),
            "Missing Conditional edge"
        );
        assert!(
            rs.contains("condition_key: \"route\".to_string()"),
            "Missing condition key"
        );

        // Sorted branches in match: option_a before option_b
        let pos_opt_a = rs
            .find("m.insert(\"option_a\"")
            .expect("Missing option_a branch insertion");
        let pos_opt_b = rs
            .find("m.insert(\"option_b\"")
            .expect("Missing option_b branch insertion");
        assert!(
            pos_opt_a < pos_opt_b,
            "Branch insertions not sorted: option_a should come before option_b"
        );

        // Default branch
        assert!(
            rs.contains("default_branch: Some(\"handler_a\".to_string())"),
            "Missing default branch"
        );

        // Default state schema
        assert!(
            rs.contains("StateSchema::default()"),
            "Missing default state schema"
        );

        // Description is None
        assert!(
            rs.contains("description: None"),
            "Missing None description"
        );

        // Entry node
        assert!(
            rs.contains("entry_node: \"router\".to_string()"),
            "Missing entry_node"
        );
    }

    // ========================================================================
    // Helpers tests
    // ========================================================================

    #[test]
    fn test_agent_step_var_name_sanitizes() {
        assert_eq!(agent_step_var_name("my-node"), "agent_my_node");
        assert_eq!(agent_step_var_name("node_1"), "agent_node_1");
        assert_eq!(agent_step_var_name("a.b.c"), "agent_a_b_c");
    }

    #[test]
    fn test_escape_ts_string() {
        assert_eq!(escape_ts_string("hello"), "hello");
        assert_eq!(escape_ts_string("he said \"hi\""), "he said \\\"hi\\\"");
        assert_eq!(escape_ts_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_generate_typescript_conditional_step_node() {
        let mut nodes = HashMap::new();
        let mut branches = HashMap::new();
        branches.insert(
            "yes".to_string(),
            sample_llm_step("yes-handler"),
        );

        nodes.insert(
            "cond".to_string(),
            GraphNode {
                id: "cond".to_string(),
                agent_step: AgentStep::ConditionalStep {
                    name: "Conditional Router".to_string(),
                    condition_key: "decision".to_string(),
                    branches,
                    default_branch: None,
                },
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        let workflow = GraphWorkflow {
            name: "Cond Node Test".to_string(),
            description: None,
            nodes,
            edges: vec![],
            entry_node: "cond".to_string(),
            state_schema: StateSchema::default(),
        };

        let ts = generate_typescript(&workflow);
        assert!(ts.contains("type: \"conditional\""), "Missing conditional type");
        assert!(ts.contains("conditionKey: \"decision\""), "Missing conditionKey");
        assert!(ts.contains("branches: [\"yes\"]"), "Missing branches array");
    }

    #[test]
    fn test_generate_rust_conditional_step_node() {
        let mut nodes = HashMap::new();
        let mut branches = HashMap::new();
        branches.insert(
            "yes".to_string(),
            sample_llm_step("yes-handler"),
        );

        nodes.insert(
            "cond".to_string(),
            GraphNode {
                id: "cond".to_string(),
                agent_step: AgentStep::ConditionalStep {
                    name: "Conditional Router".to_string(),
                    condition_key: "decision".to_string(),
                    branches,
                    default_branch: None,
                },
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        let workflow = GraphWorkflow {
            name: "Cond Node Test".to_string(),
            description: None,
            nodes,
            edges: vec![],
            entry_node: "cond".to_string(),
            state_schema: StateSchema::default(),
        };

        let rs = generate_rust(&workflow);
        assert!(rs.contains("AgentStep::ConditionalStep"), "Missing ConditionalStep");
        assert!(
            rs.contains("condition_key: \"decision\".to_string()"),
            "Missing condition_key"
        );
        assert!(rs.contains("// 1 branches"), "Missing branches count comment");
    }
}
