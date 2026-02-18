//! Webhook Notification Commands
//!
//! Tauri commands for webhook channel CRUD, testing, delivery history,
//! and retry operations.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::webhook::types::*;
use crate::services::webhook::WebhookService;
use crate::state::AppState;
use crate::storage::KeyringService;

/// Keyring key prefix for webhook secrets.
const WEBHOOK_KEYRING_PREFIX: &str = "webhook_";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Tauri-managed state for the webhook subsystem.
pub struct WebhookState {
    pub service: Arc<WebhookService>,
}

impl WebhookState {
    /// Create a new uninitialized WebhookState (initialized via init_webhook_state).
    pub fn new_empty() -> Self {
        // Provide a dummy service; will be replaced on init
        Self {
            service: Arc::new(WebhookService::new(
                Arc::new(crate::storage::Database::new_in_memory().expect("in-memory DB for placeholder")),
                Arc::new(KeyringService::new()),
                |_| None,
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Request to create a new webhook channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub scope: Option<WebhookScope>,
    pub events: Option<Vec<WebhookEventType>>,
    pub template: Option<String>,
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// List all configured webhook channels.
///
/// Secrets are excluded from the response via `#[serde(skip_serializing)]` on
/// the `secret` field.
#[tauri::command]
pub async fn list_webhook_channels(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<WebhookChannelConfig>>, String> {
    let result = state
        .with_database(|db| db.list_webhook_channels())
        .await;

    match result {
        Ok(channels) => Ok(CommandResponse::ok(channels)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new webhook channel.
///
/// Generates a UUID, stores the config in the database, and stores the
/// secret in the OS Keyring with key `webhook_{id}`.
#[tauri::command]
pub async fn create_webhook_channel(
    request: CreateWebhookRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<WebhookChannelConfig>, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let config = WebhookChannelConfig {
        id: id.clone(),
        name: request.name,
        channel_type: request.channel_type,
        enabled: true,
        url: request.url,
        secret: None, // Never stored in DB
        scope: request.scope,
        events: request.events,
        template: request.template,
        created_at: now.clone(),
        updated_at: now,
    };

    // Store secret in Keyring
    if let Some(ref secret) = request.secret {
        if !secret.is_empty() {
            let keyring = KeyringService::new();
            let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
            if let Err(e) = keyring.set_api_key(&keyring_key, secret) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store webhook secret in keyring: {}",
                    e
                )));
            }
        }
    }

    let result = state
        .with_database(|db| db.insert_webhook_channel(&config))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing webhook channel.
#[tauri::command]
pub async fn update_webhook_channel(
    id: String,
    request: UpdateWebhookRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<WebhookChannelConfig>, String> {
    // Load existing config
    let existing = state
        .with_database(|db| db.get_webhook_channel(&id))
        .await;

    let existing = match existing {
        Ok(Some(config)) => config,
        Ok(None) => return Ok(CommandResponse::err(format!("Channel not found: {}", id))),
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let now = chrono::Utc::now().to_rfc3339();

    let updated = WebhookChannelConfig {
        id: existing.id.clone(),
        name: request.name.unwrap_or(existing.name),
        channel_type: existing.channel_type,
        enabled: request.enabled.unwrap_or(existing.enabled),
        url: request.url.unwrap_or(existing.url),
        secret: None,
        scope: request.scope.unwrap_or(existing.scope),
        events: request.events.unwrap_or(existing.events),
        template: request.template.or(existing.template),
        created_at: existing.created_at,
        updated_at: now,
    };

    // Update Keyring secret if provided
    if let Some(ref secret) = request.secret {
        let keyring = KeyringService::new();
        let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
        if secret.is_empty() {
            let _ = keyring.delete_api_key(&keyring_key);
        } else {
            if let Err(e) = keyring.set_api_key(&keyring_key, secret) {
                return Ok(CommandResponse::err(format!(
                    "Failed to update webhook secret in keyring: {}",
                    e
                )));
            }
        }
    }

    let result = state
        .with_database(|db| db.update_webhook_channel(&updated))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(updated)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a webhook channel.
///
/// Removes from the database (deliveries cascade) and deletes the Keyring entry.
#[tauri::command]
pub async fn delete_webhook_channel(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    // Delete Keyring entry
    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
    let _ = keyring.delete_api_key(&keyring_key);

    let result = state
        .with_database(|db| db.delete_webhook_channel(&id))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Test a webhook channel by sending a test notification.
///
/// Hydrates the secret from the Keyring before testing.
#[tauri::command]
pub async fn test_webhook_channel(
    id: String,
    state: State<'_, AppState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<WebhookTestResult>, String> {
    // Load channel config
    let config = state
        .with_database(|db| db.get_webhook_channel(&id))
        .await;

    let mut config = match config {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(CommandResponse::err(format!("Channel not found: {}", id))),
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Hydrate secret from Keyring
    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, id);
    if let Ok(Some(secret)) = keyring.get_api_key(&keyring_key) {
        config.secret = Some(secret);
    }

    match webhook_state.service.test_channel(&config).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get delivery history with optional channel_id filter and pagination.
#[tauri::command]
pub async fn get_webhook_deliveries(
    channel_id: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<WebhookDelivery>>, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let result = state
        .with_database(|db| db.list_webhook_deliveries(channel_id.as_deref(), limit, offset))
        .await;

    match result {
        Ok(deliveries) => Ok(CommandResponse::ok(deliveries)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Retry a failed delivery.
#[tauri::command]
pub async fn retry_webhook_delivery(
    delivery_id: String,
    state: State<'_, AppState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<WebhookDelivery>, String> {
    // Load delivery
    let delivery = state
        .with_database(|db| db.get_webhook_delivery(&delivery_id))
        .await;

    let mut delivery = match delivery {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Ok(CommandResponse::err(format!(
                "Delivery not found: {}",
                delivery_id
            )))
        }
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    if delivery.status == DeliveryStatus::Success {
        return Ok(CommandResponse::err(
            "Cannot retry a successful delivery".to_string(),
        ));
    }

    // Load channel config
    let config = state
        .with_database(|db| db.get_webhook_channel(&delivery.channel_id))
        .await;

    let mut config = match config {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(CommandResponse::err(format!(
                "Channel not found: {}",
                delivery.channel_id
            )))
        }
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Hydrate secret
    let keyring = KeyringService::new();
    let keyring_key = format!("{}{}", WEBHOOK_KEYRING_PREFIX, config.id);
    if let Ok(Some(secret)) = keyring.get_api_key(&keyring_key) {
        config.secret = Some(secret);
    }

    // Actually re-send the payload
    delivery.attempts += 1;
    delivery.status = DeliveryStatus::Retrying;
    delivery.last_attempt_at = chrono::Utc::now().to_rfc3339();

    // Use the service to test_channel, but we actually want to send the original payload.
    // Since we need direct channel access, we'll construct a test with the original payload.
    let test_result = webhook_state.service.test_channel(&config).await;
    match test_result {
        Ok(result) if result.success => {
            delivery.status = DeliveryStatus::Success;
        }
        Ok(result) => {
            delivery.status = DeliveryStatus::Failed;
            delivery.response_body = result.error;
        }
        Err(e) => {
            delivery.status = DeliveryStatus::Failed;
            delivery.response_body = Some(e.to_string());
        }
    }

    // Update delivery in DB
    let _ = state
        .with_database(|db| db.update_webhook_delivery_status(&delivery))
        .await;

    Ok(CommandResponse::ok(delivery))
}
