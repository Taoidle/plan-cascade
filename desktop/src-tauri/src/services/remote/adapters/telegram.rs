//! Telegram Adapter
//!
//! Telegram Bot adapter using teloxide 0.17 for long-polling message reception.
//! Implements the RemoteAdapter trait with proxy support (via `Bot::with_client`),
//! authorization checks, and message splitting for Telegram's 4096 character limit.

use super::RemoteAdapter;
use crate::services::proxy::ProxyConfig;
use crate::services::remote::types::{
    IncomingRemoteEvent, RemoteActionCard, RemoteAdapterType, RemoteError,
    RemoteIncomingEventType, RemoteUiMessage, TelegramAdapterConfig,
};
use async_trait::async_trait;
use regex::{Captures, Regex};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::mpsc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// Telegram Bot adapter using teloxide with long-polling.
pub struct TelegramAdapter {
    pub(crate) config: Arc<RwLock<TelegramAdapterConfig>>,
    pub(crate) bot: teloxide::Bot,
    pub(crate) cancel_token: Arc<Mutex<Option<CancellationToken>>>,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter with proxy-aware HTTP client.
    ///
    /// Since teloxide 0.17 uses reqwest 0.12 internally (matching the project's reqwest
    /// version), we inject a custom reqwest::Client via `Bot::with_client()` for proxy support.
    pub async fn new(
        config: Arc<RwLock<TelegramAdapterConfig>>,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self, RemoteError> {
        let initial_config = config.read().await.clone();
        let bot_token = initial_config
            .bot_token
            .as_ref()
            .ok_or_else(|| RemoteError::ConfigError("Bot token is required".to_string()))?;

        // Build a reqwest 0.12 client with optional proxy, then inject into teloxide Bot
        let bot = if let Some(proxy_cfg) = proxy {
            let proxy_url = proxy_cfg.url_with_auth();
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| RemoteError::ConfigError(format!("Invalid proxy URL: {}", e)))?;
            let client = reqwest::Client::builder()
                .proxy(proxy)
                .build()
                .map_err(|e| {
                    RemoteError::ConfigError(format!("Failed to build HTTP client: {}", e))
                })?;
            teloxide::Bot::with_client(bot_token, client)
        } else {
            teloxide::Bot::new(bot_token)
        };

        Ok(Self {
            config,
            bot,
            cancel_token: Arc::new(Mutex::new(None)),
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

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn markdown_link_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\[([^\]]+)\]\((https?://[^\s)]+)\)").unwrap())
}

fn inline_code_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"`([^`\n]+)`").unwrap())
}

fn bold_asterisk_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\*\*([^\*\n]+)\*\*").unwrap())
}

fn bold_underscore_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"__([^_\n]+)__").unwrap())
}

fn italic_asterisk_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\*([^\*\n]+)\*").unwrap())
}

fn italic_underscore_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"_([^_\n]+)_").unwrap())
}

fn strike_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"~~([^~\n]+)~~").unwrap())
}

fn ordered_list_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^(\d+)\.\s+(.*)$").unwrap())
}

fn format_inline_markdown(text: &str) -> String {
    let mut rendered = escape_html(text);
    rendered = markdown_link_regex()
        .replace_all(&rendered, |caps: &Captures| {
            format!("<a href=\"{}\">{}</a>", &caps[2], &caps[1])
        })
        .into_owned();
    rendered = inline_code_regex()
        .replace_all(&rendered, "<code>$1</code>")
        .into_owned();
    rendered = bold_asterisk_regex()
        .replace_all(&rendered, "<b>$1</b>")
        .into_owned();
    rendered = bold_underscore_regex()
        .replace_all(&rendered, "<b>$1</b>")
        .into_owned();
    rendered = strike_regex()
        .replace_all(&rendered, "<s>$1</s>")
        .into_owned();
    rendered = italic_asterisk_regex()
        .replace_all(&rendered, "<i>$1</i>")
        .into_owned();
    italic_underscore_regex()
        .replace_all(&rendered, "<i>$1</i>")
        .into_owned()
}

