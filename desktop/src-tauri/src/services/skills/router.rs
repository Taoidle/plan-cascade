//! LLM-assisted skill reranking.
//!
//! This layer keeps the existing deterministic recall path, then optionally
//! asks the active chat/task model to rerank a small candidate set.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::timeout;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, ProviderConfig, ProviderType};
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, GlmProvider, MinimaxProvider, OllamaProvider,
    OpenAIProvider, QwenProvider,
};
use crate::services::skills::model::{InjectionPhase, MatchReason, SkillMatch, SkillSource};

const SKILL_RERANK_TIMEOUT: Duration = Duration::from_secs(8);
const SKILL_RERANK_CACHE_TTL: Duration = Duration::from_secs(90);
const SKILL_RERANK_FINAL_LIMIT: usize = 3;

static SKILL_RERANK_CACHE: OnceLock<Mutex<HashMap<String, CachedRerankResult>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRerankCandidate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub source: String,
    pub inject_into: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub user_invocable: bool,
    pub review_status: Option<String>,
    pub detected: bool,
    pub match_reason: String,
    pub lexical_score: Option<f32>,
    pub relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRerankResult {
    #[serde(default)]
    pub selected_skill_ids: Vec<String>,
    #[serde(default)]
    pub rejected_skill_ids: Vec<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRerankDiagnostics {
    pub skill_router_used: bool,
    pub skill_router_strategy: Option<String>,
    pub skill_router_reason: Option<String>,
    pub skill_router_confidence: Option<f32>,
    pub skill_router_fallback_reason: Option<String>,
    #[serde(default)]
    pub skill_router_selected_ids: Vec<String>,
    pub skill_router_latency_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct CachedRerankResult {
    stored_at: Instant,
    result: SkillRerankResult,
}

fn rerank_cache() -> &'static Mutex<HashMap<String, CachedRerankResult>> {
    SKILL_RERANK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_llm_provider_from_config(config: &ProviderConfig) -> Arc<dyn LlmProvider> {
    match config.provider {
        ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.clone())),
        ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.clone())),
        ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.clone())),
        ProviderType::Glm => Arc::new(GlmProvider::new(config.clone())),
        ProviderType::Qwen => Arc::new(QwenProvider::new(config.clone())),
        ProviderType::Minimax => Arc::new(MinimaxProvider::new(config.clone())),
        ProviderType::Ollama => Arc::new(OllamaProvider::new(config.clone())),
    }
}

fn source_label(source: &SkillSource) -> String {
    match source {
        SkillSource::Builtin => "builtin".to_string(),
        SkillSource::External { source_name } => format!("external:{source_name}"),
        SkillSource::User => "user".to_string(),
        SkillSource::ProjectLocal => "project_local".to_string(),
        SkillSource::Generated => "generated".to_string(),
    }
}

fn phase_label(phase: &InjectionPhase) -> &'static str {
    match phase {
        InjectionPhase::Planning => "planning",
        InjectionPhase::Implementation => "implementation",
        InjectionPhase::Retry => "retry",
        InjectionPhase::Always => "always",
    }
}

fn match_reason_label(reason: &MatchReason) -> String {
    match reason {
        MatchReason::AutoDetected => "auto_detected".to_string(),
        MatchReason::LexicalMatch { .. } => "lexical_match".to_string(),
        MatchReason::UserForced => "user_forced".to_string(),
    }
}

fn project_relative_path(project_root: &Path, raw_path: &Path) -> Option<String> {
    raw_path
        .strip_prefix(project_root)
        .ok()
        .map(|relative| relative.display().to_string())
        .or_else(|| {
            if raw_path.to_string_lossy().starts_with("builtin://")
                || raw_path.to_string_lossy().starts_with("generated://")
            {
                None
            } else {
                Some(raw_path.display().to_string())
            }
        })
}

