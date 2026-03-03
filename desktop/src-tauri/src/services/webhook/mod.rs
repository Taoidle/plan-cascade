//! Webhook Notification System
//!
//! Generic notification system that triggers when long-running tasks complete
//! or fail. Supports multiple channels (Slack, Feishu, Telegram, ServerChan, Custom)
//! with configurable scope (global or per-session).

pub mod channels;
pub mod http_client;
pub mod integration;
pub mod sanitize;
pub mod service;
pub mod types;

pub use channels::WebhookChannel;
pub use service::WebhookService;
pub use types::*;