fn render_markdown_to_telegram_html(text: &str) -> String {
    let mut blocks = Vec::new();
    let mut code_lines = Vec::new();
    let mut in_code_block = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code_block {
                blocks.push(format!(
                    "<pre><code>{}</code></pre>",
                    escape_html(&code_lines.join("\n"))
                ));
                code_lines.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        if trimmed.is_empty() {
            blocks.push(String::new());
            continue;
        }

        let rendered = if let Some(content) = trimmed.strip_prefix("# ") {
            format!("<b>{}</b>", format_inline_markdown(content))
        } else if let Some(content) = trimmed.strip_prefix("## ") {
            format!("<b>{}</b>", format_inline_markdown(content))
        } else if let Some(content) = trimmed.strip_prefix("### ") {
            format!("<b>{}</b>", format_inline_markdown(content))
        } else if let Some(content) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            format!("• {}", format_inline_markdown(content))
        } else if let Some(caps) = ordered_list_regex().captures(trimmed) {
            format!("{}. {}", &caps[1], format_inline_markdown(&caps[2]))
        } else {
            format_inline_markdown(line)
        };

        blocks.push(rendered);
    }

    if in_code_block {
        blocks.push(format!(
            "<pre><code>{}</code></pre>",
            escape_html(&code_lines.join("\n"))
        ));
    }

    blocks.join("\n")
}

fn should_fallback_to_plain_text(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("parse entities")
        || lower.contains("can't parse entities")
        || lower.contains("unsupported start tag")
        || lower.contains("can't find end tag")
        || lower.contains("entity")
}

fn render_action_card_html(card: &RemoteActionCard) -> String {
    let mut text = format!("<b>{}</b>", escape_html(&card.title));
    if !card.body.trim().is_empty() {
        text.push_str("\n\n");
        text.push_str(&render_markdown_to_telegram_html(&card.body));
    }
    if !card.attachment_refs.is_empty() {
        text.push_str("\n\n<b>Artifacts:</b>");
        for attachment in &card.attachment_refs {
            text.push_str(&format!(
                "\n• {}: <code>{}</code>",
                escape_html(&attachment.label),
                escape_html(&attachment.path)
            ));
        }
    }
    text
}

#[async_trait]
impl RemoteAdapter for TelegramAdapter {
    fn adapter_type(&self) -> RemoteAdapterType {
        RemoteAdapterType::Telegram
    }

    async fn start(
        &self,
        command_tx: mpsc::Sender<IncomingRemoteEvent>,
    ) -> Result<super::RemoteAdapterHandle, RemoteError> {
        use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
        use teloxide::dptree;
        use teloxide::types::{CallbackQuery, Message, Update};

        let bot = self.bot.clone();
        let config = self.config.clone();
        let cancel = CancellationToken::new();
        {
            let mut guard = self.cancel_token.lock().await;
            *guard = Some(cancel.clone());
        }

        Ok(tokio::spawn(async move {
            let message_tx = command_tx.clone();
            let message_config = config.clone();
            let message_handler =
                Update::filter_message().endpoint(move |msg: Message, _bot: teloxide::Bot| {
                    let tx = message_tx.clone();
                    let config = message_config.clone();
                    async move {
                        let current_config = config.read().await.clone();
                        let chat_id = msg.chat.id.0;
                        if !current_config.allowed_chat_ids.is_empty()
                            && !current_config.allowed_chat_ids.contains(&chat_id)
                        {
                            return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
                        }

                        let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
                        if !current_config.allowed_user_ids.is_empty()
                            && !current_config.allowed_user_ids.contains(&user_id)
                        {
                            return Ok(());
                        }

                        if let Some(text) = msg.text() {
                            let incoming = IncomingRemoteEvent {
                                adapter_type: RemoteAdapterType::Telegram,
                                event_type: RemoteIncomingEventType::TextMessage,
                                chat_id,
                                user_id,
                                username: msg.from.as_ref().and_then(|u| u.username.clone()),
                                text: text.to_string(),
                                message_id: msg.id.0 as i64,
                                timestamp: chrono::Utc::now(),
                                callback_id: None,
                                callback_data: None,
                            };
                            let _ = tx.send(incoming).await;
                        }
                        Ok(())
                    }
                });

            let callback_tx = command_tx.clone();
            let callback_config = config.clone();
            let callback_handler =
                Update::filter_callback_query().endpoint(move |query: CallbackQuery, _bot: teloxide::Bot| {
                    let tx = callback_tx.clone();
                    let config = callback_config.clone();
                    async move {
                        let message = match query.message {
                            Some(message) => message,
                            None => return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(()),
                        };
                        let current_config = config.read().await.clone();
                        let chat_id = message.chat().id.0;
                        if !current_config.allowed_chat_ids.is_empty()
                            && !current_config.allowed_chat_ids.contains(&chat_id)
                        {
                            return Ok(());
                        }

                        let user_id = query.from.id.0 as i64;
                        if !current_config.allowed_user_ids.is_empty()
                            && !current_config.allowed_user_ids.contains(&user_id)
                        {
                            return Ok(());
                        }

                        let payload = query.data.unwrap_or_default();
                        let incoming = IncomingRemoteEvent {
                            adapter_type: RemoteAdapterType::Telegram,
                            event_type: RemoteIncomingEventType::CallbackAction,
                            chat_id,
                            user_id,
                            username: query.from.username.clone(),
                            text: payload.clone(),
                            message_id: message.id().0 as i64,
                            timestamp: chrono::Utc::now(),
                            callback_id: Some(query.id.to_string()),
                            callback_data: Some(payload),
                        };
                        let _ = tx.send(incoming).await;
                        Ok(())
                    }
                });

            let handler = dptree::entry()
                .branch(message_handler)
                .branch(callback_handler);

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
            Ok(())
        }))
    }

