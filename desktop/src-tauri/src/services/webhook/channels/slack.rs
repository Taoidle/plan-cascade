//! Slack Incoming Webhook Channel
//!
//! Sends notifications via Slack Incoming Webhook URL using Block Kit JSON format.

use async_trait::async_trait;

use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;
use super::WebhookChannel;

/// Slack Incoming Webhook integration.
///
/// Uses Slack Block Kit format for rich messages with header, section, and context blocks.
/// Webhook URL format: `https://hooks.slack.com/services/T.../B.../xxx`
pub struct SlackChannel {
    client: reqwest::Client,
}

impl SlackChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::proxy::build_http_client(proxy),
        }
    }
}

#[async_trait]
impl WebhookChannel for SlackChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::Slack
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<(), WebhookError> {
        let body = self.format_message(payload, config.template.as_deref());

        let response = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(WebhookError::HttpError(format!(
                "Slack returned HTTP {}: {}",
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
        let emoji = match payload.event_type {
            WebhookEventType::TaskComplete => "white_check_mark",
            WebhookEventType::TaskFailed => "x",
            WebhookEventType::TaskCancelled => "warning",
            WebhookEventType::StoryComplete => "bookmark",
            WebhookEventType::PrdComplete => "tada",
            WebhookEventType::ProgressMilestone => "chart_with_upwards_trend",
        };

        let title = format!(":{}: {}", emoji, payload.event_type);

        let mut section_text = String::new();
        if let Some(ref name) = payload.session_name {
            section_text.push_str(&format!("*Session*: {}\n", name));
        }
        if let Some(ref path) = payload.project_path {
            section_text.push_str(&format!("*Project*: {}\n", path));
        }
        section_text.push_str(&format!("*Summary*: {}", payload.summary));

        let mut context_elements = Vec::new();
        if let Some(ms) = payload.duration_ms {
            let secs = ms / 1000;
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            let duration_str = if mins > 0 {
                format!("{}m {}s", mins, remaining_secs)
            } else {
                format!("{}s", secs)
            };
            context_elements.push(serde_json::json!({
                "type": "mrkdwn",
                "text": format!("Duration: {}", duration_str)
            }));
        }
        context_elements.push(serde_json::json!({
            "type": "mrkdwn",
            "text": format!("Timestamp: {}", payload.timestamp)
        }));

        let blocks = serde_json::json!({
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": title
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": section_text
                    }
                },
                {
                    "type": "context",
                    "elements": context_elements
                }
            ]
        });

        serde_json::to_string(&blocks).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_format_message_structure() {
        let channel = SlackChannel {
            client: reqwest::Client::new(),
        };

        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_name: Some("test-session".to_string()),
            project_path: Some("/home/user/project".to_string()),
            summary: "All tasks completed".to_string(),
            duration_ms: Some(65000),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            ..Default::default()
        };

        let msg = channel.format_message(&payload, None);
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();

        // Verify block structure
        let blocks = parsed["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0]["type"], "header");
        assert_eq!(blocks[1]["type"], "section");
        assert_eq!(blocks[2]["type"], "context");

        // Verify content
        let section_text = blocks[1]["text"]["text"].as_str().unwrap();
        assert!(section_text.contains("test-session"));
        assert!(section_text.contains("All tasks completed"));
    }
}
