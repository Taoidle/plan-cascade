[中文版](Design-Webhook-Remote-Session_zh.md)

# Plan Cascade Desktop - Webhook Notifications & Remote Session Control

**Version**: 1.0.0
**Date**: 2026-02-18
**Author**: Plan Cascade Team
**Status**: Design Phase

---

## Table of Contents

1. [Design Goals](#1-design-goals)
2. [System Architecture](#2-system-architecture)
3. [Feature 1: Webhook Notifications](#3-feature-1-webhook-notifications)
4. [Feature 2: Remote Session Control](#4-feature-2-remote-session-control)
5. [Proxy Integration](#5-proxy-integration)
6. [Security Design](#6-security-design)
7. [Database Schema](#7-database-schema)
8. [Frontend Design](#8-frontend-design)
9. [API Design](#9-api-design)
10. [Implementation Plan](#10-implementation-plan)

---

## 1. Design Goals

### 1.1 Core Objectives

1. **Webhook Notifications**: Generic notification system for long-running background tasks, supporting multiple channels (Slack, Feishu/Lark, Telegram Bot, etc.) with global and per-session scope
2. **Remote Session Control**: Enable remote interaction with the desktop client via messaging platforms (Telegram Bot, etc.), allowing users to create sessions, send commands, and monitor execution remotely
3. **Proxy Reuse**: Fully leverage the existing proxy infrastructure (`ProxyConfig`, `ProxyStrategy`, `build_http_client()`) without duplicating proxy logic
4. **Synergy**: The two features naturally compose — remote commands trigger tasks, webhook notifications report results back

### 1.2 Design Constraints

| Constraint | Description |
|------------|-------------|
| Architecture Consistency | Follow existing patterns: Tauri commands, service layer, Zustand stores |
| Proxy Reuse | Use existing `build_http_client()` and per-provider strategy mechanism |
| Security | Bot tokens and webhook secrets stored in OS Keyring, authentication required for remote access |
| Desktop Dependency | Remote control requires the desktop app to be running and network-accessible |
| Message Limits | Handle platform-specific constraints (Telegram 4096 char limit, Slack block limits) |
| Cross-Platform | All features must work on Windows, macOS, and Linux |

### 1.3 Feature Synergy

```
┌─────────────────────────────────────────────────────────────────┐
│                     Remote + Notification Flow                   │
│                                                                  │
│   Telegram Bot ──send command──→ Desktop App (execute task)      │
│        ↑                              │                          │
│        └──── Webhook Notify ←── task complete ──→ Slack/Feishu   │
│                                                                  │
│   Use Case: Send "/new ~/projects/myapp" via Telegram,           │
│   Desktop creates session & executes, notifies via all channels  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. System Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Plan Cascade Desktop                                 │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                     React Frontend (TypeScript)                     │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐     │  │
│  │  │  Webhook      │  │  Remote      │  │  Existing Components │     │  │
│  │  │  Settings UI  │  │  Control UI  │  │  (Sessions, etc.)    │     │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘     │  │
│  │         └─────────────────┴─────────────────────┘                  │  │
│  │                           │                                        │  │
│  │                  Zustand State Management                          │  │
│  │          (webhookStore, remoteControlStore)                        │  │
│  └───────────────────────────┼────────────────────────────────────────┘  │
│                              │ Tauri IPC                                  │
│  ┌───────────────────────────┼────────────────────────────────────────┐  │
│  │                     Rust Backend                                    │  │
│  │                              │                                      │  │
│  │  ┌──────────────────────────┴────────────────────────────────┐     │  │
│  │  │                   Command Layer (New)                      │     │  │
│  │  │  commands/webhook.rs  │  commands/remote.rs                │     │  │
│  │  └──────────────────────────┬────────────────────────────────┘     │  │
│  │                              │                                      │  │
│  │  ┌──────────────────────────┴────────────────────────────────┐     │  │
│  │  │                   Service Layer (New)                      │     │  │
│  │  │                                                            │     │  │
│  │  │  ┌────────────────────┐    ┌─────────────────────────┐    │     │  │
│  │  │  │  Webhook Service   │    │  Remote Gateway Service  │    │     │  │
│  │  │  │  ┌──────────────┐  │    │  ┌───────────────────┐  │    │     │  │
│  │  │  │  │ Dispatcher   │  │    │  │  Telegram Adapter  │  │    │     │  │
│  │  │  │  │ Channel Mgr  │  │    │  │  Command Router    │  │    │     │  │
│  │  │  │  │ Template Eng │  │    │  │  Session Bridge    │  │    │     │  │
│  │  │  │  └──────────────┘  │    │  │  Response Mapper   │  │    │     │  │
│  │  │  └────────────────────┘    │  └───────────────────────┘  │    │     │  │
│  │  │            │                │            │                │     │  │
│  │  │            └────────┬───────┘            │                │     │  │
│  │  └─────────────────────┼────────────────────┘                │     │  │
│  │                        │                                      │     │  │
│  │  ┌─────────────────────┴──────────────────────────────────┐  │     │  │
│  │  │              Existing Infrastructure                     │  │     │  │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │  │     │  │
│  │  │  │  Proxy   │ │ Session  │ │ Orchestr │ │  Claude   │  │  │     │  │
│  │  │  │ Service  │ │  Mgmt    │ │  ator    │ │  Code     │  │  │     │  │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │  │     │  │
│  │  └────────────────────────────────────────────────────────┘  │     │  │
│  │                                                                │     │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │     │  │
│  │  │                   Storage Layer                          │  │     │  │
│  │  │  SQLite (webhook configs, remote sessions, audit log)    │  │     │  │
│  │  │  Keyring (bot tokens, webhook secrets)                   │  │     │  │
│  │  └─────────────────────────────────────────────────────────┘  │     │  │
│  └────────────────────────────────────────────────────────────────┘     │  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 New Files Overview

```
desktop/src-tauri/src/
├── commands/
│   ├── webhook.rs              # Webhook Tauri commands (CRUD, test, history)
│   └── remote.rs               # Remote control Tauri commands (start/stop, status)
├── services/
│   ├── webhook/
│   │   ├── mod.rs              # Module exports
│   │   ├── service.rs          # WebhookService (dispatcher, event listener)
│   │   ├── channels/
│   │   │   ├── mod.rs          # Channel trait + registry
│   │   │   ├── slack.rs        # Slack Incoming Webhook
│   │   │   ├── feishu.rs       # Feishu/Lark Bot Webhook
│   │   │   ├── telegram.rs     # Telegram Bot API (sendMessage)
│   │   │   ├── discord.rs      # Discord Webhook (future)
│   │   │   └── custom.rs       # Custom HTTP webhook
│   │   ├── templates.rs        # Message template engine
│   │   └── types.rs            # Webhook types and configs
│   └── remote/
│       ├── mod.rs              # Module exports
│       ├── gateway.rs          # RemoteGatewayService (lifecycle, adapter mgmt)
│       ├── adapters/
│       │   ├── mod.rs          # Adapter trait + registry
│       │   └── telegram.rs     # Telegram Bot long-polling adapter
│       ├── command_router.rs   # Parse remote commands, dispatch to sessions
│       ├── session_bridge.rs   # Bridge between remote commands and local sessions
│       ├── response_mapper.rs  # Map streaming events to platform messages
│       └── types.rs            # Remote control types
└── models/
    ├── webhook.rs              # Webhook data models
    └── remote.rs               # Remote session models

desktop/src/
├── lib/
│   ├── webhookApi.ts           # Webhook IPC wrappers
│   └── remoteApi.ts            # Remote control IPC wrappers
├── store/
│   ├── webhook.ts              # Webhook Zustand store
│   └── remote.ts               # Remote control Zustand store
└── components/
    └── Settings/
        ├── WebhookSection.tsx   # Webhook configuration UI
        └── RemoteSection.tsx    # Remote control configuration UI
```

---

## 3. Feature 1: Webhook Notifications

### 3.1 Overview

A generic notification system that triggers when long-running tasks complete (or fail). Supports multiple notification channels with configurable scope (global or per-session).

### 3.2 Core Types

```rust
// services/webhook/types.rs

/// Supported notification channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookChannelType {
    Slack,
    Feishu,
    Telegram,
    Discord,
    Custom,
}

/// Webhook channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookChannelConfig {
    pub id: String,                          // Unique channel ID (uuid)
    pub name: String,                        // User-friendly name
    pub channel_type: WebhookChannelType,
    pub enabled: bool,
    pub url: String,                         // Webhook URL or Bot API endpoint
    #[serde(skip_serializing, default)]
    pub secret: Option<String>,              // Token/secret (stored in Keyring)
    pub scope: WebhookScope,
    pub events: Vec<WebhookEventType>,       // Which events trigger this webhook
    pub template: Option<String>,            // Custom message template (optional)
    pub created_at: String,
    pub updated_at: String,
}

/// Notification scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookScope {
    /// Triggers for all sessions
    Global,
    /// Only triggers for specific session IDs
    Sessions(Vec<String>),
}

/// Events that can trigger webhooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookEventType {
    /// Task/session completed successfully
    TaskComplete,
    /// Task/session failed with error
    TaskFailed,
    /// Task cancelled by user
    TaskCancelled,
    /// Story completed (in expert mode)
    StoryComplete,
    /// All stories in a PRD completed
    PrdComplete,
    /// Long-running task progress milestone (25%, 50%, 75%)
    ProgressMilestone,
}

/// Webhook delivery payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event_type: WebhookEventType,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub project_path: Option<String>,
    pub summary: String,                     // Human-readable summary
    pub details: Option<serde_json::Value>,  // Structured details
    pub timestamp: String,
    pub duration_ms: Option<u64>,
    pub token_usage: Option<TokenUsageSummary>,
}

/// Delivery record for audit/retry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub channel_id: String,
    pub payload: WebhookPayload,
    pub status: DeliveryStatus,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub attempts: u32,
    pub last_attempt_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
    Retrying,
}
```

### 3.3 Channel Trait

```rust
// services/webhook/channels/mod.rs

#[async_trait]
pub trait WebhookChannel: Send + Sync {
    /// Channel type identifier
    fn channel_type(&self) -> WebhookChannelType;

    /// Send a notification through this channel
    async fn send(&self, payload: &WebhookPayload, config: &WebhookChannelConfig) -> Result<(), WebhookError>;

    /// Test the channel connection
    async fn test(&self, config: &WebhookChannelConfig) -> Result<WebhookTestResult, WebhookError>;

    /// Format the payload for this channel's specific message format
    fn format_message(&self, payload: &WebhookPayload, template: Option<&str>) -> String;
}
```

### 3.4 Channel Implementations

#### 3.4.1 Slack

```rust
// services/webhook/channels/slack.rs

/// Slack Incoming Webhook integration
/// Uses Slack Block Kit format for rich messages
///
/// Webhook URL format: https://hooks.slack.com/services/T.../B.../xxx
pub struct SlackChannel {
    client: reqwest::Client,     // Proxy-aware HTTP client
}

impl SlackChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: build_http_client(proxy),
        }
    }
}

// Message format: Slack Block Kit JSON
// {
//   "blocks": [
//     { "type": "header", "text": { "type": "plain_text", "text": "✅ Task Complete" } },
//     { "type": "section", "text": { "type": "mrkdwn", "text": "*Session*: ..." } },
//     { "type": "context", "elements": [{ "type": "mrkdwn", "text": "Duration: ..." }] }
//   ]
// }
```

#### 3.4.2 Feishu/Lark

```rust
// services/webhook/channels/feishu.rs

/// Feishu/Lark Bot Webhook integration
/// Uses Feishu Interactive Card format
///
/// Webhook URL format: https://open.feishu.cn/open-apis/bot/v2/hook/xxx
/// Supports optional signature verification (timestamp + secret -> SHA256 HMAC)
pub struct FeishuChannel {
    client: reqwest::Client,
}

// Message format: Feishu Interactive Card JSON
// {
//   "msg_type": "interactive",
//   "card": {
//     "header": { "title": { "tag": "plain_text", "content": "Task Complete" } },
//     "elements": [...]
//   }
// }
```

#### 3.4.3 Telegram

```rust
// services/webhook/channels/telegram.rs

/// Telegram Bot API integration (for notifications only, not remote control)
/// Uses sendMessage API with Markdown formatting
///
/// API endpoint: https://api.telegram.org/bot<token>/sendMessage
/// Requires: bot_token (stored in Keyring) + chat_id (in config URL field)
pub struct TelegramNotifyChannel {
    client: reqwest::Client,
}

// Message format: Telegram MarkdownV2
// "✅ *Task Complete*\n\n*Session*: my\\-session\n*Duration*: 5m 32s\n..."
```

#### 3.4.4 Custom HTTP

```rust
// services/webhook/channels/custom.rs

/// Generic HTTP webhook for custom integrations
/// POSTs JSON payload to any URL with optional HMAC-SHA256 signature header
///
/// Headers:
///   Content-Type: application/json
///   X-Webhook-Signature: sha256=<HMAC of body using secret>
///   X-Webhook-Event: <event_type>
pub struct CustomChannel {
    client: reqwest::Client,
}
```

### 3.5 WebhookService

```rust
// services/webhook/service.rs

pub struct WebhookService {
    channels: HashMap<WebhookChannelType, Box<dyn WebhookChannel>>,
    db: Arc<Database>,
    keyring: Arc<KeyringService>,
}

impl WebhookService {
    /// Initialize with proxy-aware HTTP clients for each channel type
    pub fn new(
        db: Arc<Database>,
        keyring: Arc<KeyringService>,
        proxy_resolver: impl Fn(&str) -> Option<ProxyConfig>,
    ) -> Self {
        let mut channels = HashMap::new();

        // Each channel gets its own proxy-resolved HTTP client
        channels.insert(
            WebhookChannelType::Slack,
            Box::new(SlackChannel::new(proxy_resolver("webhook_slack").as_ref())),
        );
        channels.insert(
            WebhookChannelType::Feishu,
            Box::new(FeishuChannel::new(proxy_resolver("webhook_feishu").as_ref())),
        );
        channels.insert(
            WebhookChannelType::Telegram,
            Box::new(TelegramNotifyChannel::new(proxy_resolver("webhook_telegram").as_ref())),
        );
        // ... other channels

        Self { channels, db, keyring }
    }

    /// Dispatch a notification to all matching channels
    pub async fn dispatch(&self, payload: WebhookPayload) -> Vec<WebhookDelivery> {
        let configs = self.get_enabled_configs_for_event(&payload).await;
        let mut deliveries = Vec::new();

        for config in configs {
            let channel = self.channels.get(&config.channel_type);
            if let Some(channel) = channel {
                let mut delivery = WebhookDelivery::new(&config, &payload);

                match channel.send(&payload, &config).await {
                    Ok(()) => delivery.status = DeliveryStatus::Success,
                    Err(e) => {
                        delivery.status = DeliveryStatus::Failed;
                        delivery.response_body = Some(e.to_string());
                    }
                }

                self.save_delivery(&delivery).await;
                deliveries.push(delivery);
            }
        }

        deliveries
    }

    /// Retry failed deliveries (called periodically or manually)
    pub async fn retry_failed(&self, max_attempts: u32) -> Vec<WebhookDelivery> { ... }

    /// Get configs that match the event type and session scope
    async fn get_enabled_configs_for_event(&self, payload: &WebhookPayload) -> Vec<WebhookChannelConfig> { ... }
}
```

### 3.6 Event Hook Integration

The webhook system hooks into existing execution flows at the event forwarding layer:

```rust
// Integration point: commands/standalone.rs (event forwarder task)
// Integration point: commands/claude_code.rs (stream event handler)

/// Called by the event forwarder when a terminal event is detected
async fn on_execution_event(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: &WebhookService,
    start_time: Instant,
) {
    match event {
        UnifiedStreamEvent::Complete { usage, .. } => {
            let payload = WebhookPayload {
                event_type: WebhookEventType::TaskComplete,
                session_id: Some(session_id.to_string()),
                session_name: session_name.map(|s| s.to_string()),
                project_path: project_path.map(|s| s.to_string()),
                summary: format!("Task completed successfully"),
                duration_ms: Some(start_time.elapsed().as_millis() as u64),
                token_usage: usage.clone(),
                ..Default::default()
            };
            webhook_service.dispatch(payload).await;
        }
        UnifiedStreamEvent::Error { message, .. } => {
            let payload = WebhookPayload {
                event_type: WebhookEventType::TaskFailed,
                summary: format!("Task failed: {}", message),
                ..Default::default()
            };
            webhook_service.dispatch(payload).await;
        }
        _ => {}
    }
}
```

---

## 4. Feature 2: Remote Session Control

### 4.1 Overview

Enables users to remotely interact with the desktop client through messaging platforms. The initial implementation supports Telegram Bot as the primary adapter, with an extensible adapter pattern for future platforms.

### 4.2 Core Types

```rust
// services/remote/types.rs

/// Remote adapter type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteAdapterType {
    Telegram,
    // Future: Slack, Discord, WebSocket API, etc.
}

