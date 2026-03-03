//! Webhook Channel Trait and Registry
//!
//! Defines the async trait that all webhook channel implementations must satisfy,
//! plus channel module exports.

pub mod custom;
pub mod discord;
pub mod feishu;
pub mod serverchan;
pub mod slack;
pub mod telegram;

use async_trait::async_trait;

use super::types::{
    WebhookChannelConfig, WebhookChannelType, WebhookError, WebhookEventType, WebhookPayload,
    WebhookSendResult, WebhookTestResult,
};

pub(super) fn format_timestamp_for_display(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|_| timestamp.to_string())
}

#[derive(Clone, Copy)]
pub(super) enum LabelKey {
    Event,
    Session,
    Project,
    Source,
    Summary,
    Duration,
    Time,
}

pub(super) fn normalize_locale(locale: Option<&str>) -> &'static str {
    let raw = locale.unwrap_or("en").to_ascii_lowercase();
    if raw.starts_with("zh") {
        "zh"
    } else if raw.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

pub(super) fn localized_label(locale: Option<&str>, key: LabelKey) -> &'static str {
    match (normalize_locale(locale), key) {
        ("zh", LabelKey::Event) => "事件",
        ("zh", LabelKey::Session) => "会话",
        ("zh", LabelKey::Project) => "项目",
        ("zh", LabelKey::Source) => "来源",
        ("zh", LabelKey::Summary) => "摘要",
        ("zh", LabelKey::Duration) => "耗时",
        ("zh", LabelKey::Time) => "时间",
        ("ja", LabelKey::Event) => "イベント",
        ("ja", LabelKey::Session) => "セッション",
        ("ja", LabelKey::Project) => "プロジェクト",
        ("ja", LabelKey::Source) => "ソース",
        ("ja", LabelKey::Summary) => "要約",
        ("ja", LabelKey::Duration) => "所要時間",
        ("ja", LabelKey::Time) => "時刻",
        ("en", LabelKey::Event) => "Event",
        ("en", LabelKey::Session) => "Session",
        ("en", LabelKey::Project) => "Project",
        ("en", LabelKey::Source) => "Source",
        ("en", LabelKey::Summary) => "Summary",
        ("en", LabelKey::Duration) => "Duration",
        ("en", LabelKey::Time) => "Time",
        _ => "Event",
    }
}

pub(super) fn localized_event_name(
    event_type: &WebhookEventType,
    locale: Option<&str>,
) -> &'static str {
    match (normalize_locale(locale), event_type) {
        ("zh", WebhookEventType::TaskComplete) => "任务完成",
        ("zh", WebhookEventType::TaskFailed) => "任务失败",
        ("zh", WebhookEventType::TaskCancelled) => "任务已取消",
        ("zh", WebhookEventType::StoryComplete) => "故事完成",
        ("zh", WebhookEventType::PrdComplete) => "PRD完成",
        ("zh", WebhookEventType::ProgressMilestone) => "进度里程碑",
        ("ja", WebhookEventType::TaskComplete) => "タスク完了",
        ("ja", WebhookEventType::TaskFailed) => "タスク失敗",
        ("ja", WebhookEventType::TaskCancelled) => "タスクキャンセル",
        ("ja", WebhookEventType::StoryComplete) => "ストーリー完了",
        ("ja", WebhookEventType::PrdComplete) => "PRD完了",
        ("ja", WebhookEventType::ProgressMilestone) => "進捗マイルストーン",
        ("en", WebhookEventType::TaskComplete) => "TaskComplete",
        ("en", WebhookEventType::TaskFailed) => "TaskFailed",
        ("en", WebhookEventType::TaskCancelled) => "TaskCancelled",
        ("en", WebhookEventType::StoryComplete) => "StoryComplete",
        ("en", WebhookEventType::PrdComplete) => "PrdComplete",
        ("en", WebhookEventType::ProgressMilestone) => "ProgressMilestone",
        _ => "TaskComplete",
    }
}

/// Async trait for webhook channel implementations.
///
/// Each channel is responsible for formatting messages to a platform-specific
/// format and sending them via HTTP. Channels receive a proxy-aware
/// `reqwest::Client` at construction time.
#[async_trait]
pub trait WebhookChannel: Send + Sync {
    /// Channel type identifier.
    fn channel_type(&self) -> WebhookChannelType;

    /// Send a notification through this channel.
    async fn send(
        &self,
        payload: &WebhookPayload,
        config: &WebhookChannelConfig,
    ) -> Result<WebhookSendResult, WebhookError>;

    /// Test the channel connection by sending a test notification.
    async fn test(&self, config: &WebhookChannelConfig) -> Result<WebhookTestResult, WebhookError>;

    /// Format the payload into a platform-specific message string/JSON.
    fn format_message(&self, payload: &WebhookPayload, template: Option<&str>) -> String;
}
