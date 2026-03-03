//! Discord Webhook Channel
//!
//! Sends notifications to Discord Incoming Webhooks.

use async_trait::async_trait;

use super::{localized_event_name, localized_label, LabelKey, WebhookChannel};
use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;

/// Discord Incoming Webhook integration.
pub struct DiscordChannel {
    client: reqwest::Client,
}

impl DiscordChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::webhook::http_client::build_webhook_http_client(proxy),
        }
    }
}

#[async_trait]
impl WebhookChannel for DiscordChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::Discord
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<WebhookSendResult, WebhookError> {
        let body = self.format_message(payload, config.template.as_deref());
        let started = std::time::Instant::now();

        let response = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .body(body)
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
                error: Some(format!("Discord returned HTTP {}", status)),
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
        let locale = payload.locale.as_deref();
        let event_name = localized_event_name(&payload.event_type, locale);
        let color = match payload.event_type {
            WebhookEventType::TaskComplete => 0x2ECC71,
            WebhookEventType::TaskFailed => 0xE74C3C,
            WebhookEventType::TaskCancelled => 0xF1C40F,
            WebhookEventType::StoryComplete => 0x3498DB,
            WebhookEventType::PrdComplete => 0x9B59B6,
            WebhookEventType::ProgressMilestone => 0x1ABC9C,
        };

        let mut fields = Vec::new();
        if let Some(session_name) = &payload.session_name {
            fields.push(serde_json::json!({
                "name": localized_label(locale, LabelKey::Session),
                "value": session_name,
                "inline": true
            }));
        }
        if let Some(project_path) = &payload.project_path {
            fields.push(serde_json::json!({
                "name": localized_label(locale, LabelKey::Project),
                "value": project_path,
                "inline": false
            }));
        }

        let body = serde_json::json!({
            "content": format!("**{}**", event_name),
            "embeds": [
                {
                    "title": event_name,
                    "description": payload.summary,
                    "color": color,
                    "fields": fields,
                    "timestamp": payload.timestamp
                }
            ]
        });

        serde_json::to_string(&body).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn test_discord_format_message_structure() {
        let channel = DiscordChannel {
            client: reqwest::Client::new(),
        };

        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_name: Some("test-session".to_string()),
            project_path: Some("/tmp/project".to_string()),
            summary: "Everything completed".to_string(),
            timestamp: "2026-03-03T00:00:00Z".to_string(),
            ..Default::default()
        };

        let message = channel.format_message(&payload, None);
        let parsed: serde_json::Value = serde_json::from_str(&message).unwrap();

        assert_eq!(parsed["embeds"][0]["title"], "TaskComplete");
        assert_eq!(parsed["embeds"][0]["description"], "Everything completed");
        assert_eq!(parsed["embeds"][0]["timestamp"], "2026-03-03T00:00:00Z");
    }

    #[tokio::test]
    async fn test_discord_send_handles_http_status() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = vec![0; 2048];
                let _ = socket.read(&mut buffer).await;
                let response = b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 11\r\n\r\nbad request";
                let _ = socket.write_all(response).await;
                let _ = socket.shutdown().await;
            }
        });

        let channel = DiscordChannel {
            client: reqwest::Client::new(),
        };
        let payload = WebhookPayload {
            summary: "test".to_string(),
            ..Default::default()
        };
        let config = WebhookChannelConfig {
            id: "discord-test".to_string(),
            name: "discord".to_string(),
            channel_type: WebhookChannelType::Discord,
            enabled: true,
            url: format!("http://{}", addr),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-03-03T00:00:00Z".to_string(),
            updated_at: "2026-03-03T00:00:00Z".to_string(),
        };

        let result = channel.send(&payload, &config).await.unwrap();
        assert!(!result.success);
        assert!(result.status_code.unwrap_or(0) >= 400);
        assert!(result.error.unwrap_or_default().contains("HTTP"));
    }
}