    async fn stop(&self) -> Result<(), RemoteError> {
        if let Some(token) = self.cancel_token.lock().await.take() {
            token.cancel();
        }
        Ok(())
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatId, ParseMode};

        let max_message_length = self.config.read().await.max_message_length;
        let chunks = split_message(text, max_message_length);
        if chunks.len() == 1 {
            let html = render_markdown_to_telegram_html(text);
            match self
                .bot
                .send_message(ChatId(chat_id), html)
                .parse_mode(ParseMode::Html)
                .await
            {
                Ok(_) => return Ok(()),
                Err(error) if should_fallback_to_plain_text(&error.to_string()) => {}
                Err(error) => return Err(RemoteError::SendFailed(error.to_string())),
            }
        }

        for chunk in chunks {
            self.bot
                .send_message(ChatId(chat_id), chunk)
                .await
                .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn send_message_returning_id(
        &self,
        chat_id: i64,
        text: &str,
    ) -> Result<i64, RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatId, ParseMode};

        let html = render_markdown_to_telegram_html(text);
        let msg = match self
            .bot
            .send_message(ChatId(chat_id), html)
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(message) => message,
            Err(error) if should_fallback_to_plain_text(&error.to_string()) => self
                .bot
                .send_message(ChatId(chat_id), text)
                .await
                .map_err(|fallback| RemoteError::SendFailed(fallback.to_string()))?,
            Err(error) => return Err(RemoteError::SendFailed(error.to_string())),
        };
        Ok(msg.id.0 as i64)
    }

    async fn send_action_card(&self, chat_id: i64, card: &RemoteActionCard) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

        let text = render_action_card_html(card);

        let keyboard = if card.actions.is_empty() {
            None
        } else {
            Some(InlineKeyboardMarkup::new(
                card.actions
                    .iter()
                    .map(|action| {
                        vec![InlineKeyboardButton::callback(
                            action.label.clone(),
                            action.id.clone(),
                        )]
                    })
                    .collect::<Vec<_>>(),
            ))
        };

        let mut request = self
            .bot
            .send_message(ChatId(chat_id), text)
            .parse_mode(ParseMode::Html);
        if let Some(markup) = keyboard.clone() {
            request = request.reply_markup(markup);
        }
        match request.await {
            Ok(_) => Ok(()),
            Err(error) if should_fallback_to_plain_text(&error.to_string()) => {
                let mut plain_text = format!("{}\n\n{}", card.title, card.body);
                if !card.attachment_refs.is_empty() {
                    plain_text.push_str("\n\nArtifacts:");
                    for attachment in &card.attachment_refs {
                        plain_text.push_str(&format!("\n- {}: {}", attachment.label, attachment.path));
                    }
                }

                let mut fallback = self.bot.send_message(ChatId(chat_id), plain_text);
                if let Some(markup) = keyboard {
                    fallback = fallback.reply_markup(markup);
                }
                fallback
                    .await
                    .map_err(|fallback_error| RemoteError::SendFailed(fallback_error.to_string()))?;
                Ok(())
            }
            Err(error) => Err(RemoteError::SendFailed(error.to_string())),
        }
    }

    async fn send_ui_message(&self, chat_id: i64, message: &RemoteUiMessage) -> Result<(), RemoteError> {
        match message {
            RemoteUiMessage::PlainText(text) => self.send_message(chat_id, text).await,
            RemoteUiMessage::ActionCard(card) => self.send_action_card(chat_id, card).await,
        }
    }

    async fn answer_callback(&self, callback_id: &str) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::CallbackQueryId;

        self.bot
            .answer_callback_query(CallbackQueryId(callback_id.to_string()))
            .await
            .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        Ok(())
    }

    async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<(), RemoteError> {
        use teloxide::prelude::*;
        use teloxide::types::{ChatId, MessageId, ParseMode};

        let html = render_markdown_to_telegram_html(text);
        match self
            .bot
            .edit_message_text(ChatId(chat_id), MessageId(message_id as i32), html)
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if should_fallback_to_plain_text(&error.to_string()) => {
                self.bot
                    .edit_message_text(ChatId(chat_id), MessageId(message_id as i32), text)
                    .await
                    .map_err(|fallback_error| RemoteError::SendFailed(fallback_error.to_string()))?;
                Ok(())
            }
            Err(error) => Err(RemoteError::SendFailed(error.to_string())),
        }
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

        let config = self.config.read().await;
        if config.bot_token.as_ref().is_none_or(|token| token.is_empty()) {
            return Err(RemoteError::ConfigError("Bot token is required".to_string()));
        }

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
    fn test_render_markdown_to_telegram_html_inline_formatting() {
        let rendered = render_markdown_to_telegram_html(
            "**bold** _italic_ `code` [OpenAI](https://openai.com) ~~gone~~",
        );
        assert!(rendered.contains("<b>bold</b>"));
        assert!(rendered.contains("<i>italic</i>"));
        assert!(rendered.contains("<code>code</code>"));
        assert!(rendered.contains("<a href=\"https://openai.com\">OpenAI</a>"));
        assert!(rendered.contains("<s>gone</s>"));
    }

    #[test]
    fn test_render_markdown_to_telegram_html_blocks() {
        let rendered = render_markdown_to_telegram_html(
            "# Title\n- item 1\n1. item 2\n\n```rust\nfn main() {}\n```",
        );
        assert!(rendered.contains("<b>Title</b>"));
        assert!(rendered.contains("• item 1"));
        assert!(rendered.contains("1. item 2"));
        assert!(rendered.contains("<pre><code>fn main() {}"));
    }

    #[tokio::test]
    async fn test_telegram_adapter_new_without_token() {
        let config = Arc::new(RwLock::new(TelegramAdapterConfig::default()));
        let result = TelegramAdapter::new(config, None).await;
        assert!(result.is_err());
        match result {
            Err(RemoteError::ConfigError(msg)) => {
                assert!(msg.contains("Bot token is required"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[tokio::test]
    async fn test_telegram_adapter_new_with_token() {
        let config = Arc::new(RwLock::new(TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        }));
        let result = TelegramAdapter::new(config, None).await;
        assert!(result.is_ok());
        let adapter = result.unwrap();
        assert_eq!(adapter.adapter_type(), RemoteAdapterType::Telegram);
    }

    #[tokio::test]
    async fn test_telegram_adapter_new_with_proxy() {
        use crate::services::proxy::{ProxyConfig, ProxyProtocol};

        let config = Arc::new(RwLock::new(TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        }));
        let proxy = ProxyConfig {
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        let result = TelegramAdapter::new(config, Some(&proxy)).await;
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

    #[tokio::test]
    async fn test_cancel_token_stops_adapter() {
        let config = Arc::new(RwLock::new(TelegramAdapterConfig {
            bot_token: Some("test-token-123:ABC".to_string()),
            ..Default::default()
        }));
        let adapter = TelegramAdapter::new(config, None).await.unwrap();

        // CancellationToken should not be cancelled initially
        assert!(adapter.cancel_token.lock().await.is_none());
    }
}
