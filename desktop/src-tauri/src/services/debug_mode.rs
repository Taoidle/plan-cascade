use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services::task_mode::context_provider::ContextSourceConfig;
use crate::services::tools::{
    definitions::get_tool_definitions_from_registry,
    runtime_tools::{self, RuntimeToolMetadata},
};
use crate::services::workflow_kernel::{HandoffContextBundle, ModeQualitySnapshot, WorkflowMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugLifecyclePhase {
    Intaking,
    Clarifying,
    GatheringSignal,
    Reproducing,
    Hypothesizing,
    TestingHypothesis,
    IdentifyingRootCause,
    ProposingFix,
    PatchReview,
    Patching,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl DebugLifecyclePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Intaking => "intaking",
            Self::Clarifying => "clarifying",
            Self::GatheringSignal => "gathering_signal",
            Self::Reproducing => "reproducing",
            Self::Hypothesizing => "hypothesizing",
            Self::TestingHypothesis => "testing_hypothesis",
            Self::IdentifyingRootCause => "identifying_root_cause",
            Self::ProposingFix => "proposing_fix",
            Self::PatchReview => "patch_review",
            Self::Patching => "patching",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for DebugSeverity {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugEnvironment {
    Dev,
    Staging,
    Prod,
}

impl Default for DebugEnvironment {
    fn default() -> Self {
        Self::Dev
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugCapabilityClass {
    Observe,
    Experiment,
    Mutate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugCapabilityProfile {
    DevFull,
    StagingLimited,
    ProdObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DebugEvidenceRef {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub created_at: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DebugHypothesis {
    pub id: String,
    pub statement: String,
    pub confidence: f64,
    #[serde(default)]
    pub supporting_evidence_ids: Vec<String>,
    #[serde(default)]
    pub contradicting_evidence_ids: Vec<String>,
    #[serde(default)]
    pub next_checks: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RootCauseReport {
    pub conclusion: String,
    #[serde(default)]
    pub supporting_evidence_ids: Vec<String>,
    #[serde(default)]
    pub contradictions: Vec<String>,
    pub confidence: f64,
    #[serde(default)]
    pub impact_scope: Vec<String>,
    pub recommended_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DebugPatchOperation {
    pub id: String,
    pub kind: String,
    pub file_path: String,
    pub description: String,
    pub find_text: Option<String>,
    pub replace_text: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub create_if_missing: bool,
    pub expected_occurrences: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FixProposal {
    pub summary: String,
    #[serde(default)]
    pub change_scope: Vec<String>,
    pub risk_level: DebugSeverity,
    #[serde(default)]
    pub files_or_systems_touched: Vec<String>,
    #[serde(default)]
    pub manual_approvals_required: Vec<String>,
    #[serde(default)]
    pub verification_plan: Vec<String>,
    pub patch_preview_ref: Option<String>,
    #[serde(default)]
    pub patch_operations: Vec<DebugPatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerificationCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerificationReport {
    pub summary: String,
    #[serde(default)]
    pub checks: Vec<VerificationCheck>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DebugPendingApproval {
    pub kind: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub required_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugRuntimeCapabilities {
    pub profile: DebugCapabilityProfile,
    #[serde(default)]
    pub allowed_classes: Vec<DebugCapabilityClass>,
    #[serde(default)]
    pub allowed_tool_categories: Vec<String>,
    #[serde(default)]
    pub approval_required_for: Vec<DebugCapabilityClass>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugToolClassification {
    pub capability_class: DebugCapabilityClass,
    pub tool_category: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugToolCapability {
    pub tool_name: String,
    pub description: String,
    pub source: String,
    pub capability_class: DebugCapabilityClass,
    pub tool_category: Option<String>,
    #[serde(default)]
    pub debug_categories: Vec<String>,
    #[serde(default)]
    pub environment_allowlist: Vec<String>,
    pub write_behavior: Option<String>,
    pub allowed: bool,
    pub requires_approval: bool,
    pub blocked_reason: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugCapabilitySnapshot {
    pub profile: DebugCapabilityProfile,
    pub runtime_capabilities: DebugRuntimeCapabilities,
    #[serde(default)]
    pub tools: Vec<DebugToolCapability>,
}

#[derive(Debug, Clone)]
pub struct DebugToolAccessDecision {
    pub classification: DebugToolClassification,
    pub allowed: bool,
    pub requires_approval: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugState {
    pub case_id: Option<String>,
    pub phase: String,
    pub severity: DebugSeverity,
    pub environment: DebugEnvironment,
    pub symptom_summary: String,
    pub title: Option<String>,
    pub project_path: Option<String>,
    pub expected_behavior: Option<String>,
    pub actual_behavior: Option<String>,
    #[serde(default)]
    pub repro_steps: Vec<String>,
    #[serde(default)]
    pub affected_surface: Vec<String>,
    pub recent_changes: Option<String>,
    pub target_url_or_entry: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<DebugEvidenceRef>,
    #[serde(default)]
    pub active_hypotheses: Vec<DebugHypothesis>,
    pub selected_root_cause: Option<RootCauseReport>,
    pub fix_proposal: Option<FixProposal>,
    pub pending_approval: Option<DebugPendingApproval>,
    pub verification_report: Option<VerificationReport>,
    pub pending_prompt: Option<String>,
    pub capability_profile: DebugCapabilityProfile,
    pub tool_block_reason: Option<String>,
    pub background_status: Option<String>,
    pub last_checkpoint_id: Option<String>,
    #[serde(default)]
    pub entry_handoff: HandoffContextBundle,
    #[serde(default)]
    pub quality: Option<ModeQualitySnapshot>,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            case_id: None,
            phase: DebugLifecyclePhase::Intaking.as_str().to_string(),
            severity: DebugSeverity::Medium,
            environment: DebugEnvironment::Dev,
            symptom_summary: String::new(),
            title: None,
            project_path: None,
            expected_behavior: None,
            actual_behavior: None,
            repro_steps: Vec::new(),
            affected_surface: Vec::new(),
            recent_changes: None,
            target_url_or_entry: None,
            evidence_refs: Vec::new(),
            active_hypotheses: Vec::new(),
            selected_root_cause: None,
            fix_proposal: None,
            pending_approval: None,
            verification_report: None,
            pending_prompt: None,
            capability_profile: DebugCapabilityProfile::DevFull,
            tool_block_reason: None,
            background_status: None,
            last_checkpoint_id: None,
            entry_handoff: HandoffContextBundle::default(),
            quality: Some(ModeQualitySnapshot::for_mode(WorkflowMode::Debug)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugModeSession {
    pub session_id: String,
    pub kernel_session_id: Option<String>,
    pub project_path: Option<String>,
    pub state: DebugState,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugExecutionReport {
    pub case_id: Option<String>,
    pub summary: String,
    pub root_cause_conclusion: Option<String>,
    pub fix_applied: bool,
    pub verification: Option<VerificationReport>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugArtifactDescriptor {
    pub path: String,
    pub file_name: String,
    pub kind: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub updated_at: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugArtifactContent {
    pub artifact: DebugArtifactDescriptor,
    #[serde(default)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugProgressPayload {
    pub session_id: String,
    pub phase: String,
    pub card_type: Option<String>,
    pub message: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterDebugModeRequest {
    pub description: String,
    pub environment: Option<DebugEnvironment>,
    pub kernel_session_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub locale: Option<String>,
}

pub fn capability_profile_for_environment(environment: DebugEnvironment) -> DebugCapabilityProfile {
    match environment {
        DebugEnvironment::Dev => DebugCapabilityProfile::DevFull,
        DebugEnvironment::Staging => DebugCapabilityProfile::StagingLimited,
        DebugEnvironment::Prod => DebugCapabilityProfile::ProdObserveOnly,
    }
}

pub fn runtime_capabilities_for_profile(
    profile: DebugCapabilityProfile,
) -> DebugRuntimeCapabilities {
    match profile {
        DebugCapabilityProfile::DevFull => DebugRuntimeCapabilities {
            profile,
            allowed_classes: vec![
                DebugCapabilityClass::Observe,
                DebugCapabilityClass::Experiment,
                DebugCapabilityClass::Mutate,
            ],
            allowed_tool_categories: vec![
                "debug:logs".to_string(),
                "debug:browser".to_string(),
                "debug:test_runner".to_string(),
                "debug:db_read".to_string(),
                "debug:db_write".to_string(),
                "debug:cache_read".to_string(),
                "debug:cache_write".to_string(),
                "debug:queue".to_string(),
                "debug:metrics".to_string(),
                "debug:trace".to_string(),
                "debug:k8s".to_string(),
                "debug:runbook".to_string(),
            ],
            approval_required_for: vec![DebugCapabilityClass::Mutate],
        },
        DebugCapabilityProfile::StagingLimited => DebugRuntimeCapabilities {
            profile,
            allowed_classes: vec![
                DebugCapabilityClass::Observe,
                DebugCapabilityClass::Experiment,
            ],
            allowed_tool_categories: vec![
                "debug:logs".to_string(),
                "debug:browser".to_string(),
                "debug:metrics".to_string(),
                "debug:trace".to_string(),
                "debug:db_read".to_string(),
                "debug:cache_read".to_string(),
                "debug:queue".to_string(),
                "debug:k8s".to_string(),
                "debug:runbook".to_string(),
                "debug:test_runner".to_string(),
            ],
            approval_required_for: vec![
                DebugCapabilityClass::Experiment,
                DebugCapabilityClass::Mutate,
            ],
        },
        DebugCapabilityProfile::ProdObserveOnly => DebugRuntimeCapabilities {
            profile,
            allowed_classes: vec![DebugCapabilityClass::Observe],
            allowed_tool_categories: vec![
                "debug:logs".to_string(),
                "debug:metrics".to_string(),
                "debug:trace".to_string(),
                "debug:db_read".to_string(),
                "debug:cache_read".to_string(),
                "debug:queue".to_string(),
                "debug:browser".to_string(),
                "debug:runbook".to_string(),
            ],
            approval_required_for: vec![
                DebugCapabilityClass::Experiment,
                DebugCapabilityClass::Mutate,
            ],
        },
    }
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn environment_token_for_profile(profile: DebugCapabilityProfile) -> &'static str {
    match profile {
        DebugCapabilityProfile::DevFull => "dev",
        DebugCapabilityProfile::StagingLimited => "staging",
        DebugCapabilityProfile::ProdObserveOnly => "prod",
    }
}

fn read_string_arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn classify_bash_command(command: &str) -> DebugToolClassification {
    let normalized = normalize_token(command);

    if normalized.is_empty() {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Experiment,
            tool_category: None,
            rationale: "Bash execution defaults to experiment scope until command intent is clearer."
                .to_string(),
        };
    }

    if contains_any(
        &normalized,
        &[
            "rm ", " rm", "mv ", " mv", "chmod ", " chown ", "sed -i", "tee ", "truncate ",
            "kill ", "pkill ", "restart", "reboot", "kubectl apply", "kubectl delete",
            "helm upgrade", "redis-cli set", "redis-cli del", "redis-cli flush", "psql -c insert",
            "psql -c update", "psql -c delete", "mysql -e insert", "mysql -e update",
            "mysql -e delete", "git commit", "git push", "npm install", "pnpm install",
            "yarn add", "cargo add", "cargo install",
        ],
    ) || normalized.contains(" >")
        || normalized.contains(">>")
    {
        let tool_category = if contains_any(&normalized, &["redis", "cache"]) {
            Some("debug:cache_write".to_string())
        } else if contains_any(&normalized, &["psql", "mysql", "postgres", "sqlite", "db "]) {
            Some("debug:db_write".to_string())
        } else if contains_any(&normalized, &["kubectl", "helm", "k8s", "kubernetes"]) {
            Some("debug:k8s".to_string())
        } else {
            None
        };
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Mutate,
            tool_category,
            rationale: "Shell command appears to modify files, services, or external systems."
                .to_string(),
        };
    }

    if contains_any(
        &normalized,
        &[
            "tail ", "head ", "cat ", "less ", "more ", "grep ", "rg ", "find ", "ls ", "pwd",
            "printenv", "env", "which ", "ps ", "docker logs", "kubectl logs", "kubectl get",
            "kubectl describe", "redis-cli get", "redis-cli keys", "psql -c select",
            "mysql -e select", "curl -i", "curl -I", "wget --spider",
        ],
    ) {
        let tool_category = if contains_any(&normalized, &["docker logs", "kubectl logs", "tail ", "grep "]) {
            Some("debug:logs".to_string())
        } else if contains_any(&normalized, &["redis"]) {
            Some("debug:cache_read".to_string())
        } else if contains_any(&normalized, &["psql", "mysql", "postgres", "sqlite"]) {
            Some("debug:db_read".to_string())
        } else if contains_any(&normalized, &["kubectl"]) {
            Some("debug:k8s".to_string())
        } else {
            None
        };
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category,
            rationale: "Shell command looks read-only and is suitable for evidence gathering."
                .to_string(),
        };
    }

    if contains_any(
        &normalized,
        &[
            "npm test", "pnpm test", "yarn test", "vitest", "jest", "playwright", "cypress",
            "cargo test", "pytest", "go test", "curl ", "wget ", "http ", "grpcurl", "kubectl exec",
        ],
    ) {
        let tool_category = if contains_any(&normalized, &["vitest", "jest", "playwright", "cypress", "cargo test", "pytest", "go test"]) {
            Some("debug:test_runner".to_string())
        } else if contains_any(&normalized, &["kubectl"]) {
            Some("debug:k8s".to_string())
        } else {
            None
        };
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Experiment,
            tool_category,
            rationale: "Shell command looks like a controlled diagnostic experiment or verification run."
                .to_string(),
        };
    }

    DebugToolClassification {
        capability_class: DebugCapabilityClass::Experiment,
        tool_category: None,
        rationale: "Shell command is treated as an experiment by default in debug mode.".to_string(),
    }
}

pub fn classify_debug_tool_invocation(
    tool_name: &str,
    description: Option<&str>,
    args: &Value,
    runtime_metadata: Option<&RuntimeToolMetadata>,
) -> DebugToolClassification {
    if let Some(metadata) = runtime_metadata {
        let capability_class = metadata
            .capability_class
            .as_deref()
            .and_then(|value| match value {
                "observe" => Some(DebugCapabilityClass::Observe),
                "experiment" => Some(DebugCapabilityClass::Experiment),
                "mutate" => Some(DebugCapabilityClass::Mutate),
                _ => None,
            })
            .or_else(|| match metadata.write_behavior.as_deref() {
                Some("read_only") => Some(DebugCapabilityClass::Observe),
                Some("experiment") => Some(DebugCapabilityClass::Experiment),
                Some("mutating") => Some(DebugCapabilityClass::Mutate),
                _ => None,
            });
        if let Some(capability_class) = capability_class {
            return DebugToolClassification {
                capability_class,
                tool_category: metadata.debug_categories.first().cloned(),
                rationale: format!(
                    "Runtime metadata from {} declared this tool as {:?}.",
                    if metadata.source.is_empty() {
                        "runtime registry"
                    } else {
                        metadata.source.as_str()
                    },
                    capability_class
                ),
            };
        }
    }

    let normalized_name = normalize_token(tool_name);
    let normalized_desc = normalize_token(description.unwrap_or_default());
    let combined = if normalized_desc.is_empty() {
        normalized_name.clone()
    } else {
        format!("{normalized_name} {normalized_desc}")
    };

    match normalized_name.as_str() {
        "read" | "glob" | "grep" | "ls" | "cwd" | "codebasesearch" | "analyze"
        | "searchknowledge" => {
            return DebugToolClassification {
                capability_class: DebugCapabilityClass::Observe,
                tool_category: None,
                rationale: "Read/search tool is suitable for evidence gathering.".to_string(),
            }
        }
        "write" | "edit" | "notebookedit" => {
            return DebugToolClassification {
                capability_class: DebugCapabilityClass::Mutate,
                tool_category: None,
                rationale: "File mutation tools require debug patch approval.".to_string(),
            }
        }
        "browser" => {
            let action_name = read_string_arg(args, "action").to_ascii_lowercase();
            let (capability_class, rationale) = match action_name.as_str() {
                "navigate" | "open_page" | "screenshot" | "extract_text" | "wait_for"
                | "capture_dom_snapshot" | "capture_console_logs" | "capture_network_log"
                | "read_storage" | "read_cookie_names" | "collect_performance_entries" => (
                    DebugCapabilityClass::Observe,
                    "Browser action is read-only evidence collection or page inspection.",
                ),
                "set_viewport" | "emulate_device" | "click" | "type_text" => (
                    DebugCapabilityClass::Experiment,
                    "Browser action changes viewport state or drives page interaction for reproduction.",
                ),
                _ => (
                    DebugCapabilityClass::Experiment,
                    "Browser automation is treated as a controlled reproduction experiment.",
                ),
            };
            return DebugToolClassification {
                capability_class,
                tool_category: Some("debug:browser".to_string()),
                rationale: rationale.to_string(),
            }
        }
        "webfetch" | "websearch" => {
            return DebugToolClassification {
                capability_class: DebugCapabilityClass::Observe,
                tool_category: Some("debug:runbook".to_string()),
                rationale: "Web lookup tools are treated as read-only runbook/reference access."
                    .to_string(),
            }
        }
        "bash" => {
            let command = ["command", "cmd", "script"]
                .iter()
                .map(|key| read_string_arg(args, key))
                .find(|value| !value.is_empty())
                .unwrap_or_default();
            return classify_bash_command(&command);
        }
        "task" => {
            return DebugToolClassification {
                capability_class: DebugCapabilityClass::Mutate,
                tool_category: None,
                rationale: "Delegating to general-purpose task agents is treated as a mutate-capable action."
                    .to_string(),
            }
        }
        _ => {}
    }

    let read_hint = contains_any(&combined, &["read", "query", "select", "lookup", "fetch", "inspect", "list"]);
    let write_hint =
        contains_any(&combined, &["write", "update", "insert", "delete", "flush", "clear", "set "]);

    if contains_any(&combined, &["log", "sentry", "datadog", "elk", "cloudwatch"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category: Some("debug:logs".to_string()),
            rationale: "Tool appears to target log collection or log search.".to_string(),
        };
    }
    if contains_any(&combined, &["metric", "prometheus", "grafana"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category: Some("debug:metrics".to_string()),
            rationale: "Tool appears to read metrics or dashboards.".to_string(),
        };
    }
    if contains_any(&combined, &["trace", "jaeger", "tempo", "zipkin"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category: Some("debug:trace".to_string()),
            rationale: "Tool appears to read distributed tracing data.".to_string(),
        };
    }
    if contains_any(&combined, &["redis", "cache"]) {
        return DebugToolClassification {
            capability_class: if write_hint {
                DebugCapabilityClass::Mutate
            } else {
                DebugCapabilityClass::Observe
            },
            tool_category: Some(
                if write_hint {
                    "debug:cache_write"
                } else {
                    "debug:cache_read"
                }
                .to_string(),
            ),
            rationale: "Tool appears to target cache/Redis diagnostics.".to_string(),
        };
    }
    if contains_any(&combined, &["postgres", "mysql", "sqlite", "database", "db ", "sql"]) {
        return DebugToolClassification {
            capability_class: if write_hint && !read_hint {
                DebugCapabilityClass::Mutate
            } else {
                DebugCapabilityClass::Observe
            },
            tool_category: Some(
                if write_hint && !read_hint {
                    "debug:db_write"
                } else {
                    "debug:db_read"
                }
                .to_string(),
            ),
            rationale: "Tool appears to target database inspection or mutation.".to_string(),
        };
    }
    if contains_any(&combined, &["queue", "kafka", "sqs", "rabbitmq"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category: Some("debug:queue".to_string()),
            rationale: "Tool appears to inspect queue state.".to_string(),
        };
    }
    if contains_any(&combined, &["k8s", "kubernetes", "kubectl", "helm"]) {
        return DebugToolClassification {
            capability_class: if write_hint {
                DebugCapabilityClass::Mutate
            } else {
                DebugCapabilityClass::Experiment
            },
            tool_category: Some("debug:k8s".to_string()),
            rationale: "Tool appears to interact with cluster resources or workloads.".to_string(),
        };
    }
    if contains_any(&combined, &["browser", "playwright", "puppeteer", "devtools"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Experiment,
            tool_category: Some("debug:browser".to_string()),
            rationale: "Tool appears to drive or inspect a browser runtime.".to_string(),
        };
    }
    if contains_any(&combined, &["test", "vitest", "jest", "cypress", "playwright"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Experiment,
            tool_category: Some("debug:test_runner".to_string()),
            rationale: "Tool appears to run verification or diagnostic tests.".to_string(),
        };
    }
    if contains_any(&combined, &["runbook", "handbook", "knowledge", "wiki"]) {
        return DebugToolClassification {
            capability_class: DebugCapabilityClass::Observe,
            tool_category: Some("debug:runbook".to_string()),
            rationale: "Tool appears to provide reference or runbook data.".to_string(),
        };
    }

    DebugToolClassification {
        capability_class: if write_hint {
            DebugCapabilityClass::Mutate
        } else {
            DebugCapabilityClass::Experiment
        },
        tool_category: None,
        rationale: "Tool has no explicit debug category metadata; using conservative heuristic classification."
            .to_string(),
    }
}

pub fn evaluate_debug_tool_access(
    runtime_capabilities: &DebugRuntimeCapabilities,
    tool_name: &str,
    description: Option<&str>,
    args: &Value,
    runtime_metadata: Option<&RuntimeToolMetadata>,
) -> DebugToolAccessDecision {
    let classification = classify_debug_tool_invocation(tool_name, description, args, runtime_metadata);
    let class_allowed = runtime_capabilities
        .allowed_classes
        .contains(&classification.capability_class);
    let category_allowed = classification.tool_category.as_ref().map_or(true, |category| {
        runtime_capabilities
            .allowed_tool_categories
            .iter()
            .any(|candidate| candidate == category)
    });
    let environment_allowed = runtime_metadata.map_or(true, |metadata| {
        metadata.environment_allowlist.is_empty()
            || metadata
                .environment_allowlist
                .iter()
                .any(|env| env == environment_token_for_profile(runtime_capabilities.profile))
    });
    let blocked_reason = if !class_allowed {
        Some(format!(
            "Debug capability profile '{:?}' blocks {:?} tools.",
            runtime_capabilities.profile, classification.capability_class
        ))
    } else if !category_allowed {
        classification
            .tool_category
            .as_ref()
            .map(|category| format!("Debug capability profile '{:?}' does not allow {category}.", runtime_capabilities.profile))
    } else if !environment_allowed {
        Some(format!(
            "Tool '{tool_name}' is not allowed in {} debug sessions.",
            environment_token_for_profile(runtime_capabilities.profile)
        ))
    } else {
        None
    };

    DebugToolAccessDecision {
        requires_approval: runtime_metadata
            .and_then(|metadata| metadata.approval_required)
            .unwrap_or_else(|| {
                runtime_capabilities
                    .approval_required_for
                    .contains(&classification.capability_class)
            }),
        allowed: blocked_reason.is_none(),
        classification,
        blocked_reason,
    }
}

pub fn build_debug_capability_snapshot(profile: DebugCapabilityProfile) -> DebugCapabilitySnapshot {
    let runtime_capabilities = runtime_capabilities_for_profile(profile);
    let runtime_tool_names: HashSet<String> = runtime_tools::names().into_iter().collect();
    let tools = get_tool_definitions_from_registry()
        .into_iter()
        .map(|definition| {
            let runtime_metadata = runtime_tools::metadata_for(&definition.name);
            let access = evaluate_debug_tool_access(
                &runtime_capabilities,
                &definition.name,
                Some(&definition.description),
                &Value::Null,
                runtime_metadata.as_ref(),
            );
            DebugToolCapability {
                tool_name: definition.name.clone(),
                description: definition.description,
                source: if runtime_tool_names.contains(&definition.name) {
                    "runtime".to_string()
                } else {
                    "builtin".to_string()
                },
                capability_class: access.classification.capability_class,
                tool_category: access.classification.tool_category,
                debug_categories: runtime_metadata
                    .as_ref()
                    .map(|metadata| metadata.debug_categories.clone())
                    .unwrap_or_default(),
                environment_allowlist: runtime_metadata
                    .as_ref()
                    .map(|metadata| metadata.environment_allowlist.clone())
                    .unwrap_or_default(),
                write_behavior: runtime_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.write_behavior.clone()),
                allowed: access.allowed,
                requires_approval: access.requires_approval,
                blocked_reason: access.blocked_reason,
                rationale: access.classification.rationale,
            }
        })
        .collect();

    DebugCapabilitySnapshot {
        profile,
        runtime_capabilities,
        tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_declared_tool_classification_takes_precedence() {
        let metadata = RuntimeToolMetadata {
            source: "mcp:observability".to_string(),
            capability_class: Some("observe".to_string()),
            debug_categories: vec!["debug:logs".to_string()],
            environment_allowlist: vec!["prod".to_string()],
            write_behavior: Some("read_only".to_string()),
            approval_required: Some(true),
        };

        let classification = classify_debug_tool_invocation(
            "mcp:observability:logs",
            Some("Mutating description should be ignored"),
            &Value::Null,
            Some(&metadata),
        );

        assert_eq!(classification.capability_class, DebugCapabilityClass::Observe);
        assert_eq!(classification.tool_category.as_deref(), Some("debug:logs"));
    }

    #[test]
    fn metadata_environment_allowlist_blocks_wrong_profile() {
        let runtime_capabilities =
            runtime_capabilities_for_profile(DebugCapabilityProfile::ProdObserveOnly);
        let metadata = RuntimeToolMetadata {
            source: "mcp:browser".to_string(),
            capability_class: Some("observe".to_string()),
            debug_categories: vec!["debug:browser".to_string()],
            environment_allowlist: vec!["staging".to_string()],
            write_behavior: Some("read_only".to_string()),
            approval_required: Some(false),
        };

        let access = evaluate_debug_tool_access(
            &runtime_capabilities,
            "mcp:browser:console",
            Some("Read browser console"),
            &Value::Null,
            Some(&metadata),
        );

        assert!(!access.allowed);
        assert!(access
            .blocked_reason
            .as_deref()
            .unwrap_or_default()
            .contains("prod"));
    }
}
