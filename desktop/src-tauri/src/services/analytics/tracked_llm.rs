use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::models::analytics::{AnalyticsAttribution, AnalyticsUsageEvent, CostStatus};
use crate::services::analytics::{CostCalculator, TrackerMessage};
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{
    FallbackToolFormatMode, LlmRequestOptions, LlmResponse, LlmResult, Message, ProviderConfig,
    ToolCallReliability, ToolDefinition,
};
use plan_cascade_core::streaming::UnifiedStreamEvent;

pub async fn send_message_tracked(
    provider: &dyn LlmProvider,
    messages: Vec<Message>,
    system: Option<String>,
    tools: Vec<ToolDefinition>,
    request_options: LlmRequestOptions,
) -> LlmResult<LlmResponse> {
    provider
        .send_message(messages, system, tools, request_options)
        .await
}

fn build_usage_event(
    attribution: &AnalyticsAttribution,
    provider_name: &str,
    model_name: &str,
    response: &LlmResponse,
    cost_calculator: &CostCalculator,
) -> AnalyticsUsageEvent {
    let input_tokens = response.usage.input_tokens as i64;
    let output_tokens = response.usage.output_tokens as i64;
    let thinking_tokens = response.usage.thinking_tokens.unwrap_or(0) as i64;
    let cache_read_tokens = response.usage.cache_read_tokens.unwrap_or(0) as i64;
    let cache_write_tokens = response.usage.cache_creation_tokens.unwrap_or(0) as i64;

    let cost_total = cost_calculator.calculate_cost(provider_name, model_name, input_tokens, output_tokens);

    AnalyticsUsageEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp_utc: chrono::Utc::now().timestamp(),
        provider: provider_name.to_string(),
        model: model_name.to_string(),
        input_tokens,
        output_tokens,
        thinking_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost_total,
        cost_status: if cost_total > 0 {
            CostStatus::Estimated
        } else {
            CostStatus::Missing
        },
        project_id: attribution.project_id.clone(),
        kernel_session_id: attribution.kernel_session_id.clone(),
        mode_session_id: attribution.mode_session_id.clone(),
        workflow_mode: attribution.workflow_mode.clone(),
        phase_id: attribution.phase_id.clone(),
        execution_scope: attribution.execution_scope.clone(),
        execution_id: attribution.execution_id.clone(),
        parent_execution_id: attribution.parent_execution_id.clone(),
        agent_role: attribution.agent_role.clone(),
        agent_name: attribution.agent_name.clone(),
        step_id: attribution.step_id.clone(),
        story_id: attribution.story_id.clone(),
        gate_id: attribution.gate_id.clone(),
        attempt: attribution.attempt,
        request_sequence: attribution.request_sequence,
        call_site: attribution.call_site.clone(),
        metadata_json: attribution.metadata_json.clone(),
    }
}

pub fn next_request_sequence(counter: &Arc<AtomicI64>) -> i64 {
    counter.fetch_add(1, Ordering::Relaxed)
}

pub fn wrap_provider_with_tracking(
    inner: Arc<dyn LlmProvider>,
    analytics_tx: mpsc::Sender<TrackerMessage>,
    cost_calculator: Arc<CostCalculator>,
    attribution: AnalyticsAttribution,
) -> Arc<dyn LlmProvider> {
    Arc::new(TrackedLlmProvider::new(
        inner,
        analytics_tx,
        cost_calculator,
        attribution,
    ))
}

pub struct TrackedLlmProvider {
    inner: Arc<dyn LlmProvider>,
    analytics_tx: mpsc::Sender<TrackerMessage>,
    cost_calculator: Arc<CostCalculator>,
    attribution: AnalyticsAttribution,
    request_sequence: Arc<AtomicI64>,
}

impl TrackedLlmProvider {
    pub fn new(
        inner: Arc<dyn LlmProvider>,
        analytics_tx: mpsc::Sender<TrackerMessage>,
        cost_calculator: Arc<CostCalculator>,
        mut attribution: AnalyticsAttribution,
    ) -> Self {
        if attribution.request_sequence.is_none() {
            attribution.request_sequence = Some(1);
        }
        let starting_sequence = attribution.request_sequence.unwrap_or(1);
        Self {
            inner,
            analytics_tx,
            cost_calculator,
            attribution,
            request_sequence: Arc::new(AtomicI64::new(starting_sequence)),
        }
    }

    fn build_attribution_for_call(&self) -> AnalyticsAttribution {
        let mut attribution = self.attribution.clone();
        attribution.request_sequence = Some(next_request_sequence(&self.request_sequence));
        attribution
    }

    fn record_usage(&self, response: &LlmResponse, attribution: &AnalyticsAttribution) {
        let provider_name = self.inner.name();
        let model_name = response.model.as_str();
        let event = build_usage_event(
            attribution,
            provider_name,
            model_name,
            response,
            self.cost_calculator.as_ref(),
        );
        let _ = self.analytics_tx.try_send(TrackerMessage::TrackEvent(event));
    }
}

#[async_trait]
impl LlmProvider for TrackedLlmProvider {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn supports_thinking(&self) -> bool {
        self.inner.supports_thinking()
    }

    fn supports_tools(&self) -> bool {
        self.inner.supports_tools()
    }

    fn tool_call_reliability(&self) -> ToolCallReliability {
        self.inner.tool_call_reliability()
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        self.inner.default_fallback_mode()
    }

    fn supports_multimodal(&self) -> bool {
        self.inner.supports_multimodal()
    }

    fn supports_native_search(&self) -> bool {
        self.inner.supports_native_search()
    }

    fn context_window(&self) -> u32 {
        self.inner.context_window()
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let attribution = self.build_attribution_for_call();
        let response = self
            .inner
            .send_message(messages, system, tools, request_options)
            .await?;
        self.record_usage(&response, &attribution);
        Ok(response)
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let attribution = self.build_attribution_for_call();
        let response = self
            .inner
            .stream_message(messages, system, tools, tx, request_options)
            .await?;
        self.record_usage(&response, &attribution);
        Ok(response)
    }

    async fn health_check(&self) -> LlmResult<()> {
        self.inner.health_check().await
    }

    fn config(&self) -> &ProviderConfig {
        self.inner.config()
    }

    async fn list_models(&self) -> LlmResult<Option<Vec<String>>> {
        self.inner.list_models().await
    }
}
