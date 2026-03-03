//! ServerChan Webhook Channel
//!
//! Sends notifications via ServerChan API (`text` + `desp` form fields).
//! `desp` supports Markdown rendering.

use std::collections::HashMap;

use async_trait::async_trait;

use super::{format_timestamp_for_display, WebhookChannel};
use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;

/// ServerChan integration.
///
/// Configuration:
/// - `config.url`: API base URL (for example `https://sctapi.ftqq.com`) or full `*.send` URL.
/// - `config.secret`: SENDKEY (recommended, stored in keyring).
pub struct ServerChanChannel {
    client: reqwest::Client,
}

impl ServerChanChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::webhook::http_client::build_webhook_http_client(proxy),
        }
    }

    fn parse_sctp_node(send_key: &str) -> Option<&str> {
        let suffix = send_key.strip_prefix("sctp")?;
        let digits_len = suffix
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digits_len == 0 {
            return None;
        }
        let (digits, rest) = suffix.split_at(digits_len);
        if !rest.starts_with('t') {
            return None;
        }
        Some(digits)
    }

    fn resolve_endpoint(config: &WebhookChannelConfig) -> Result<String, WebhookError> {
        let target = config.url.trim();
        if target.is_empty() {
            return Err(WebhookError::InvalidConfig(
                "ServerChan API URL is required".to_string(),
            ));
        }

        let send_key = config
            .secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if let Some(send_key) = send_key {
            if target.contains("{sendkey}") {
                return Ok(target.replace("{sendkey}", send_key));
            }

            if let Ok(parsed) = url::Url::parse(target) {
                if parsed.path().ends_with(".send") {
                    return Ok(target.to_string());
                }
            }

            if let Some(node) = Self::parse_sctp_node(send_key) {
                return Ok(format!(
                    "https://{}.push.ft07.com/send/{}.send",
                    node, send_key
                ));
            }

            return Ok(format!(
                "{}/{}.send",
                target.trim_end_matches('/'),
                send_key
            ));
        }

        if target.contains("{sendkey}") {
            return Err(WebhookError::InvalidConfig(
                "ServerChan sendkey placeholder requires secret".to_string(),
            ));
        }

        let parsed = url::Url::parse(target)
            .map_err(|_| WebhookError::InvalidConfig("Invalid ServerChan API URL".to_string()))?;
        if parsed.path().ends_with(".send") {
            Ok(target.to_string())
        } else {
            Err(WebhookError::InvalidConfig(
                "ServerChan sendkey not configured".to_string(),
            ))
        }
    }

    fn parse_api_error(response_body: Option<&str>) -> Option<String> {
        let response_body = response_body?;
        let parsed: serde_json::Value = serde_json::from_str(response_body).ok()?;
        let code = parsed.get("code").and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })?;

        if code == 0 {
            return None;
        }

        let message = parsed
            .get("message")
            .or_else(|| parsed.get("msg"))
            .or_else(|| parsed.get("error"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown error");

        Some(format!("ServerChan API error {}: {}", code, message))
    }

    fn escape_markdown(text: &str) -> String {
        text.replace('\\', "\\\\").replace('`', "\\`")
    }
}

