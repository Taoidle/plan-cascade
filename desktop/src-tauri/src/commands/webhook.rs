//! Webhook Notification Commands
//!
//! Tauri commands for webhook channel CRUD, testing, delivery history,
//! retry operations, and worker health.

use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::webhook::sanitize::sanitize_for_user;
use crate::services::webhook::service::{
    WebhookService, WEBHOOK_MAX_ATTEMPTS, WEBHOOK_RETENTION_DAYS,
};
use crate::services::webhook::types::*;
use crate::state::AppState;
use crate::storage::{Database, KeyringService};

/// Keyring key prefix for webhook secrets.
const WEBHOOK_KEYRING_PREFIX: &str = "webhook_";
const MAX_NAME_LEN: usize = 80;
const MAX_URL_LEN: usize = 2048;
const MAX_TEMPLATE_LEN: usize = 4000;
const MAX_SCOPE_SESSIONS: usize = 100;
const MAX_SESSION_ID_LEN: usize = 128;
const DELIVERY_LIMIT_DEFAULT: u32 = 50;
const DELIVERY_LIMIT_MAX: u32 = 200;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Tauri-managed state for the webhook subsystem.
pub struct WebhookState {
    service: RwLock<Option<Arc<WebhookService>>>,
    worker_started: Arc<AtomicBool>,
    worker_running: Arc<AtomicBool>,
    last_retry_at: Arc<RwLock<Option<String>>>,
    retry_cycle_count: Arc<AtomicU64>,
    last_retry_error: Arc<RwLock<Option<String>>>,
}

impl WebhookState {
    pub fn new() -> Self {
        Self {
            service: RwLock::new(None),
            worker_started: Arc::new(AtomicBool::new(false)),
            worker_running: Arc::new(AtomicBool::new(false)),
            last_retry_at: Arc::new(RwLock::new(None)),
            retry_cycle_count: Arc::new(AtomicU64::new(0)),
            last_retry_error: Arc::new(RwLock::new(None)),
        }
    }

    /// Lazily initialize the real webhook service (DB + keyring + proxy resolver).
    pub async fn get_or_init(&self, app_state: &AppState) -> Result<Arc<WebhookService>, String> {
        if let Some(service) = self.service.read().await.clone() {
            return Ok(service);
        }

        let db = app_state
            .with_database(|db| Ok(Arc::new(db.clone())))
            .await
            .map_err(|e| sanitize_error_message(e.to_string()))?;
        let keyring = Arc::new(KeyringService::new());
        if let Err(error) = cleanup_orphan_webhook_secrets(db.as_ref(), keyring.as_ref()) {
            tracing::warn!(error = %sanitize_error_message(error), "failed to cleanup orphan webhook secrets");
        }

        let service = Arc::new(WebhookService::new_default(db, keyring));
        let mut guard = self.service.write().await;
        if guard.is_none() {
            *guard = Some(service.clone());
        }
        Ok(guard.as_ref().cloned().unwrap_or(service))
    }

    /// Start retry/cleanup worker once. Safe to call repeatedly.
    pub async fn start_worker_if_needed(&self, app_state: &AppState) -> Result<(), String> {
        if self.worker_running.load(Ordering::SeqCst)
            || self.worker_started.load(Ordering::SeqCst)
        {
            return Ok(());
        }
        if self
            .worker_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let service = self.get_or_init(app_state).await?;
        let worker_started = self.worker_started.clone();
        let worker_running = self.worker_running.clone();
        let last_retry_at = self.last_retry_at.clone();
        let retry_cycle_count = self.retry_cycle_count.clone();
        let last_retry_error = self.last_retry_error.clone();

        tokio::spawn(async move {
            worker_running.store(true, Ordering::SeqCst);
            let worker_loop = async {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let mut last_cleanup = chrono::Utc::now();

                loop {
                    interval.tick().await;
                    let now = chrono::Utc::now();
                    retry_cycle_count.fetch_add(1, Ordering::Relaxed);
                    {
                        let mut guard = last_retry_at.write().await;
                        *guard = Some(now.to_rfc3339());
                    }

                    match service.retry_failed(WEBHOOK_MAX_ATTEMPTS).await {
                        Ok(retries) => {
                            if !retries.is_empty() {
                                tracing::info!(
                                    retried = retries.len(),
                                    "webhook retry worker processed due deliveries"
                                );
                            }
                            let mut guard = last_retry_error.write().await;
                            *guard = None;
                        }
                        Err(error) => {
                            let message = sanitize_error_message(error.to_string());
                            tracing::warn!(error = %message, "webhook retry cycle failed");
                            let mut guard = last_retry_error.write().await;
                            *guard = Some(message);
                        }
                    }

                    if now.signed_duration_since(last_cleanup).num_hours() >= 24 {
                        let deleted = service.cleanup_old_deliveries(WEBHOOK_RETENTION_DAYS);
                        last_cleanup = now;
                        tracing::info!(
                            deleted,
                            retention_days = WEBHOOK_RETENTION_DAYS,
                            "webhook delivery retention cleanup completed"
                        );
                    }
                }
            };

            if AssertUnwindSafe(worker_loop).catch_unwind().await.is_err() {
                tracing::error!("webhook retry worker crashed");
            }
            worker_running.store(false, Ordering::SeqCst);
            worker_started.store(false, Ordering::SeqCst);
        });

        Ok(())
    }

