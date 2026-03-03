//! Feishu/Lark Bot Webhook Channel
//!
//! Supports two endpoint modes:
//! - Custom bot webhook (`/open-apis/bot/v2/hook/...`): sends interactive card payload.
//! - Bot assistant webhook trigger: sends `msg_type + key/value` JSON payload.
//! Custom bot mode supports optional HMAC-SHA256 signature verification.

use async_trait::async_trait;

use super::{format_timestamp_for_display, WebhookChannel};
use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;

/// Feishu/Lark Bot Webhook integration.
///
/// Uses endpoint-aware payload formatting:
/// - Custom bot webhook URL: `https://open.feishu.cn/open-apis/bot/v2/hook/xxx`
/// - Webhook trigger URL: arbitrary HTTPS URL from bot assistant workflow
pub struct FeishuChannel {
    client: reqwest::Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeishuEndpointMode {
    CustomBotWebhook,
    WebhookTrigger,
}

impl FeishuChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::webhook::http_client::build_webhook_http_client(proxy),
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

    /// Escape text for Lark Markdown fields.
    fn escape_lark_md(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn escape_inline_code(text: &str) -> String {
        Self::escape_lark_md(text).replace('`', "\\`")
    }

    fn detect_endpoint_mode(url: &str) -> FeishuEndpointMode {
        let Ok(parsed) = url::Url::parse(url) else {
            return FeishuEndpointMode::WebhookTrigger;
        };
        let path = parsed.path().to_ascii_lowercase();
        if path.starts_with("/open-apis/bot/v2/hook/") || path == "/open-apis/bot/v2/hook" {
            return FeishuEndpointMode::CustomBotWebhook;
        }
        FeishuEndpointMode::WebhookTrigger
    }

    fn format_markdown_lines_lark(payload: &WebhookPayload) -> String {
        let mut markdown_lines = Vec::new();
        markdown_lines.push(format!(
            "**Event**: {}",
            Self::escape_lark_md(&payload.event_type.to_string())
        ));
        if let Some(ref name) = payload.session_name {
            markdown_lines.push(format!("**Session**: {}", Self::escape_lark_md(name)));
        }
        if let Some(ref path) = payload.project_path {
            markdown_lines.push(format!("**Project**: `{}`", Self::escape_inline_code(path)));
        }
        if let Some(ref source) = payload.remote_source {
            markdown_lines.push(format!("**Source**: {}", Self::escape_lark_md(source)));
        }
        markdown_lines.push(format!(
            "**Summary**: {}",
            Self::escape_lark_md(&payload.summary)
        ));
        if let Some(ms) = payload.duration_ms {
            let secs = ms / 1000;
            markdown_lines.push(format!("**Duration**: {}s", secs));
        }
        markdown_lines.push(format!(
            "**Time**: {}",
            Self::escape_lark_md(&format_timestamp_for_display(&payload.timestamp))
        ));
        markdown_lines.join("\n")
    }

    fn format_markdown_lines_trigger(payload: &WebhookPayload) -> String {
        let mut lines = Vec::new();
        lines.push(format!("**Event**: {}", payload.event_type));
        if let Some(ref name) = payload.session_name {
            lines.push(format!("**Session**: {}", name));
        }
        if let Some(ref path) = payload.project_path {
            lines.push(format!("**Project**: `{}`", path.replace('`', "\\`")));
        }
        if let Some(ref source) = payload.remote_source {
            lines.push(format!("**Source**: {}", source));
        }
        lines.push(format!("**Summary**: {}", payload.summary));
        if let Some(ms) = payload.duration_ms {
            lines.push(format!("**Duration**: {}s", ms / 1000));
        }
        lines.push(format!(
            "**Time**: {}",
            format_timestamp_for_display(&payload.timestamp)
        ));
        lines.join("\n")
    }

    fn build_custom_bot_body(payload: &WebhookPayload) -> serde_json::Value {
        let title = payload.event_type.to_string();
        let markdown_content = Self::format_markdown_lines_lark(payload);
        serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": title
                    }
                },
                "elements": [
                    {
                        "tag": "div",
                        "text": {
                            "tag": "lark_md",
                            "content": markdown_content
                        }
                    }
                ]
            }
        })
    }

    fn build_webhook_trigger_body(payload: &WebhookPayload) -> serde_json::Value {
        let mut body = serde_json::Map::new();
        body.insert("msg_type".to_string(), serde_json::json!("text"));
        body.insert(
            "event_type".to_string(),
            serde_json::json!(payload.event_type.to_string()),
        );
        body.insert(
            "title".to_string(),
            serde_json::json!(payload.event_type.to_string()),
        );
        body.insert("summary".to_string(), serde_json::json!(payload.summary));
        body.insert("content".to_string(), serde_json::json!(payload.summary));
        body.insert(
            "markdown".to_string(),
            serde_json::json!(Self::format_markdown_lines_trigger(payload)),
        );
        body.insert(
            "timestamp".to_string(),
            serde_json::json!(payload.timestamp),
        );

        if let Some(ref session_id) = payload.session_id {
            body.insert("session_id".to_string(), serde_json::json!(session_id));
        }
        if let Some(ref session_name) = payload.session_name {
            body.insert("session_name".to_string(), serde_json::json!(session_name));
        }
        if let Some(ref project_path) = payload.project_path {
            body.insert("project_path".to_string(), serde_json::json!(project_path));
        }
        if let Some(duration_ms) = payload.duration_ms {
            body.insert("duration_ms".to_string(), serde_json::json!(duration_ms));
        }
        if let Some(ref remote_source) = payload.remote_source {
            body.insert(
                "remote_source".to_string(),
                serde_json::json!(remote_source),
            );
        }
        if let Some(ref token_usage) = payload.token_usage {
            body.insert("token_usage".to_string(), serde_json::json!(token_usage));
        }
        if let Some(ref details) = payload.details {
            body.insert("details".to_string(), details.clone());
        }

        serde_json::Value::Object(body)
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
    ) -> Result<WebhookSendResult, WebhookError> {
        let endpoint_mode = Self::detect_endpoint_mode(&config.url);
        let mut body = match endpoint_mode {
            FeishuEndpointMode::CustomBotWebhook => Self::build_custom_bot_body(payload),
            FeishuEndpointMode::WebhookTrigger => Self::build_webhook_trigger_body(payload),
        };
        let started = std::time::Instant::now();

        // Signature is only supported by custom bot webhook endpoints.
        if endpoint_mode == FeishuEndpointMode::CustomBotWebhook {
            if let Some(ref secret) = config.secret {
                if !secret.is_empty() {
                    let timestamp = chrono::Utc::now().timestamp().to_string();
                    let sign = Self::compute_signature(&timestamp, secret);
                    body["timestamp"] = serde_json::json!(timestamp);
                    body["sign"] = serde_json::json!(sign);
                }
            }
        }

        let response = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let status = response.status().as_u16();
        let response_body = response.text().await.ok().filter(|s| !s.is_empty());

        if status >= 200 && status < 300 {
            Ok(WebhookSendResult {
                success: true,
                status_code: Some(status),
                latency_ms: started.elapsed().as_millis() as u32,
                response_body,
                error: None,
            })
        } else {
            Ok(WebhookSendResult {
                success: false,
                status_code: Some(status),
                latency_ms: started.elapsed().as_millis() as u32,
                response_body: response_body.clone(),
                error: Some(format!("Feishu returned HTTP {}", status)),
            })
        }
    }

    async fn test(&self, config: &WebhookChannelConfig) -> Result<WebhookTestResult, WebhookError> {
        let test_payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            summary: "Test notification from Plan Cascade".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        };

        match self.send(&test_payload, config).await {
            Ok(send_result) => Ok(WebhookTestResult {
                success: send_result.success,
                latency_ms: Some(send_result.latency_ms),
                error: send_result.error,
            }),
            Err(e) => Ok(WebhookTestResult {
                success: false,
                latency_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }

    fn format_message(&self, payload: &WebhookPayload, _template: Option<&str>) -> String {
        let body = Self::build_custom_bot_body(payload);
        serde_json::to_string(&body).unwrap_or_default()
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
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0]["text"]["tag"], "lark_md");
        let content = elements[0]["text"]["content"].as_str().unwrap();
        assert!(content.contains("**Summary**"));
        assert!(content.contains("All done"));
    }

    #[test]
    fn test_feishu_detect_endpoint_mode() {
        assert_eq!(
            FeishuChannel::detect_endpoint_mode("https://open.feishu.cn/open-apis/bot/v2/hook/abc"),
            FeishuEndpointMode::CustomBotWebhook
        );
        assert_eq!(
            FeishuChannel::detect_endpoint_mode("https://open.feishu.cn/open-apis/bot/v2/hook"),
            FeishuEndpointMode::CustomBotWebhook
        );
        assert_eq!(
            FeishuChannel::detect_endpoint_mode("https://botbuilder.feishu.cn/flow/webhook/abc"),
            FeishuEndpointMode::WebhookTrigger
        );
    }

    #[test]
    fn test_feishu_webhook_trigger_payload_uses_msg_type_text() {
        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskFailed,
            session_id: Some("sess-123".to_string()),
            session_name: Some("demo".to_string()),
            summary: "Build failed".to_string(),
            timestamp: "2026-03-03T00:00:00Z".to_string(),
            duration_ms: Some(15000),
            ..Default::default()
        };

        let body = FeishuChannel::build_webhook_trigger_body(&payload);
        assert_eq!(body["msg_type"], "text");
        assert_eq!(body["event_type"], "TaskFailed");
        assert_eq!(body["session_id"], "sess-123");
        assert_eq!(body["summary"], "Build failed");
        assert!(body["markdown"]
            .as_str()
            .unwrap_or_default()
            .contains("**Summary**: Build failed"));
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
