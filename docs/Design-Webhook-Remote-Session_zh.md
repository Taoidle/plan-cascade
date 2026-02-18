[English](Design-Webhook-Remote-Session.md)

# Plan Cascade Desktop - Webhook 通知与远程会话控制

**版本**: 1.0.0
**日期**: 2026-02-18
**作者**: Plan Cascade Team
**状态**: 设计阶段

---

## 目录

1. [设计目标](#1-设计目标)
2. [系统架构](#2-系统架构)
3. [功能一：Webhook 通知](#3-功能一webhook-通知)
4. [功能二：远程会话控制](#4-功能二远程会话控制)
5. [代理集成](#5-代理集成)
6. [安全设计](#6-安全设计)
7. [数据库设计](#7-数据库设计)
8. [前端设计](#8-前端设计)
9. [API 设计](#9-api-设计)
10. [实施计划](#10-实施计划)

---

## 1. 设计目标

### 1.1 核心目标

1. **Webhook 通知**：为后台长时间运行的任务提供通用通知系统，支持多渠道（Slack、飞书、Telegram Bot 等），支持全局和按会话粒度配置
2. **远程会话控制**：通过消息平台（Telegram Bot 等）远程与桌面客户端交互，支持创建会话、发送指令、监控执行状态
3. **代理复用**：完全复用现有代理基础设施（`ProxyConfig`、`ProxyStrategy`、`build_http_client()`），无需重复实现代理逻辑
4. **功能协同**：两个功能天然组合——远程指令触发任务，Webhook 通知回传结果

### 1.2 设计约束

| 约束 | 说明 |
|------|------|
| 架构一致性 | 遵循现有模式：Tauri commands、service 层、Zustand stores |
| 代理复用 | 使用现有 `build_http_client()` 和 per-provider strategy 机制 |
| 安全性 | Bot Token 和 Webhook Secret 存储于 OS Keyring，远程访问需认证 |
| 桌面依赖 | 远程控制依赖桌面应用运行中且可联网 |
| 消息限制 | 处理平台特定限制（Telegram 4096 字符限制、Slack Block 限制） |
| 跨平台 | 所有功能需支持 Windows、macOS、Linux |

### 1.3 功能协同

```
┌─────────────────────────────────────────────────────────────────┐
│                     远程控制 + 通知 完整流程                      │
│                                                                  │
│   Telegram Bot ──发送指令──→ 桌面应用（执行任务）                  │
│        ↑                              │                          │
│        └──── Webhook 通知 ←── 任务完成 ──→ Slack / 飞书          │
│                                                                  │
│   使用场景：通过 Telegram 发送 "/new ~/projects/myapp"，           │
│   桌面端创建会话并执行，完成后通过所有渠道推送通知                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. 系统架构

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Plan Cascade Desktop                                 │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                     React 前端 (TypeScript)                        │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐     │  │
│  │  │  Webhook      │  │  远程控制    │  │  现有组件            │     │  │
│  │  │  设置 UI      │  │  控制 UI     │  │  (会话等)            │     │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘     │  │
│  │         └─────────────────┴─────────────────────┘                  │  │
│  │                           │                                        │  │
│  │                  Zustand 状态管理                                   │  │
│  │          (webhookStore, remoteControlStore)                        │  │
│  └───────────────────────────┼────────────────────────────────────────┘  │
│                              │ Tauri IPC                                  │
│  ┌───────────────────────────┼────────────────────────────────────────┐  │
│  │                     Rust 后端                                       │  │
│  │                              │                                      │  │
│  │  ┌──────────────────────────┴────────────────────────────────┐     │  │
│  │  │                   命令层（新增）                            │     │  │
│  │  │  commands/webhook.rs  │  commands/remote.rs                │     │  │
│  │  └──────────────────────────┬────────────────────────────────┘     │  │
│  │                              │                                      │  │
│  │  ┌──────────────────────────┴────────────────────────────────┐     │  │
│  │  │                   服务层（新增）                            │     │  │
│  │  │                                                            │     │  │
│  │  │  ┌────────────────────┐    ┌─────────────────────────┐    │     │  │
│  │  │  │  Webhook 服务      │    │  远程网关服务             │    │     │  │
│  │  │  │  ┌──────────────┐  │    │  ┌───────────────────┐  │    │     │  │
│  │  │  │  │ 分发器       │  │    │  │  Telegram 适配器   │  │    │     │  │
│  │  │  │  │ 渠道管理器   │  │    │  │  命令路由器        │  │    │     │  │
│  │  │  │  │ 模板引擎     │  │    │  │  会话桥接器        │  │    │     │  │
│  │  │  │  └──────────────┘  │    │  │  响应映射器        │  │    │     │  │
│  │  │  └────────────────────┘    │  └───────────────────┘  │    │     │  │
│  │  │            │                │            │                │     │  │
│  │  │            └────────┬───────┘            │                │     │  │
│  │  └─────────────────────┼────────────────────┘                │     │  │
│  │                        │                                      │     │  │
│  │  ┌─────────────────────┴──────────────────────────────────┐  │     │  │
│  │  │                现有基础设施                               │  │     │  │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │  │     │  │
│  │  │  │  代理    │ │ 会话     │ │ 编排器   │ │  Claude   │  │  │     │  │
│  │  │  │  服务    │ │  管理    │ │          │ │  Code     │  │  │     │  │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │  │     │  │
│  │  └────────────────────────────────────────────────────────┘  │     │  │
│  │                                                                │     │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │     │  │
│  │  │                   存储层                                  │  │     │  │
│  │  │  SQLite (webhook 配置、远程会话映射、审计日志)              │  │     │  │
│  │  │  Keyring (bot token、webhook secret)                     │  │     │  │
│  │  └─────────────────────────────────────────────────────────┘  │     │  │
│  └────────────────────────────────────────────────────────────────┘     │  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 新增文件概览

```
desktop/src-tauri/src/
├── commands/
│   ├── webhook.rs              # Webhook Tauri 命令（增删改查、测试、历史）
│   └── remote.rs               # 远程控制 Tauri 命令（启停、状态）
├── services/
│   ├── webhook/
│   │   ├── mod.rs              # 模块导出
│   │   ├── service.rs          # WebhookService（分发器、事件监听）
│   │   ├── channels/
│   │   │   ├── mod.rs          # Channel trait + 注册表
│   │   │   ├── slack.rs        # Slack Incoming Webhook
│   │   │   ├── feishu.rs       # 飞书 Bot Webhook
│   │   │   ├── telegram.rs     # Telegram Bot API（sendMessage）
│   │   │   ├── discord.rs      # Discord Webhook（未来扩展）
│   │   │   └── custom.rs       # 自定义 HTTP Webhook
│   │   ├── templates.rs        # 消息模板引擎
│   │   └── types.rs            # Webhook 类型定义
│   └── remote/
│       ├── mod.rs              # 模块导出
│       ├── gateway.rs          # RemoteGatewayService（生命周期、适配器管理）
│       ├── adapters/
│       │   ├── mod.rs          # Adapter trait + 注册表
│       │   └── telegram.rs     # Telegram Bot 长轮询适配器
│       ├── command_router.rs   # 解析远程命令，分发到会话
│       ├── session_bridge.rs   # 远程命令与本地会话的桥接
│       ├── response_mapper.rs  # 流式事件到平台消息的映射
│       └── types.rs            # 远程控制类型定义
└── models/
    ├── webhook.rs              # Webhook 数据模型
    └── remote.rs               # 远程会话模型

desktop/src/
├── lib/
│   ├── webhookApi.ts           # Webhook IPC 封装
│   └── remoteApi.ts            # 远程控制 IPC 封装
├── store/
│   ├── webhook.ts              # Webhook Zustand store
│   └── remote.ts               # 远程控制 Zustand store
└── components/
    └── Settings/
        ├── WebhookSection.tsx   # Webhook 配置 UI
        └── RemoteSection.tsx    # 远程控制配置 UI
```

---

## 3. 功能一：Webhook 通知

### 3.1 概述

通用的通知系统，在后台长时间运行的任务完成（或失败）时触发。支持多个通知渠道，可配置作用范围（全局或按会话）。

### 3.2 核心类型

```rust
// services/webhook/types.rs

/// 支持的通知渠道类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookChannelType {
    Slack,
    Feishu,
    Telegram,
    Discord,
    Custom,
}

/// Webhook 渠道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookChannelConfig {
    pub id: String,                          // 唯一渠道 ID (uuid)
    pub name: String,                        // 用户友好名称
    pub channel_type: WebhookChannelType,
    pub enabled: bool,
    pub url: String,                         // Webhook URL 或 Bot API 端点
    #[serde(skip_serializing, default)]
    pub secret: Option<String>,              // Token/Secret（存储在 Keyring 中）
    pub scope: WebhookScope,
    pub events: Vec<WebhookEventType>,       // 触发此 webhook 的事件类型
    pub template: Option<String>,            // 自定义消息模板（可选）
    pub created_at: String,
    pub updated_at: String,
}

/// 通知作用范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookScope {
    /// 对所有会话触发
    Global,
    /// 仅对指定会话 ID 触发
    Sessions(Vec<String>),
}

/// 可触发 webhook 的事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookEventType {
    /// 任务/会话成功完成
    TaskComplete,
    /// 任务/会话失败
    TaskFailed,
    /// 任务被用户取消
    TaskCancelled,
    /// Story 完成（专家模式）
    StoryComplete,
    /// PRD 中所有 Story 完成
    PrdComplete,
    /// 长时间任务进度里程碑（25%、50%、75%）
    ProgressMilestone,
}

/// Webhook 投递载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event_type: WebhookEventType,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub project_path: Option<String>,
    pub summary: String,                     // 人类可读摘要
    pub details: Option<serde_json::Value>,  // 结构化详情
    pub timestamp: String,
    pub duration_ms: Option<u64>,
    pub token_usage: Option<TokenUsageSummary>,
}

/// 投递记录（用于审计和重试）
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

### 3.3 渠道 Trait

```rust
// services/webhook/channels/mod.rs

#[async_trait]
pub trait WebhookChannel: Send + Sync {
    /// 渠道类型标识
    fn channel_type(&self) -> WebhookChannelType;

    /// 通过此渠道发送通知
    async fn send(&self, payload: &WebhookPayload, config: &WebhookChannelConfig) -> Result<(), WebhookError>;

    /// 测试渠道连接
    async fn test(&self, config: &WebhookChannelConfig) -> Result<WebhookTestResult, WebhookError>;

    /// 将载荷格式化为此渠道特定的消息格式
    fn format_message(&self, payload: &WebhookPayload, template: Option<&str>) -> String;
}
```

### 3.4 渠道实现

#### 3.4.1 Slack

```rust
// services/webhook/channels/slack.rs

/// Slack Incoming Webhook 集成
/// 使用 Slack Block Kit 格式的富文本消息
///
/// Webhook URL 格式：https://hooks.slack.com/services/T.../B.../xxx
pub struct SlackChannel {
    client: reqwest::Client,     // 支持代理的 HTTP 客户端
}

impl SlackChannel {
    pub fn new(proxy: Option<&ProxyConfig>) -> Self {
        Self {
            client: build_http_client(proxy),
        }
    }
}

// 消息格式：Slack Block Kit JSON
// {
//   "blocks": [
//     { "type": "header", "text": { "type": "plain_text", "text": "✅ 任务完成" } },
//     { "type": "section", "text": { "type": "mrkdwn", "text": "*会话*: ..." } },
//     { "type": "context", "elements": [{ "type": "mrkdwn", "text": "耗时: ..." }] }
//   ]
// }
```

#### 3.4.2 飞书

```rust
// services/webhook/channels/feishu.rs

/// 飞书 Bot Webhook 集成
/// 使用飞书互动卡片格式
///
/// Webhook URL 格式：https://open.feishu.cn/open-apis/bot/v2/hook/xxx
/// 支持可选的签名验证（timestamp + secret -> SHA256 HMAC）
pub struct FeishuChannel {
    client: reqwest::Client,
}

// 消息格式：飞书互动卡片 JSON
// {
//   "msg_type": "interactive",
//   "card": {
//     "header": { "title": { "tag": "plain_text", "content": "任务完成" } },
//     "elements": [...]
//   }
// }
```

#### 3.4.3 Telegram

```rust
// services/webhook/channels/telegram.rs

/// Telegram Bot API 集成（仅用于通知，不用于远程控制）
/// 使用 sendMessage API + Markdown 格式
///
/// API 端点：https://api.telegram.org/bot<token>/sendMessage
/// 需要：bot_token（存于 Keyring）+ chat_id（在配置 URL 字段中）
pub struct TelegramNotifyChannel {
    client: reqwest::Client,
}

// 消息格式：Telegram MarkdownV2
// "✅ *任务完成*\n\n*会话*: my\\-session\n*耗时*: 5分32秒\n..."
```

#### 3.4.4 自定义 HTTP

```rust
// services/webhook/channels/custom.rs

/// 通用 HTTP Webhook，支持自定义集成
/// 将 JSON 载荷 POST 到任意 URL，可选 HMAC-SHA256 签名头
///
/// 请求头：
///   Content-Type: application/json
///   X-Webhook-Signature: sha256=<使用 secret 对 body 的 HMAC>
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
    /// 使用各渠道类型的代理感知 HTTP 客户端初始化
    pub fn new(
        db: Arc<Database>,
        keyring: Arc<KeyringService>,
        proxy_resolver: impl Fn(&str) -> Option<ProxyConfig>,
    ) -> Self {
        let mut channels = HashMap::new();

        // 每个渠道获取独立的代理解析 HTTP 客户端
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
        // ... 其他渠道

        Self { channels, db, keyring }
    }

    /// 将通知分发到所有匹配的渠道
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

    /// 重试失败的投递（定期或手动触发）
    pub async fn retry_failed(&self, max_attempts: u32) -> Vec<WebhookDelivery> { ... }

    /// 获取匹配事件类型和会话范围的配置
    async fn get_enabled_configs_for_event(&self, payload: &WebhookPayload) -> Vec<WebhookChannelConfig> { ... }
}
```

### 3.6 事件钩子集成

Webhook 系统在事件转发层接入现有执行流程：

```rust
// 集成点：commands/standalone.rs（事件转发任务）
// 集成点：commands/claude_code.rs（流事件处理器）

/// 当检测到终端事件时由事件转发器调用
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
                summary: format!("任务成功完成"),
                duration_ms: Some(start_time.elapsed().as_millis() as u64),
                token_usage: usage.clone(),
                ..Default::default()
            };
            webhook_service.dispatch(payload).await;
        }
        UnifiedStreamEvent::Error { message, .. } => {
            let payload = WebhookPayload {
                event_type: WebhookEventType::TaskFailed,
                summary: format!("任务失败: {}", message),
                ..Default::default()
            };
            webhook_service.dispatch(payload).await;
        }
        _ => {}
    }
}
```

---

## 4. 功能二：远程会话控制

### 4.1 概述

允许用户通过消息平台远程与桌面客户端交互。初始实现以 Telegram Bot 为主适配器，采用可扩展的适配器模式以支持未来更多平台。

### 4.2 核心类型

```rust
// services/remote/types.rs

/// 远程适配器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteAdapterType {
    Telegram,
    // 未来：Slack、Discord、WebSocket API 等
}

/// 远程网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGatewayConfig {
    pub enabled: bool,
    pub adapter: RemoteAdapterType,
    pub auto_start: bool,                      // 应用启动时自动启动网关
}

/// Telegram 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAdapterConfig {
    #[serde(skip_serializing, default)]
    pub bot_token: Option<String>,             // 存储在 Keyring 中
    pub allowed_chat_ids: Vec<i64>,            // 授权的聊天 ID 白名单
    pub allowed_user_ids: Vec<i64>,            // 授权的用户 ID 白名单
    pub require_password: bool,                // 可选的密码门控
    #[serde(skip_serializing, default)]
    pub access_password: Option<String>,       // 存储在 Keyring 中
    pub max_message_length: usize,             // 默认: 4000（Telegram 限制约 4096）
    pub streaming_mode: StreamingMode,
}

/// 流式 LLM 输出的处理方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingMode {
    /// 等待完成后发送最终结果
    WaitForComplete,
    /// 定期发送进度更新（每 N 秒）
    PeriodicUpdate { interval_secs: u32 },
    /// 原地编辑消息更新最新内容（Telegram editMessageText）
    LiveEdit { throttle_ms: u64 },
}

/// 从用户消息解析的远程命令
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    /// /new <path> [provider] [model] — 创建新会话
    NewSession {
        project_path: String,
        provider: Option<String>,
        model: Option<String>,
    },
    /// /send <message> 或纯文本 — 向活动会话发送消息
    SendMessage { content: String },
    /// /sessions — 列出活动会话
    ListSessions,
    /// /switch <session_id> — 切换活动会话
    SwitchSession { session_id: String },
    /// /status — 获取当前会话状态
    Status,
    /// /cancel — 取消当前执行
    Cancel,
    /// /close — 关闭当前会话
    CloseSession,
    /// /help — 显示可用命令
    Help,
}

/// 网关运行时状态
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

/// 远程聊天与本地会话的映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSessionMapping {
    pub chat_id: i64,                          // 远程聊天标识
    pub user_id: i64,                          // 远程用户标识
    pub local_session_id: Option<String>,       // 当前活动的本地会话
    pub session_type: SessionType,             // Claude Code 或 Standalone
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionType {
    ClaudeCode,
    Standalone { provider: String, model: String },
}
```

### 4.3 远程适配器 Trait

```rust
// services/remote/adapters/mod.rs

#[async_trait]
pub trait RemoteAdapter: Send + Sync {
    /// 适配器类型标识
    fn adapter_type(&self) -> RemoteAdapterType;

    /// 启动适配器（开始接收消息）
    async fn start(&self, command_tx: mpsc::Sender<IncomingRemoteMessage>) -> Result<(), RemoteError>;

    /// 优雅停止适配器
    async fn stop(&self) -> Result<(), RemoteError>;

    /// 向远程聊天发送文本回复
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), RemoteError>;

    /// 编辑已有消息（用于实时更新流式输出）
    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<(), RemoteError>;

    /// 发送"正在输入"指示器
    async fn send_typing(&self, chat_id: i64) -> Result<(), RemoteError>;

    /// 检查适配器健康状态/连接性
    async fn health_check(&self) -> Result<(), RemoteError>;
}

/// 来自远程平台的传入消息
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

### 4.4 Telegram 适配器实现

```rust
// services/remote/adapters/telegram.rs

use teloxide::prelude::*;

pub struct TelegramAdapter {
    config: TelegramAdapterConfig,
    bot: Bot,                                    // teloxide Bot 实例
    cancel_token: CancellationToken,
}

impl TelegramAdapter {
    pub fn new(config: TelegramAdapterConfig, proxy: Option<&ProxyConfig>) -> Result<Self, RemoteError> {
        // 构建支持代理的 reqwest 客户端
        let http_client = build_http_client(proxy);

        // 使用自定义 HTTP 客户端创建 teloxide Bot
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
            // 使用 teloxide 的长轮询分发器
            let handler = Update::filter_message().endpoint(
                move |msg: Message, bot: Bot| {
                    let tx = command_tx.clone();
                    let allowed_chats = allowed_chat_ids.clone();
                    let allowed_users = allowed_user_ids.clone();
                    async move {
                        // 授权检查
                        let chat_id = msg.chat.id.0;
                        let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

                        if !allowed_chats.is_empty() && !allowed_chats.contains(&chat_id) {
                            return Ok(());  // 静默忽略未授权的聊天
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
        // 处理 Telegram 的 4096 字符限制，按行边界分割
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

/// 在行边界处分割长消息以符合平台限制
fn split_message(text: &str, max_len: usize) -> Vec<String> {
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

### 4.5 命令路由器

```rust
// services/remote/command_router.rs

pub struct CommandRouter;

impl CommandRouter {
    /// 将传入消息文本解析为 RemoteCommand
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
            // 纯文本 → 视为发送到活动会话的消息
            RemoteCommand::SendMessage {
                content: text.to_string(),
            }
        }
    }
}
```

### 4.6 会话桥接器

```rust
// services/remote/session_bridge.rs

/// 将远程命令桥接到本地会话操作
pub struct SessionBridge {
    /// 映射：chat_id -> 本地会话
    sessions: RwLock<HashMap<i64, RemoteSessionMapping>>,
    /// 对 standalone state 的引用，用于访问编排器
    standalone_state: Arc<StandaloneState>,
    /// 对 claude code state 的引用，用于访问 CLI 会话
    claude_code_state: Arc<ClaudeCodeState>,
    /// Webhook 服务，用于通知
    webhook_service: Arc<WebhookService>,
    /// 数据库，用于持久化
    db: Arc<Database>,
}

impl SessionBridge {
    /// 为远程聊天创建新的本地会话
    pub async fn create_session(
        &self,
        chat_id: i64,
        user_id: i64,
        project_path: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<String, RemoteError> {
        // 根据 provider 确定会话类型
        let session_type = match provider {
            Some("claude-code") | None => SessionType::ClaudeCode,
            Some(p) => SessionType::Standalone {
                provider: p.to_string(),
                model: model.unwrap_or("default").to_string(),
            },
        };

        let session_id = match &session_type {
            SessionType::ClaudeCode => {
                // 使用 ClaudeCodeState 启动新的聊天会话
                self.claude_code_state
                    .session_manager
                    .start_session(project_path)
                    .await?
            }
            SessionType::Standalone { provider, model } => {
                // 创建 standalone 编排器会话
                self.standalone_state
                    .create_session(project_path, provider, model)
                    .await?
            }
        };

        // 存储映射
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

    /// 向本地会话发送消息并收集响应
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

    /// 将流式响应收集为最终文本结果
    async fn send_to_standalone(
        &self,
        session_id: &str,
        content: &str,
    ) -> Result<RemoteResponse, RemoteError> {
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(256);

        // 触发编排器执行
        let orchestrator = self.standalone_state
            .get_orchestrator(session_id)
            .ok_or(RemoteError::SessionNotFound)?;

        let orchestrator = orchestrator.clone();
        let content = content.to_string();
        tokio::spawn(async move {
            let _ = orchestrator.execute(&content, tx).await;
        });

        // 将流式事件收集为最终响应
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

    // ... send_to_claude_code 类似模式
}
```

### 4.7 远程网关服务

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
    /// 启动远程网关
    pub async fn start(&self) -> Result<(), RemoteError> {
        let config = self.config.read().await;
        if !config.enabled {
            return Err(RemoteError::NotEnabled);
        }

        let (tx, mut rx) = mpsc::channel::<IncomingRemoteMessage>(100);

        // 启动适配器
        {
            let adapter = self.adapter.read().await;
            if let Some(adapter) = adapter.as_ref() {
                adapter.start(tx).await?;
            }
        }

        // 启动命令处理循环
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

        // 更新状态
        let mut status = self.status.write().await;
        status.running = true;
        status.connected_since = Some(chrono::Utc::now().to_rfc3339());

        Ok(())
    }

    /// 处理传入的远程消息
    async fn handle_message(
        msg: &IncomingRemoteMessage,
        bridge: &SessionBridge,
        adapter: &RwLock<Option<Box<dyn RemoteAdapter>>>,
        status: &RwLock<GatewayStatus>,
        webhook: &WebhookService,
    ) {
        // 更新统计
        {
            let mut s = status.write().await;
            s.total_commands_processed += 1;
            s.last_command_at = Some(chrono::Utc::now().to_rfc3339());
        }

        let command = CommandRouter::parse(&msg.text);
        let adapter_guard = adapter.read().await;
        let adapter = adapter_guard.as_ref().unwrap();

        // 发送"正在输入"指示器
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
                    Ok(id) => format!("✅ 会话已创建: {}\n项目: {}", id, project_path),
                    Err(e) => format!("❌ 创建会话失败: {}", e),
                }
            }
            RemoteCommand::SendMessage { content } => {
                match bridge.send_message(msg.chat_id, &content).await {
                    Ok(resp) => {
                        let mut result = resp.text.clone();
                        if let Some(tools) = &resp.tool_summary {
                            result = format!("{}\n\n📎 使用的工具:\n{}", result, tools);
                        }
                        result
                    }
                    Err(RemoteError::NoActiveSession) => {
                        "⚠️ 没有活动会话。使用 /new <path> 创建一个。".to_string()
                    }
                    Err(e) => format!("❌ 错误: {}", e),
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
                    Ok(()) => "🛑 执行已取消。".to_string(),
                    Err(e) => format!("❌ 取消失败: {}", e),
                }
            }
            RemoteCommand::Help => {
                HELP_TEXT.to_string()
            }
            _ => "未知命令。输入 /help 查看可用命令。".to_string(),
        };

        let _ = adapter.send_message(msg.chat_id, &response).await;
    }

    /// 优雅停止网关
    pub async fn stop(&self) -> Result<(), RemoteError> {
        self.cancel_token.cancel();
        if let Some(adapter) = self.adapter.read().await.as_ref() {
            adapter.stop().await?;
        }
        let mut status = self.status.write().await;
        status.running = false;
        Ok(())
    }

    /// 获取当前网关状态
    pub async fn get_status(&self) -> GatewayStatus {
        self.status.read().await.clone()
    }
}

const HELP_TEXT: &str = r#"🤖 Plan Cascade 远程控制

可用命令：
  /new <path> [provider] [model]  — 创建新会话
  /send <message>                 — 发送消息（或直接输入文本）
  /sessions                       — 列出活动会话
  /switch <id>                    — 切换到某个会话
  /status                         — 当前会话状态
  /cancel                         — 取消正在运行的执行
  /close                          — 关闭当前会话
  /help                           — 显示此帮助

示例：
  /new ~/projects/myapp
  /new ~/projects/api anthropic claude-sonnet-4-5-20250929
  如何修复登录 bug？
  /cancel
"#;
```

### 4.8 流式响应策略

对于长时间运行的 LLM 响应，支持三种策略：

```
┌─────────────────────────────────────────────────────────────────┐
│                     流式输出模式选项                              │
│                                                                  │
│  1. WaitForComplete（默认）                                      │
│     用户发送消息 → "⏳ 处理中..." → 最终结果                      │
│     ✅ 简单、可靠                                                │
│     ❌ 等待时间长，无进度可见性                                    │
│                                                                  │
│  2. PeriodicUpdate（间隔: 10s）                                  │
│     用户发送消息 → "⏳ 处理中..." →                               │
│       "[10s] 正在分析..." →                                      │
│       "[20s] 执行工具: Grep..." →                                │
│       最终结果                                                    │
│     ✅ 良好的进度可见性                                           │
│     ❌ 聊天中多条消息                                             │
│                                                                  │
│  3. LiveEdit（节流: 2000ms）                                     │
│     用户发送消息 → 原地编辑同一条消息更新最新内容                    │
│     ✅ 聊天整洁，实时感                                           │
│     ❌ 速率限制（Telegram: 每聊天每分钟 30 次编辑）                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. 代理集成

### 5.1 复用现有基础设施

两个新功能无需修改即可集成现有代理系统：

```
现有代理系统                          新功能
──────────                          ────
ProxyConfig（类型）          ────→   WebhookChannel.new(proxy)
ProxyStrategy（per-provider）────→   新增 provider ID
build_http_client()          ────→   所有 HTTP 客户端
resolve_provider_proxy()     ────→   服务初始化时调用
Keyring（密码存储）          ────→   Bot Token 与 Webhook Secret
NetworkSection.tsx（UI）     ────→   扩展新 provider 显示
```

### 5.2 新增 Provider ID

向代理系统的 `PROVIDER_IDS` 数组添加新条目：

```rust
// commands/proxy.rs — 扩展后的 PROVIDER_IDS

const PROVIDER_IDS: &[&str] = &[
    // 现有 LLM providers
    "anthropic", "openai", "deepseek", "qwen", "glm", "minimax", "ollama",
    // 现有后端
    "claude_code",
    // 现有 embedding providers
    "embedding_openai", "embedding_qwen", "embedding_glm", "embedding_ollama",
    // 新增：Webhook 通知渠道
    "webhook_slack",
    "webhook_feishu",
    "webhook_telegram",
    "webhook_discord",
    "webhook_custom",
    // 新增：远程控制适配器
    "remote_telegram",
];
```

### 5.3 新 Provider 的默认策略

```rust
// commands/proxy.rs — 扩展后的 default_strategy_for()

fn default_strategy_for(provider: &str) -> ProxyStrategy {
    match provider {
        // 国际服务 → 默认 UseGlobal
        "anthropic" | "openai" | "claude_code" | "embedding_openai" => ProxyStrategy::UseGlobal,
        "webhook_slack" | "webhook_discord" => ProxyStrategy::UseGlobal,
        "remote_telegram" | "webhook_telegram" => ProxyStrategy::UseGlobal,

        // 国内服务 → 默认 NoProxy
        "qwen" | "glm" | "deepseek" | "minimax" | "ollama" => ProxyStrategy::NoProxy,
        "embedding_qwen" | "embedding_glm" | "embedding_ollama" => ProxyStrategy::NoProxy,
        "webhook_feishu" => ProxyStrategy::NoProxy,

        // 自定义 Webhook → UseGlobal（外部目标通常需要）
        "webhook_custom" => ProxyStrategy::UseGlobal,

        _ => ProxyStrategy::UseGlobal,
    }
}
```

### 5.4 代理解析流程

```
┌──────────────────────────────────────────────────────────────┐
│              新功能的代理解析流程                                │
│                                                                │
│  WebhookService::new()                                         │
│    ├─ resolve_provider_proxy("webhook_slack")  → ProxyConfig?  │
│    ├─ resolve_provider_proxy("webhook_feishu") → ProxyConfig?  │
│    ├─ resolve_provider_proxy("webhook_telegram") → ProxyConfig?│
│    └─ 每个渠道: build_http_client(proxy) → reqwest::Client     │
│                                                                │
│  TelegramAdapter::new()                                        │
│    ├─ resolve_provider_proxy("remote_telegram") → ProxyConfig? │
│    └─ build_http_client(proxy) → reqwest::Client → Bot         │
└──────────────────────────────────────────────────────────────┘
```

---

## 6. 安全设计

### 6.1 凭据存储

| 凭据 | 存储位置 | Keyring Key |
|------|---------|-------------|
| Webhook secret/token | OS Keyring | `webhook_{channel_id}` |
| Telegram Bot Token | OS Keyring | `remote_telegram_bot_token` |
| 远程访问密码 | OS Keyring | `remote_access_password` |
| 代理密码 | OS Keyring（现有） | `proxy_{provider}` |

### 6.2 远程访问认证

```
┌─────────────────────────────────────────────────────────────┐
│                远程访问安全分层                                 │
│                                                               │
│  第 1 层：Bot Token（固有）                                    │
│    只有你的 bot 通过其唯一 token 接收消息                       │
│                                                               │
│  第 2 层：Chat ID 白名单                                       │
│    只有预配置的 chat ID 的消息会被处理                           │
│    所有其他消息被静默丢弃                                       │
│                                                               │
│  第 3 层：User ID 白名单                                       │
│    只有允许聊天中的特定用户 ID 可以发送命令                      │
│                                                               │
│  第 4 层：访问密码（可选）                                      │
│    首条消息必须是 "/auth <password>"                            │
│    会话在认证前受密码门控保护                                    │
│                                                               │
│  第 5 层：项目路径限制（可配置）                                 │
│    限制哪些目录可以作为会话打开                                  │
│    防止访问敏感的系统目录                                       │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 审计日志

所有远程命令记录到 SQLite：

```rust
pub struct RemoteAuditEntry {
    pub id: String,
    pub adapter_type: String,
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
    pub command: String,
    pub command_type: String,
    pub result_status: String,        // "success"、"error"、"unauthorized"
    pub error_message: Option<String>,
    pub created_at: String,
}
```

---

## 7. 数据库设计

### 7.1 新增表

```sql
-- Webhook 渠道配置
CREATE TABLE IF NOT EXISTS webhook_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    channel_type TEXT NOT NULL,          -- 'slack', 'feishu', 'telegram', 'discord', 'custom'
    enabled INTEGER NOT NULL DEFAULT 1,
    url TEXT NOT NULL,
    scope_type TEXT NOT NULL DEFAULT 'global',  -- 'global' 或 'sessions'
    scope_sessions TEXT,                  -- JSON 数组的会话 ID（当 scope_type = 'sessions' 时）
    events TEXT NOT NULL,                 -- JSON 数组的事件类型
    template TEXT,                        -- 自定义消息模板
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Webhook 投递历史（用于审计和重试）
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,               -- JSON 序列化的 WebhookPayload
    status TEXT NOT NULL,                -- 'pending', 'success', 'failed', 'retrying'
    status_code INTEGER,
    response_body TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES webhook_channels(id) ON DELETE CASCADE
);

-- 投递重试查询索引
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status
    ON webhook_deliveries(status, last_attempt_at);

-- 远程会话映射
CREATE TABLE IF NOT EXISTS remote_session_mappings (
    chat_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    adapter_type TEXT NOT NULL,
    local_session_id TEXT,
    session_type TEXT NOT NULL,          -- JSON: {"ClaudeCode"} 或 {"Standalone":{"provider":"...","model":"..."}}
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (adapter_type, chat_id)
);

-- 远程命令审计日志
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

-- 审计查询索引
CREATE INDEX IF NOT EXISTS idx_remote_audit_created
    ON remote_audit_log(created_at DESC);
```

---

## 8. 前端设计

### 8.1 Webhook 设置 UI

位于 `Settings > 通知`：

```
┌──────────────────────────────────────────────────────────────┐
│  通知                                                         │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Webhook 渠道                               [+ 添加]  │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ 🟢 Slack — #dev-notifications                  │   │    │
│  │  │    事件: 任务完成, 任务失败                       │   │    │
│  │  │    范围: 全局         [测试] [编辑] [删除]       │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ 🟢 飞书 — 项目更新机器人                        │   │    │
│  │  │    事件: 全部                                    │   │    │
│  │  │    范围: 全局         [测试] [编辑] [删除]       │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  │                                                        │    │
│  │  ┌────────────────────────────────────────────────┐   │    │
│  │  │ ⚪ Telegram — @my_notify_bot                   │   │    │
│  │  │    事件: 任务完成                                │   │    │
│  │  │    范围: 2 个会话     [测试] [编辑] [删除]       │   │    │
│  │  └────────────────────────────────────────────────┘   │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  投递历史                                [查看全部 →]  │    │
│  │                                                        │    │
│  │  ✅ 2分钟前  Slack   任务完成  "会话 abc..."          │    │
│  │  ❌ 5分钟前  飞书    任务失败  "错误: 超时"           │    │
│  │  ✅ 12分钟前 Slack   PRD完成  "所有 5 个 Story..."    │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 8.2 远程控制设置 UI

位于 `Settings > 远程控制`：

```
┌──────────────────────────────────────────────────────────────┐
│  远程控制                                                     │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  网关状态                          [启动] / [停止]     │    │
│  │                                                        │    │
│  │  状态: 🟢 运行中（自 14:30 连接）                      │    │
│  │  适配器: Telegram Bot (@my_cascade_bot)                │    │
│  │  已处理命令: 47                                        │    │
│  │  活动远程会话: 2                                       │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Telegram Bot 配置                                    │    │
│  │                                                        │    │
│  │  Bot Token: ●●●●●●●●●●●●●●●●●●●●          [更改]     │    │
│  │  自动启动: [✓]                                         │    │
│  │                                                        │    │
│  │  允许的 Chat ID:                                       │    │
│  │    ┌────────────────────┐  [+ 添加]                    │    │
│  │    │ 123456789  [×]     │                              │    │
│  │    │ 987654321  [×]     │                              │    │
│  │    └────────────────────┘                              │    │
│  │                                                        │    │
│  │  允许的 User ID:                                       │    │
│  │    ┌────────────────────┐  [+ 添加]                    │    │
│  │    │ 111222333  [×]     │                              │    │
│  │    └────────────────────┘                              │    │
│  │                                                        │    │
│  │  密码保护: [✓]                                         │    │
│  │  访问密码: ●●●●●●●●                    [更改]          │    │
│  │                                                        │    │
│  │  流式输出模式: [等待完成 ▼]                             │    │
│  │                                                        │    │
│  │  允许的项目路径:                                        │    │
│  │    ┌────────────────────────────────┐  [+ 添加]        │    │
│  │    │ ~/projects       [×]           │                  │    │
│  │    │ ~/work           [×]           │                  │    │
│  │    └────────────────────────────────┘                  │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  活动远程会话                                          │    │
│  │                                                        │    │
│  │  Chat 123456789 → 会话 abc-123 (ClaudeCode)           │    │
│  │    项目: ~/projects/myapp                              │    │
│  │    最近活动: 2分钟前                                    │    │
│  │                                                        │    │
│  │  Chat 987654321 → 会话 def-456 (Anthropic/Sonnet)     │    │
│  │    项目: ~/work/api-server                             │    │
│  │    最近活动: 15分钟前                                   │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  审计日志                                 [查看全部 →]  │    │
│  │                                                        │    │
│  │  14:35  @user  /new ~/projects/myapp      ✅ 成功      │    │
│  │  14:35  @user  "修复登录 bug"             ✅ 成功      │    │
│  │  14:32  @other /new ~/secret              ❌ 未授权    │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 8.3 主 UI 中的远程会话

远程创建的会话在主会话列表中可见，带有远程标识：

```
会话列表
├─ 📱 abc-123（通过 Telegram @user）    — ~/projects/myapp
├─ 📱 def-456（通过 Telegram @user）    — ~/work/api-server
├─    ghi-789                           — ~/projects/other
└─    jkl-012                           — ~/work/frontend
```

---

## 9. API 设计

### 9.1 Webhook Tauri 命令

```rust
// commands/webhook.rs

/// 列出所有已配置的 webhook 渠道
#[tauri::command]
pub async fn list_webhook_channels(state: State<'_, AppState>) -> Result<CommandResponse<Vec<WebhookChannelConfig>>, String>

/// 创建新的 webhook 渠道
#[tauri::command]
pub async fn create_webhook_channel(config: CreateWebhookRequest, state: State<'_, AppState>) -> Result<CommandResponse<WebhookChannelConfig>, String>

/// 更新现有的 webhook 渠道
#[tauri::command]
pub async fn update_webhook_channel(id: String, config: UpdateWebhookRequest, state: State<'_, AppState>) -> Result<CommandResponse<WebhookChannelConfig>, String>

/// 删除 webhook 渠道
#[tauri::command]
pub async fn delete_webhook_channel(id: String, state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// 测试 webhook 渠道（发送测试通知）
#[tauri::command]
pub async fn test_webhook_channel(id: String, state: State<'_, AppState>) -> Result<CommandResponse<WebhookTestResult>, String>

/// 获取投递历史（分页）
#[tauri::command]
pub async fn get_webhook_deliveries(channel_id: Option<String>, limit: Option<u32>, offset: Option<u32>, state: State<'_, AppState>) -> Result<CommandResponse<Vec<WebhookDelivery>>, String>

/// 重试失败的投递
#[tauri::command]
pub async fn retry_webhook_delivery(delivery_id: String, state: State<'_, AppState>) -> Result<CommandResponse<WebhookDelivery>, String>
```

### 9.2 远程控制 Tauri 命令

```rust
// commands/remote.rs

/// 获取远程网关状态
#[tauri::command]
pub async fn get_remote_gateway_status(state: State<'_, RemoteState>) -> Result<CommandResponse<GatewayStatus>, String>

/// 启动远程网关
#[tauri::command]
pub async fn start_remote_gateway(state: State<'_, RemoteState>, app_state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// 停止远程网关
#[tauri::command]
pub async fn stop_remote_gateway(state: State<'_, RemoteState>) -> Result<CommandResponse<()>, String>

/// 获取远程网关配置
#[tauri::command]
pub async fn get_remote_config(state: State<'_, AppState>) -> Result<CommandResponse<RemoteGatewayConfig>, String>

/// 更新远程网关配置（Telegram 设置）
#[tauri::command]
pub async fn update_remote_config(config: UpdateRemoteConfigRequest, state: State<'_, AppState>) -> Result<CommandResponse<()>, String>

/// 列出活动的远程会话映射
#[tauri::command]
pub async fn list_remote_sessions(state: State<'_, RemoteState>) -> Result<CommandResponse<Vec<RemoteSessionMapping>>, String>

/// 断开远程会话
#[tauri::command]
pub async fn disconnect_remote_session(chat_id: i64, state: State<'_, RemoteState>) -> Result<CommandResponse<()>, String>

/// 获取远程审计日志（分页）
#[tauri::command]
pub async fn get_remote_audit_log(limit: Option<u32>, offset: Option<u32>, state: State<'_, AppState>) -> Result<CommandResponse<Vec<RemoteAuditEntry>>, String>
```

### 9.3 新增 Tauri State

```rust
// main.rs — 新增 managed state

pub struct WebhookState {
    pub service: Arc<WebhookService>,
}

pub struct RemoteState {
    pub gateway: Arc<RemoteGatewayService>,
}

// 在 main() 中：
app.manage(WebhookState { service: webhook_service });
app.manage(RemoteState { gateway: remote_gateway });
```

---

## 10. 实施计划

### 10.1 阶段划分

```
阶段一：Webhook 通知（基础）
├── 1.1 核心类型与渠道 trait
├── 1.2 Slack 渠道实现
├── 1.3 飞书渠道实现
├── 1.4 Telegram 通知渠道
├── 1.5 自定义 HTTP 渠道
├── 1.6 WebhookService（分发器 + 重试）
├── 1.7 数据库 schema + 迁移
├── 1.8 Tauri 命令（增删改查 + 测试 + 历史）
├── 1.9 事件钩子集成（standalone + claude_code）
├── 1.10 代理集成（新增 provider ID）
├── 1.11 前端：webhookApi.ts + webhook store
└── 1.12 前端：WebhookSection.tsx 设置 UI

阶段二：远程会话控制
├── 2.1 核心类型与适配器 trait
├── 2.2 命令路由器
├── 2.3 会话桥接器
├── 2.4 响应映射器（流式 → 文本）
├── 2.5 Telegram 适配器（teloxide）
├── 2.6 RemoteGatewayService（生命周期 + 消息循环）
├── 2.7 数据库 schema（映射 + 审计）
├── 2.8 Tauri 命令（启停 + 配置 + 审计）
├── 2.9 代理集成
├── 2.10 安全机制（认证层 + 审计日志）
├── 2.11 前端：remoteApi.ts + remote store
└── 2.12 前端：RemoteSection.tsx 设置 UI

阶段三：集成与完善
├── 3.1 远程命令触发 webhook 通知
├── 3.2 远程会话在主 UI 中可见
├── 3.3 应用启动时自动启动网关
├── 3.4 国际化（en、zh、ja）
└── 3.5 测试与文档
```

### 10.2 依赖关系

```
                    ┌──────────────┐
                    │   代理系统    │（现有，共享）
                    └──────┬───────┘
                           │
              ┌────────────┴────────────┐
              │                         │
    ┌─────────▼─────────┐    ┌─────────▼─────────┐
    │ 阶段一：Webhook    │    │ 阶段二：远程       │
    │ 通知               │    │ 会话控制           │
    └─────────┬─────────┘    └─────────┬─────────┘
              │                         │
              └────────────┬────────────┘
                           │
                 ┌─────────▼─────────┐
                 │ 阶段三：集成       │
                 └───────────────────┘
```

阶段一和阶段二可以**并行开发**，因为它们是独立的。阶段三负责整合。

### 10.3 Cargo 依赖（新增）

```toml
# desktop/src-tauri/Cargo.toml — 新增依赖

[dependencies]
# 远程控制 - Telegram bot
teloxide = { version = "0.13", features = ["macros"] }

# Webhook - HMAC 签名
hmac = "0.12"
sha2 = "0.10"

# 工具类
chrono = { version = "0.4", features = ["serde"] }
```

### 10.4 风险缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Telegram API 速率限制 | 消息投递延迟 | 在适配器中实现节流 + 指数退避 |
| 长轮询网络不稳定 | 网关断连 | 自动重连 + 退避策略，UI 中显示状态监控 |
| 大量 LLM 响应超出消息限制 | 输出截断 | 在逻辑边界处智能分割消息 |
| 未授权的远程访问 | 安全风险 | 多层认证（Chat ID + User ID + 密码） |
| 桌面应用离线 | 远程控制不可用 | Bot 返回明确错误信息，恢复后自动重连 |
| 代理配置错误 | Webhook 投递失败 | 每个渠道提供测试按钮，投递历史显示错误详情 |
