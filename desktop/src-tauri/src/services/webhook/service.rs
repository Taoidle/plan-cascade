//! Webhook Service
//!
//! Central dispatcher that matches events to enabled channels and delivers
//! notifications with delivery recording and retry support.

use std::collections::HashMap;
use std::sync::Arc;

use crate::services::proxy::ProxyConfig;
use crate::storage::{Database, KeyringService};

use super::channels::custom::CustomChannel;
use super::channels::discord::DiscordChannel;
use super::channels::feishu::FeishuChannel;
use super::channels::slack::SlackChannel;
use super::channels::telegram::TelegramNotifyChannel;
use super::channels::WebhookChannel;
use super::types::*;

/// Keyring key prefix for webhook secrets.
const WEBHOOK_KEYRING_PREFIX: &str = "webhook_";

/// Retry backoff policy in seconds.
pub const WEBHOOK_RETRY_BACKOFF_SECONDS: [u64; 5] = [10, 30, 60, 120, 300];
/// Maximum delivery attempts including the first send.
pub const WEBHOOK_MAX_ATTEMPTS: u32 = 5;
/// Retention period for delivery history.
pub const WEBHOOK_RETENTION_DAYS: i64 = 30;

/// Central webhook dispatcher.
///
/// Holds a registry of channel implementations, a database reference for
/// delivery recording, and a keyring reference for secret hydration.
pub struct WebhookService {
    channels: HashMap<WebhookChannelType, Box<dyn WebhookChannel>>,
    db: Arc<Database>,
    keyring: Arc<KeyringService>,
}

impl WebhookService {
    /// Create a new WebhookService with proxy-aware HTTP clients for each channel.
    ///
    /// The `proxy_resolver` closure resolves proxy configuration for a given
    /// provider ID (e.g., "webhook_slack", "webhook_feishu").
    pub fn new(
        db: Arc<Database>,
        keyring: Arc<KeyringService>,
        proxy_resolver: impl Fn(&str) -> Option<ProxyConfig>,
    ) -> Self {
        let mut channels: HashMap<WebhookChannelType, Box<dyn WebhookChannel>> = HashMap::new();

        channels.insert(
            WebhookChannelType::Slack,
            Box::new(SlackChannel::new(proxy_resolver("webhook_slack").as_ref())),
        );
        channels.insert(
            WebhookChannelType::Feishu,
            Box::new(FeishuChannel::new(
                proxy_resolver("webhook_feishu").as_ref(),
            )),
        );
        channels.insert(
            WebhookChannelType::Telegram,
            Box::new(TelegramNotifyChannel::new(
                proxy_resolver("webhook_telegram").as_ref(),
            )),
        );
        channels.insert(
            WebhookChannelType::Discord,
            Box::new(DiscordChannel::new(
                proxy_resolver("webhook_discord").as_ref(),
            )),
        );
        channels.insert(
            WebhookChannelType::Custom,
            Box::new(CustomChannel::new(
                proxy_resolver("webhook_custom").as_ref(),
            )),
        );

        Self {
            channels,
            db,
            keyring,
        }
    }

    /// Build a service using the shared proxy resolution strategy.
    pub fn new_default(db: Arc<Database>, keyring: Arc<KeyringService>) -> Self {
        let db_for_proxy = db.clone();
        let keyring_for_proxy = keyring.clone();
        Self::new(db, keyring, move |provider| {
            crate::commands::proxy::resolve_provider_proxy(
                keyring_for_proxy.as_ref(),
                db_for_proxy.as_ref(),
                provider,
            )
        })
    }

    /// Dispatch a notification to all matching channels.
    ///
    /// Loads enabled channel configs from the database, filters by scope and
    /// event type, hydrates secrets from Keyring, sends via matching channel
    /// implementations, and records delivery status.
    pub async fn dispatch(&self, payload: WebhookPayload) -> Vec<WebhookDelivery> {
        let configs = self.get_enabled_configs_for_event(&payload);
        let mut deliveries = Vec::new();

        for mut config in configs {
            let channel = match self.channels.get(&config.channel_type) {
                Some(channel) => channel,
                None => continue,
            };
            self.hydrate_secret(&mut config);

            let mut effective_payload = payload.clone();
            if let Some(rendered_summary) =
                self.try_render_summary(config.template.as_deref(), &effective_payload)
            {
                effective_payload.summary = rendered_summary;
            }

            let mut delivery = WebhookDelivery::new(&config, &effective_payload);
            delivery.attempts = 1;
            self.send_once(channel.as_ref(), &config, &effective_payload, &mut delivery)
                .await;

            self.save_delivery(&delivery);
            deliveries.push(delivery);
        }

        deliveries
    }

    /// Retry failed deliveries according to retry policy and due timestamp.
    pub async fn retry_failed(&self, max_attempts: u32) -> Vec<WebhookDelivery> {
        let now = chrono::Utc::now().to_rfc3339();
        let failed = self
            .db
            .get_deliveries_due_for_retry(max_attempts, &now)
            .unwrap_or_default();
        let mut results = Vec::new();

        for delivery in failed {
            match self.retry_delivery_internal(delivery).await {
                Ok(updated) => results.push(updated),
                Err(e) => {
                    tracing::warn!(error = %e, "webhook retry failed unexpectedly");
                }
            }
        }

        results
    }