    pub async fn get_health(&self, app_state: &AppState) -> Result<WebhookHealth, String> {
        let service = self.get_or_init(app_state).await?;
        let last_retry_at = self.last_retry_at.read().await.clone();
        Ok(WebhookHealth {
            worker_running: self.worker_running.load(Ordering::SeqCst),
            failed_queue_length: service.failed_queue_length(),
            last_retry_at,
            persistence_failures: Some(service.persistence_failure_count()),
            retry_cycle_count: Some(self.retry_cycle_count.load(Ordering::Relaxed)),
            last_retry_error: self.last_retry_error.read().await.clone(),
        })
    }
}

impl Default for WebhookState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Request to create a new webhook channel.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub channel_type: WebhookChannelType,
    pub url: String,
    pub secret: Option<String>,
    pub scope: WebhookScope,
    pub events: Vec<WebhookEventType>,
    pub template: Option<String>,
}

/// Request to update an existing webhook channel.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub scope: Option<WebhookScope>,
    pub events: Option<Vec<WebhookEventType>>,
    #[serde(default)]
    pub template: TemplatePatch,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub enum TemplatePatch {
    #[default]
    Unchanged,
    Clear,
    Set(String),
}

impl<'de> Deserialize<'de> for TemplatePatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        Ok(match value {
            Some(value) => TemplatePatch::Set(value),
            None => TemplatePatch::Clear,
        })
    }
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// List all configured webhook channels.
#[tauri::command]
pub async fn list_webhook_channels(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<WebhookChannelConfig>>, String> {
    let result = state.with_database(|db| db.list_webhook_channels()).await;
    match result {
        Ok(channels) => Ok(CommandResponse::ok(channels)),
        Err(e) => Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    }
}

/// Create a new webhook channel.
#[tauri::command]
pub async fn create_webhook_channel(
    request: CreateWebhookRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<WebhookChannelConfig>, String> {
    if let Err(err) = validate_new_channel(&request) {
        return Ok(CommandResponse::err(err));
    }
    let CreateWebhookRequest {
        name,
        channel_type,
        url,
        secret,
        scope,
        events,
        template,
    } = request;
    let normalized_secret = secret.and_then(normalize_optional_text);
    if let Err(error) =
        validate_channel_secret_on_create(&channel_type, normalized_secret.as_deref())
    {
        return Ok(CommandResponse::err(error));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let config = WebhookChannelConfig {
        id: id.clone(),
        name: name.trim().to_string(),
        channel_type,
        enabled: true,
        url: url.trim().to_string(),
        secret: None, // Never stored in DB
        scope,
        events,
        template: template.and_then(normalize_optional_text),
        created_at: now.clone(),
        updated_at: now,
    };

    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
    if let Some(secret) = normalized_secret.as_deref() {
        if let Err(e) = keyring.set_api_key(&keyring_key, secret) {
            tracing::warn!(error = %e, "failed to persist webhook secret");
            return Ok(CommandResponse::err(
                "Failed to save webhook secret".to_string(),
            ));
        }
    }

    let result = state
        .with_database(|db| db.insert_webhook_channel(&config))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(config)),
        Err(e) => {
            if normalized_secret.is_some() {
                if let Err(cleanup_err) = keyring.delete_api_key(&keyring_key) {
                    tracing::warn!(
                        channel_id = %id,
                        error = %sanitize_error_message(cleanup_err.to_string()),
                        "failed to rollback webhook secret after create db failure"
                    );
                }
            }
            Ok(CommandResponse::err(sanitize_error_message(e.to_string())))
        }
    }
}

/// Update an existing webhook channel.
#[tauri::command]
pub async fn update_webhook_channel(
    id: String,
    request: UpdateWebhookRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<WebhookChannelConfig>, String> {
    let existing = state.with_database(|db| db.get_webhook_channel(&id)).await;

    let existing = match existing {
        Ok(Some(config)) => config,
        Ok(None) => return Ok(CommandResponse::err(format!("Channel not found: {}", id))),
        Err(e) => return Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    };
    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
    let old_secret = match keyring.get_api_key(&keyring_key) {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(channel_id = %id, error = %sanitize_error_message(e.to_string()), "failed to load existing webhook secret");
            None
        }
    };
    if let Err(error) = validate_channel_secret_on_update(
        &existing.channel_type,
        old_secret.as_deref(),
        request.secret.as_deref(),
    ) {
        return Ok(CommandResponse::err(error));
    }

    let now = chrono::Utc::now().to_rfc3339();

    let updated = WebhookChannelConfig {
        id: existing.id.clone(),
        name: request
            .name
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or(existing.name),
        channel_type: existing.channel_type,
        enabled: request.enabled.unwrap_or(existing.enabled),
        url: request
            .url
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or(existing.url),
        secret: None,
        scope: request.scope.clone().unwrap_or(existing.scope),
        events: request.events.clone().unwrap_or(existing.events),
        template: match request.template.clone() {
            TemplatePatch::Unchanged => existing.template,
            TemplatePatch::Clear => None,
            TemplatePatch::Set(value) => normalize_optional_text(value),
        },
        created_at: existing.created_at,
        updated_at: now,
    };

    if let Err(err) = validate_channel_config(
        &updated.name,
        &updated.channel_type,
        &updated.url,
        &updated.events,
        &updated.scope,
        updated.template.as_deref(),
    ) {
        return Ok(CommandResponse::err(err));
    }

    let mut secret_mutated = false;
    if let Some(secret) = request.secret.as_deref() {
        secret_mutated = true;
        if secret.trim().is_empty() {
            if let Err(e) = keyring.delete_api_key(&keyring_key) {
                tracing::warn!(channel_id = %id, error = %sanitize_error_message(e.to_string()), "failed to clear webhook secret");
                return Ok(CommandResponse::err(
                    "Failed to update webhook secret".to_string(),
                ));
            }
        } else if let Err(e) = keyring.set_api_key(&keyring_key, secret.trim()) {
            tracing::warn!(error = %e, "failed to update webhook secret");
            return Ok(CommandResponse::err(
                "Failed to update webhook secret".to_string(),
            ));
        }
    }

    let result = state
        .with_database(|db| db.update_webhook_channel(&updated))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(updated)),
        Err(e) => {
            if secret_mutated {
                if let Err(rollback_err) =
                    restore_webhook_secret(&keyring, &keyring_key, old_secret.as_deref())
                {
                    tracing::warn!(
                        channel_id = %id,
                        error = %sanitize_error_message(rollback_err),
                        "failed to rollback webhook secret after update db failure"
                    );
                }
            }
            Ok(CommandResponse::err(sanitize_error_message(e.to_string())))
        }
    }
}

