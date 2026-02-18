//! Custom HTTP Webhook Channel
//!
//! Generic HTTP POST webhook with optional HMAC-SHA256 signature header.

use async_trait::async_trait;

use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;
use super::WebhookChannel;

/// Generic HTTP webhook for custom integrations.
///
/// POSTs JSON payload to any URL with optional HMAC-SHA256 signature header.
///
/// Headers included:
/// - `Content-Type: application/json`
/// - `X-Webhook-Signature: sha256=<HMAC of body using secret>` (when secret is set)
/// - `X-Webhook-Event: <event_type>`
pub struct CustomChannel {
    client: reqwest::Client,
}

impl CustomChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::proxy::build_http_client(proxy),
        }
    }

    /// Compute HMAC-SHA256 of the body using the secret, returning hex-encoded signature.
    pub fn compute_hmac_sha256(secret: &str, body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
        mac.update(body.as_bytes());
        let result = mac.finalize();

        // Hex encode
        result
            .into_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[async_trait]
impl WebhookChannel for CustomChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::Custom
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<(), WebhookError> {
        let body = self.format_message(payload, config.template.as_deref());

        let mut request = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Event", format!("{}", payload.event_type));

        // Add HMAC signature if secret is configured
        if let Some(ref secret) = config.secret {
            if !secret.is_empty() {
                let signature = Self::compute_hmac_sha256(secret, &body);
                request = request.header("X-Webhook-Signature", format!("sha256={}", signature));
            }
        }

        let response = request.body(body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(WebhookError::HttpError(format!(
                "Custom webhook returned HTTP {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn test(
        &self,
        config: &WebhookChannelConfig,
    ) -> Result<WebhookTestResult, WebhookError> {
        let test_payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            summary: "Test notification from Plan Cascade".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        match self.send(&test_payload, config).await {
            Ok(()) => Ok(WebhookTestResult {
                success: true,
                latency_ms: Some(start.elapsed().as_millis() as u32),
                error: None,
            }),
            Err(e) => Ok(WebhookTestResult {
                success: false,
                latency_ms: Some(start.elapsed().as_millis() as u32),
                error: Some(e.to_string()),
            }),
        }
    }

    fn format_message(&self, payload: &WebhookPayload, _template: Option<&str>) -> String {
        // Custom channel sends the raw payload as JSON
        serde_json::to_string(payload).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_hmac_sha256_computation() {
        let sig = CustomChannel::compute_hmac_sha256("my-secret", "hello world");
        assert!(!sig.is_empty());
        // Should be hex-encoded (64 chars for SHA-256)
        assert_eq!(sig.len(), 64);
        // All hex characters
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));

        // Same inputs should produce the same signature
        let sig2 = CustomChannel::compute_hmac_sha256("my-secret", "hello world");
        assert_eq!(sig, sig2);

        // Different inputs should produce different signatures
        let sig3 = CustomChannel::compute_hmac_sha256("other-secret", "hello world");
        assert_ne!(sig, sig3);
    }

    #[test]
    fn test_custom_format_message_is_raw_payload() {
        let channel = CustomChannel {
            client: reqwest::Client::new(),
        };

        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskFailed,
            summary: "Something went wrong".to_string(),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            ..Default::default()
        };

        let msg = channel.format_message(&payload, None);
        let parsed: WebhookPayload = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed.summary, "Something went wrong");
        assert_eq!(parsed.event_type, WebhookEventType::TaskFailed);
    }
}
