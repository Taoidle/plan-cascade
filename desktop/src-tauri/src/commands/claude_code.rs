//! Claude Code Commands
//!
//! Tauri commands for Claude Code CLI integration.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::commands::webhook::WebhookState;
use crate::models::claude_code::{
    ActiveSessionInfo, CancelExecutionResponse, ClaudeCodeSession, SendMessageRequest,
    SendMessageResponse, StartChatRequest, StartChatResponse,
};
use crate::models::response::CommandResponse;
use crate::services::claude_code::{
    channels, ActiveSessionManager, ChatHandler, StreamEventPayload,
};
use crate::services::file_change_tracker::WorkspaceChangeSnapshot;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::webhook::integration::dispatch_on_event as dispatch_webhook_on_event;
use crate::services::workflow_kernel::{ChatRuntimeDispatch, WorkflowKernelState};
use crate::state::AppState;

/// State for Claude Code services
pub struct ClaudeCodeState {
    pub session_manager: Arc<ActiveSessionManager>,
    pub chat_handler: Arc<RwLock<ChatHandler>>,
}

impl ClaudeCodeState {
    pub fn new() -> Self {
        let session_manager = Arc::new(ActiveSessionManager::new());
        let chat_handler = Arc::new(RwLock::new(ChatHandler::new(session_manager.clone())));
        Self {
            session_manager,
            chat_handler,
        }
    }
}

impl Default for ClaudeCodeState {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_tool_file_paths(tool_name: &str, arguments: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return Vec::new();
    };
    match tool_name.to_lowercase().as_str() {
        "write" | "edit" => json
            .get("file_path")
            .and_then(|value| value.as_str())
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn resolve_tool_file_path(project_root: &Path, raw_path: &str) -> PathBuf {
    let candidate = PathBuf::from(raw_path);
    if candidate.is_absolute() {
        candidate
    } else {
        project_root.join(candidate)
    }
}

fn tracker_relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Start a new Claude Code chat session
///
/// Creates a new session associated with the given project path.
/// Optionally specify a model or resume an existing session.
#[tauri::command]
pub async fn start_chat(
    request: StartChatRequest,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<StartChatResponse>, String> {
    let is_resume = request.resume_session_id.is_some();

    let session = if let Some(ref resume_id) = request.resume_session_id {
        // Resume existing session
        match state
            .session_manager
            .resume_session(resume_id, &request.project_path)
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        }
    } else {
        // Start new session
        match state
            .session_manager
            .start_session(&request.project_path, request.model.as_deref())
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        }
    };

    Ok(CommandResponse::ok(StartChatResponse {
        session_id: session.id,
        is_resumed: is_resume,
    }))
}

/// Send a message to a Claude Code session
///
/// This triggers the streaming response. Stream events are forwarded
/// from the mpsc channel to Tauri events so the frontend can receive them.
#[tauri::command]
pub async fn send_message(
    request: SendMessageRequest,
    state: State<'_, ClaudeCodeState>,
    app_state: State<'_, AppState>,
    file_changes_state: State<'_, crate::commands::file_changes::FileChangesState>,
    workflow_state: State<'_, WorkflowKernelState>,
    webhook_state: State<'_, WebhookState>,
    app: AppHandle,
) -> Result<CommandResponse<SendMessageResponse>, String> {
    let mut chat_handler = state.chat_handler.write().await;
    let session_for_webhook = state.session_manager.get_session(&request.session_id).await;
    let webhook_service = webhook_state.get_or_init(app_state.inner()).await.ok();
    let _ = webhook_state
        .start_worker_if_needed(app_state.inner())
        .await;
    let webhook_project_path = session_for_webhook
        .as_ref()
        .map(|session| session.project_path.clone());
    let tracker_session_id = request
        .kernel_session_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| request.session_id.clone());
    let tracker_project_root = session_for_webhook
        .as_ref()
        .map(|session| PathBuf::from(session.project_path.clone()));
    let tracker_bundle = if let Some(project_root) = tracker_project_root.as_ref() {
        let tracker = file_changes_state
            .get_or_create(&tracker_session_id, &project_root.to_string_lossy())
            .await;
        let mut turn_index = None;
        if let Ok(mut guard) = tracker.lock() {
            guard.set_app_handle(app.clone());
            let next = guard.turn_index().saturating_add(1);
            guard.set_turn_index(next);
            turn_index = Some(next);
        }
        turn_index.map(|idx| (tracker, project_root.clone(), idx))
    } else {
        None
    };