/// Delete a webhook channel.
#[tauri::command]
pub async fn delete_webhook_channel(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    let result = state
        .with_database(|db| db.delete_webhook_channel(&id))
        .await;
    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);

    match result {
        Ok(()) => {
            if let Err(error) = keyring.delete_api_key(&keyring_key) {
                tracing::warn!(
                    channel_id = %id,
                    error = %sanitize_error_message(error.to_string()),
                    "failed to cleanup webhook secret after channel deletion"
                );
            }
            Ok(CommandResponse::ok(()))
        }
        Err(e) => Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    }
}

/// Test a webhook channel by sending a test notification.
#[tauri::command]
pub async fn test_webhook_channel(
    id: String,
    state: State<'_, AppState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<WebhookTestResult>, String> {
    let config = state.with_database(|db| db.get_webhook_channel(&id)).await;

    let mut config = match config {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(CommandResponse::err(format!("Channel not found: {}", id))),
        Err(e) => return Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    };

    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
    if let Ok(Some(secret)) = keyring.get_api_key(&keyring_key) {
        config.secret = Some(secret);
    }

    let service = match webhook_state.get_or_init(state.inner()).await {
        Ok(svc) => svc,
        Err(err) => return Ok(CommandResponse::err(err)),
    };
    let _ = webhook_state.start_worker_if_needed(state.inner()).await;

    match service.test_channel(&config).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    }
}

