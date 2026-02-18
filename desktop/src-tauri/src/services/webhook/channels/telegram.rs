//! Telegram Bot API Notification Channel
//!
//! Sends notifications via Telegram Bot API sendMessage endpoint.
//! Uses MarkdownV2 formatting with proper escaping of special characters.

use async_trait::async_trait;

use crate::services::proxy::ProxyConfig;
use crate::services::webhook::types::*;
use super::WebhookChannel;

/// Telegram Bot API integration for notifications only.
///
/// Uses sendMessage API with MarkdownV2 formatting.
/// API endpoint: `https://api.telegram.org/bot<token>/sendMessage`
///
/// Configuration:
/// - `config.url` stores the chat_id
/// - `config.secret` stores the bot_token (from Keyring)
pub struct TelegramNotifyChannel {
    client: reqwest::Client,
}

impl TelegramNotifyChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: crate::services::proxy::build_http_client(proxy),
        }
    }

    /// Escape special characters for Telegram MarkdownV2 format.
    pub fn escape_markdown_v2(text: &str) -> String {
        let special_chars = [
            '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.',
            '!',
        ];
        let mut result = String::with_capacity(text.len() * 2);
        for ch in text.chars() {
            if special_chars.contains(&ch) {
                result.push('\\');
            }
            result.push(ch);
        }
        result
    }
}

#[async_trait]
impl WebhookChannel for TelegramNotifyChannel {
    fn channel_type(&self) -> WebhookChannelType {
        WebhookChannelType::Telegram
    }

    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<(), WebhookError> {
        let bot_token = config.secret.as_deref().ok_or_else(|| {
            WebhookError::InvalidConfig("Telegram bot token not configured".to_string())
        })?;

        let chat_id = &config.url;
        let text = self.format_message(payload, config.template.as_deref());

        let api_url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "MarkdownV2"
        });

        let response = self
            .client
            .post(&api_url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(WebhookError::HttpError(format!(
                "Telegram returned HTTP {}: {}",
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
            WebhookEventType::TaskComplete => "\u{2705}",
            WebhookEventType::TaskFailed => "\u{274C}",
            WebhookEventType::TaskCancelled => "\u{26A0}\u{FE0F}",
            WebhookEventType::StoryComplete => "\u{1F516}",
            WebhookEventType::PrdComplete => "\u{1F389}",
            WebhookEventType::ProgressMilestone => "\u{1F4C8}",
        };

        let event_name = Self::escape_markdown_v2(&format!("{}", payload.event_type));

        let mut lines = vec![format!("{} *{}*", emoji, event_name)];
        lines.push(String::new());

        if let Some(ref name) = payload.session_name {
            lines.push(format!(
                "*Session*: {}",
                Self::escape_markdown_v2(name)
            ));
        }

        if let Some(ref path) = payload.project_path {
            lines.push(format!(
                "*Project*: {}",
                Self::escape_markdown_v2(path)
            ));
        }

        lines.push(format!(
            "*Summary*: {}",
            Self::escape_markdown_v2(&payload.summary)
        ));

        if let Some(ms) = payload.duration_ms {
            let secs = ms / 1000;
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            let duration_str = if mins > 0 {
                format!("{}m {}s", mins, remaining_secs)
            } else {
                format!("{}s", secs)
            };
            lines.push(format!(
                "*Duration*: {}",
                Self::escape_markdown_v2(&duration_str)
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_markdown_v2() {
        assert_eq!(
            TelegramNotifyChannel::escape_markdown_v2("hello_world"),
            "hello\\_world"
        );
        assert_eq!(
            TelegramNotifyChannel::escape_markdown_v2("a.b.c"),
            "a\\.b\\.c"
        );
        assert_eq!(
            TelegramNotifyChannel::escape_markdown_v2("test (value)"),
            "test \\(value\\)"
        );
        assert_eq!(
            TelegramNotifyChannel::escape_markdown_v2("no special"),
            "no special"
        );
    }

    #[test]
    fn test_telegram_format_message() {
        let channel = TelegramNotifyChannel {
            client: reqwest::Client::new(),
        };

        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_name: Some("my-session".to_string()),
            project_path: Some("/home/user/project".to_string()),
            summary: "All tasks done".to_string(),
            duration_ms: Some(332000),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            ..Default::default()
        };

        let msg = channel.format_message(&payload, None);
        assert!(msg.contains("*TaskComplete*"));
        assert!(msg.contains("my\\-session"));
        assert!(msg.contains("5m 32s"));
    }
}
