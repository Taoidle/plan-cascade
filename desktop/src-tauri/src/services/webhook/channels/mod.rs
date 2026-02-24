//! Webhook Channel Trait and Registry
//!
//! Defines the async trait that all webhook channel implementations must satisfy,
//! plus channel module exports.

pub mod custom;
pub mod feishu;
pub mod slack;
pub mod telegram;

use async_trait::async_trait;

use super::types::{
    WebhookChannelConfig, WebhookChannelType, WebhookError, WebhookPayload, WebhookTestResult,
};

/// Async trait for webhook channel implementations.
///
/// Each channel is responsible for formatting messages to a platform-specific
/// format and sending them via HTTP. Channels receive a proxy-aware
/// `reqwest::Client` at construction time.
#[async_trait]
pub trait WebhookChannel: Send + Sync {
    /// Channel type identifier.
    fn channel_type(&self) -> WebhookChannelType;

    /// Send a notification through this channel.
    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<(), WebhookError>;

    /// Test the channel connection by sending a test notification.
    async fn test(&self, config: &WebhookChannelConfig) -> Result<WebhookTestResult, WebhookError>;

    /// Format the payload into a platform-specific message string/JSON.
    fn format_message(&self, payload: &WebhookPayload, template: Option<&str>) -> String;
}