/// Get delivery history with optional channel filter and pagination.
#[tauri::command]
pub async fn get_webhook_deliveries(
    channel_id: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<WebhookDelivery>>, String> {
    let limit = limit
        .unwrap_or(DELIVERY_LIMIT_DEFAULT)
        .clamp(1, DELIVERY_LIMIT_MAX);
    let offset = offset.unwrap_or(0);

    let result = state
        .with_database(|db| db.list_webhook_deliveries(channel_id.as_deref(), limit, offset))
        .await;

    match result {
        Ok(deliveries) => Ok(CommandResponse::ok(deliveries)),
        Err(e) => Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    }
}

/// Retry a failed delivery by re-sending the original payload.
#[tauri::command]
pub async fn retry_webhook_delivery(
    delivery_id: String,
    state: State<'_, AppState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<WebhookDelivery>, String> {
    let service = match webhook_state.get_or_init(state.inner()).await {
        Ok(svc) => svc,
        Err(err) => return Ok(CommandResponse::err(err)),
    };
    let _ = webhook_state.start_worker_if_needed(state.inner()).await;

    match service.retry_delivery_by_id(&delivery_id).await {
        Ok(delivery) => Ok(CommandResponse::ok(delivery)),
        Err(e) => Ok(CommandResponse::err(sanitize_error_message(e.to_string()))),
    }
}

/// Query webhook retry worker health.
#[tauri::command]
pub async fn get_webhook_health(
    state: State<'_, AppState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<WebhookHealth>, String> {
    let _ = webhook_state.start_worker_if_needed(state.inner()).await;
    match webhook_state.get_health(state.inner()).await {
        Ok(health) => Ok(CommandResponse::ok(health)),
        Err(err) => Ok(CommandResponse::err(err)),
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_new_channel(request: &CreateWebhookRequest) -> Result<(), String> {
    validate_channel_config(
        request.name.trim(),
        &request.channel_type,
        request.url.trim(),
        &request.events,
        &request.scope,
        request.template.as_deref(),
    )
}

fn validate_channel_config(
    name: &str,
    channel_type: &WebhookChannelType,
    url: &str,
    events: &[WebhookEventType],
    scope: &WebhookScope,
    template: Option<&str>,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("Channel name is required".to_string());
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(format!(
            "Channel name must be <= {} characters",
            MAX_NAME_LEN
        ));
    }

    if url.is_empty() {
        return Err("Channel target is required".to_string());
    }
    if url.chars().count() > MAX_URL_LEN {
        return Err(format!(
            "Channel target must be <= {} characters",
            MAX_URL_LEN
        ));
    }

    if events.is_empty() {
        return Err("At least one event must be selected".to_string());
    }

    match scope {
        WebhookScope::Global => {}
        WebhookScope::Sessions(ids) => {
            if ids.is_empty() {
                return Err("Session scope requires at least one session id".to_string());
            }
            if ids.len() > MAX_SCOPE_SESSIONS {
                return Err(format!(
                    "Session scope supports up to {} sessions",
                    MAX_SCOPE_SESSIONS
                ));
            }
            if ids
                .iter()
                .any(|id| id.trim().is_empty() || id.chars().count() > MAX_SESSION_ID_LEN)
            {
                return Err(format!(
                    "Each session id must be 1-{} characters",
                    MAX_SESSION_ID_LEN
                ));
            }
        }
    }

    if let Some(template) = template {
        if template.chars().count() > MAX_TEMPLATE_LEN {
            return Err(format!(
                "Template must be <= {} characters",
                MAX_TEMPLATE_LEN
            ));
        }
    }

    validate_channel_target(channel_type, url)
}

fn validate_channel_target(channel_type: &WebhookChannelType, target: &str) -> Result<(), String> {
    match channel_type {
        WebhookChannelType::Slack
        | WebhookChannelType::Feishu
        | WebhookChannelType::Discord
        | WebhookChannelType::ServerChan => {
            let parsed = url::Url::parse(target).map_err(|_| "Invalid webhook URL".to_string())?;
            if parsed.scheme() != "https" {
                return Err("This channel requires an HTTPS webhook URL".to_string());
            }
            if parsed.host_str().is_none() {
                return Err("Webhook URL host is required".to_string());
            }
            Ok(())
        }
        WebhookChannelType::Telegram => {
            if is_valid_telegram_chat_id(target) {
                Ok(())
            } else {
                Err("Telegram chat id must be numeric or @channel_name".to_string())
            }
        }
        WebhookChannelType::Custom => {
            let parsed = url::Url::parse(target).map_err(|_| "Invalid webhook URL".to_string())?;
            match parsed.scheme() {
                "https" => Ok(()),
                "http" => {
                    let host = parsed
                        .host_str()
                        .ok_or_else(|| "Webhook URL host is required".to_string())?;
                    if crate::services::tools::url_validation::is_private_host(host) {
                        Ok(())
                    } else {
                        Err(
                            "HTTP custom webhook is only allowed for localhost or private hosts"
                                .to_string(),
                        )
                    }
                }
                _ => Err("Custom webhook URL must use http or https".to_string()),
            }
        }
    }
}

fn is_valid_telegram_chat_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('@') {
        let name = trimmed.trim_start_matches('@');
        if name.is_empty() {
            return false;
        }
        return name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    }
    trimmed.parse::<i64>().is_ok()
}

fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn required_secret_error(channel_type: &WebhookChannelType) -> Option<&'static str> {
    match channel_type {
        WebhookChannelType::Telegram => Some("Telegram channel requires a bot token secret"),
        WebhookChannelType::ServerChan => Some("ServerChan channel requires SENDKEY secret"),
        _ => None,
    }
}