/// Remote gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGatewayConfig {
    pub enabled: bool,
    pub adapter: RemoteAdapterType,
    pub auto_start: bool,                      // Start gateway when app launches
}

/// Telegram-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAdapterConfig {
    #[serde(skip_serializing, default)]
    pub bot_token: Option<String>,             // Stored in Keyring
    pub allowed_chat_ids: Vec<i64>,            // Whitelist of authorized chat IDs
    pub allowed_user_ids: Vec<i64>,            // Whitelist of authorized user IDs
    pub require_password: bool,                // Optional password gate
    #[serde(skip_serializing, default)]
    pub access_password: Option<String>,       // Stored in Keyring
    pub max_message_length: usize,             // Default: 4000 (Telegram limit ~4096)
    pub streaming_mode: StreamingMode,
}

/// How to handle streaming LLM output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingMode {
    /// Wait for completion, send final result
    WaitForComplete,
    /// Send periodic progress updates (every N seconds)
    PeriodicUpdate { interval_secs: u32 },
    /// Edit message in-place with latest content (Telegram editMessageText)
    LiveEdit { throttle_ms: u64 },
}

/// Remote command parsed from user message
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    /// /new <path> [provider] [model] — Create new session
    NewSession {
        project_path: String,
        provider: Option<String>,
        model: Option<String>,
    },
    /// /send <message> or plain text — Send message to active session
    SendMessage { content: String },
    /// /sessions — List active sessions
    ListSessions,
    /// /switch <session_id> — Switch active session
    SwitchSession { session_id: String },
    /// /status — Get current session status
    Status,
    /// /cancel — Cancel current execution
    Cancel,
    /// /close — Close current session
    CloseSession,
    /// /help — Show available commands
    Help,
}