    /// Retry one delivery by ID using the original payload.
    pub async fn retry_delivery_by_id(
        &self,
        delivery_id: &str,
    ) -> Result<WebhookDelivery, WebhookError> {
        let delivery = self
            .db
            .get_webhook_delivery(delivery_id)
            .map_err(|e| WebhookError::DatabaseError(e.to_string()))?
            .ok_or_else(|| WebhookError::DatabaseError(format!("Delivery not found: {}", delivery_id)))?;

        if delivery.status == DeliveryStatus::Success {
            return Err(WebhookError::InvalidConfig(
                "Cannot retry a successful delivery".to_string(),
            ));
        }

        self.retry_delivery_internal(delivery).await
    }

    /// Test a specific channel by sending a test notification.
    pub async fn test_channel(
        &self,
        config: &WebhookChannelConfig,
    ) -> Result<WebhookTestResult, WebhookError> {
        let channel = self
            .channels
            .get(&config.channel_type)
            .ok_or_else(|| WebhookError::ChannelNotFound(config.channel_type.to_string()))?;

        channel.test(config).await
    }

    /// Query failed delivery queue size.
    pub fn failed_queue_length(&self) -> u32 {
        self.db.count_failed_webhook_deliveries().unwrap_or(0)
    }

    /// Cleanup old delivery records and return deleted count.
    pub fn cleanup_old_deliveries(&self, retention_days: i64) -> u32 {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
        self.db
            .delete_webhook_deliveries_before(&cutoff.to_rfc3339())
            .unwrap_or(0)
    }

    /// Get enabled channel configs that match the event type and session scope.
    fn get_enabled_configs_for_event(&self, payload: &WebhookPayload) -> Vec<WebhookChannelConfig> {
        let all_configs = match self.db.list_webhook_channels() {
            Ok(configs) => configs,
            Err(_) => return Vec::new(),
        };

        all_configs
            .into_iter()
            .filter(|config| {
                if !config.enabled {
                    return false;
                }

                if !config.events.contains(&payload.event_type) {
                    return false;
                }

                match &config.scope {
                    WebhookScope::Global => true,
                    WebhookScope::Sessions(session_ids) => {
                        if let Some(ref session_id) = payload.session_id {
                            session_ids.contains(session_id)
                        } else {
                            false
                        }
                    }
                }
            })
            .collect()
    }

    /// Load a single channel config from the database.
    fn get_channel_config(&self, channel_id: &str) -> Option<WebhookChannelConfig> {
        self.db.get_webhook_channel(channel_id).ok().flatten()
    }

    /// Persist one delivery record.
    fn save_delivery(&self, delivery: &WebhookDelivery) {
        let _ = self.db.insert_webhook_delivery(delivery);
    }

    /// Update an existing delivery record.
    fn update_delivery_status(&self, delivery: &WebhookDelivery) {
        let _ = self.db.update_webhook_delivery_status(delivery);
    }