#[async_trait]
impl WebhookChannel for ServerChanChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::ServerChan
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<WebhookSendResult, WebhookError> {
        let endpoint = Self::resolve_endpoint(config)?;
        let started = std::time::Instant::now();

        let mut form = HashMap::new();
        form.insert("text", format!("{}", payload.event_type));
        form.insert(
            "desp",
            self.format_message(payload, config.template.as_deref()),
        );

        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&form)
            .send()
            .await?;

        let status = response.status().as_u16();
        let response_body = response.text().await.ok().filter(|s| !s.is_empty());

        if status >= 200 && status < 300 {
            if let Some(error) = Self::parse_api_error(response_body.as_deref()) {
                return Ok(WebhookSendResult {
                    success: false,
                    status_code: Some(status),
                    latency_ms: started.elapsed().as_millis() as u32,
                    response_body,
                    error: Some(error),
                });
            }

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
                error: Some(format!("ServerChan returned HTTP {}", status)),
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
            Err(error) => Ok(WebhookTestResult {
                success: false,
                latency_ms: None,
                error: Some(error.to_string()),
            }),
        }
    }

    fn format_message(&self, payload: &WebhookPayload, _template: Option<&str>) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "**Event**: {}",
            Self::escape_markdown(&payload.event_type.to_string())
        ));
        if let Some(ref session_name) = payload.session_name {
            lines.push(format!(
                "**Session**: {}",
                Self::escape_markdown(session_name)
            ));
        }
        if let Some(ref project_path) = payload.project_path {
            lines.push(format!(
                "**Project**: `{}`",
                Self::escape_markdown(project_path)
            ));
        }
        if let Some(ref remote_source) = payload.remote_source {
            lines.push(format!(
                "**Source**: {}",
                Self::escape_markdown(remote_source)
            ));
        }
        lines.push(format!(
            "**Summary**: {}",
            Self::escape_markdown(&payload.summary)
        ));
        if let Some(ms) = payload.duration_ms {
            lines.push(format!("**Duration**: {}s", ms / 1000));
        }
        lines.push(format!(
            "**Time**: {}",
            Self::escape_markdown(&format_timestamp_for_display(&payload.timestamp))
        ));
        lines.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(url: &str, secret: Option<&str>) -> WebhookChannelConfig {
        WebhookChannelConfig {
            id: "serverchan-test".to_string(),
            name: "ServerChan".to_string(),
            channel_type: WebhookChannelType::ServerChan,
            enabled: true,
            url: url.to_string(),
            secret: secret.map(str::to_string),
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-03-03T00:00:00Z".to_string(),
            updated_at: "2026-03-03T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_resolve_endpoint_with_standard_sendkey() {
        let config = make_config("https://sctapi.ftqq.com", Some("SCT123456"));
        let endpoint = ServerChanChannel::resolve_endpoint(&config).unwrap();
        assert_eq!(endpoint, "https://sctapi.ftqq.com/SCT123456.send");
    }

    #[test]
    fn test_resolve_endpoint_with_sctp_sendkey() {
        let config = make_config("https://sctapi.ftqq.com", Some("sctp123456tAbCdEf"));
        let endpoint = ServerChanChannel::resolve_endpoint(&config).unwrap();
        assert_eq!(
            endpoint,
            "https://123456.push.ft07.com/send/sctp123456tAbCdEf.send"
        );
    }

    #[test]
    fn test_resolve_endpoint_with_explicit_send_url_without_secret() {
        let config = make_config("https://sctapi.ftqq.com/SCT123456.send", None);
        let endpoint = ServerChanChannel::resolve_endpoint(&config).unwrap();
        assert_eq!(endpoint, "https://sctapi.ftqq.com/SCT123456.send");
    }

    #[test]
    fn test_format_message_markdown() {
        let channel = ServerChanChannel {
            client: reqwest::Client::new(),
        };
        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            summary: "All done".to_string(),
            session_name: Some("session-1".to_string()),
            duration_ms: Some(42000),
            timestamp: "2026-03-03T00:00:00Z".to_string(),
            ..Default::default()
        };

        let message = channel.format_message(&payload, None);
        assert!(message.contains("**Summary**"));
        assert!(message.contains("All done"));
        assert!(message.contains("**Duration**: 42s"));
    }

    #[test]
    fn test_parse_api_error_non_zero_code() {
        let body = r#"{"code":40001,"message":"invalid sendkey"}"#;
        let error = ServerChanChannel::parse_api_error(Some(body));
        assert_eq!(
            error,
            Some("ServerChan API error 40001: invalid sendkey".to_string())
        );
    }
}