fn validate_channel_secret_on_create(
    channel_type: &WebhookChannelType,
    secret: Option<&str>,
) -> Result<(), String> {
    if let Some(error) = required_secret_error(channel_type) {
        if secret
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        {
            Ok(())
        } else {
            Err(error.to_string())
        }
    } else {
        Ok(())
    }
}

fn validate_channel_secret_on_update(
    channel_type: &WebhookChannelType,
    existing_secret: Option<&str>,
    request_secret: Option<&str>,
) -> Result<(), String> {
    let Some(error_message) = required_secret_error(channel_type) else {
        return Ok(());
    };

    if let Some(secret) = request_secret {
        if secret.trim().is_empty() {
            return Err(error_message.to_string());
        }
        return Ok(());
    }
    if existing_secret
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        Ok(())
    } else {
        Err(error_message.to_string())
    }
}

fn restore_webhook_secret(
    keyring: &KeyringService,
    keyring_key: &str,
    secret: Option<&str>,
) -> Result<(), String> {
    match secret {
        Some(secret) => keyring
            .set_api_key(keyring_key, secret)
            .map_err(|e| e.to_string()),
        None => keyring
            .delete_api_key(keyring_key)
            .map_err(|e| e.to_string()),
    }
}

fn cleanup_orphan_webhook_secrets(db: &Database, keyring: &KeyringService) -> Result<(), String> {
    let channels = db.list_webhook_channels().map_err(|e| e.to_string())?;
    let channel_ids = channels
        .into_iter()
        .map(|channel| channel.id)
        .collect::<HashSet<_>>();
    let keys = keyring
        .list_keys_with_prefix(WEBHOOK_KEYRING_PREFIX)
        .map_err(|e| e.to_string())?;

    let mut removed = 0usize;
    for key in keys {
        let Some(channel_id) = key.strip_prefix(WEBHOOK_KEYRING_PREFIX) else {
            continue;
        };
        if channel_ids.contains(channel_id) {
            continue;
        }
        match keyring.delete_api_key(&key) {
            Ok(()) => removed += 1,
            Err(error) => {
                tracing::warn!(
                    key = %key,
                    error = %sanitize_error_message(error.to_string()),
                    "failed to delete orphan webhook secret"
                );
            }
        }
    }

    if removed > 0 {
        tracing::info!(removed, "cleaned orphan webhook secrets");
    }
    Ok(())
}

