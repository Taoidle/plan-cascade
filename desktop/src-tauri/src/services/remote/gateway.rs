//! Remote Gateway Service
//!
//! Manages the adapter lifecycle, processes incoming messages via CommandRouter,
//! and coordinates with SessionBridge. Implements the 5-layer security model
//! and audit logging. Dispatches webhook notifications when remote commands
//! trigger task completions.

use super::adapters::{RemoteAdapter, RemoteAdapterFactory};
use super::command_router::{CommandRouter, HELP_TEXT};
use super::response_mapper::ResponseMapper;
use super::session_bridge::SessionBridge;
use super::types::{
    GatewayStatus, IncomingRemoteEvent, RemoteActionButton, RemoteActionCard, ReconnectConfig,
    RemoteCommand, RemoteError, RemoteGatewayConfig, RemoteIncomingEventType,
    RemoteUiMessage, RemoteWorkflowSession, RemoteWorkspaceEntry, TelegramAdapterConfig,
};
use super::workflow_facade::{RemoteWorkflowExecution, RemoteWorkflowFacade};
use crate::commands::standalone::normalize_provider_name;
use crate::services::orchestrator::permissions::PermissionLevel;
use crate::services::proxy::ProxyConfig;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::task_mode::context_provider::ContextSourceConfig;
use crate::services::webhook::integration::{dispatch_on_remote_event, format_remote_source};
use crate::services::webhook::types::WebhookEventType;
use crate::services::webhook::WebhookService;
use crate::services::workflow_kernel::{HandoffContextBundle, UserInputIntent, UserInputIntentType, WorkflowKernelState, WorkflowMode};
use crate::storage::{ConfigService, Database};
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tauri::AppHandle;

/// Remote Gateway Service managing adapter lifecycle and message processing.
#[derive(Clone)]
pub struct RemoteGatewayService {
    pub(crate) config: Arc<RwLock<RemoteGatewayConfig>>,
    pub(crate) telegram_config: Arc<RwLock<TelegramAdapterConfig>>,
    pub(crate) adapter: Arc<RwLock<Option<Arc<dyn RemoteAdapter>>>>,
    pub(crate) session_bridge: Arc<SessionBridge>,
    pub(crate) webhook_service: Option<Arc<WebhookService>>,
    pub(crate) status: Arc<RwLock<GatewayStatus>>,
    pub(crate) processor_cancel: Arc<RwLock<Option<CancellationToken>>>,
    pub(crate) processor_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    pub(crate) adapter_watch_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    pub(crate) proxy: Arc<RwLock<Option<ProxyConfig>>>,
    pub(crate) manual_stop: Arc<AtomicBool>,
    pub(crate) db: Arc<Database>,
    pub(crate) workflow_kernel: Option<WorkflowKernelState>,
    pub(crate) workflow_facade: Option<RemoteWorkflowFacade>,
    pub(crate) workflow_sessions: Arc<RwLock<HashMap<i64, RemoteWorkflowSession>>>,
    /// Chats that have authenticated with password (Layer 4)
    pub(crate) authenticated_chats: Arc<RwLock<HashSet<i64>>>,
    /// Configuration for reconnect behavior with exponential backoff.
    pub(crate) reconnect_config: ReconnectConfig,
}

