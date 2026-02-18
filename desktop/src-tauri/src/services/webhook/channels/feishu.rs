//! Feishu/Lark Bot Webhook Channel
//!
//! Sends notifications via Feishu/Lark Bot Webhook using Interactive Card format.
//! Supports optional HMAC-SHA256 signature verification.

use async_trait::async_trait;

use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;
use super::WebhookChannel;

/// Feishu/Lark Bot Webhook integration.
///
/// Uses Interactive Card format with optional HMAC-SHA256 signature.
/// Webhook URL format: `https://open.feishu.cn/open-apis/bot/v2/hook/xxx`
pub struct FeishuChannel {
    client: reqwest::Client,
}

impl FeishuChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::proxy::build_http_client(proxy),
        }
    }

    /// Compute HMAC-SHA256 signature for Feishu webhook verification.
    ///
    /// The signature is computed as: HMAC-SHA256(secret, "timestamp\nsecret")
    /// where timestamp is Unix epoch seconds as a string.
    pub fn compute_signature(timestamp: &str, secret: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let string_to_sign = format!("{}\n{}", timestamp, secret);
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();

        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            result.into_bytes(),
        )
    }
}

#[async_trait]
impl WebhookChannel for FeishuChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::Feishu
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<(), WebhookError> {
        let message = self.format_message(payload, config.template.as_deref());
        let mut body: serde_json::Value = serde_json::from_str(&message)?;

        // Add signature if secret is configured
        if let Some(ref secret) = config.secret {
            if !secret.is_empty() {
                let timestamp = chrono::Utc::now().timestamp().to_string();
                let sign = Self::compute_signature(&timestamp, secret);
                body["timestamp"] = serde_json::json!(timestamp);
                body["sign"] = serde_json::json!(sign);
            }
        }

        let response = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(WebhookError::HttpError(format!(
                "Feishu returned HTTP {}: {}",
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
        let title = format!("{}", payload.event_type);

        let mut elements = Vec::new();

        if let Some(ref name) = payload.session_name {
            elements.push(serde_json::json!({
                "tag": "div",
                "text": {
                    "tag": "plain_text",
                    "content": format!("Session: {}", name)
                }
            }));
        }

        if let Some(ref path) = payload.project_path {
            elements.push(serde_json::json!({
                "tag": "div",
                "text": {
                    "tag": "plain_text",
                    "content": format!("Project: {}", path)
                }
            }));
        }

        elements.push(serde_json::json!({
            "tag": "div",
            "text": {
                "tag": "plain_text",
                "content": format!("Summary: {}", payload.summary)
            }
        }));

        if let Some(ms) = payload.duration_ms {
            let secs = ms / 1000;
            elements.push(serde_json::json!({
                "tag": "div",
                "text": {
                    "tag": "plain_text",
                    "content": format!("Duration: {}s", secs)
                }
            }));
        }

        let card = serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": title
                    }
                },
                "elements": elements
            }
        });

        serde_json::to_string(&card).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_format_message_structure() {
        let channel = FeishuChannel {
            client: reqwest::Client::new(),
        };

        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_name: Some("test-session".to_string()),
            summary: "All done".to_string(),
            duration_ms: Some(30000),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            ..Default::default()
        };

        let msg = channel.format_message(&payload, None);
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();

        assert_eq!(parsed["msg_type"], "interactive");
        assert!(parsed["card"]["header"]["title"]["content"]
            .as_str()
            .unwrap()
            .contains("TaskComplete"));
        let elements = parsed["card"]["elements"].as_array().unwrap();
        assert!(elements.len() >= 2);
    }

    #[test]
    fn test_feishu_hmac_signature_computation() {
        // Known test vector: verify the signature is stable and non-empty
        let sig = FeishuChannel::compute_signature("1234567890", "test-secret");
        assert!(!sig.is_empty());

        // Same inputs should produce the same signature
        let sig2 = FeishuChannel::compute_signature("1234567890", "test-secret");
        assert_eq!(sig, sig2);

        // Different inputs should produce different signatures
        let sig3 = FeishuChannel::compute_signature("1234567891", "test-secret");
        assert_ne!(sig, sig3);
    }
}