fn sanitize_error_message(message: impl Into<String>) -> String {
    sanitize_for_user(&message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_slack_requires_https() {
        let err = validate_channel_target(&WebhookChannelType::Slack, "http://hooks.slack.com/x")
            .unwrap_err();
        assert!(err.contains("HTTPS"));
    }

    #[test]
    fn test_validate_telegram_chat_id() {
        assert!(is_valid_telegram_chat_id("-100123456789"));
        assert!(is_valid_telegram_chat_id("@plancascade"));
        assert!(!is_valid_telegram_chat_id("@"));
        assert!(!is_valid_telegram_chat_id("https://t.me/abc"));
    }

    #[test]
    fn test_validate_custom_http_private_only() {
        assert!(
            validate_channel_target(&WebhookChannelType::Custom, "http://127.0.0.1:8080/hook")
                .is_ok()
        );
        assert!(
            validate_channel_target(&WebhookChannelType::Custom, "http://example.com/hook")
                .is_err()
        );
        assert!(
            validate_channel_target(&WebhookChannelType::Custom, "https://example.com/hook")
                .is_ok()
        );
    }

    #[test]
    fn test_validate_discord_feishu_and_serverchan_require_https() {
        assert!(validate_channel_target(
            &WebhookChannelType::Discord,
            "http://discord.com/api/webhooks/x"
        )
        .is_err());
        assert!(validate_channel_target(
            &WebhookChannelType::Feishu,
            "http://open.feishu.cn/hook/x"
        )
        .is_err());
        assert!(validate_channel_target(
            &WebhookChannelType::Discord,
            "https://discord.com/api/webhooks/x"
        )
        .is_ok());
        assert!(validate_channel_target(
            &WebhookChannelType::ServerChan,
            "http://sctapi.ftqq.com"
        )
        .is_err());
        assert!(
            validate_channel_target(&WebhookChannelType::ServerChan, "https://sctapi.ftqq.com")
                .is_ok()
        );
    }

    #[test]
    fn test_validate_custom_protocol_rejected() {
        let err = validate_channel_target(&WebhookChannelType::Custom, "ftp://example.com/hook")
            .unwrap_err();
        assert!(err.contains("http or https"));
    }

    #[test]
    fn test_validate_scope_sessions_limits() {
        let ids = (0..101).map(|i| format!("s{}", i)).collect::<Vec<_>>();
        let result = validate_channel_config(
            "name",
            &WebhookChannelType::Custom,
            "https://example.com/hook",
            &[WebhookEventType::TaskComplete],
            &WebhookScope::Sessions(ids),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_channel_config_rejects_empty_events() {
        let result = validate_channel_config(
            "name",
            &WebhookChannelType::Slack,
            "https://hooks.slack.com/services/x",
            &[],
            &WebhookScope::Global,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_error_message_hides_sensitive_content() {
        let msg = sanitize_error_message("invalid token: abc123");
        assert!(msg.contains("[REDACTED]"));
    }

    #[test]
    fn test_update_template_patch_deserialization() {
        let missing: UpdateWebhookRequest = serde_json::from_str("{}").unwrap();
        assert!(matches!(missing.template, TemplatePatch::Unchanged));

        let clear: UpdateWebhookRequest = serde_json::from_str(r#"{"template":null}"#).unwrap();
        assert!(matches!(clear.template, TemplatePatch::Clear));

        let set: UpdateWebhookRequest =
            serde_json::from_str(r#"{"template":"hello world"}"#).unwrap();
        assert!(matches!(set.template, TemplatePatch::Set(value) if value == "hello world"));
    }

    #[test]
    fn test_validate_channel_secret_requirements() {
        assert!(validate_channel_secret_on_create(&WebhookChannelType::Telegram, None).is_err());
        assert!(
            validate_channel_secret_on_create(&WebhookChannelType::Telegram, Some("token"))
                .is_ok()
        );
        assert!(validate_channel_secret_on_create(&WebhookChannelType::ServerChan, None).is_err());
        assert!(
            validate_channel_secret_on_create(&WebhookChannelType::ServerChan, Some("sctp123tX"))
                .is_ok()
        );

        assert!(
            validate_channel_secret_on_update(&WebhookChannelType::Telegram, None, None).is_err()
        );
        assert!(validate_channel_secret_on_update(
            &WebhookChannelType::Telegram,
            Some("existing"),
            None
        )
        .is_ok());
        assert!(validate_channel_secret_on_update(
            &WebhookChannelType::Telegram,
            Some("existing"),
            Some("   ")
        )
        .is_err());
        assert!(
            validate_channel_secret_on_update(&WebhookChannelType::ServerChan, None, None).is_err()
        );
        assert!(validate_channel_secret_on_update(
            &WebhookChannelType::ServerChan,
            Some("sctp123tX"),
            None
        )
        .is_ok());
    }
}