fn candidate_payload(project_root: &Path, matches: &[SkillMatch]) -> Vec<SkillRerankCandidate> {
    matches
        .iter()
        .map(|item| SkillRerankCandidate {
            id: item.skill.id.clone(),
            name: item.skill.name.clone(),
            description: item.skill.description.clone(),
            tags: item.skill.tags.clone(),
            source: source_label(&item.skill.source),
            inject_into: item
                .skill
                .inject_into
                .iter()
                .map(|phase| phase_label(phase).to_string())
                .collect(),
            allowed_tools: item.skill.allowed_tools.clone(),
            user_invocable: item.skill.user_invocable,
            review_status: item.skill.review_status.as_ref().map(|status| {
                match status {
                    crate::services::skills::model::SkillReviewStatus::PendingReview => {
                        "pending_review"
                    }
                    crate::services::skills::model::SkillReviewStatus::Approved => "approved",
                    crate::services::skills::model::SkillReviewStatus::Rejected => "rejected",
                    crate::services::skills::model::SkillReviewStatus::Archived => "archived",
                }
                .to_string()
            }),
            detected: item.skill.detected,
            match_reason: match_reason_label(&item.match_reason),
            lexical_score: matches!(&item.match_reason, MatchReason::LexicalMatch { .. })
                .then_some(item.score),
            relative_path: project_relative_path(project_root, &item.skill.path),
        })
        .collect()
}

fn build_rerank_cache_key(
    project_root: &Path,
    query: &str,
    phase: &InjectionPhase,
    provider_config: &ProviderConfig,
    matches: &[SkillMatch],
) -> String {
    let normalized_query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let ids = matches
        .iter()
        .map(|item| format!("{}:{:.4}", item.skill.id, item.score))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "{}::{}::{}::{}::{}::{}",
        project_root.display(),
        normalized_query,
        phase_label(phase),
        provider_config.provider,
        provider_config.model,
        ids
    )
}

fn get_cached_rerank(cache_key: &str) -> Option<SkillRerankResult> {
    let mut guard = rerank_cache().lock().ok()?;
    let now = Instant::now();
    guard.retain(|_, cached| now.duration_since(cached.stored_at) <= SKILL_RERANK_CACHE_TTL);
    guard.get(cache_key).map(|cached| cached.result.clone())
}

fn store_cached_rerank(cache_key: String, result: SkillRerankResult) {
    if let Ok(mut guard) = rerank_cache().lock() {
        guard.insert(
            cache_key,
            CachedRerankResult {
                stored_at: Instant::now(),
                result,
            },
        );
    }
}

fn extract_response_text(response: &crate::services::llm::types::LlmResponse) -> Option<String> {
    response
        .content
        .as_ref()
        .filter(|text| !text.trim().is_empty())
        .cloned()
        .or_else(|| {
            response
                .thinking
                .as_ref()
                .filter(|text| !text.trim().is_empty())
                .cloned()
        })
}