    fn hydrate_secret(&self, config: &mut WebhookChannelConfig) {
        let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, config.id);
        if let Ok(Some(secret)) = self.keyring.get_api_key(&keyring_key) {
            config.secret = Some(secret);
        }
    }

    async fn retry_delivery_internal(
        &self,
        mut delivery: WebhookDelivery,
    ) -> Result<WebhookDelivery, WebhookError> {
        let mut config = self
            .get_channel_config(&delivery.channel_id)
            .ok_or_else(|| WebhookError::ChannelNotFound(delivery.channel_id.clone()))?;
        self.hydrate_secret(&mut config);

        let channel = self
            .channels
            .get(&config.channel_type)
            .ok_or_else(|| WebhookError::ChannelNotFound(config.channel_type.to_string()))?;

        delivery.status = DeliveryStatus::Retrying;
        delivery.attempts = delivery.attempts.saturating_add(1);
        delivery.last_attempt_at = chrono::Utc::now().to_rfc3339();
        delivery.next_retry_at = None;

        self.send_once(channel.as_ref(), &config, &delivery.payload.clone(), &mut delivery)
            .await;
        self.update_delivery_status(&delivery);

        Ok(delivery)
    }

    async fn send_once(
        &self,
        channel: &dyn WebhookChannel,
        config: &WebhookChannelConfig,
        payload: &WebhookPayload,
        delivery: &mut WebhookDelivery,
    ) {
        match channel.send(payload, config).await {
            Ok(send_result) => {
                delivery.status_code = send_result.status_code;
                delivery.response_body = send_result.response_body.clone();
                delivery.last_attempt_at = chrono::Utc::now().to_rfc3339();

                if send_result.success {
                    delivery.status = DeliveryStatus::Success;
                    delivery.next_retry_at = None;
                    delivery.last_error = None;
                } else {
                    self.mark_failed(
                        delivery,
                        send_result.error.unwrap_or_else(|| "delivery failed".to_string()),
                    );
                }

                let status_str = if send_result.success { "success" } else { "failed" };
                tracing::info!(
                    channel_id = %delivery.channel_id,
                    event_type = %delivery.event_type,
                    delivery_id = %delivery.id,
                    attempt = delivery.attempts,
                    latency_ms = send_result.latency_ms,
                    status = status_str,
                    status_code = ?delivery.status_code,
                    "webhook delivery attempted"
                );
            }
            Err(e) => {
                delivery.status_code = None;
                delivery.response_body = None;
                delivery.last_attempt_at = chrono::Utc::now().to_rfc3339();
                self.mark_failed(delivery, e.to_string());
                tracing::warn!(
                    channel_id = %delivery.channel_id,
                    event_type = %delivery.event_type,
                    delivery_id = %delivery.id,
                    attempt = delivery.attempts,
                    status = "failed",
                    error = %delivery.last_error.as_deref().unwrap_or("unknown"),
                    "webhook delivery failed"
                );
            }
        }
    }

    fn mark_failed(&self, delivery: &mut WebhookDelivery, error: String) {
        delivery.status = DeliveryStatus::Failed;
        delivery.last_error = Some(sanitize_error(&error));
        if delivery.attempts >= WEBHOOK_MAX_ATTEMPTS {
            delivery.next_retry_at = None;
        } else {
            delivery.next_retry_at = next_retry_at_for_attempt(delivery.attempts);
        }
    }

    fn try_render_summary(&self, template: Option<&str>, payload: &WebhookPayload) -> Option<String> {
        let Some(template) = template else {
            return None;
        };
        if template.trim().is_empty() {
            return None;
        }

        match render_summary_template(template, payload) {
            Ok(summary) => Some(summary),
            Err(err) => {
                tracing::warn!(
                    event_type = %payload.event_type,
                    session_id = ?payload.session_id,
                    error = %err,
                    "webhook summary template render failed; fallback to default summary"
                );
                None
            }
        }
    }
}

fn next_retry_at_for_attempt(attempts: u32) -> Option<String> {
    if attempts == 0 {
        return None;
    }
    let index = attempts.saturating_sub(1) as usize;
    let delay = WEBHOOK_RETRY_BACKOFF_SECONDS.get(index)?;
    Some((chrono::Utc::now() + chrono::Duration::seconds(*delay as i64)).to_rfc3339())
}

fn render_summary_template(template: &str, payload: &WebhookPayload) -> Result<String, String> {
    let mut rendered = template.to_string();
    let replacements = [
        ("event_type", format!("{}", payload.event_type)),
        ("session_id", payload.session_id.clone().unwrap_or_default()),
        ("session_name", payload.session_name.clone().unwrap_or_default()),
        ("project_path", payload.project_path.clone().unwrap_or_default()),
        ("summary", payload.summary.clone()),
        ("timestamp", payload.timestamp.clone()),
        (
            "duration_ms",
            payload.duration_ms.map(|v| v.to_string()).unwrap_or_default(),
        ),
        ("remote_source", payload.remote_source.clone().unwrap_or_default()),
        (
            "details",
            payload
                .details
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
    ];

    for (key, value) in replacements {
        rendered = rendered.replace(&format!("{{{{{}}}}}", key), &value);
    }

    if rendered.contains("{{") || rendered.contains("}}") {
        return Err("template contains unsupported placeholders".to_string());
    }
    if rendered.trim().is_empty() {
        return Err("rendered summary is empty".to_string());
    }

    Ok(rendered)
}

fn sanitize_error(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("token") || lower.contains("secret") || lower.contains("password") {
        "delivery failed (sensitive details omitted)".to_string()
    } else {
        message.to_string()
    }
}

impl std::fmt::Debug for WebhookService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookService")
            .field("channels", &self.channels.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_retry_at_for_attempts() {
        assert!(next_retry_at_for_attempt(0).is_none());
        assert!(next_retry_at_for_attempt(1).is_some());
        assert!(next_retry_at_for_attempt(5).is_some());
        assert!(next_retry_at_for_attempt(6).is_none());
    }

    #[test]
    fn test_render_summary_template_success() {
        let payload = WebhookPayload {
            summary: "hello".to_string(),
            session_id: Some("s1".to_string()),
            ..Default::default()
        };
        let rendered =
            render_summary_template("Session {{session_id}}: {{summary}}", &payload).unwrap();
        assert_eq!(rendered, "Session s1: hello");
    }

    #[test]
    fn test_render_summary_template_fails_on_unknown_placeholder() {
        let payload = WebhookPayload {
            summary: "hello".to_string(),
            ..Default::default()
        };
        let err = render_summary_template("{{unknown}}", &payload).unwrap_err();
        assert!(err.contains("unsupported placeholders"));
    }

    #[test]
    fn test_sanitize_error() {
        assert_eq!(
            sanitize_error("invalid token supplied"),
            "delivery failed (sensitive details omitted)"
        );
        assert_eq!(sanitize_error("http 500"), "http 500");
    }
}