    match chat_handler
        .send_message(&request.session_id, &request.prompt)
        .await
    {
        Ok(stream) => {
            let crate::services::claude_code::SendMessageStream {
                execution_id,
                receiver,
            } = stream;
            if let Some(kernel_session_id) = request
                .kernel_session_id
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                if let Err(error) = workflow_state
                    .register_chat_runtime_dispatch(ChatRuntimeDispatch {
                        session_id: kernel_session_id.to_string(),
                        backend_kind: "claude".to_string(),
                        binding_session_id: format!("claude:{}", request.session_id),
                        run_id: Some(execution_id.clone()),
                    })
                    .await
                {
                    return Ok(CommandResponse::err(error));
                }
            }
            let mut rx = receiver;
            let session_id = request.session_id.clone();
            let response_execution_id = execution_id.clone();
            let workflow_kernel = workflow_state.inner().clone();
            let tracker_for_events = tracker_bundle
                .as_ref()
                .map(|(tracker, _, _)| Arc::clone(tracker));
            let tracker_project_root = tracker_bundle
                .as_ref()
                .map(|(_, project_root, _)| project_root.clone());
            let tracker_turn_index = tracker_bundle
                .as_ref()
                .map(|(_, _, turn_index)| *turn_index);
            let tracker_metadata = crate::services::file_change_tracker::FileChangeMetadata {
                source_mode: Some(crate::services::file_change_tracker::FileChangeSourceMode::Chat),
                actor_kind: Some(
                    crate::services::file_change_tracker::FileChangeActorKind::RootAgent,
                ),
                actor_id: Some("claude-chat".to_string()),
                actor_label: Some("Main Agent".to_string()),
                sub_agent_depth: None,
                origin_session_id: request.kernel_session_id.clone(),
            };
            eprintln!(
                "[DEBUG] send_message: spawning event forwarder for session {} execution {}",
                session_id, execution_id
            );
            // Spawn a task to forward events from the mpsc channel to Tauri events
            tokio::spawn(async move {
                let mut event_count = 0u32;
                let mut tool_names = HashMap::<String, String>::new();
                let mut tool_arguments = HashMap::<String, String>::new();
                let mut tool_file_snapshots =
                    HashMap::<String, Vec<(String, Option<String>)>>::new();
                let mut tool_workspace_snapshots =
                    HashMap::<String, WorkspaceChangeSnapshot>::new();
                while let Some(event) = rx.recv().await {
                    event_count += 1;
                    if let Some(service) = webhook_service.clone() {
                        dispatch_webhook_on_event(
                            &event,
                            &session_id,
                            Some(&execution_id),
                            None,
                            webhook_project_path.as_deref(),
                            service,
                            None,
                        );
                    }
                    eprintln!(
                        "[DEBUG] forwarding event #{} to frontend for session {}",
                        event_count, session_id
                    );
                    match &event {
                        UnifiedStreamEvent::ToolStart {
                            tool_id,
                            tool_name,
                            arguments,
                        } => {
                            tool_names.insert(tool_id.clone(), tool_name.clone());
                            if let Some(arguments) = arguments.clone() {
                                tool_arguments.insert(tool_id.clone(), arguments.clone());
                                if let (Some(tracker), Some(project_root)) =
                                    (tracker_for_events.as_ref(), tracker_project_root.as_ref())
                                {
                                    if tool_name.eq_ignore_ascii_case("bash") {
                                        if let Ok(guard) = tracker.lock() {
                                            if let Ok(snapshot) = guard.capture_workspace_snapshot()
                                            {
                                                tool_workspace_snapshots
                                                    .insert(tool_id.clone(), snapshot);
                                            }
                                        }
                                    } else if tool_name.eq_ignore_ascii_case("write")
                                        || tool_name.eq_ignore_ascii_case("edit")
                                    {
                                        if let Ok(guard) = tracker.lock() {
                                            let snapshots =
                                                extract_tool_file_paths(tool_name, &arguments)
                                                    .into_iter()
                                                    .map(|raw_path| {
                                                        let absolute_path = resolve_tool_file_path(
                                                            project_root,
                                                            &raw_path,
                                                        );
                                                        let relative_path = tracker_relative_path(
                                                            project_root,
                                                            &absolute_path,
                                                        );
                                                        let before_hash =
                                                            guard.capture_before(&absolute_path);
                                                        (relative_path, before_hash)
                                                    })
                                                    .collect::<Vec<_>>();
                                            if !snapshots.is_empty() {
                                                tool_file_snapshots
                                                    .insert(tool_id.clone(), snapshots);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        UnifiedStreamEvent::ToolComplete {
                            tool_id,
                            tool_name,
                            arguments,
                        } => {
                            tool_arguments.insert(tool_id.clone(), arguments.clone());
                            tool_names
                                .entry(tool_id.clone())
                                .or_insert_with(|| tool_name.clone());
                            if let (Some(tracker), Some(project_root)) =
                                (tracker_for_events.as_ref(), tracker_project_root.as_ref())
                            {
                                if tool_name.eq_ignore_ascii_case("bash")
                                    && !tool_workspace_snapshots.contains_key(tool_id)
                                {
                                    if let Ok(guard) = tracker.lock() {
                                        if let Ok(snapshot) = guard.capture_workspace_snapshot() {
                                            tool_workspace_snapshots
                                                .insert(tool_id.clone(), snapshot);
                                        }
                                    }
                                } else if (tool_name.eq_ignore_ascii_case("write")
                                    || tool_name.eq_ignore_ascii_case("edit"))
                                    && !tool_file_snapshots.contains_key(tool_id)
                                {
                                    if let Ok(guard) = tracker.lock() {
                                        let snapshots =
                                            extract_tool_file_paths(tool_name, arguments)
                                                .into_iter()
                                                .map(|raw_path| {
                                                    let absolute_path = resolve_tool_file_path(
                                                        project_root,
                                                        &raw_path,
                                                    );
                                                    let relative_path = tracker_relative_path(
                                                        project_root,
                                                        &absolute_path,
                                                    );
                                                    let before_hash =
                                                        guard.capture_before(&absolute_path);
                                                    (relative_path, before_hash)
                                                })
                                                .collect::<Vec<_>>();
                                        if !snapshots.is_empty() {
                                            tool_file_snapshots.insert(tool_id.clone(), snapshots);
                                        }
                                    }
                                }
                            }
                        }
                        UnifiedStreamEvent::ToolResult { tool_id, .. } => {
                            if let (Some(tracker), Some(project_root), Some(turn_index)) = (
                                tracker_for_events.as_ref(),
                                tracker_project_root.as_ref(),
                                tracker_turn_index,
                            ) {
                                let tool_name = tool_names.remove(tool_id);
                                let arguments = tool_arguments.remove(tool_id);
                                if let Some(tool_name) = tool_name {
                                    if tool_name.eq_ignore_ascii_case("bash") {
                                        if let Some(before_snapshot) =
                                            tool_workspace_snapshots.remove(tool_id)
                                        {
                                            if let Ok(mut guard) = tracker.lock() {
                                                if let Ok(after_snapshot) =
                                                    guard.capture_workspace_snapshot()
                                                {
                                                    guard.record_workspace_delta_between_at_with_metadata(
                                                        turn_index,
                                                        &format!("claude-{tool_id}"),
                                                        "Bash",
                                                        &before_snapshot,
                                                        &after_snapshot,
                                                        "Claude Code Bash",
                                                        Some(&tracker_metadata),
                                                    );
                                                }
                                            }
                                        }
                                    } else if (tool_name.eq_ignore_ascii_case("write")
                                        || tool_name.eq_ignore_ascii_case("edit"))
                                        && arguments.is_some()
                                    {
                                        if let Some(before_files) =
                                            tool_file_snapshots.remove(tool_id)
                                        {
                                            if let Ok(mut guard) = tracker.lock() {
                                                for (idx, (relative_path, before_hash)) in
                                                    before_files.into_iter().enumerate()
                                                {
                                                    let absolute_path =
                                                        project_root.join(&relative_path);
                                                    let after_hash = if absolute_path.exists() {
                                                        std::fs::read(&absolute_path).ok().and_then(
                                                            |bytes| {
                                                                guard.store_content(&bytes).ok()
                                                            },
                                                        )
                                                    } else {
                                                        None
                                                    };
                                                    let description = match (
                                                        before_hash.as_ref(),
                                                        after_hash.as_ref(),
                                                    ) {
                                                        (None, Some(_)) => {
                                                            format!(
                                                                "Claude Code {} created file",
                                                                tool_name
                                                            )
                                                        }
                                                        (Some(_), None) => {
                                                            format!(
                                                                "Claude Code {} deleted file",
                                                                tool_name
                                                            )
                                                        }
                                                        _ => {
                                                            format!(
                                                                "Claude Code {} modified file",
                                                                tool_name
                                                            )
                                                        }
                                                    };
                                                    guard.record_change_at_with_metadata(
                                                        turn_index,
                                                        &format!("claude-{tool_id}-{idx}"),
                                                        &tool_name,
                                                        &relative_path,
                                                        before_hash,
                                                        after_hash.as_deref(),
                                                        &description,
                                                        Some(&tracker_metadata),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    let payload = StreamEventPayload {
                        event,
                        session_id: session_id.clone(),
                        execution_id: execution_id.clone(),
                    };
                    if let Err(e) = app.emit(channels::STREAM, &payload) {
                        eprintln!("[WARN] Failed to emit stream event: {}", e);
                    }
                    let binding_session_id = format!("claude:{}", session_id);
                    match workflow_kernel
                        .sync_chat_runtime_event(
                            &binding_session_id,
                            Some(execution_id.as_str()),
                            &payload.event,
                        )
                        .await
                    {
                        Ok(Some(mutation)) => {
                            let _ = crate::commands::workflow::emit_workflow_session_mutation(
                                &app,
                                &mutation,
                                "claude_code.send_message",
                            );
                            let _ = crate::commands::workflow::emit_kernel_update_for_session(
                                &app,
                                &workflow_kernel,
                                &mutation.session.session_id,
                                "claude_code.send_message",
                            )
                            .await;
                            let _ = crate::commands::workflow::emit_session_catalog_update(
                                &app,
                                &workflow_kernel,
                                "claude_code.send_message",
                            )
                            .await;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            eprintln!(
                                "[WARN] Failed to sync Claude chat event to workflow kernel: {}",
                                error
                            );
                        }
                    }
                }
                eprintln!(
                    "[DEBUG] event forwarder ended after {} events for session {} execution {}",
                    event_count, session_id, execution_id
                );
            });
            Ok(CommandResponse::ok(SendMessageResponse {
                execution_id: response_execution_id,
            }))
        }
        Err(e) => {
            if let Some(kernel_session_id) = request
                .kernel_session_id
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                let _ = workflow_state
                    .mark_chat_turn_failed(kernel_session_id, &e.to_string())
                    .await;
            }
            Ok(CommandResponse::err(e.to_string()))
        }
    }
}

/// Cancel the current execution in a session
///
/// Kills the running process and marks the session as cancelled.
#[tauri::command]
pub async fn cancel_execution(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
    workflow_state: State<'_, WorkflowKernelState>,
    permission_state: State<'_, super::permissions::PermissionState>,
    app: AppHandle,
) -> Result<CommandResponse<CancelExecutionResponse>, String> {
    permission_state
        .gate
        .cancel_session_requests(&session_id)
        .await;
    match state.session_manager.cancel_session(&session_id).await {
        Ok(execution_id) => {
            let binding_session_id = format!("claude:{}", session_id);
            if let Ok(Some(mutation)) = workflow_state
                .sync_chat_runtime_event(
                    &binding_session_id,
                    Some(execution_id.as_str()),
                    &UnifiedStreamEvent::Error {
                        message: "Execution cancelled".to_string(),
                        code: Some("cancelled".to_string()),
                    },
                )
                .await
            {
                let _ = crate::commands::workflow::emit_workflow_session_mutation(
                    &app,
                    &mutation,
                    "claude_code.cancel_execution",
                );
                let _ = crate::commands::workflow::emit_kernel_update_for_session(
                    &app,
                    &workflow_state,
                    &mutation.session.session_id,
                    "claude_code.cancel_execution",
                )
                .await;
                let _ = crate::commands::workflow::emit_session_catalog_update(
                    &app,
                    &workflow_state,
                    "claude_code.cancel_execution",
                )
                .await;
            }
            Ok(CommandResponse::ok(CancelExecutionResponse {
                cancelled: true,
                session_id,
                execution_id: Some(execution_id),
                reason: None,
            }))
        }
        Err(crate::utils::error::AppError::NotFound(message)) => {
            Ok(CommandResponse::ok(CancelExecutionResponse {
                cancelled: false,
                session_id,
                execution_id: None,
                reason: Some(message),
            }))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the history/details for a session
///
/// Returns session metadata including message count and state.
#[tauri::command]
pub async fn get_session_history(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ClaudeCodeSession>, String> {
    match state.session_manager.get_session(&session_id).await {
        Some(session) => Ok(CommandResponse::ok(session)),
        None => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// List all active Claude Code sessions
///
/// Returns information about all sessions including their process status.
#[tauri::command]
pub async fn list_active_sessions(
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<Vec<ActiveSessionInfo>>, String> {
    let sessions = state.session_manager.list_session_infos().await;
    Ok(CommandResponse::ok(sessions))
}

/// Remove a session completely
///
/// Cancels any running process and removes the session from memory.
#[tauri::command]
pub async fn remove_session(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    match state.session_manager.remove_session(&session_id).await {
        Ok(_) => match app_state
            .with_memory_store(|store| store.clear_session_memories(&session_id))
            .await
        {
            Ok(_) => Ok(CommandResponse::ok(true)),
            Err(e) => Ok(CommandResponse::err(format!(
                "Session removed but failed to clear session memories: {}",
                e
            ))),
        },
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get information about a specific session including process status
#[tauri::command]
pub async fn get_session_info(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ActiveSessionInfo>, String> {
    match state.session_manager.get_session_info(&session_id).await {
        Some(info) => Ok(CommandResponse::ok(info)),
        None => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_state_creation() {
        let state = ClaudeCodeState::new();
        // Verify state is created without panic
        let _ = state;
    }

    #[tokio::test]
    async fn test_start_chat_request() {
        let request = StartChatRequest {
            project_path: "/test/project".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
            resume_session_id: None,
        };

        assert_eq!(request.project_path, "/test/project");
        assert_eq!(request.model, Some("claude-sonnet-4-20250514".to_string()));
        assert!(request.resume_session_id.is_none());
    }

    #[tokio::test]
    async fn test_send_message_request() {
        let request = SendMessageRequest {
            session_id: "test-session".to_string(),
            prompt: "Hello, Claude!".to_string(),
            kernel_session_id: None,
        };

        assert_eq!(request.session_id, "test-session");
        assert_eq!(request.prompt, "Hello, Claude!");
    }
}