fn extract_json_from_response(response_text: &str) -> String {
    let trimmed = response_text.trim();
    if trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```JSON")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if !without_fence.is_empty() {
            return without_fence.to_string();
        }
    }

    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return trimmed[start..=end].to_string();
            }
        }
    }
    trimmed.to_string()
}

fn ordered_matches_by_ids(matches: &[SkillMatch], selected_ids: &[String]) -> Vec<SkillMatch> {
    let by_id = matches
        .iter()
        .map(|item| (item.skill.id.clone(), item.clone()))
        .collect::<HashMap<_, _>>();
    selected_ids
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .take(SKILL_RERANK_FINAL_LIMIT)
        .collect()
}

async fn rerank_with_provider(
    provider: Arc<dyn LlmProvider>,
    project_root: &Path,
    query: &str,
    phase: &InjectionPhase,
    matches: &[SkillMatch],
) -> Result<SkillRerankResult, String> {
    let candidates = candidate_payload(project_root, matches);
    let system_prompt = format!(
        "You are a strict skill reranker for a coding agent.\n\
         Choose the smallest set of skills that best helps the current task.\n\
         Rules:\n\
         - Only select from the provided candidates.\n\
         - Return at most {SKILL_RERANK_FINAL_LIMIT} skill ids.\n\
         - Prefer highly task-relevant skills and avoid redundant overlap.\n\
         - Prefer project-local rules when they are directly relevant.\n\
         - Do not invent skill ids.\n\
         - Respond with JSON only."
    );
    let user_prompt = serde_json::to_string_pretty(&json!({
        "task": query,
        "phase": phase_label(phase),
        "max_selected_skills": SKILL_RERANK_FINAL_LIMIT,
        "candidates": candidates,
        "response_schema": {
            "selected_skill_ids": ["skill-id"],
            "rejected_skill_ids": ["skill-id"],
            "reason": "short explanation",
            "confidence": 0.0
        }
    }))
    .map_err(|error| format!("Failed to serialize skill rerank request: {error}"))?;

    let response = timeout(
        SKILL_RERANK_TIMEOUT,
        provider.send_message(
            vec![Message::user(&user_prompt)],
            Some(system_prompt),
            vec![],
            LlmRequestOptions {
                temperature_override: Some(0.1),
                ..Default::default()
            },
        ),
    )
    .await
    .map_err(|_| "timeout".to_string())?
    .map_err(|error| error.to_string())?;

    let response_text = extract_response_text(&response)
        .ok_or_else(|| "empty_response".to_string())?;
    let json_text = extract_json_from_response(&response_text);
    serde_json::from_str::<SkillRerankResult>(&json_text)
        .map_err(|error| format!("parse_error:{error}"))
}

pub async fn rerank_skill_matches(
    project_root: &Path,
    query: &str,
    phase: &InjectionPhase,
    candidates: &[SkillMatch],
    provider_config: Option<&ProviderConfig>,
) -> (Option<Vec<SkillMatch>>, SkillRerankDiagnostics) {
    if candidates.len() <= 1 {
        return (
            Some(
                candidates
                    .iter()
                    .take(SKILL_RERANK_FINAL_LIMIT)
                    .cloned()
                    .collect(),
            ),
            SkillRerankDiagnostics::default(),
        );
    }

    if query.trim().is_empty() {
        return (
            None,
            SkillRerankDiagnostics {
                skill_router_used: false,
                skill_router_strategy: Some("hybrid".to_string()),
                skill_router_reason: None,
                skill_router_confidence: None,
                skill_router_fallback_reason: Some("empty_query".to_string()),
                skill_router_selected_ids: Vec::new(),
                skill_router_latency_ms: None,
            },
        );
    }

    let Some(provider_config) = provider_config else {
        return (
            None,
            SkillRerankDiagnostics {
                skill_router_used: false,
                skill_router_strategy: Some("hybrid".to_string()),
                skill_router_reason: None,
                skill_router_confidence: None,
                skill_router_fallback_reason: Some("provider_unavailable".to_string()),
                skill_router_selected_ids: Vec::new(),
                skill_router_latency_ms: None,
            },
        );
    };

    let cache_key = build_rerank_cache_key(project_root, query, phase, provider_config, candidates);
    if let Some(cached) = get_cached_rerank(&cache_key) {
        let selected_matches = ordered_matches_by_ids(candidates, &cached.selected_skill_ids);
        return (
            Some(selected_matches),
            SkillRerankDiagnostics {
                skill_router_used: true,
                skill_router_strategy: Some("hybrid".to_string()),
                skill_router_reason: Some(cached.reason.clone()),
                skill_router_confidence: cached.confidence,
                skill_router_fallback_reason: None,
                skill_router_selected_ids: cached.selected_skill_ids,
                skill_router_latency_ms: Some(0),
            },
        );
    }

    let started_at = Instant::now();
    let provider = build_llm_provider_from_config(provider_config);
    match rerank_with_provider(provider, project_root, query, phase, candidates).await {
        Ok(result) => {
            let valid_ids = candidates
                .iter()
                .map(|item| item.skill.id.as_str())
                .collect::<std::collections::HashSet<_>>();
            let mut selected_ids = result
                .selected_skill_ids
                .iter()
                .filter(|id| valid_ids.contains(id.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            selected_ids.dedup();

            if selected_ids.is_empty() && result.confidence.is_none() {
                return (
                    None,
                    SkillRerankDiagnostics {
                        skill_router_used: true,
                        skill_router_strategy: Some("hybrid".to_string()),
                        skill_router_reason: None,
                        skill_router_confidence: None,
                        skill_router_fallback_reason: Some("invalid_empty_selection".to_string()),
                        skill_router_selected_ids: Vec::new(),
                        skill_router_latency_ms: Some(started_at.elapsed().as_millis() as u64),
                    },
                );
            }

            let ordered = ordered_matches_by_ids(candidates, &selected_ids);
            let normalized = SkillRerankResult {
                selected_skill_ids: selected_ids.clone(),
                rejected_skill_ids: result.rejected_skill_ids,
                reason: result.reason.clone(),
                confidence: result.confidence,
            };
            store_cached_rerank(cache_key, normalized);
            (
                Some(ordered),
                SkillRerankDiagnostics {
                    skill_router_used: true,
                    skill_router_strategy: Some("hybrid".to_string()),
                    skill_router_reason: Some(result.reason),
                    skill_router_confidence: result.confidence,
                    skill_router_fallback_reason: None,
                    skill_router_selected_ids: selected_ids,
                    skill_router_latency_ms: Some(started_at.elapsed().as_millis() as u64),
                },
            )
        }
        Err(error) => (
            None,
            SkillRerankDiagnostics {
                skill_router_used: true,
                skill_router_strategy: Some("hybrid".to_string()),
                skill_router_reason: None,
                skill_router_confidence: None,
                skill_router_fallback_reason: Some(error),
                skill_router_selected_ids: Vec::new(),
                skill_router_latency_ms: Some(started_at.elapsed().as_millis() as u64),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::services::llm::types::{LlmResponse, LlmResult, ProviderConfig, ToolDefinition};
    use tokio::sync::mpsc;
    use plan_cascade_core::streaming::UnifiedStreamEvent;

    #[derive(Clone)]
    struct MockProvider {
        text: String,
        config: ProviderConfig,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &'static str { "mock" }
        fn model(&self) -> &str { &self.config.model }
        fn supports_thinking(&self) -> bool { false }
        fn supports_tools(&self) -> bool { false }
        async fn send_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            Ok(LlmResponse {
                model: self.config.model.clone(),
                content: Some(self.text.clone()),
                thinking: None,
                usage: None,
                finish_reason: None,
                stop_reason: None,
                tool_calls: vec![],
                raw_response: None,
            })
        }
        async fn stream_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _tx: mpsc::Sender<UnifiedStreamEvent>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            unimplemented!()
        }
        async fn health_check(&self) -> LlmResult<()> { Ok(()) }
        fn config(&self) -> &ProviderConfig { &self.config }
    }

    fn make_match(id: &str, name: &str) -> SkillMatch {
        SkillMatch {
            score: 3.0,
            match_reason: MatchReason::LexicalMatch {
                query: "test".to_string(),
            },
            skill: crate::services::skills::model::SkillSummary {
                id: id.to_string(),
                name: name.to_string(),
                description: format!("{name} description"),
                version: None,
                tags: vec!["tag".to_string()],
                tool_policy_mode: crate::services::skills::model::SkillToolPolicyMode::Advisory,
                allowed_tools: vec![],
                source: SkillSource::Builtin,
                priority: 10,
                enabled: true,
                detected: false,
                user_invocable: false,
                has_hooks: false,
                inject_into: vec![InjectionPhase::Always],
                path: std::path::PathBuf::from(format!("builtin://{id}")),
                review_status: None,
                review_notes: None,
                reviewed_at: None,
            },
        }
    }

    #[tokio::test]
    async fn rerank_respects_selected_order() {
        let provider = Arc::new(MockProvider {
            text: r#"{"selected_skill_ids":["skill-b","skill-a"],"reason":"picked","confidence":0.91}"#.to_string(),
            config: ProviderConfig {
                model: "mock".to_string(),
                ..Default::default()
            },
        });
        let matches = vec![make_match("skill-a", "Skill A"), make_match("skill-b", "Skill B")];
        let result = rerank_with_provider(
            provider,
            Path::new("/tmp/project"),
            "update react page",
            &InjectionPhase::Implementation,
            &matches,
        )
        .await
        .expect("rerank succeeds");
        assert_eq!(result.selected_skill_ids, vec!["skill-b", "skill-a"]);
    }
}
