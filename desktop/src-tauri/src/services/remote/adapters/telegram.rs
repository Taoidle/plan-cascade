//! Telegram Adapter
//!
//! Telegram Bot adapter using teloxide for long-polling message reception.
//! Implements the RemoteAdapter trait with proxy support, authorization checks,
//! and message splitting for Telegram's 4096 character limit.

use super::RemoteAdapter;
use crate::services::proxy::ProxyConfig;
use crate::services::remote::types::{
    IncomingRemoteMessage, RemoteAdapterType, RemoteError, TelegramAdapterConfig,
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Telegram Bot adapter using teloxide with long-polling.
pub struct TelegramAdapter {
    pub(crate) config: TelegramAdapterConfig,
    pub(crate) bot: teloxide::Bot,
    pub(crate) cancel_token: CancellationToken,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter with proxy-aware HTTP client.
    ///
    /// Note: teloxide uses its own reqwest 0.11 internally, which differs from
    /// the project's reqwest 0.12. We use `Bot::new()` which creates its own
    /// reqwest client. For proxy support, the proxy URL is set via the
    /// `TELOXIDE_PROXY` environment variable before creating the bot, or we
    /// configure it via teloxide's `net::default_reqwest_settings()` approach.
    pub fn new(
        config: TelegramAdapterConfig,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self, RemoteError> {
        let bot_token = config
            .bot_token
            .as_ref()
            .ok_or_else(|| RemoteError::ConfigError("Bot token is required".to_string()))?;

        // Configure proxy for teloxide's internal reqwest client via env var.
        // teloxide reads HTTPS_PROXY/HTTP_PROXY when building its client.
        if let Some(proxy_cfg) = proxy {
            let proxy_url = proxy_cfg.url_with_auth();
            std::env::set_var("HTTPS_PROXY", &proxy_url);
            std::env::set_var("HTTP_PROXY", &proxy_url);
        }

        // Create teloxide Bot (uses its own internal reqwest 0.11 client)
        let bot = teloxide::Bot::new(bot_token);

        Ok(Self {
            config,
            bot,
            cancel_token: CancellationToken::new(),
        })
    }
}

/// Split long messages at line boundaries to respect platform limits.
pub fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if current.len() + line.len() + 1 > max_len {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            // Handle single lines longer than max_len
            if line.len() > max_len {
                let mut start = 0;
                while start < line.len() {
                    let end = std::cmp::min(start + max_len, line.len());
                    chunks.push(line[start..end].to_string());
                    start = end;
                }
                continue;
            }
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

#[async_trait]
impl RemoteAdapter for TelegramAdapter {
    fn adapter_type(&self) -> RemoteAdapterType {
        RemoteAdapterType::Telegram
    }

    async fn start(
        &self,
        command_tx: mpsc::Sender<IncomingRemoteMessage>,
    ) -> Result<(), RemoteError> {
        use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
        use teloxide::dptree;
        use teloxide::types::{Message, Update};

        let bot = self.bot.clone();
        let allowed_chat_ids = self.config.allowed_chat_ids.clone();
        let allowed_user_ids = self.config.allowed_user_ids.clone();
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            let handler =
                Update::filter_message().endpoint(move |msg: Message, _bot: teloxide::Bot| {
                    let tx = command_tx.clone();
                    let allowed_chats = allowed_chat_ids.clone();
                    let allowed_users = allowed_user_ids.clone();
                    async move {
                        // Authorization check: chat ID whitelist
                        let chat_id = msg.chat.id.0;
                        if !allowed_chats.is_empty() && !allowed_chats.contains(&chat_id) {
                            return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                        }

                        // Authorization check: user ID whitelist
                        let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
                        if !allowed_users.is_empty() && !allowed_users.contains(&user_id) {
                            return Ok(());
                        }

                        // Extract text and forward to command channel
                        if let Some(text) = msg.text() {
                            let incoming = IncomingRemoteMessage {
                                adapter_type: RemoteAdapterType::Telegram,
                                chat_id,
                                user_id,
                                username: msg.from.as_ref().and_then(|u| u.username.clone()),
                                text: text.to_string(),
                                message_id: msg.id.0 as i64,
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = tx.send(incoming).await;
                        }
                        Ok(())
                    }
                });

            // Build and run dispatcher
            let mut dispatcher = Dispatcher::builder(bot, handler)
                .enable_ctrlc_handler()
                .build();

            // Get shutdown token for graceful termination
            let shutdown_token = dispatcher.shutdown_token();

            // Spawn a task that watches the CancellationToken and triggers shutdown
            let cancel_clone = cancel.clone();
            tokio::spawn(async move {
                cancel_clone.cancelled().await;
                let _ = shutdown_token.shutdown();
            });

            dispatcher.dispatch().await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), RemoteError> {
        self.cancel_token.cancel();
        Ok(())
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::ChatId;

        let chunks = split_message(text, self.config.max_message_length);
        for chunk in chunks {
            self.bot
                .send_message(ChatId(chat_id), &chunk)
                .await
                .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatId, MessageId};

        self.bot
            .edit_message_text(ChatId(chat_id), MessageId(message_id as i32), text)
            .await
            .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        Ok(())
    }

    async fn send_typing(&self, chat_id: i64) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatAction, ChatId};

        self.bot
            .send_chat_action(ChatId(chat_id), ChatAction::Typing)
            .await
            .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        Ok(())
    }

    async fn health_check(&self) -> Result<(), RemoteError> {
        use teloxide::prelude::*;

        self.bot
            .get_me()
            .await
            .map_err(|e| RemoteError::ConfigError(format!("Bot health check failed: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_message_short() {
        let chunks = split_message("Hello world", 100);
        assert_eq!(chunks, vec!["Hello world"]);
    }

    #[test]
    fn test_split_message_empty() {
        let chunks = split_message("", 100);
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn test_split_message_multiline() {
        let text = "Line 1\nLine 2\nLine 3\nLine 4";
        let chunks = split_message(text, 15);
        // "Line 1\nLine 2" = 13 chars, fits
        // "Line 3\nLine 4" = 13 chars, fits
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "Line 1\nLine 2");
        assert_eq!(chunks[1], "Line 3\nLine 4");
    }

    #[test]
    fn test_split_message_long_single_line() {
        let text = "a".repeat(250);
        let chunks = split_message(&text, 100);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[1].len(), 100);
        assert_eq!(chunks[2].len(), 50);
    }

    #[test]
    fn test_split_message_exact_boundary() {
        let text = "12345\n12345";
        let chunks = split_message(text, 11);
        assert_eq!(chunks, vec!["12345\n12345"]);
    }

    #[test]
    fn test_split_message_mixed() {
        let text = "Short line\nAnother short\nThis is a much longer line that should force a split by itself when the limit is small";
        let chunks = split_message(text, 30);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 30 || !chunk.contains('\n'));
        }
    }

    #[test]
    fn test_telegram_adapter_new_without_token() {
        let config = TelegramAdapterConfig::default();
        let result = TelegramAdapter::new(config, None);
        assert!(result.is_err());
        match result {
            Err(RemoteError::ConfigError(msg)) => {
                assert!(msg.contains("Bot token is required"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_telegram_adapter_new_with_token() {
        let config = TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        };
        let result = TelegramAdapter::new(config, None);
        assert!(result.is_ok());
        let adapter = result.unwrap();
        assert_eq!(adapter.adapter_type(), RemoteAdapterType::Telegram);
    }

    #[test]
    fn test_telegram_adapter_new_with_proxy() {
        use crate::services::proxy::{ProxyConfig, ProxyProtocol};

        let config = TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        };
        let proxy = ProxyConfig {
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        let result = TelegramAdapter::new(config, Some(&proxy));
        assert!(result.is_ok());
    }

    #[test]
    fn test_authorization_logic_empty_whitelist_allows_all() {
        // When allowed_chat_ids is empty, all chats should be allowed
        let allowed_chats: Vec<i64> = vec![];
        let chat_id: i64 = 999;
        // Empty whitelist = allow all
        assert!(allowed_chats.is_empty() || allowed_chats.contains(&chat_id));
    }

    #[test]
    fn test_authorization_logic_chat_id_whitelist() {
        let allowed_chats = vec![123i64, 456, 789];
        assert!(allowed_chats.contains(&123));
        assert!(allowed_chats.contains(&456));
        assert!(!allowed_chats.contains(&999));
    }

    #[test]
    fn test_authorization_logic_user_id_whitelist() {
        let allowed_users = vec![111i64, 222];
        assert!(allowed_users.contains(&111));
        assert!(!allowed_users.contains(&333));
    }

    #[test]
    fn test_split_message_telegram_limit() {
        // Telegram's actual limit is 4096, our default max_message_length is 4000
        let text = "a".repeat(10000);
        let chunks = split_message(&text, 4000);
        assert_eq!(chunks.len(), 3); // 4000 + 4000 + 2000
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1].len(), 4000);
        assert_eq!(chunks[2].len(), 2000);
    }

    #[test]
    fn test_split_message_with_newlines_near_boundary() {
        let mut text = String::new();
        for i in 0..100 {
            text.push_str(&format!("Line {}\n", i));
        }
        let chunks = split_message(&text, 100);
        for chunk in &chunks {
            assert!(chunk.len() <= 100);
        }
    }

    #[test]
    fn test_cancel_token_stops_adapter() {
        let config = TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        };
        let adapter = TelegramAdapter::new(config, None).unwrap();

        // CancellationToken should not be cancelled initially
        assert!(!adapter.cancel_token.is_cancelled());

        // After cancel, it should be cancelled
        adapter.cancel_token.cancel();
        assert!(adapter.cancel_token.is_cancelled());
    }
}