/// Gateway runtime status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub running: bool,
    pub adapter_type: RemoteAdapterType,
    pub connected_since: Option<String>,
    pub active_remote_sessions: u32,
    pub total_commands_processed: u64,
    pub last_command_at: Option<String>,
    pub error: Option<String>,
}

/// Mapping between remote chat and local session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSessionMapping {
    pub chat_id: i64,                          // Remote chat identifier
    pub user_id: i64,                          // Remote user identifier
    pub local_session_id: Option<String>,       // Currently active local session
    pub session_type: SessionType,             // Claude Code or Standalone
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionType {
    ClaudeCode,
    Standalone { provider: String, model: String },
}
```

### 4.3 Remote Adapter Trait

```rust
// services/remote/adapters/mod.rs

#[async_trait]
pub trait RemoteAdapter: Send + Sync {
    /// Adapter type identifier
    fn adapter_type(&self) -> RemoteAdapterType;

    /// Start the adapter (begin receiving messages)
    async fn start(&self, command_tx: mpsc::Sender<IncomingRemoteMessage>) -> Result<(), RemoteError>;

    /// Stop the adapter gracefully
    async fn stop(&self) -> Result<(), RemoteError>;

    /// Send a text response to a remote chat
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError>;

    /// Edit an existing message (for live-update streaming)
    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<(), RemoteError>;