impl RemoteGatewayService {
    /// Create a new RemoteGatewayService.
    pub fn new(
        config: RemoteGatewayConfig,
        telegram_config: Option<TelegramAdapterConfig>,
        session_bridge: Arc<SessionBridge>,
        db: Arc<Database>,
        workflow_kernel: Option<WorkflowKernelState>,
        app_handle: Option<AppHandle>,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            telegram_config: Arc::new(RwLock::new(telegram_config.unwrap_or_default())),
            adapter: Arc::new(RwLock::new(None)),
            session_bridge,
            webhook_service: None,
            status: Arc::new(RwLock::new(GatewayStatus::default())),
            processor_cancel: Arc::new(RwLock::new(None)),
            processor_handle: Arc::new(RwLock::new(None)),
            adapter_watch_handle: Arc::new(RwLock::new(None)),
            proxy: Arc::new(RwLock::new(None)),
            manual_stop: Arc::new(AtomicBool::new(false)),
            db,
            workflow_kernel,
            workflow_facade: app_handle.map(RemoteWorkflowFacade::new),
            workflow_sessions: Arc::new(RwLock::new(HashMap::new())),
            authenticated_chats: Arc::new(RwLock::new(HashSet::new())),
            reconnect_config: ReconnectConfig::default(),
        }
    }

    /// Set a custom reconnect configuration.
    pub fn set_reconnect_config(&mut self, config: ReconnectConfig) {
        self.reconnect_config = config;
    }

    /// Set the webhook service for dispatching notifications on remote command completion.
    pub fn set_webhook_service(&mut self, webhook_service: Arc<WebhookService>) {
        self.webhook_service = Some(webhook_service);
    }

    /// Get current gateway status.
    pub async fn get_status(&self) -> GatewayStatus {
        let mut status = self.status.read().await.clone();
        status.active_remote_sessions = self.session_bridge.active_session_count().await;
        status
    }

    /// Start the remote gateway with adapter and message processing loop.
    pub async fn start(&self, proxy: Option<&ProxyConfig>) -> Result<(), RemoteError> {
        if self.status.read().await.running {
            return Ok(());
        }
        self.manual_stop.store(false, Ordering::SeqCst);

        let config = self.config.read().await.clone();
        if !config.enabled {
            return Err(RemoteError::NotEnabled);
        }

        let allowed_paths = Self::normalize_allowed_paths(&config.allowed_project_roots)?;
        self.session_bridge
            .update_allowed_paths(allowed_paths)
            .await?;

        {
            let mut proxy_guard = self.proxy.write().await;
            *proxy_guard = proxy.cloned();
        }

        let adapter =
            RemoteAdapterFactory::create(&config.adapter, self.telegram_config.clone(), proxy)
                .await?;
        adapter.health_check().await?;

        // Create message channel
        let (tx, mut rx) = mpsc::channel::<IncomingRemoteEvent>(100);

        // Start adapter
        let adapter_handle = adapter.start(tx).await?;

        // Store adapter
        {
            let mut adapter_guard = self.adapter.write().await;
            *adapter_guard = Some(adapter);
        }

        // Spawn message processing loop
        let bridge = self.session_bridge.clone();
        let adapter_ref = self.adapter.clone();
        let gateway_config = self.config.clone();
        let telegram_config = self.telegram_config.clone();
        let workflow_kernel = self.workflow_kernel.clone();
        let workflow_facade = self.workflow_facade.clone();
        let status_ref = self.status.clone();
        let db_ref = self.db.clone();
        let cancel = CancellationToken::new();
        let cancel_for_loop = cancel.clone();
        let authenticated_chats = self.authenticated_chats.clone();
        let workflow_sessions = self.workflow_sessions.clone();
        let webhook_service = self.webhook_service.clone();

        let processor_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        let bridge = bridge.clone();
                        let adapter_ref = adapter_ref.clone();
                        let gateway_config = gateway_config.clone();
                        let telegram_config = telegram_config.clone();
                        let workflow_kernel = workflow_kernel.clone();
                        let workflow_facade = workflow_facade.clone();
                        let status_ref = status_ref.clone();
                        let db_ref = db_ref.clone();
                        let workflow_sessions = workflow_sessions.clone();
                        let authenticated_chats = authenticated_chats.clone();
                        let webhook_service = webhook_service.clone();
                        tokio::spawn(async move {
                            Self::handle_message(
                                &msg,
                                &bridge,
                                &adapter_ref,
                                &gateway_config,
                                &telegram_config,
                                &workflow_kernel,
                                workflow_facade.as_ref(),
                                &status_ref,
                                &db_ref,
                                &workflow_sessions,
                                &authenticated_chats,
                                webhook_service.as_ref(),
                            ).await;
                        });
                    }
                    _ = cancel_for_loop.cancelled() => {
                        break;
                    }
                }
            }
        });
        {
            let mut cancel_guard = self.processor_cancel.write().await;
            *cancel_guard = Some(cancel.clone());
        }
        {
            let mut handle_guard = self.processor_handle.write().await;
            *handle_guard = Some(processor_handle);
        }

        let gateway = self.clone();
        let watch_handle = tokio::spawn(async move {
            let result = match adapter_handle.await {
                Ok(result) => result,
                Err(error) => Err(RemoteError::SendFailed(format!(
                    "Adapter task panicked: {}",
                    error
                ))),
            };
            gateway.handle_adapter_exit(result).await;
        });
        {
            let mut watch_guard = self.adapter_watch_handle.write().await;
            *watch_guard = Some(watch_handle);
        }

        // Update status
        let mut status = self.status.write().await;
        status.running = true;
        status.adapter_type = config.adapter;
        status.connected_since = Some(chrono::Utc::now().to_rfc3339());
        status.error = None;
        status.reconnecting = false;

        Ok(())
    }

    /// Handle an incoming remote message.
    async fn handle_message(
        msg: &IncomingRemoteEvent,
        bridge: &SessionBridge,
        adapter: &RwLock<Option<Arc<dyn RemoteAdapter>>>,
        gateway_config: &RwLock<RemoteGatewayConfig>,
        telegram_config: &RwLock<TelegramAdapterConfig>,
        workflow_kernel: &Option<WorkflowKernelState>,
        workflow_facade: Option<&RemoteWorkflowFacade>,
        status: &RwLock<GatewayStatus>,
        db: &Database,
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        authenticated_chats: &RwLock<HashSet<i64>>,
        webhook_service: Option<&Arc<WebhookService>>,
    ) {
        // Update stats
        {
            let mut s = status.write().await;
            s.total_commands_processed += 1;
            s.last_command_at = Some(chrono::Utc::now().to_rfc3339());
        }

        let adapter_guard = adapter.read().await;
        let adapter = match adapter_guard.as_ref() {
            Some(a) => a,
            None => return,
        };
        let current_gateway_config = gateway_config.read().await.clone();
        let current_config = telegram_config.read().await.clone();

        if let Some(callback_id) = msg.callback_id.as_deref() {
            let _ = adapter.answer_callback(callback_id).await;
        }

        if matches!(msg.event_type, RemoteIncomingEventType::TextMessage) {
            let _ = adapter.send_typing(msg.chat_id).await;
        }

        // Layer 4: Password gate check
        if current_config.require_password {
            let is_authenticated = authenticated_chats.read().await.contains(&msg.chat_id);
            if !is_authenticated {
                // Check if this message is an /auth command
                let text = msg.text.trim();
                if text.starts_with("/auth ") {
                    let provided_password = text[6..].trim();
                    if let Some(expected) = current_config.access_password.as_deref() {
                        if provided_password == expected {
                            authenticated_chats.write().await.insert(msg.chat_id);
                            let _ = adapter
                                .send_message(msg.chat_id, "Authenticated successfully.")
                                .await;
                            Self::write_audit_log(db, msg, "Auth", "success", None);
                            return;
                        }
                    }
                    let _ = adapter
                        .send_message(msg.chat_id, "Authentication failed. Invalid password.")
                        .await;
                    Self::write_audit_log(
                        db,
                        msg,
                        "Auth",
                        "unauthorized",
                        Some("Invalid password"),
                    );
                    return;
                } else {
                    let _ = adapter
                        .send_message(
                            msg.chat_id,
                            "Authentication required. Send /auth <password> to authenticate.",
                        )
                        .await;
                    Self::write_audit_log(
                        db,
                        msg,
                        "Unauthenticated",
                        "unauthorized",
                        Some("Not authenticated"),
                    );
                    return;
                }
            }
        }

        // Parse command
        let command = match msg.event_type {
            RemoteIncomingEventType::CallbackAction => msg
                .callback_data
                .as_deref()
                .and_then(CommandRouter::parse_callback)
                .unwrap_or(RemoteCommand::Help),
            _ => CommandRouter::parse(&msg.text),
        };
        let command_type = Self::command_type_name(&command);

        // Build remote source identifier for webhook notifications
        let remote_source =
            format_remote_source(&msg.adapter_type.to_string(), msg.username.as_deref());

        // Track whether this is a task-producing command for webhook dispatch
        let mut should_dispatch_webhook = false;
        let mut webhook_event_type = WebhookEventType::TaskComplete;
        // Track whether the bridge already sent the response via adapter (streaming modes)
        let mut already_sent_by_bridge = false;

        // Process command through SessionBridge
        let response = match command {
            RemoteCommand::Start | RemoteCommand::Home | RemoteCommand::Menu => {
                RemoteUiMessage::ActionCard(
                    Self::build_home_card(
                        &current_gateway_config,
                        &current_config,
                        workflow_sessions,
                        msg.chat_id,
                        msg.user_id,
                        msg.username.as_deref(),
                    )
                    .await,
                )
            }
            RemoteCommand::WhoAmI => RemoteUiMessage::PlainText(Self::build_whoami_text(
                &current_gateway_config,
                &current_config,
                msg.chat_id,
                msg.user_id,
                msg.username.as_deref(),
            )),
            RemoteCommand::SwitchMode { mode } => {
                let session = {
                    let mut sessions = workflow_sessions.write().await;
                    sessions
                        .entry(msg.chat_id)
                        .or_insert_with(|| Self::new_workflow_session(msg, &current_config))
                        .clone()
                };
                let (updated_session, summary) = if let Some(facade) = workflow_facade {
                    let fallback_session = session.clone();
                    match facade.switch_mode(session, mode).await {
                        Ok(updated) => (updated, format!("Switched to {:?} mode.", mode)),
                        Err(error) => (
                            fallback_session,
                            format!("Failed to switch mode: {error}"),
                        ),
                    }
                } else {
                    let mut fallback = session;
                    if fallback.kernel_session_id.is_empty() {
                        if let Some(kernel) = workflow_kernel.as_ref() {
                            if let Ok(root_session_id) = Self::open_workflow_root_session(
                                kernel,
                                fallback.project_path.clone(),
                                fallback.workspace_label.clone(),
                            )
                            .await
                            {
                                fallback.kernel_session_id = root_session_id;
                            }
                        }
                    }
                    if let (Some(kernel), false) =
                        (workflow_kernel.as_ref(), fallback.kernel_session_id.is_empty())
                    {
                        let _ = kernel
                            .transition_mode(&fallback.kernel_session_id, mode, None)
                            .await;
                    }
                    fallback.active_mode = mode;
                    fallback.updated_at = chrono::Utc::now().to_rfc3339();
                    (fallback, format!("Switched to {:?} mode.", mode))
                };
                workflow_sessions
                    .write()
                    .await
                    .insert(msg.chat_id, updated_session);
                RemoteUiMessage::ActionCard(Self::render_status_card(
                    workflow_facade,
                    workflow_sessions,
                    msg.chat_id,
                    Some(summary),
                ).await)
            }
            RemoteCommand::PlanGenerate
            | RemoteCommand::PlanApprove
            | RemoteCommand::TaskConfirmConfig
            | RemoteCommand::TaskGeneratePrd
            | RemoteCommand::TaskApprovePrd
            | RemoteCommand::DebugApprovePatch
            | RemoteCommand::SetContextPreset { .. }
            | RemoteCommand::ToggleContextSource { .. }
            | RemoteCommand::SetPermissionLevel { .. }
            | RemoteCommand::RespondPermission { .. } => {
                if let Some(facade) = workflow_facade {
                    let session = {
                        let mut sessions = workflow_sessions.write().await;
                        sessions
                            .entry(msg.chat_id)
                            .or_insert_with(|| Self::new_workflow_session(msg, &current_config))
                            .clone()
                    };
                    match facade.handle_action(session, &command).await {
                        Ok((updated_session, ui)) => {
                            workflow_sessions
                                .write()
                                .await
                                .insert(msg.chat_id, updated_session);
                            ui
                        }
                        Err(error) => RemoteUiMessage::PlainText(format!("Error: {error}")),
                    }
                } else {
                    RemoteUiMessage::PlainText(
                        "Error: workflow actions are unavailable in this runtime.".to_string(),
                    )
                }
            }
            RemoteCommand::NewSession {
                project_path,
                provider,
                model,
            } => {
                let (resolved_project_path, resolved_provider, resolved_model, workspace_label) =
                    Self::resolve_workspace_selection(
                        &current_gateway_config,
                        &current_config,
                        project_path,
                        provider,
                        model,
                    );
                match bridge
                    .create_session_with_source(
                        msg.chat_id,
                        msg.user_id,
                        &resolved_project_path,
                        resolved_provider.as_deref(),
                        resolved_model.as_deref(),
                        Some(&msg.adapter_type.to_string()),
                        msg.username.as_deref(),
                    )
                    .await
                {
                    Ok(id) => {
                        let kernel_session_id = if let Some(kernel) = workflow_kernel.as_ref() {
                            Self::open_workflow_root_session(
                                kernel,
                                Some(resolved_project_path.clone()),
                                workspace_label.clone(),
                            )
                            .await
                            .ok()
                        } else {
                            None
                        };
                        {
                            let mut sessions = workflow_sessions.write().await;
                            let session = sessions
                                .entry(msg.chat_id)
                                .or_insert_with(|| Self::new_workflow_session(msg, &current_config));
                            session.kernel_session_id =
                                kernel_session_id.unwrap_or_else(|| id.clone());
                            session.project_path = Some(resolved_project_path.clone());
                            session.workspace_label = workspace_label;
                            session.provider = resolved_provider.clone();
                            session.model = resolved_model.clone();
                            session.linked_mode_sessions.insert("chat".to_string(), id.clone());
                            session.active_mode = WorkflowMode::Chat;
                            session.updated_at = chrono::Utc::now().to_rfc3339();
                        }
                        RemoteUiMessage::ActionCard(Self::render_status_card(
                            workflow_facade,
                            workflow_sessions,
                            msg.chat_id,
                            Some(ResponseMapper::format_session_created(&id, &resolved_project_path)),
                        ).await)
                    }
                    Err(e) => RemoteUiMessage::PlainText(ResponseMapper::format_error(&e)),
                }
            }
            RemoteCommand::SendMessage { content } => {
                let current_remote_session = {
                    let mut sessions = workflow_sessions.write().await;
                    sessions
                        .entry(msg.chat_id)
                        .or_insert_with(|| Self::new_workflow_session(msg, &current_config))
                        .clone()
                };
                if current_remote_session.active_mode != WorkflowMode::Chat {
                    if let Some(facade) = workflow_facade {
                        match facade.handle_text_input(current_remote_session, &content).await {
                            Ok(RemoteWorkflowExecution::Ui { session, message }) => {
                                workflow_sessions.write().await.insert(msg.chat_id, session);
                                message
                            }
                            Ok(RemoteWorkflowExecution::ChatFallback(session)) => {
                                workflow_sessions.write().await.insert(msg.chat_id, session);
                                Self::send_chat_message(
                                msg,
                                &content,
                                bridge,
                                adapter.as_ref(),
                                workflow_kernel,
                                workflow_sessions,
                                &current_gateway_config,
                                &current_config,
                                &mut should_dispatch_webhook,
                                &mut webhook_event_type,
                                &mut already_sent_by_bridge,
                            )
                                .await
                            }
                            Err(error) => RemoteUiMessage::PlainText(format!("Error: {error}")),
                        }
                    } else {
                        RemoteUiMessage::PlainText(
                            "Error: workflow facade is unavailable in this runtime.".to_string(),
                        )
                    }
                } else {
                    workflow_sessions
                        .write()
                        .await
                        .insert(msg.chat_id, current_remote_session);
                    Self::send_chat_message(
                        msg,
                        &content,
                        bridge,
                        adapter.as_ref(),
                        workflow_kernel,
                        workflow_sessions,
                        &current_gateway_config,
                        &current_config,
                        &mut should_dispatch_webhook,
                        &mut webhook_event_type,
                        &mut already_sent_by_bridge,
                    )
                    .await
                }
            }
            RemoteCommand::ListSessions => {
                RemoteUiMessage::PlainText(bridge.list_sessions_text(msg.chat_id).await)
            }
            RemoteCommand::SwitchSession { session_id } => {
                match bridge.switch_session(msg.chat_id, &session_id).await {
                    Ok(()) => {
                        if let Some(session) = workflow_sessions.write().await.get_mut(&msg.chat_id) {
                            session
                                .linked_mode_sessions
                                .insert("chat".to_string(), session_id.clone());
                            session.updated_at = chrono::Utc::now().to_rfc3339();
                        }
                        RemoteUiMessage::PlainText(format!("Switched to session: {}", session_id))
                    }
                    Err(e) => RemoteUiMessage::PlainText(ResponseMapper::format_error(&e)),
                }
            }
            RemoteCommand::Status => {
                RemoteUiMessage::ActionCard(
                    Self::render_status_card(workflow_facade, workflow_sessions, msg.chat_id, None)
                        .await,
                )
            }
            RemoteCommand::Cancel => match bridge.cancel_execution(msg.chat_id).await {
                Ok(()) => {
                    should_dispatch_webhook = true;
                    webhook_event_type = WebhookEventType::TaskCancelled;
                    RemoteUiMessage::PlainText("Execution cancelled.".to_string())
                }
                Err(e) => RemoteUiMessage::PlainText(ResponseMapper::format_error(&e)),
            },
            RemoteCommand::CloseSession => match bridge.close_session(msg.chat_id).await {
                Ok(()) => {
                    workflow_sessions.write().await.remove(&msg.chat_id);
                    RemoteUiMessage::PlainText("Session closed.".to_string())
                }
                Err(e) => RemoteUiMessage::PlainText(ResponseMapper::format_error(&e)),
            },
            RemoteCommand::Help => {
                RemoteUiMessage::ActionCard(Self::build_help_card(&current_config))
            }
            RemoteCommand::Context => {
                if let Some(facade) = workflow_facade {
                    let session = workflow_sessions
                        .read()
                        .await
                        .get(&msg.chat_id)
                        .cloned()
                        .unwrap_or_else(|| Self::new_workflow_session(msg, &current_config));
                    RemoteUiMessage::ActionCard(facade.render_context_card(&session, None).await)
                } else {
                    RemoteUiMessage::ActionCard(
                        Self::build_context_card(workflow_sessions, msg.chat_id).await,
                    )
                }
            }
            RemoteCommand::Permission => {
                if let Some(facade) = workflow_facade {
                    let session = workflow_sessions
                        .read()
                        .await
                        .get(&msg.chat_id)
                        .cloned()
                        .unwrap_or_else(|| Self::new_workflow_session(msg, &current_config));
                    RemoteUiMessage::ActionCard(facade.render_permission_card(&session, None).await)
                } else {
                    RemoteUiMessage::ActionCard(
                        Self::build_permission_card(workflow_sessions, msg.chat_id).await,
                    )
                }
            }
            RemoteCommand::Resume => {
                if let Some(kernel) = workflow_kernel.as_ref() {
                    let maybe_root = {
                        let sessions = workflow_sessions.read().await;
                        sessions
                            .get(&msg.chat_id)
                            .map(|session| session.kernel_session_id.clone())
                    };
                    if let Some(root_session_id) =
                        maybe_root.filter(|value| !value.trim().is_empty())
                    {
                        let _ = kernel.recover_session(&root_session_id).await;
                    }
                }
                let status_text = bridge.get_status_text(msg.chat_id).await;
                RemoteUiMessage::ActionCard(Self::render_status_card(
                    workflow_facade,
                    workflow_sessions,
                    msg.chat_id,
                    Some(format!("Resumed current remote session.\n\n{}", status_text)),
                ).await)
            }
            RemoteCommand::Artifacts => {
                if let Some(facade) = workflow_facade {
                    if let Some(session) = workflow_sessions.read().await.get(&msg.chat_id).cloned() {
                        RemoteUiMessage::ActionCard(facade.render_artifacts_card(&session, None).await)
                    } else {
                        RemoteUiMessage::ActionCard(Self::build_artifacts_card())
                    }
                } else {
                    RemoteUiMessage::ActionCard(Self::build_artifacts_card())
                }
            }
        };

        // Send response (skip if bridge already sent via streaming mode)
        let result_status = match &response {
            RemoteUiMessage::PlainText(text) if text.contains("Error:") => "error",
            _ => "success",
        };

        if !already_sent_by_bridge {
            let _ = adapter.send_ui_message(msg.chat_id, &response).await;
        }

        // Write audit log
        Self::write_audit_log(db, msg, command_type, result_status, None);

        // Dispatch webhook notification for task-producing commands
        if should_dispatch_webhook {
            if let Some(webhook_svc) = webhook_service {
                let session_id = bridge
                    .get_active_session_id(msg.chat_id)
                    .await
                    .unwrap_or_default();

                let svc = webhook_svc.clone();
                let event = match webhook_event_type {
                    WebhookEventType::TaskComplete => {
                        UnifiedStreamEvent::Complete { stop_reason: None }
                    }
                    WebhookEventType::TaskCancelled => UnifiedStreamEvent::Complete {
                        stop_reason: Some("cancelled".to_string()),
                    },
                    WebhookEventType::TaskFailed => UnifiedStreamEvent::Error {
                        message: match &response {
                            RemoteUiMessage::PlainText(text) => text.clone(),
                            RemoteUiMessage::ActionCard(card) => card.body.clone(),
                        },
                        code: Some("remote_error".to_string()),
                    },
                    _ => UnifiedStreamEvent::Complete { stop_reason: None },
                };
                tokio::spawn(async move {
                    dispatch_on_remote_event(
                        &event,
                        if session_id.is_empty() {
                            "remote-session"
                        } else {
                            &session_id
                        },
                        None,
                        None,
                        None,
                        svc,
                        None,
                        &remote_source,
                    );
                });
            }
        }
    }

    /// Get the command type name for audit logging.
    fn command_type_name(command: &RemoteCommand) -> &'static str {
        match command {
            RemoteCommand::Start => "Start",
            RemoteCommand::Home => "Home",
            RemoteCommand::Menu => "Menu",
            RemoteCommand::SwitchMode { .. } => "SwitchMode",
            RemoteCommand::PlanGenerate => "PlanGenerate",
            RemoteCommand::PlanApprove => "PlanApprove",
            RemoteCommand::TaskConfirmConfig => "TaskConfirmConfig",
            RemoteCommand::TaskGeneratePrd => "TaskGeneratePrd",
            RemoteCommand::TaskApprovePrd => "TaskApprovePrd",
            RemoteCommand::DebugApprovePatch => "DebugApprovePatch",
            RemoteCommand::SetContextPreset { .. } => "SetContextPreset",
            RemoteCommand::ToggleContextSource { .. } => "ToggleContextSource",
            RemoteCommand::SetPermissionLevel { .. } => "SetPermissionLevel",
            RemoteCommand::RespondPermission { .. } => "RespondPermission",
            RemoteCommand::NewSession { .. } => "NewSession",
            RemoteCommand::SendMessage { .. } => "SendMessage",
            RemoteCommand::ListSessions => "ListSessions",
            RemoteCommand::SwitchSession { .. } => "SwitchSession",
            RemoteCommand::Status => "Status",
            RemoteCommand::Cancel => "Cancel",
            RemoteCommand::CloseSession => "CloseSession",
            RemoteCommand::WhoAmI => "WhoAmI",
            RemoteCommand::Help => "Help",
            RemoteCommand::Context => "Context",
            RemoteCommand::Permission => "Permission",
            RemoteCommand::Resume => "Resume",
            RemoteCommand::Artifacts => "Artifacts",
        }
    }

    async fn send_chat_message(
        msg: &IncomingRemoteEvent,
        content: &str,
        bridge: &SessionBridge,
        adapter: &dyn super::adapters::RemoteAdapter,
        workflow_kernel: &Option<WorkflowKernelState>,
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        gateway_config: &RemoteGatewayConfig,
        current_config: &TelegramAdapterConfig,
        should_dispatch_webhook: &mut bool,
        webhook_event_type: &mut WebhookEventType,
        already_sent_by_bridge: &mut bool,
    ) -> RemoteUiMessage {
        let workflow_session_id = {
            let sessions = workflow_sessions.read().await;
            sessions.get(&msg.chat_id).and_then(|session| {
                (!session.kernel_session_id.is_empty()).then_some(session.kernel_session_id.clone())
            })
        };
        if let (Some(kernel), Some(root_session_id)) =
            (workflow_kernel.as_ref(), workflow_session_id.as_deref())
        {
            let _ = kernel
                .submit_input(
                    root_session_id,
                    UserInputIntent {
                        intent_type: UserInputIntentType::ChatMessage,
                        content: content.to_string(),
                        metadata: serde_json::json!({
                            "source": "telegram_remote",
                            "chatId": msg.chat_id,
                        }),
                    },
                )
                .await;
        }
        match bridge
            .send_message(
                msg.chat_id,
                content,
                &current_config.streaming_mode,
                Some(adapter),
                workflow_session_id.as_deref(),
            )
            .await
        {
            Ok(resp) => {
                *should_dispatch_webhook = true;
                *webhook_event_type = WebhookEventType::TaskComplete;
                if resp.already_sent {
                    *already_sent_by_bridge = true;
                }
                RemoteUiMessage::PlainText(ResponseMapper::format_response(&resp))
            }
            Err(RemoteError::NoActiveSession) => RemoteUiMessage::ActionCard(
                Self::build_home_card(
                    gateway_config,
                    current_config,
                    workflow_sessions,
                    msg.chat_id,
                    msg.user_id,
                    msg.username.as_deref(),
                )
                .await,
            ),
            Err(e) => {
                *should_dispatch_webhook = true;
                *webhook_event_type = WebhookEventType::TaskFailed;
                RemoteUiMessage::PlainText(ResponseMapper::format_error(&e))
            }
        }
    }

    /// Write an audit log entry to the remote_audit_log table.
    fn write_audit_log(
        db: &Database,
        msg: &IncomingRemoteEvent,
        command_type: &str,
        result_status: &str,
        error_message: Option<&str>,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let adapter_type = msg.adapter_type.to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        if let Ok(conn) = db.get_connection() {
            let _ = conn.execute(
                "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, username, command_text, command_type, result_status, error_message, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    adapter_type,
                    msg.chat_id,
                    msg.user_id,
                    msg.username,
                    msg.text,
                    command_type,
                    result_status,
                    error_message,
                    created_at,
                ],
            );
        }
    }

    fn new_workflow_session(
        msg: &IncomingRemoteEvent,
        config: &TelegramAdapterConfig,
    ) -> RemoteWorkflowSession {
        let now = chrono::Utc::now().to_rfc3339();
        RemoteWorkflowSession {
            chat_id: msg.chat_id,
            user_id: msg.user_id,
            kernel_session_id: String::new(),
            active_mode: WorkflowMode::Chat,
            linked_mode_sessions: HashMap::new(),
            project_path: None,
            workspace_label: None,
            provider: None,
            model: None,
            base_url: None,
            permission_level: PermissionLevel::Strict,
            context_sources: Some(ContextSourceConfig::default()),
            streaming_mode: config.streaming_mode.clone(),
            pending_interaction: None,
            last_rendered_message_refs: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    async fn open_workflow_root_session(
        workflow_kernel: &WorkflowKernelState,
        project_path: Option<String>,
        workspace_label: Option<String>,
    ) -> Result<String, String> {
        let initial_context = project_path.as_ref().map(|path| {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "workspacePath".to_string(),
                serde_json::Value::String(path.clone()),
            );
            if let Some(label) = workspace_label.as_ref().filter(|value| !value.trim().is_empty()) {
                metadata.insert(
                    "workspaceLabel".to_string(),
                    serde_json::Value::String(label.clone()),
                );
            }
            HandoffContextBundle {
                metadata,
                ..Default::default()
            }
        });
        let session = workflow_kernel
            .open_session(Some(WorkflowMode::Chat), initial_context)
            .await?;
        if let Some(label) = workspace_label.as_ref().filter(|value| !value.trim().is_empty()) {
            let _ = workflow_kernel.rename_session(&session.session_id, label).await;
        }
        Ok(session.session_id)
    }

    fn resolve_workspace_selection(
        gateway_config: &RemoteGatewayConfig,
        config: &TelegramAdapterConfig,
        project_path: String,
        provider: Option<String>,
        model: Option<String>,
    ) -> (String, Option<String>, Option<String>, Option<String>) {
        let _ = config;
        let explicit_provider = provider
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(normalize_provider_name)
            .map(str::to_string);
        let explicit_model = model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let (default_provider, default_model) =
            Self::resolve_llm_selection(provider.clone(), model.clone());
        if !project_path.trim().is_empty() {
            return (project_path, default_provider, default_model, None);
        }
        if let Some(workspace) = gateway_config.allowed_project_roots.first() {
            let workspace_provider = explicit_provider
                .or_else(|| workspace.default_provider.clone())
                .or(default_provider.clone());
            let workspace_model = explicit_model
                .or_else(|| workspace.default_model.clone())
                .or_else(|| {
                    workspace_provider
                        .as_deref()
                        .and_then(Self::default_model_for_provider)
                })
                .or(default_model.clone());
            return (
                workspace.path.clone(),
                workspace_provider,
                workspace_model,
                workspace.label.clone(),
            );
        }
        (project_path, default_provider, default_model, None)
    }

    fn resolve_llm_selection(
        provider: Option<String>,
        model: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let explicit_provider = provider
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(normalize_provider_name)
            .map(str::to_string);
        let explicit_model = model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        let config = ConfigService::new().ok().map(|svc| svc.get_config_clone());
        let provider_value = explicit_provider.or_else(|| {
            config
                .as_ref()
                .map(|cfg| cfg.default_provider.trim().to_string())
                .filter(|value| !value.is_empty())
        });
        let model_value = explicit_model.or_else(|| {
            provider_value.as_deref().and_then(|provider_name| {
                config.as_ref().and_then(|cfg| {
                    let model = cfg.model_for_provider(provider_name);
                    (!model.trim().is_empty()).then_some(model)
                })
            })
        })
        .or_else(|| {
            config
                .as_ref()
                .map(|cfg| cfg.default_model.trim().to_string())
                .filter(|value| !value.is_empty())
        });

        (provider_value, model_value)
    }

    fn default_model_for_provider(provider: &str) -> Option<String> {
        let canonical = normalize_provider_name(provider)?;
        let config = ConfigService::new().ok().map(|svc| svc.get_config_clone())?;
        let model = config.model_for_provider(canonical);
        if !model.trim().is_empty() {
            return Some(model);
        }
        let default_model = config.default_model.trim().to_string();
        (!default_model.is_empty()).then_some(default_model)
    }

    async fn build_home_card(
        gateway_config: &RemoteGatewayConfig,
        config: &TelegramAdapterConfig,
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        chat_id: i64,
        user_id: i64,
        username: Option<&str>,
    ) -> RemoteActionCard {
        let session = workflow_sessions.read().await.get(&chat_id).cloned();
        let mode = session
            .as_ref()
            .map(|value| format!("{:?}", value.active_mode))
            .unwrap_or_else(|| "Chat".to_string());
        let permission = session
            .as_ref()
            .map(|value| format!("{:?}", value.permission_level))
            .unwrap_or_else(|| "Strict".to_string());
        let workspace = session
            .as_ref()
            .and_then(|value| value.workspace_label.clone().or(value.project_path.clone()))
            .unwrap_or_else(|| "Not selected".to_string());
        let session_id = session
            .as_ref()
            .map(|value| value.kernel_session_id.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Not started");
        let context_summary = session
            .as_ref()
            .and_then(|value| value.context_sources.as_ref())
            .map(Self::summarize_context)
            .unwrap_or_else(|| "Default".to_string());
        let password = if config.require_password { "Enabled" } else { "Disabled" };
        let auth_hint = if config.require_password {
            "Password gate is on. If this chat is not authenticated yet, send /auth <password> first."
        } else {
            "Password gate is off. If you use Allowed User IDs, add your Telegram user_id from /whoami."
        };
        let workspace_list = if gateway_config.allowed_project_roots.is_empty() {
            "Configured Workspaces: none".to_string()
        } else {
            format!(
                "Configured Workspaces: {}",
                gateway_config
                    .allowed_project_roots
                    .iter()
                    .map(RemoteWorkspaceEntry::display_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        RemoteActionCard {
            title: "Plan Cascade Remote Console".to_string(),
            body: format!(
                "Workspace: {workspace}\nMode: {mode}\nPermission: {permission}\nContext: {context_summary}\nSession: {session_id}\nPassword Gate: {password}\nTelegram user_id: {user_id}\nTelegram chat_id: {chat_id}\nUsername: {}\n{workspace_list}\n\nQuick start:\n1. Send /whoami and copy your user_id into Allowed User IDs if needed.\n2. Send /new to open the first configured workspace, or /new <path> for a specific one.\n3. Use /chat, /plan, /task, or /debug, then type normally.\n4. Use /help for the full command list.\n\n{auth_hint}",
                username.unwrap_or("(not set)")
            ),
            actions: vec![
                RemoteActionButton { id: "remote:mode:switch:chat".to_string(), label: "Chat".to_string(), style: None },
                RemoteActionButton { id: "remote:mode:switch:plan".to_string(), label: "Plan".to_string(), style: None },
                RemoteActionButton { id: "remote:mode:switch:task".to_string(), label: "Task".to_string(), style: None },
                RemoteActionButton { id: "remote:mode:switch:debug".to_string(), label: "Debug".to_string(), style: None },
                RemoteActionButton { id: "remote:status".to_string(), label: "Status".to_string(), style: None },
                RemoteActionButton { id: "remote:context".to_string(), label: "Context".to_string(), style: None },
                RemoteActionButton { id: "remote:permission".to_string(), label: "Permission".to_string(), style: None },
                RemoteActionButton { id: "remote:sessions".to_string(), label: "Sessions".to_string(), style: None },
                RemoteActionButton { id: "remote:artifacts".to_string(), label: "Artifacts".to_string(), style: None },
                RemoteActionButton { id: "remote:whoami".to_string(), label: "Who Am I".to_string(), style: None },
                RemoteActionButton { id: "remote:help".to_string(), label: "Help".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn build_help_card(config: &TelegramAdapterConfig) -> RemoteActionCard {
        let auth_hint = if config.require_password {
            "Password gate: enabled. Before other commands, send /auth <password> in this chat."
        } else {
            "Password gate: disabled."
        };
        RemoteActionCard {
            title: "Remote Help".to_string(),
            body: format!(
                "{HELP_TEXT}\n{auth_hint}\n\nTip: send /whoami to get your Telegram user_id and chat_id for whitelist setup."
            ),
            actions: vec![
                RemoteActionButton { id: "remote:home".to_string(), label: "Console".to_string(), style: None },
                RemoteActionButton { id: "remote:whoami".to_string(), label: "Who Am I".to_string(), style: None },
                RemoteActionButton { id: "remote:status".to_string(), label: "Status".to_string(), style: None },
                RemoteActionButton { id: "remote:mode:switch:chat".to_string(), label: "Chat".to_string(), style: None },
                RemoteActionButton { id: "remote:mode:switch:plan".to_string(), label: "Plan".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn build_whoami_text(
        gateway_config: &RemoteGatewayConfig,
        config: &TelegramAdapterConfig,
        chat_id: i64,
        user_id: i64,
        username: Option<&str>,
    ) -> String {
        let allowed_user = if config.allowed_user_ids.is_empty() {
            "Allowed User IDs is empty, so user-id filtering is currently open."
        } else if config.allowed_user_ids.contains(&user_id) {
            "Your user_id is already in Allowed User IDs."
        } else {
            "Your user_id is not in Allowed User IDs yet."
        };
        let allowed_chat = if config.allowed_chat_ids.is_empty() {
            "Allowed Chat IDs is empty, so chat-id filtering is currently open."
        } else if config.allowed_chat_ids.contains(&chat_id) {
            "This chat_id is already in Allowed Chat IDs."
        } else {
            "This chat_id is not in Allowed Chat IDs yet."
        };
        let workspace_hint = gateway_config
            .allowed_project_roots
            .first()
            .map(RemoteWorkspaceEntry::display_name)
            .unwrap_or_else(|| "no configured workspace".to_string());
        format!(
            "Telegram identity\n\nuser_id: {user_id}\nchat_id: {chat_id}\nusername: {}\n\n{allowed_user}\n{allowed_chat}\nPassword gate: {}\nConfigured workspace example: {workspace_hint}\n\nUse these IDs in Settings > Remote Control > Telegram Bot Configuration.",
            username.unwrap_or("(not set)"),
            if config.require_password { "enabled" } else { "disabled" },
        )
    }

    async fn render_status_card(
        workflow_facade: Option<&RemoteWorkflowFacade>,
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        chat_id: i64,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        if let Some(facade) = workflow_facade {
            if let Some(session) = workflow_sessions.read().await.get(&chat_id).cloned() {
                return facade.render_status_card(&session, prefix).await;
            }
        }
        Self::build_status_card(workflow_sessions, chat_id, prefix).await
    }

    async fn build_status_card(
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        chat_id: i64,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let session = workflow_sessions.read().await.get(&chat_id).cloned();
        let mut body = prefix.unwrap_or_default();
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        if let Some(session) = session {
            let linked_sessions = if session.linked_mode_sessions.is_empty() {
                "none".to_string()
            } else {
                session
                    .linked_mode_sessions
                    .iter()
                    .map(|(mode, session_id)| format!("{mode}:{session_id}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let pending = session
                .pending_interaction
                .as_ref()
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|| "none".to_string());
            body.push_str(&format!(
                "Mode: {:?}\nPermission: {:?}\nWorkspace: {}\nKernel Session: {}\nLinked Sessions: {}\nPending: {}\nUpdated: {}",
                session.active_mode,
                session.permission_level,
                session
                    .workspace_label
                    .clone()
                    .or(session.project_path.clone())
                    .unwrap_or_else(|| "Not selected".to_string()),
                if session.kernel_session_id.is_empty() {
                    "Not started"
                } else {
                    session.kernel_session_id.as_str()
                },
                linked_sessions,
                pending,
                session.updated_at
            ));
        } else {
            body.push_str("No remote workflow session yet. Use /new <path> or open /start.");
        }
        RemoteActionCard {
            title: "Remote Session Status".to_string(),
            body,
            actions: vec![
                RemoteActionButton { id: "remote:home".to_string(), label: "Home".to_string(), style: None },
                RemoteActionButton { id: "remote:resume".to_string(), label: "Resume".to_string(), style: None },
                RemoteActionButton { id: "remote:cancel".to_string(), label: "Cancel".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    async fn build_context_card(
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        chat_id: i64,
    ) -> RemoteActionCard {
        let summary = workflow_sessions
            .read()
            .await
            .get(&chat_id)
            .and_then(|session| session.context_sources.as_ref())
            .map(Self::summarize_context)
            .unwrap_or_else(|| "Default context preset".to_string());
        RemoteActionCard {
            title: "Context Sources".to_string(),
            body: format!(
                "{}\n\nTelegram remote now preserves the same context model type as Simple. Advanced editing will use guided actions next.",
                summary
            ),
            actions: vec![
                RemoteActionButton { id: "remote:home".to_string(), label: "Home".to_string(), style: None },
                RemoteActionButton { id: "remote:status".to_string(), label: "Status".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    async fn build_permission_card(
        workflow_sessions: &RwLock<HashMap<i64, RemoteWorkflowSession>>,
        chat_id: i64,
    ) -> RemoteActionCard {
        let level = workflow_sessions
            .read()
            .await
            .get(&chat_id)
            .map(|session| format!("{:?}", session.permission_level))
            .unwrap_or_else(|| "Strict".to_string());
        RemoteActionCard {
            title: "Permission Level".to_string(),
            body: format!(
                "Current remote permission level: {level}\n\nTool approval cards will be routed here when the active workflow requests permission."
            ),
            actions: vec![
                RemoteActionButton { id: "remote:home".to_string(), label: "Home".to_string(), style: None },
                RemoteActionButton { id: "remote:status".to_string(), label: "Status".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn build_artifacts_card() -> RemoteActionCard {
        RemoteActionCard {
            title: "Artifacts".to_string(),
            body: "Artifact browsing is enabled at the protocol layer. Telegram attachment-backed artifact delivery will be wired to workflow/debug reports next.".to_string(),
            actions: vec![
                RemoteActionButton { id: "remote:home".to_string(), label: "Home".to_string(), style: None },
                RemoteActionButton { id: "remote:status".to_string(), label: "Status".to_string(), style: None },
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn summarize_context(config: &ContextSourceConfig) -> String {
        let knowledge = config
            .knowledge
            .as_ref()
            .map(|value| if value.enabled { "on" } else { "off" })
            .unwrap_or("off");
        let memory = config
            .memory
            .as_ref()
            .map(|value| if value.enabled { "on" } else { "off" })
            .unwrap_or("off");
        let skills = config
            .skills
            .as_ref()
            .map(|value| if value.enabled { "on" } else { "off" })
            .unwrap_or("off");
        format!(
            "Project: {}\nKnowledge: {}\nMemory: {}\nSkills: {}",
            config.project_id, knowledge, memory, skills
        )
    }

    /// Record a connection error in the gateway status and attempt reconnect.
    ///
    /// Returns `true` if a reconnect was attempted and succeeded, `false` otherwise.
    pub async fn record_connection_error(&self, error: &str) -> bool {
        let should_reconnect = {
            let mut status = self.status.write().await;
            status.running = false;
            status.connected_since = None;
            status.error = Some(error.to_string());
            status.last_error_at = Some(chrono::Utc::now().to_rfc3339());
            status.reconnect_attempts += 1;

            let attempt = status.reconnect_attempts;
            if attempt <= self.reconnect_config.max_attempts {
                status.reconnecting = true;
                tracing::warn!(
                    "Connection error (attempt {}/{}): {}",
                    attempt,
                    self.reconnect_config.max_attempts,
                    error
                );
                Some(attempt)
            } else {
                status.reconnecting = false;
                tracing::error!(
                    "Max reconnect attempts ({}) exceeded. Giving up.",
                    self.reconnect_config.max_attempts
                );
                None
            }
        };

        if let Some(attempt) = should_reconnect {
            let delay = self.reconnect_config.delay_for_attempt(attempt - 1);
            tracing::info!(
                "Waiting {}ms before reconnect attempt {}...",
                delay,
                attempt
            );
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

            let proxy = self.proxy.read().await.clone();
            match self.start(proxy.as_ref()).await {
                Ok(()) => {
                    let mut status = self.status.write().await;
                    status.reconnect_attempts = 0;
                    status.reconnecting = false;
                    status.error = None;
                    tracing::info!("Reconnected successfully after {} attempt(s)", attempt);
                    true
                }
                Err(e) => {
                    tracing::warn!("Reconnect attempt {} failed: {}", attempt, e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Reset reconnect state (e.g., after a successful manual start).
    pub async fn reset_reconnect_state(&self) {
        let mut status = self.status.write().await;
        status.reconnect_attempts = 0;
        status.last_error_at = None;
        status.reconnecting = false;
    }

    /// Stop the gateway gracefully.
    pub async fn stop(&self) -> Result<(), RemoteError> {
        self.manual_stop.store(true, Ordering::SeqCst);
        if let Some(cancel) = self.processor_cancel.write().await.take() {
            cancel.cancel();
        }
        if let Some(adapter) = self.adapter.read().await.clone() {
            adapter.stop().await?;
        }
        if let Some(handle) = self.processor_handle.write().await.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.adapter_watch_handle.write().await.take() {
            let _ = handle.await;
        }
        *self.adapter.write().await = None;
        let mut status = self.status.write().await;
        status.running = false;
        status.connected_since = None;
        // Reset reconnect state on clean stop
        status.reconnect_attempts = 0;
        status.reconnecting = false;
        Ok(())
    }

    /// Update gateway configuration.
    pub async fn update_config(&self, config: RemoteGatewayConfig) -> Result<(), RemoteError> {
        let allowed_paths = Self::normalize_allowed_paths(&config.allowed_project_roots)?;
        self.session_bridge
            .update_allowed_paths(allowed_paths)
            .await?;
        *self.config.write().await = config;
        Ok(())
    }

    /// Update Telegram adapter configuration.
    pub async fn update_telegram_config(
        &self,
        config: TelegramAdapterConfig,
    ) -> Result<(), RemoteError> {
        let existing = self.telegram_config.read().await.clone();
        let password_changed = existing.require_password != config.require_password
            || existing.access_password != config.access_password;
        *self.telegram_config.write().await = config;
        if password_changed {
            self.authenticated_chats.write().await.clear();
        }
        Ok(())
    }

    /// Disconnect a specific remote session by chat_id.
    pub async fn disconnect_session(&self, chat_id: i64) -> Result<(), RemoteError> {
        self.session_bridge.close_session(chat_id).await
    }

    /// Get all remote session mappings.
    pub async fn list_sessions(&self) -> Vec<super::types::RemoteSessionMapping> {
        self.session_bridge.list_all_sessions().await
    }

    async fn handle_adapter_exit(&self, result: Result<(), RemoteError>) {
        if self.manual_stop.load(Ordering::SeqCst) {
            return;
        }

        let message = match result {
            Ok(()) => "Adapter stopped unexpectedly".to_string(),
            Err(error) => error.to_string(),
        };

        if let Some(cancel) = self.processor_cancel.write().await.take() {
            cancel.cancel();
        }
        if let Some(handle) = self.processor_handle.write().await.take() {
            handle.abort();
        }
        *self.adapter.write().await = None;

        let gateway = self.clone();
        let reconnect_message = message.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(runtime) = tokio::runtime::Runtime::new() {
                let _ = runtime.block_on(async move {
                    gateway.record_connection_error(&reconnect_message).await;
                });
            }
        });
    }

    fn normalize_allowed_paths(paths: &[RemoteWorkspaceEntry]) -> Result<Vec<PathBuf>, RemoteError> {
        if paths.is_empty() {
            return Err(RemoteError::ConfigError(
                "At least one allowed project root must be configured".to_string(),
            ));
        }

        let mut normalized = Vec::with_capacity(paths.len());
        for workspace in paths {
            let path = workspace.path.as_str();
            let expanded = if path.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    home.join(path.strip_prefix("~/").unwrap_or(&path[1..]))
                } else {
                    PathBuf::from(path)
                }
            } else {
                PathBuf::from(path)
            };

            let canonical = expanded.canonicalize().map_err(|error| {
                RemoteError::ConfigError(format!(
                    "Allowed project root '{}' does not exist or is not accessible: {}",
                    path, error
                ))
            })?;
            if !canonical.is_dir() {
                return Err(RemoteError::ConfigError(format!(
                    "Allowed project root '{}' is not a directory",
                    path
                )));
            }
            normalized.push(canonical);
        }
        normalized.sort();
        normalized.dedup();
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::session_bridge::{BridgeRuntimeConfig, BridgeServices};
    use crate::services::remote::types::{
        IncomingRemoteEvent, RemoteAdapterType, RemoteCommand, RemoteIncomingEventType,
        RemoteWorkspaceEntry,
    };
    use crate::services::webhook::integration::format_remote_source;
    use crate::storage::KeyringService;

    #[test]
    fn test_command_type_name() {
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Help),
            "Help"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Status),
            "Status"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Cancel),
            "Cancel"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::ListSessions),
            "ListSessions"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::CloseSession),
            "CloseSession"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::NewSession {
                project_path: "".to_string(),
                provider: None,
                model: None,
            }),
            "NewSession"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::SendMessage {
                content: "hi".to_string(),
            }),
            "SendMessage"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::SwitchSession {
                session_id: "x".to_string(),
            }),
            "SwitchSession"
        );
    }

    #[test]
    fn test_write_audit_log() {
        let db = Database::new_in_memory().unwrap();
        let msg = IncomingRemoteEvent {
            adapter_type: RemoteAdapterType::Telegram,
            event_type: RemoteIncomingEventType::TextMessage,
            chat_id: 123,
            user_id: 456,
            username: Some("testuser".to_string()),
            text: "/help".to_string(),
            message_id: 1,
            timestamp: chrono::Utc::now(),
            callback_id: None,
            callback_data: None,
        };

        RemoteGatewayService::write_audit_log(&db, &msg, "Help", "success", None);

        let conn = db.get_connection().unwrap();
        let (cmd_type, status): (String, String) = conn
            .query_row(
                "SELECT command_type, result_status FROM remote_audit_log ORDER BY created_at DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(cmd_type, "Help");
        assert_eq!(status, "success");
    }

    #[test]
    fn test_write_audit_log_with_error() {
        let db = Database::new_in_memory().unwrap();
        let msg = IncomingRemoteEvent {
            adapter_type: RemoteAdapterType::Telegram,
            event_type: RemoteIncomingEventType::TextMessage,
            chat_id: 123,
            user_id: 456,
            username: None,
            text: "/new ~/secret".to_string(),
            message_id: 2,
            timestamp: chrono::Utc::now(),
            callback_id: None,
            callback_data: None,
        };

        RemoteGatewayService::write_audit_log(
            &db,
            &msg,
            "NewSession",
            "error",
            Some("Unauthorized path"),
        );

        let conn = db.get_connection().unwrap();
        let error_msg: Option<String> = conn
            .query_row(
                "SELECT error_message FROM remote_audit_log ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error_msg, Some("Unauthorized path".to_string()));
    }

    #[tokio::test]
    async fn test_gateway_new() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        let status = gateway.get_status().await;
        assert!(!status.running);
        assert_eq!(status.total_commands_processed, 0);
        assert!(gateway.webhook_service.is_none());
    }

    #[tokio::test]
    async fn test_gateway_with_webhook_service() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db.clone(), None, None);
        assert!(gateway.webhook_service.is_none());

        let keyring = Arc::new(crate::storage::KeyringService::new());
        let webhook_svc = Arc::new(WebhookService::new(db, keyring, |_| None));
        gateway.set_webhook_service(webhook_svc);
        assert!(gateway.webhook_service.is_some());
    }

    #[tokio::test]
    async fn test_gateway_start_not_enabled() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false,
            ..Default::default()
        };

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        let result = gateway.start(None).await;
        assert!(result.is_err());
        match result {
            Err(RemoteError::NotEnabled) => {}
            _ => panic!("Expected NotEnabled error"),
        }
    }

    #[tokio::test]
    async fn test_gateway_status_tracking() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        // Manually update status to simulate running
        {
            let mut status = gateway.status.write().await;
            status.running = true;
            status.total_commands_processed = 42;
            status.last_command_at = Some("2026-02-18T14:30:00Z".to_string());
        }

        let status = gateway.get_status().await;
        assert!(status.running);
        assert_eq!(status.total_commands_processed, 42);
    }

    #[tokio::test]
    async fn test_gateway_stop() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        // Set running status
        {
            let mut status = gateway.status.write().await;
            status.running = true;
        }

        gateway.stop().await.unwrap();

        let status = gateway.get_status().await;
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_gateway_update_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new_with_services(
            db.clone(),
            BridgeServices {
                keyring: Arc::new(KeyringService::new()),
                orchestrators: Arc::new(RwLock::new(HashMap::new())),
                runtime_config: Arc::new(BridgeRuntimeConfig::new(vec![], 2000)),
                workflow_kernel: None,
            },
        ));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        let new_config = RemoteGatewayConfig {
            enabled: true,
            auto_start: true,
            allowed_project_roots: vec![RemoteWorkspaceEntry {
                path: "/tmp".to_string(),
                label: Some("Tmp".to_string()),
                default_provider: None,
                default_model: None,
            }],
            ..Default::default()
        };

        gateway.update_config(new_config).await.unwrap();

        let config = gateway.config.read().await;
        assert!(config.enabled);
        assert!(config.auto_start);
    }

    #[tokio::test]
    async fn test_gateway_update_telegram_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        let tg_config = TelegramAdapterConfig {
            bot_token: Some("test-token".to_string()),
            allowed_chat_ids: vec![123],
            ..Default::default()
        };

        gateway.update_telegram_config(tg_config).await.unwrap();

        let tg = gateway.telegram_config.read().await;
        assert_eq!(tg.allowed_chat_ids, vec![123]);
    }

    #[tokio::test]
    async fn test_password_authentication_logic() {
        // Test the authentication flow
        let authenticated: RwLock<HashSet<i64>> = RwLock::new(HashSet::new());

        // Chat 123 is not authenticated
        assert!(!authenticated.read().await.contains(&123));

        // Authenticate chat 123
        authenticated.write().await.insert(123);

        // Now chat 123 is authenticated
        assert!(authenticated.read().await.contains(&123));

        // Chat 456 is still not authenticated
        assert!(!authenticated.read().await.contains(&456));
    }

    #[test]
    fn test_remote_source_formatting_in_gateway() {
        let source = format_remote_source("Telegram", Some("testuser"));
        assert_eq!(source, "via Telegram @testuser");

        let source_no_user = format_remote_source("Telegram", None);
        assert_eq!(source_no_user, "via Telegram");
    }

    #[tokio::test]
    async fn test_gateway_reconnect_config_default() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        assert_eq!(gateway.reconnect_config.max_attempts, 5);
        assert_eq!(gateway.reconnect_config.base_delay_ms, 1000);
        assert_eq!(gateway.reconnect_config.max_delay_ms, 30000);
    }

    #[tokio::test]
    async fn test_gateway_set_reconnect_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 5000,
        });
        assert_eq!(gateway.reconnect_config.max_attempts, 3);
        assert_eq!(gateway.reconnect_config.base_delay_ms, 500);
        assert_eq!(gateway.reconnect_config.max_delay_ms, 5000);
    }

    #[tokio::test]
    async fn test_gateway_reconnect_state_tracking() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        // Simulate connection error by directly updating status
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 2;
            status.last_error_at = Some("2026-02-18T15:00:00Z".to_string());
            status.reconnecting = true;
            status.error = Some("Connection lost".to_string());
        }

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 2);
        assert!(status.last_error_at.is_some());
        assert!(status.reconnecting);
        assert_eq!(status.error, Some("Connection lost".to_string()));
    }

    #[tokio::test]
    async fn test_gateway_reset_reconnect_state() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        // Simulate some reconnect state
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 3;
            status.last_error_at = Some("2026-02-18T15:00:00Z".to_string());
            status.reconnecting = true;
        }

        // Reset
        gateway.reset_reconnect_state().await;

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 0);
        assert!(status.last_error_at.is_none());
        assert!(!status.reconnecting);
    }

    #[tokio::test]
    async fn test_gateway_stop_resets_reconnect_state() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);

        // Set running and reconnect state
        {
            let mut status = gateway.status.write().await;
            status.running = true;
            status.reconnect_attempts = 2;
            status.reconnecting = true;
        }

        gateway.stop().await.unwrap();

        let status = gateway.get_status().await;
        assert!(!status.running);
        assert_eq!(status.reconnect_attempts, 0);
        assert!(!status.reconnecting);
    }

    #[tokio::test]
    async fn test_gateway_record_connection_error_increments_attempts() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false, // Keep disabled so start() fails fast in reconnect
            ..Default::default()
        };

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        // Use fast backoff for test
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 2,
            base_delay_ms: 1, // 1ms for fast tests
            max_delay_ms: 10,
        });

        // First error
        let reconnected = gateway.record_connection_error("test error 1").await;
        assert!(!reconnected); // start() fails because not enabled

        let status = gateway.get_status().await;
        // After failed reconnect, attempts stays at 1
        assert_eq!(status.reconnect_attempts, 1);
        assert!(status.error.is_some());
        assert!(status.last_error_at.is_some());
    }

    #[tokio::test]
    async fn test_gateway_record_connection_error_max_attempts() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false,
            ..Default::default()
        };

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db, None, None);
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 2,
            base_delay_ms: 1,
            max_delay_ms: 10,
        });

        // Manually set attempts to max
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 2;
        }

        // Next error should exceed max, returning false without attempting reconnect
        let reconnected = gateway.record_connection_error("final error").await;
        assert!(!reconnected);

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 3); // incremented past max
        assert!(!status.reconnecting); // gave up
    }
}
