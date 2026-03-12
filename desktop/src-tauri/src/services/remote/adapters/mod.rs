//! Remote Adapters
//!
//! Trait definition for remote platform adapters and adapter registry.
//! Each adapter implements platform-specific message receiving and sending.

pub mod telegram;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use super::types::{
    IncomingRemoteEvent, RemoteActionCard, RemoteAdapterType, RemoteError, RemoteUiMessage,
};
use super::TelegramAdapterConfig;
use crate::services::proxy::ProxyConfig;

pub type RemoteAdapterHandle = JoinHandle<Result<(), RemoteError>>;

/// Remote adapter trait for platform-specific message handling.
///
/// Adapters are responsible for:
/// - Receiving messages from the remote platform (long-polling or webhook)
/// - Sending text responses back to the platform
/// - Editing existing messages (for live-update streaming)
/// - Sending typing indicators
/// - Health checking connectivity
#[async_trait]
pub trait RemoteAdapter: Send + Sync {
    /// Adapter type identifier
    fn adapter_type(&self) -> RemoteAdapterType;

    /// Start the adapter (begin receiving messages).
    ///
    /// Messages are forwarded through the provided mpsc sender channel.
    /// The adapter should spawn its own task for the message loop.
    async fn start(
        &self,
        command_tx: mpsc::Sender<IncomingRemoteEvent>,
    ) -> Result<RemoteAdapterHandle, RemoteError>;

    /// Stop the adapter gracefully.
    async fn stop(&self) -> Result<(), RemoteError>;

    /// Send a text response to a remote chat.
    ///
    /// Must handle platform-specific message length limits by splitting
    /// long messages as needed.
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError>;

    /// Edit an existing message (for live-update streaming mode).
    async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<(), RemoteError>;

    /// Send a typing indicator to show the bot is processing.
    async fn send_typing(&self, chat_id: i64) -> Result<(), RemoteError>;

    /// Send a text response and return the platform message ID.
    ///
    /// Used by LiveEdit streaming mode to obtain the message ID for subsequent edits.
    /// Default implementation delegates to `send_message()` and returns 0.
    async fn send_message_returning_id(
        &self,
        chat_id: i64,
        text: &str,
    ) -> Result<i64, RemoteError> {
        self.send_message(chat_id, text).await?;
        Ok(0)
    }

    async fn send_action_card(&self, chat_id: i64, card: &RemoteActionCard) -> Result<(), RemoteError> {
        let mut body = format!("{}\n\n{}", card.title, card.body);
        if !card.actions.is_empty() {
            body.push_str("\n\nActions:");
            for action in &card.actions {
                body.push_str(&format!("\n- {}", action.label));
            }
        }
        self.send_message(chat_id, &body).await
    }

    async fn send_ui_message(&self, chat_id: i64, message: &RemoteUiMessage) -> Result<(), RemoteError> {
        match message {
            RemoteUiMessage::PlainText(text) => self.send_message(chat_id, text).await,
            RemoteUiMessage::ActionCard(card) => self.send_action_card(chat_id, card).await,
        }
    }

    async fn answer_callback(&self, _callback_id: &str) -> Result<(), RemoteError> {
        Ok(())
    }

    /// Check adapter health/connectivity.
    ///
    /// For Telegram, this calls the getMe API to verify the bot token.
    async fn health_check(&self) -> Result<(), RemoteError>;
}

pub struct RemoteAdapterFactory;

impl RemoteAdapterFactory {
    pub async fn create(
        adapter_type: &RemoteAdapterType,
        telegram_config: Arc<RwLock<TelegramAdapterConfig>>,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Arc<dyn RemoteAdapter>, RemoteError> {
        match adapter_type {
            RemoteAdapterType::Telegram => {
                Ok(Arc::new(
                    telegram::TelegramAdapter::new(telegram_config, proxy).await?,
                ))
            }
        }
    }
}