    /// Send a typing indicator
    async fn send_typing(&self, chat_id: i64) -> Result<(), RemoteError>;

    /// Check adapter health/connectivity
    async fn health_check(&self) -> Result<(), RemoteError>;
}

/// Incoming message from remote platform
#[derive(Debug, Clone)]
pub struct IncomingRemoteMessage {
    pub adapter_type: RemoteAdapterType,
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
    pub text: String,
    pub message_id: i64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

### 4.4 Telegram Adapter Implementation

```rust
// services/remote/adapters/telegram.rs

use teloxide::prelude::*;

pub struct TelegramAdapter {
    config: TelegramAdapterConfig,
    bot: Bot,                                    // teloxide Bot instance
    cancel_token: CancellationToken,
}

impl TelegramAdapter {
    pub fn new(config: TelegramAdapterConfig, proxy: Option<&ProxyConfig>) -> Result<Self, RemoteError> {
        // Build proxy-aware reqwest client
        let http_client = build_http_client(proxy);

        // Create teloxide Bot with custom HTTP client
        let bot = Bot::with_client(&config.bot_token.as_ref().unwrap(), http_client);

        Ok(Self {
            config,
            bot,
            cancel_token: CancellationToken::new(),
        })
    }
}

#[async_trait]
impl RemoteAdapter for TelegramAdapter {
    async fn start(&self, command_tx: mpsc::Sender<IncomingRemoteMessage>) -> Result<(), RemoteError> {
        let bot = self.bot.clone();
        let allowed_chat_ids = self.config.allowed_chat_ids.clone();
        let allowed_user_ids = self.config.allowed_user_ids.clone();
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            // Use teloxide's long-polling dispatcher
            let handler = Update::filter_message().endpoint(
                move |msg: Message, bot: Bot| {
                    let tx = command_tx.clone();
                    let allowed_chats = allowed_chat_ids.clone();
                    let allowed_users = allowed_user_ids.clone();
                    async move {
                        // Authorization check
                        let chat_id = msg.chat.id.0;
                        let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

                        if !allowed_chats.is_empty() && !allowed_chats.contains(&chat_id) {
                            return Ok(());  // Silently ignore unauthorized chats
                        }
                        if !allowed_users.is_empty() && !allowed_users.contains(&user_id) {
                            return Ok(());
                        }

                        if let Some(text) = msg.text() {
                            let incoming = IncomingRemoteMessage {
                                adapter_type: RemoteAdapterType::Telegram,
                                chat_id,
                                user_id,
                                username: msg.from().and_then(|u| u.username.clone()),
                                text: text.to_string(),
                                message_id: msg.id.0 as i64,
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = tx.send(incoming).await;
                        }
                        Ok(())
                    }
                },
            );

            Dispatcher::builder(bot, handler)
                .enable_ctrlc_handler()
                .build()
                .dispatch()
                .await;
        });

        Ok(())
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError> {
        // Handle Telegram's 4096 character limit by splitting
        let chunks = split_message(text, self.config.max_message_length);
        for chunk in chunks {
            self.bot.send_message(ChatId(chat_id), &chunk)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
                .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<(), RemoteError> {
        self.bot.edit_message_text(ChatId(chat_id), MessageId(message_id as i32), text)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await
            .map_err(|e| RemoteError::SendFailed(e.to_string()))?;
        Ok(())
    }

    async fn stop(&self) -> Result<(), RemoteError> {
        self.cancel_token.cancel();
        Ok(())
    }

    // ...
}

/// Split long messages at line boundaries to respect platform limits
fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    // Split at newline boundaries, keeping each chunk under max_len
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
                for chunk in line.as_bytes().chunks(max_len) {
                    chunks.push(String::from_utf8_lossy(chunk).to_string());
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
```

### 4.5 Command Router

```rust
// services/remote/command_router.rs

pub struct CommandRouter;

impl CommandRouter {
    /// Parse incoming message text into a RemoteCommand
    pub fn parse(text: &str) -> RemoteCommand {
        let text = text.trim();

        if text.starts_with("/new ") {
            let args: Vec<&str> = text[5..].trim().splitn(3, ' ').collect();
            RemoteCommand::NewSession {
                project_path: args.get(0).unwrap_or(&"").to_string(),
                provider: args.get(1).map(|s| s.to_string()),
                model: args.get(2).map(|s| s.to_string()),
            }
        } else if text == "/sessions" {
            RemoteCommand::ListSessions
        } else if text.starts_with("/switch ") {
            RemoteCommand::SwitchSession {
                session_id: text[8..].trim().to_string(),
            }
        } else if text == "/status" {
            RemoteCommand::Status
        } else if text == "/cancel" {
            RemoteCommand::Cancel
        } else if text == "/close" {
            RemoteCommand::CloseSession
        } else if text == "/help" {
            RemoteCommand::Help
        } else if text.starts_with("/send ") {
            RemoteCommand::SendMessage {
                content: text[6..].to_string(),
            }
        } else {
            // Plain text → treat as message to active session
            RemoteCommand::SendMessage {
                content: text.to_string(),
            }
        }
    }
}
```

### 4.6 Session Bridge

```rust
// services/remote/session_bridge.rs

/// Bridges remote commands to local session operations
pub struct SessionBridge {
    /// Mapping: chat_id -> local session
    sessions: RwLock<HashMap<i64, RemoteSessionMapping>>,
    /// Reference to standalone state for orchestrator access
    standalone_state: Arc<StandaloneState>,
    /// Reference to claude code state for CLI session access
    claude_code_state: Arc<ClaudeCodeState>,
    /// Webhook service for notifications
    webhook_service: Arc<WebhookService>,
    /// Database for persistence
    db: Arc<Database>,
}

impl SessionBridge {
    /// Create a new local session for a remote chat
    pub async fn create_session(
        &self,
        chat_id: i64,
        user_id: i64,
        project_path: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<String, RemoteError> {
        // Determine session type based on provider
        let session_type = match provider {
            Some("claude-code") | None => SessionType::ClaudeCode,
            Some(p) => SessionType::Standalone {
                provider: p.to_string(),
                model: model.unwrap_or("default").to_string(),
            },
        };

        let session_id = match &session_type {
            SessionType::ClaudeCode => {
                // Use ClaudeCodeState to start a new chat session
                self.claude_code_state
                    .session_manager
                    .start_session(project_path)
                    .await?
            }
            SessionType::Standalone { provider, model } => {
                // Create standalone orchestrator session
                self.standalone_state
                    .create_session(project_path, provider, model)
                    .await?
            }
        };

        // Store mapping
        let mapping = RemoteSessionMapping {
            chat_id,
            user_id,
            local_session_id: Some(session_id.clone()),
            session_type,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.sessions.write().await.insert(chat_id, mapping.clone());
        self.save_mapping(&mapping).await?;

        Ok(session_id)
    }

    /// Send a message to the local session and collect the response
    pub async fn send_message(
        &self,
        chat_id: i64,
        content: &str,
    ) -> Result<RemoteResponse, RemoteError> {
        let sessions = self.sessions.read().await;
        let mapping = sessions.get(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;

        let session_id = mapping.local_session_id.as_ref()
            .ok_or(RemoteError::NoActiveSession)?;

        match &mapping.session_type {
            SessionType::ClaudeCode => {
                self.send_to_claude_code(session_id, content).await
            }
            SessionType::Standalone { .. } => {
                self.send_to_standalone(session_id, content).await
            }
        }
    }

    /// Collect streaming response into a final text result
    async fn send_to_standalone(
        &self,
        session_id: &str,
        content: &str,
    ) -> Result<RemoteResponse, RemoteError> {
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(256);

        // Trigger orchestrator execution
        let orchestrator = self.standalone_state
            .get_orchestrator(session_id)
            .ok_or(RemoteError::SessionNotFound)?;

        let orchestrator = orchestrator.clone();
        let content = content.to_string();
        tokio::spawn(async move {
            let _ = orchestrator.execute(&content, tx).await;
        });

        // Collect streaming events into final response
        let mut text_parts = Vec::new();
        let mut thinking_parts = Vec::new();
        let mut tool_calls = Vec::new();

        while let Some(event) = rx.recv().await {
            match event {
                UnifiedStreamEvent::TextDelta { text, .. } => text_parts.push(text),
                UnifiedStreamEvent::ThinkingDelta { text, .. } => thinking_parts.push(text),
                UnifiedStreamEvent::ToolComplete { name, result, .. } => {
                    tool_calls.push(format!("[{}]: {}", name, truncate(&result, 200)));
                }
                UnifiedStreamEvent::Complete { .. } => break,
                UnifiedStreamEvent::Error { message, .. } => {
                    return Err(RemoteError::ExecutionFailed(message));
                }
                _ => {}
            }
        }

        Ok(RemoteResponse {
            text: text_parts.join(""),
            thinking: if thinking_parts.is_empty() { None } else { Some(thinking_parts.join("")) },
            tool_summary: if tool_calls.is_empty() { None } else { Some(tool_calls.join("\n")) },
        })
    }

    // ... send_to_claude_code similar pattern
}
```

### 4.7 Remote Gateway Service

```rust
// services/remote/gateway.rs

pub struct RemoteGatewayService {
    config: RwLock<RemoteGatewayConfig>,
    adapter: RwLock<Option<Box<dyn RemoteAdapter>>>,
    session_bridge: Arc<SessionBridge>,
    webhook_service: Arc<WebhookService>,
    status: RwLock<GatewayStatus>,
    cancel_token: CancellationToken,
}

impl RemoteGatewayService {
    /// Start the remote gateway
    pub async fn start(&self) -> Result<(), RemoteError> {
        let config = self.config.read().await;
        if !config.enabled {
            return Err(RemoteError::NotEnabled);
        }

        let (tx, mut rx) = mpsc::channel::<IncomingRemoteMessage>(100);

        // Start adapter
        {
            let adapter = self.adapter.read().await;
            if let Some(adapter) = adapter.as_ref() {
                adapter.start(tx).await?;
            }
        }

        // Start command processing loop
        let bridge = self.session_bridge.clone();
        let adapter_ref = self.adapter.clone();
        let status = self.status.clone();
        let webhook = self.webhook_service.clone();
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        Self::handle_message(
                            &msg,
                            &bridge,
                            &adapter_ref,
                            &status,
                            &webhook,
                        ).await;
                    }
                    _ = cancel.cancelled() => {
                        break;
                    }
                }
            }
        });

        // Update status
        let mut status = self.status.write().await;
        status.running = true;
        status.connected_since = Some(chrono::Utc::now().to_rfc3339());

        Ok(())
    }

    /// Handle an incoming remote message
    async fn handle_message(
        msg: &IncomingRemoteMessage,
        bridge: &SessionBridge,
        adapter: &RwLock<Option<Box<dyn RemoteAdapter>>>,
        status: &RwLock<GatewayStatus>,
        webhook: &WebhookService,
    ) {
        // Update stats
        {
            let mut s = status.write().await;
            s.total_commands_processed += 1;
            s.last_command_at = Some(chrono::Utc::now().to_rfc3339());
        }

        let command = CommandRouter::parse(&msg.text);
        let adapter_guard = adapter.read().await;
        let adapter = adapter_guard.as_ref().unwrap();

        // Send typing indicator
        let _ = adapter.send_typing(msg.chat_id).await;

        let response = match command {
            RemoteCommand::NewSession { project_path, provider, model } => {
                match bridge.create_session(
                    msg.chat_id,
                    msg.user_id,
                    &project_path,
                    provider.as_deref(),
                    model.as_deref(),
                ).await {
                    Ok(id) => format!("✅ Session created: {}\nProject: {}", id, project_path),
                    Err(e) => format!("❌ Failed to create session: {}", e),
                }
            }
            RemoteCommand::SendMessage { content } => {
                match bridge.send_message(msg.chat_id, &content).await {
                    Ok(resp) => {
                        let mut result = resp.text.clone();
                        if let Some(tools) = &resp.tool_summary {
                            result = format!("{}\n\n📎 Tools used:\n{}", result, tools);
                        }
                        result
                    }
                    Err(RemoteError::NoActiveSession) => {
                        "⚠️ No active session. Use /new <path> to create one.".to_string()
                    }
                    Err(e) => format!("❌ Error: {}", e),
                }
            }
            RemoteCommand::ListSessions => {
                bridge.list_sessions_text(msg.chat_id).await
            }
            RemoteCommand::Status => {
                bridge.get_status_text(msg.chat_id).await
            }
            RemoteCommand::Cancel => {
                match bridge.cancel_execution(msg.chat_id).await {
                    Ok(()) => "🛑 Execution cancelled.".to_string(),
                    Err(e) => format!("❌ Cancel failed: {}", e),
                }
            }
            RemoteCommand::Help => {
                HELP_TEXT.to_string()
            }
            _ => "Unknown command. Type /help for available commands.".to_string(),
        };

        let _ = adapter.send_message(msg.chat_id, &response).await;
    }

    /// Stop the gateway gracefully
    pub async fn stop(&self) -> Result<(), RemoteError> {
        self.cancel_token.cancel();
        if let Some(adapter) = self.adapter.read().await.as_ref() {
            adapter.stop().await?;
        }
        let mut status = self.status.write().await;
        status.running = false;
        Ok(())
    }

    /// Get current gateway status
    pub async fn get_status(&self) -> GatewayStatus {
        self.status.read().await.clone()
    }
}

const HELP_TEXT: &str = r#"🤖 Plan Cascade Remote Control

Available commands:
  /new <path> [provider] [model]  — Create new session
  /send <message>                 — Send message (or just type directly)
  /sessions                       — List active sessions
  /switch <id>                    — Switch to a session
  /status                         — Current session status
  /cancel                         — Cancel running execution
  /close                          — Close current session
  /help                           — Show this help

Examples:
  /new ~/projects/myapp
  /new ~/projects/api anthropic claude-sonnet-4-5-20250929
  How do I fix the login bug?
  /cancel
"#;
```

### 4.8 Streaming Response Strategies

For long-running LLM responses, three strategies are supported:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Streaming Mode Options                       │
│                                                                  │
│  1. WaitForComplete (Default)                                    │
│     User sends message → "⏳ Processing..." → final result       │
│     ✅ Simple, reliable                                          │
│     ❌ Long wait, no progress visibility                         │
│                                                                  │
│  2. PeriodicUpdate (interval: 10s)                               │
│     User sends message → "⏳ Processing..." →                    │
│       "[10s] Working on analysis..." →                           │
│       "[20s] Running tool: Grep..." →                            │
│       final result                                               │
│     ✅ Good progress visibility                                  │
│     ❌ Multiple messages in chat                                 │
│                                                                  │
│  3. LiveEdit (throttle: 2000ms)                                  │
│     User sends message → edits same message with latest content  │
│     ✅ Clean chat, real-time feel                                │
│     ❌ Rate limits (Telegram: 30 edits/min per chat)             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Proxy Integration

### 5.1 Reusing Existing Infrastructure

Both new features integrate with the existing proxy system without modification:

```
Existing Proxy System                    New Features
─────────────────────                    ────────────
ProxyConfig (types)          ────────→   WebhookChannel.new(proxy)
ProxyStrategy (per-provider) ────────→   New provider IDs registered
build_http_client()          ────────→   All HTTP clients
resolve_provider_proxy()     ────────→   At service initialization
Keyring (password storage)   ────────→   Bot tokens & webhook secrets
NetworkSection.tsx (UI)      ────────→   Extended with new providers
```

### 5.2 New Provider IDs

Add new entries to the proxy system's `PROVIDER_IDS` array:

```rust
// commands/proxy.rs — Extended PROVIDER_IDS

const PROVIDER_IDS: &[&str] = &[
    // Existing LLM providers
    "anthropic", "openai", "deepseek", "qwen", "glm", "minimax", "ollama",
    // Existing backends
    "claude_code",
    // Existing embedding providers
    "embedding_openai", "embedding_qwen", "embedding_glm", "embedding_ollama",
    // NEW: Webhook notification channels
    "webhook_slack",
    "webhook_feishu",
    "webhook_telegram",
    "webhook_discord",
    "webhook_custom",
    // NEW: Remote control adapters
    "remote_telegram",
];
```

### 5.3 Default Strategy for New Providers

```rust
// commands/proxy.rs — Extended default_strategy_for()

fn default_strategy_for(provider: &str) -> ProxyStrategy {
    match provider {
        // International services → default UseGlobal
        "anthropic" | "openai" | "claude_code" | "embedding_openai" => ProxyStrategy::UseGlobal,
        "webhook_slack" | "webhook_discord" => ProxyStrategy::UseGlobal,
        "remote_telegram" | "webhook_telegram" => ProxyStrategy::UseGlobal,

        // Domestic services → default NoProxy
        "qwen" | "glm" | "deepseek" | "minimax" | "ollama" => ProxyStrategy::NoProxy,
        "embedding_qwen" | "embedding_glm" | "embedding_ollama" => ProxyStrategy::NoProxy,
        "webhook_feishu" => ProxyStrategy::NoProxy,

        // Custom webhooks → UseGlobal (external targets likely)
        "webhook_custom" => ProxyStrategy::UseGlobal,

        _ => ProxyStrategy::UseGlobal,
    }
}
```

### 5.4 Proxy Resolution Flow

```
┌──────────────────────────────────────────────────────────────┐
│              Proxy Resolution for New Features                 │
│                                                                │
│  WebhookService::new()                                         │
│    ├─ resolve_provider_proxy("webhook_slack")  → ProxyConfig?  │
│    ├─ resolve_provider_proxy("webhook_feishu") → ProxyConfig?  │
│    ├─ resolve_provider_proxy("webhook_telegram") → ProxyConfig?│
│    └─ each channel: build_http_client(proxy) → reqwest::Client │
│                                                                │
│  TelegramAdapter::new()                                        │
│    ├─ resolve_provider_proxy("remote_telegram") → ProxyConfig? │
│    └─ build_http_client(proxy) → reqwest::Client → Bot         │
└──────────────────────────────────────────────────────────────┘
```

---

## 6. Security Design

### 6.1 Credential Storage

| Credential | Storage | Keyring Key |
|------------|---------|-------------|
| Webhook secrets/tokens | OS Keyring | `webhook_{channel_id}` |
| Telegram Bot token | OS Keyring | `remote_telegram_bot_token` |
| Remote access password | OS Keyring | `remote_access_password` |
| Proxy passwords | OS Keyring (existing) | `proxy_{provider}` |

### 6.2 Remote Access Authentication

```
┌─────────────────────────────────────────────────────────────┐
│                Remote Access Security Layers                  │
│                                                               │
│  Layer 1: Bot Token (inherent)                                │
│    Only your bot receives messages via its unique token       │
│                                                               │
│  Layer 2: Chat ID Whitelist                                   │
│    Only messages from pre-configured chat IDs are processed   │
│    All other messages are silently dropped                    │
│                                                               │
│  Layer 3: User ID Whitelist                                   │
│    Only specific user IDs within allowed chats can send cmds  │
│                                                               │
│  Layer 4: Access Password (optional)                          │
│    First message must be "/auth <password>"                   │
│    Session is password-gated until authenticated              │
│                                                               │
│  Layer 5: Project Path Restriction (configurable)             │
│    Limit which directories can be opened as sessions          │
│    Prevent access to sensitive system directories             │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Audit Logging

All remote commands are logged to SQLite:

```rust
pub struct RemoteAuditEntry {
    pub id: String,
    pub adapter_type: String,
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
    pub command: String,
    pub command_type: String,
    pub result_status: String,        // "success", "error", "unauthorized"
    pub error_message: Option<String>,
    pub created_at: String,
}
```

---

## 7. Database Schema

### 7.1 New Tables

```sql
-- Webhook channel configurations
CREATE TABLE IF NOT EXISTS webhook_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    channel_type TEXT NOT NULL,          -- 'slack', 'feishu', 'telegram', 'discord', 'custom'
    enabled INTEGER NOT NULL DEFAULT 1,
    url TEXT NOT NULL,
    scope_type TEXT NOT NULL DEFAULT 'global',  -- 'global' or 'sessions'
    scope_sessions TEXT,                  -- JSON array of session IDs (when scope_type = 'sessions')
    events TEXT NOT NULL,                 -- JSON array of event types
    template TEXT,                        -- Custom message template
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Webhook delivery history (for audit and retry)
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,               -- JSON serialized WebhookPayload
    status TEXT NOT NULL,                -- 'pending', 'success', 'failed', 'retrying'
    status_code INTEGER,
    response_body TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES webhook_channels(id) ON DELETE CASCADE
);

