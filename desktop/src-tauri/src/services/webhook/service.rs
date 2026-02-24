//! Webhook Service
//!
//! Central dispatcher that matches events to enabled channels and delivers
//! notifications with delivery recording and retry support.

use std::collections::HashMap;
use std::sync::Arc;

use crate::services::proxy::ProxyConfig;
use crate::storage::{Database, KeyringService};

use super::channels::custom::CustomChannel;
use super::channels::feishu::FeishuChannel;
use super::channels::slack::SlackChannel;
use super::channels::telegram::TelegramNotifyChannel;
use super::channels::WebhookChannel;
use super::types::*;

/// Keyring key prefix for webhook secrets.
const WEBHOOK_KEYRING_PREFIX: &str = "webhook_";

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

    /// Dispatch a notification to all matching channels.
    ///
    /// Loads enabled channel configs from the database, filters by scope and
    /// event type, hydrates secrets from Keyring, sends via matching channel
    /// implementations, and records delivery status.
    pub async fn dispatch(&self, payload: WebhookPayload) -> Vec<WebhookDelivery> {
        let configs = self.get_enabled_configs_for_event(&payload);
        let mut deliveries = Vec::new();

        for mut config in configs {
            let channel = self.channels.get(&config.channel_type);
            if let Some(channel) = channel {
                // Hydrate secret from Keyring
                let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, config.id);
                if let Ok(Some(secret)) = self.keyring.get_api_key(&keyring_key) {
                    config.secret = Some(secret);
                }

                let mut delivery = WebhookDelivery::new(&config, &payload);
                delivery.attempts = 1;

                match channel.send(&payload, &config).await {
                    Ok(()) => {
                        delivery.status = DeliveryStatus::Success;
                    }
                    Err(e) => {
                        delivery.status = DeliveryStatus::Failed;
                        delivery.response_body = Some(e.to_string());
                    }
                }

                delivery.last_attempt_at = chrono::Utc::now().to_rfc3339();
                self.save_delivery(&delivery);
                deliveries.push(delivery);
            }
        }

        deliveries
    }

    /// Retry failed deliveries.
    ///
    /// Loads failed deliveries with attempts < max_attempts, respects
    /// exponential backoff timing (2^attempts seconds since last attempt),
    /// re-attempts sending, and updates delivery status.
    pub async fn retry_failed(&self, max_attempts: u32) -> Vec<WebhookDelivery> {
        let failed = self.get_failed_deliveries_for_retry(max_attempts);
        let mut results = Vec::new();

        let now = chrono::Utc::now();

        for mut delivery in failed {
            // Check exponential backoff: last_attempt_at + 2^attempts seconds < now
            if let Ok(last_attempt) =
                chrono::DateTime::parse_from_rfc3339(&delivery.last_attempt_at)
            {
                let backoff_secs = 2i64.pow(delivery.attempts);
                let next_allowed = last_attempt + chrono::Duration::seconds(backoff_secs);
                if now < next_allowed {
                    continue; // Too early to retry
                }
            }

            // Load channel config for this delivery
            let config = self.get_channel_config(&delivery.channel_id);
            if config.is_none() {
                continue;
            }
            let mut config = config.unwrap();

            // Hydrate secret
            let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, config.id);
            if let Ok(Some(secret)) = self.keyring.get_api_key(&keyring_key) {
                config.secret = Some(secret);
            }

            let channel = self.channels.get(&config.channel_type);
            if let Some(channel) = channel {
                delivery.status = DeliveryStatus::Retrying;
                delivery.attempts += 1;
                delivery.last_attempt_at = now.to_rfc3339();

                match channel.send(&delivery.payload, &config).await {
                    Ok(()) => {
                        delivery.status = DeliveryStatus::Success;
                    }
                    Err(e) => {
                        delivery.status = DeliveryStatus::Failed;
                        delivery.response_body = Some(e.to_string());
                    }
                }

                self.update_delivery_status(&delivery);
                results.push(delivery);
            }
        }

        results
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

                // Check event type match
                if !config.events.contains(&payload.event_type) {
                    return false;
                }

                // Check scope match
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

    /// Save a delivery record to the database.
    fn save_delivery(&self, delivery: &WebhookDelivery) {
        let _ = self.db.insert_webhook_delivery(delivery);
    }

    /// Update an existing delivery status in the database.
    fn update_delivery_status(&self, delivery: &WebhookDelivery) {
        let _ = self.db.update_webhook_delivery_status(delivery);
    }

    /// Get failed deliveries eligible for retry.
    fn get_failed_deliveries_for_retry(&self, max_attempts: u32) -> Vec<WebhookDelivery> {
        self.db
            .get_failed_deliveries(max_attempts)
            .unwrap_or_default()
    }
}

impl std::fmt::Debug for WebhookService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookService")
            .field("channels", &self.channels.keys().collect::<Vec<_>>())
            .finish()
    }
}
