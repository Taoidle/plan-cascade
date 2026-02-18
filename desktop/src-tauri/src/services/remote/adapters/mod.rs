//! Remote Adapters
//!
//! Trait definition for remote platform adapters and adapter registry.
//! Each adapter implements platform-specific message receiving and sending.

pub mod telegram;

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::types::{IncomingRemoteMessage, RemoteAdapterType, RemoteError};

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
        command_tx: mpsc::Sender<IncomingRemoteMessage>,
    ) -> Result<(), RemoteError>;

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

    /// Check adapter health/connectivity.
    ///
    /// For Telegram, this calls the getMe API to verify the bot token.
    async fn health_check(&self) -> Result<(), RemoteError>;
}