-- Index for delivery retry queries
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status
    ON webhook_deliveries(status, last_attempt_at);

-- Remote session mappings
CREATE TABLE IF NOT EXISTS remote_session_mappings (
    chat_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    adapter_type TEXT NOT NULL,
    local_session_id TEXT,
    session_type TEXT NOT NULL,          -- JSON: {"ClaudeCode"} or {"Standalone":{"provider":"...","model":"..."}}
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (adapter_type, chat_id)
);

-- Remote command audit log
CREATE TABLE IF NOT EXISTS remote_audit_log (
    id TEXT PRIMARY KEY,
    adapter_type TEXT NOT NULL,
    chat_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    username TEXT,
    command_text TEXT NOT NULL,
    command_type TEXT NOT NULL,
    result_status TEXT NOT NULL,         -- 'success', 'error', 'unauthorized'
    error_message TEXT,
    created_at TEXT NOT NULL
);

-- Index for audit queries
CREATE INDEX IF NOT EXISTS idx_remote_audit_created
    ON remote_audit_log(created_at DESC);
```

---

## 8. Frontend Design

### 8.1 Webhook Settings UI

Located in `Settings > Notifications`:

```
┌──────────────────────────────────────────────────────────────┐
│  Notifications                                                │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Webhook Channels                              [+ Add] │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ 🟢 Slack — #dev-notifications                  │   │    │
│  │  │    Events: Task Complete, Task Failed           │   │    │
│  │  │    Scope: Global           [Test] [Edit] [Del]  │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ 🟢 Feishu — Project Updates Bot                │   │    │
│  │  │    Events: All                                  │   │    │
│  │  │    Scope: Global           [Test] [Edit] [Del]  │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ ⚪ Telegram — @my_notify_bot                   │   │    │
│  │  │    Events: Task Complete                        │   │    │
│  │  │    Scope: 2 sessions       [Test] [Edit] [Del]  │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Delivery History                         [View All →] │    │
│  │                                                        │    │
│  │  ✅ 2m ago   Slack  TaskComplete  "Session abc..."     │    │
│  │  ❌ 5m ago   Feishu TaskFailed    "Error: timeout"     │    │
│  │  ✅ 12m ago  Slack  PrdComplete   "All 5 stories..."   │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 8.2 Remote Control Settings UI

Located in `Settings > Remote Control`:

```
┌──────────────────────────────────────────────────────────────┐
│  Remote Control                                               │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Gateway Status               [Start] / [Stop]       │    │
│  │                                                        │    │
│  │  Status: 🟢 Running (connected since 14:30)           │    │
│  │  Adapter: Telegram Bot (@my_cascade_bot)               │    │
│  │  Commands processed: 47                                │    │
│  │  Active remote sessions: 2                             │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Telegram Bot Configuration                           │    │
│  │                                                        │    │
│  │  Bot Token: ●●●●●●●●●●●●●●●●●●●●           [Change]  │    │
│  │  Auto-start: [✓]                                       │    │
│  │                                                        │    │
│  │  Allowed Chat IDs:                                     │    │
│  │    ┌────────────────────┐  [+ Add]                     │    │
│  │    │ 123456789  [×]     │                              │    │
│  │    │ 987654321  [×]     │                              │    │
│  │    └────────────────────┘                              │    │
│  │                                                        │    │
│  │  Allowed User IDs:                                     │    │
│  │    ┌────────────────────┐  [+ Add]                     │    │
│  │    │ 111222333  [×]     │                              │    │
│  │    └────────────────────┘                              │    │
│  │                                                        │    │
│  │  Password Protection: [✓]                              │    │
│  │  Access Password: ●●●●●●●●             [Change]       │    │
│  │                                                        │    │
│  │  Streaming Mode: [WaitForComplete ▼]                   │    │
│  │                                                        │    │
│  │  Allowed Project Paths:                                │    │
│  │    ┌────────────────────────────────┐  [+ Add]         │    │
│  │    │ ~/projects       [×]           │                  │    │
│  │    │ ~/work           [×]           │                  │    │
│  │    └────────────────────────────────┘                  │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Active Remote Sessions                               │    │
│  │                                                        │    │
│  │  Chat 123456789 → Session abc-123 (ClaudeCode)        │    │
│  │    Project: ~/projects/myapp                           │    │
│  │    Last activity: 2m ago                               │    │
│  │                                                        │    │
│  │  Chat 987654321 → Session def-456 (Anthropic/Sonnet)  │    │
│  │    Project: ~/work/api-server                          │    │
│  │    Last activity: 15m ago                              │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Audit Log                                [View All →] │    │
│  │                                                        │    │
│  │  14:35  @user  /new ~/projects/myapp      ✅ success   │    │
│  │  14:35  @user  "fix the login bug"        ✅ success   │    │
│  │  14:32  @other /new ~/secret              ❌ unauth    │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 8.3 Remote Sessions in Main UI

Remote-created sessions are visible in the main session list with a remote indicator:

```
Sessions
├─ 📱 abc-123 (via Telegram @user)     — ~/projects/myapp
├─ 📱 def-456 (via Telegram @user)     — ~/work/api-server
├─    ghi-789                           — ~/projects/other
└─    jkl-012                           — ~/work/frontend
```

---

## 9. API Design

### 9.1 Webhook Tauri Commands

```rust
// commands/webhook.rs

/// List all configured webhook channels
#[tauri::command]
pub async fn list_webhook_channels(state: State<'_, AppState>) -> Result<CommandResponse<Vec<WebhookChannelConfig>>, String>

/// Create a new webhook channel
#[tauri::command]
pub async fn create_webhook_channel(config: CreateWebhookRequest, state: State<'_, AppState>) -> Result<CommandResponse<WebhookChannelConfig>, String>

/// Update an existing webhook channel
#[tauri::command]
pub async fn update_webhook_channel(id: String, config: UpdateWebhookRequest, state: State<'_, AppState>) -> Result<CommandResponse<WebhookChannelConfig>, String>

/// Delete a webhook channel
#[tauri::command]
pub async fn delete_webhook_channel(id: String, state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// Test a webhook channel (send test notification)
#[tauri::command]
pub async fn test_webhook_channel(id: String, state: State<'_, AppState>) -> Result<CommandResponse<WebhookTestResult>, String>

/// Get delivery history (with pagination)
#[tauri::command]
pub async fn get_webhook_deliveries(channel_id: Option<String>, limit: Option<u32>, offset: Option<u32>, state: State<'_, AppState>) -> Result<CommandResponse<Vec<WebhookDelivery>>, String>

/// Retry a failed delivery
#[tauri::command]
pub async fn retry_webhook_delivery(delivery_id: String, state: State<'_, AppState>) -> Result<CommandResponse<WebhookDelivery>, String>
```

### 9.2 Remote Control Tauri Commands

```rust
// commands/remote.rs

/// Get remote gateway status
#[tauri::command]
pub async fn get_remote_gateway_status(state: State<'_, RemoteState>) -> Result<CommandResponse<GatewayStatus>, String>

/// Start the remote gateway
#[tauri::command]
pub async fn start_remote_gateway(state: State<'_, RemoteState>, app_state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// Stop the remote gateway
#[tauri::command]
pub async fn stop_remote_gateway(state: State<'_, RemoteState>) -> Result<CommandResponse<()>, String>

/// Get remote gateway configuration
#[tauri::command]
pub async fn get_remote_config(state: State<'_, AppState>) -> Result<CommandResponse<RemoteGatewayConfig>, String>

/// Update remote gateway configuration (Telegram settings)
#[tauri::command]
pub async fn update_remote_config(config: UpdateRemoteConfigRequest, state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// List active remote session mappings
#[tauri::command]
pub async fn list_remote_sessions(state: State<'_, RemoteState>) -> Result<CommandResponse<Vec<RemoteSessionMapping>>, String>

/// Disconnect a remote session
#[tauri::command]
pub async fn disconnect_remote_session(chat_id: i64, state: State<'_, RemoteState>) -> Result<CommandResponse<()>, String>

/// Get remote audit log (with pagination)
#[tauri::command]
pub async fn get_remote_audit_log(limit: Option<u32>, offset: Option<u32>, state: State<'_, AppState>) -> Result<CommandResponse<Vec<RemoteAuditEntry>>, String>
```

### 9.3 New Tauri State

```rust
// main.rs — New managed state

pub struct WebhookState {
    pub service: Arc<WebhookService>,
}

pub struct RemoteState {
    pub gateway: Arc<RemoteGatewayService>,
}

// In main():
app.manage(WebhookState { service: webhook_service });
app.manage(RemoteState { gateway: remote_gateway });
```

---

## 10. Implementation Plan

### 10.1 Phases

```
Phase 1: Webhook Notifications (Foundation)
├── 1.1 Core types and channel trait
├── 1.2 Slack channel implementation
├── 1.3 Feishu channel implementation
├── 1.4 Telegram notification channel
├── 1.5 Custom HTTP channel
├── 1.6 WebhookService (dispatcher + retry)
├── 1.7 Database schema + migrations
├── 1.8 Tauri commands (CRUD + test + history)
├── 1.9 Event hook integration (standalone + claude_code)
├── 1.10 Proxy integration (new provider IDs)
├── 1.11 Frontend: webhookApi.ts + webhook store
└── 1.12 Frontend: WebhookSection.tsx settings UI

Phase 2: Remote Session Control
├── 2.1 Core types and adapter trait
├── 2.2 Command router
├── 2.3 Session bridge
├── 2.4 Response mapper (streaming → text)
├── 2.5 Telegram adapter (teloxide)
├── 2.6 RemoteGatewayService (lifecycle + message loop)
├── 2.7 Database schema (mappings + audit)
├── 2.8 Tauri commands (start/stop + config + audit)
├── 2.9 Proxy integration
├── 2.10 Security (auth layers + audit logging)
├── 2.11 Frontend: remoteApi.ts + remote store
└── 2.12 Frontend: RemoteSection.tsx settings UI

Phase 3: Integration & Polish
├── 3.1 Remote commands trigger webhook notifications
├── 3.2 Remote session visibility in main UI
├── 3.3 Auto-start gateway on app launch
├── 3.4 i18n (en, zh, ja)
└── 3.5 Testing and documentation
```

### 10.2 Dependencies

```
                    ┌──────────────┐
                    │ Proxy System │ (existing, shared)
                    └──────┬───────┘
                           │
              ┌────────────┴────────────┐
              │                         │
    ┌─────────▼─────────┐    ┌─────────▼─────────┐
    │ Phase 1: Webhook   │    │ Phase 2: Remote    │
    │ Notifications      │    │ Session Control    │
    └─────────┬─────────┘    └─────────┬─────────┘
              │                         │
              └────────────┬────────────┘
                           │
                 ┌─────────▼─────────┐
                 │ Phase 3: Integration│
                 └───────────────────┘
```

Phase 1 and Phase 2 can be developed **in parallel** since they are independent. Phase 3 integrates them.

### 10.3 Cargo Dependencies (New)

```toml
# desktop/src-tauri/Cargo.toml — New dependencies

[dependencies]
# Remote control - Telegram bot
teloxide = { version = "0.13", features = ["macros"] }

# Webhook - HMAC signature
hmac = "0.12"
sha2 = "0.10"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
```

### 10.4 Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Telegram API rate limits | Message delivery delays | Implement throttling + exponential backoff in adapter |
| Long-polling network instability | Gateway disconnection | Auto-reconnect with backoff, status monitoring in UI |
| Large LLM responses exceed message limits | Truncated output | Smart message splitting at logical boundaries |
| Unauthorized remote access | Security breach | Multi-layer auth (chat ID + user ID + password) |
| Desktop app goes offline | Remote control unavailable | Clear error messaging in bot, auto-reconnect on resume |
| Proxy configuration errors | Failed webhook delivery | Test button per channel, delivery history with error details |
