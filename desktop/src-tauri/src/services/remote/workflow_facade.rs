use super::types::{
    RemoteActionButton, RemoteActionCard, RemoteAttachmentRef, RemotePendingInteraction,
    RemoteUiMessage, RemoteWorkflowSession,
};
use crate::commands::debug_mode::{
    approve_debug_patch, enter_debug_mode, submit_debug_clarification, ApproveDebugPatchRequest,
    DebugModeState, SubmitDebugClarificationRequest,
};
use crate::commands::file_changes::FileChangesState;
use crate::commands::knowledge::KnowledgeState;
use crate::commands::permissions::PermissionState;
use crate::commands::plan_mode::planning_execution_commands::{approve_plan, generate_plan};
use crate::commands::plan_mode::session_analysis_commands::{
    enter_plan_mode, submit_plan_clarification,
};
use crate::commands::plan_mode::{
    ApprovePlanRequest, EnterPlanModeRequest, GeneratePlanRequest, PlanModeState,
    SubmitPlanClarificationRequest,
};
use crate::commands::plugins::PluginState;
use crate::commands::standalone::StandaloneState;
use crate::commands::task_mode::execution_commands::approve_task_prd;
use crate::commands::task_mode::generation_commands::generate_task_prd;
use crate::commands::task_mode::session_lifecycle_commands::{
    confirm_task_configuration, enter_task_mode,
};
use crate::commands::task_mode::{
    ApproveTaskPrdRequest, ConfirmTaskConfigurationRequest, EnterTaskModeRequest,
    GenerateTaskPrdRequest, TaskConfigConfirmationState, TaskModeSession, TaskModeState,
    TaskModeStatus, TaskWorkflowConfig,
};
use crate::commands::task_mode::StoryExecutionMode;
use crate::models::CommandResponse;
use crate::services::debug_mode::EnterDebugModeRequest;
use crate::services::orchestrator::permission_gate::PendingPermissionRequestSnapshot;
use crate::services::orchestrator::permissions::PermissionResponse;
use crate::services::orchestrator::permissions::PermissionLevel;
use crate::services::plan_mode::types::{PlanModePhase, PlanModeSession};
use crate::services::strategy::analyzer::RecommendedWorkflowConfig;
use crate::services::debug_mode::DebugModeSession;
use crate::services::task_mode::context_provider::{
    ContextSourceConfig, KnowledgeSourceConfig, MemorySourceConfig, SkillsSourceConfig,
};
use crate::services::workflow_kernel::{HandoffContextBundle, WorkflowKernelState, WorkflowMode};
use crate::state::AppState;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct RemoteWorkflowFacade {
    app: AppHandle,
}

pub enum RemoteWorkflowExecution {
    ChatFallback(RemoteWorkflowSession),
    Ui {
        session: RemoteWorkflowSession,
        message: RemoteUiMessage,
    },
}

impl RemoteWorkflowFacade {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub async fn switch_mode(
        &self,
        mut session: RemoteWorkflowSession,
        mode: WorkflowMode,
    ) -> Result<RemoteWorkflowSession, String> {
        self.ensure_root_session(&mut session).await?;
        self.kernel_state()
            .transition_mode(&session.kernel_session_id, mode, None)
            .await?;
        session.active_mode = mode;
        session.updated_at = chrono::Utc::now().to_rfc3339();
        self.apply_permission_level(&session.kernel_session_id, session.permission_level)
            .await;
        Ok(session)
    }

    pub async fn handle_text_input(
        &self,
        mut session: RemoteWorkflowSession,
        content: &str,
    ) -> Result<RemoteWorkflowExecution, String> {
        let content = content.trim();
        if content.is_empty() {
            return Err("Input content cannot be empty".to_string());
        }

        self.ensure_root_session(&mut session).await?;

        match session.active_mode {
            WorkflowMode::Chat => Ok(RemoteWorkflowExecution::ChatFallback(session)),
            WorkflowMode::Plan => {
                let ui = self.handle_plan_text_input(&mut session, content).await?;
                Ok(RemoteWorkflowExecution::Ui { session, message: ui })
            }
            WorkflowMode::Task => {
                let ui = self.handle_task_text_input(&mut session, content).await?;
                Ok(RemoteWorkflowExecution::Ui { session, message: ui })
            }
            WorkflowMode::Debug => {
                let ui = self.handle_debug_text_input(&mut session, content).await?;
                Ok(RemoteWorkflowExecution::Ui { session, message: ui })
            }
        }
    }

    pub async fn render_context_card(
        &self,
        session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        Self::context_action_card(session, prefix)
    }

    pub async fn render_status_card(
        &self,
        session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let pending_requests = self.pending_permission_requests(session).await;
        let pending_summary = pending_requests
            .first()
            .map(|request| {
                format!(
                    "tool approval: {} [{}] ({})",
                    request.tool_name, request.risk, request.request_id
                )
            })
            .unwrap_or_else(|| {
                session
                    .pending_interaction
                    .as_ref()
                    .map(Self::pending_interaction_label)
                    .unwrap_or_else(|| "none".to_string())
            });
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
        let workspace = session
            .workspace_label
            .clone()
            .or(session.project_path.clone())
            .unwrap_or_else(|| "Not selected".to_string());
        let context_summary = session
            .context_sources
            .as_ref()
            .map(Self::summarize_context)
            .unwrap_or_else(|| "Default context preset".to_string());
        let mode_phase = self
            .current_mode_phase(session)
            .await
            .unwrap_or_else(|| "idle".to_string());

        let mut body = prefix.unwrap_or_default();
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&format!(
            "Mode: {:?}\nMode Phase: {}\nPermission: {:?}\nWorkspace: {}\nKernel Session: {}\nLinked Sessions: {}\nPending: {}\nPending Approvals: {}\nContext:\n{}\nUpdated: {}",
            session.active_mode,
            mode_phase,
            session.permission_level,
            workspace,
            if session.kernel_session_id.is_empty() {
                "Not started"
            } else {
                session.kernel_session_id.as_str()
            },
            linked_sessions,
            pending_summary,
            pending_requests.len(),
            context_summary,
            session.updated_at
        ));

        RemoteActionCard {
            title: "Remote Session Status".to_string(),
            body,
            actions: vec![
                Self::button("remote:home", "Home"),
                Self::button("remote:resume", "Resume"),
                Self::button("remote:context", "Context"),
                Self::button("remote:permission", "Permission"),
                Self::button("remote:artifacts", "Artifacts"),
                Self::button("remote:cancel", "Cancel"),
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    pub async fn render_permission_card(
        &self,
        session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let pending = self.pending_permission_requests(session).await;
        Self::permission_action_card(session, &pending, prefix)
    }

    pub async fn render_artifacts_card(
        &self,
        session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let (mut lines, attachments) = self.collect_artifacts(session).await;
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.insert(0, String::new());
            lines.insert(0, prefix);
        }
        if lines.is_empty() {
            lines.push("No workflow artifacts are available yet.".to_string());
        }
        RemoteActionCard {
            title: "Artifacts".to_string(),
            body: lines.join("\n"),
            actions: vec![
                Self::button("remote:home", "Home"),
                Self::button("remote:status", "Status"),
                Self::button("remote:permission", "Permission"),
            ],
            metadata: HashMap::new(),
            attachment_refs: attachments,
        }
    }

    pub async fn pending_permission_requests(
        &self,
        session: &RemoteWorkflowSession,
    ) -> Vec<PendingPermissionRequestSnapshot> {
        let mut session_ids = Vec::new();
        if !session.kernel_session_id.trim().is_empty() {
            session_ids.push(session.kernel_session_id.clone());
        }
        session_ids.extend(
            session
                .linked_mode_sessions
                .values()
                .filter(|value| !value.trim().is_empty())
                .cloned(),
        );
        self.app
            .state::<PermissionState>()
            .gate
            .pending_requests_for_sessions(&session_ids)
            .await
    }

    pub async fn handle_action(
        &self,
        mut session: RemoteWorkflowSession,
        command: &super::types::RemoteCommand,
    ) -> Result<(RemoteWorkflowSession, RemoteUiMessage), String> {
        self.ensure_root_session(&mut session).await?;
        let message = match command {
            super::types::RemoteCommand::PlanGenerate => {
                self.handle_plan_generate(&mut session).await?
            }
            super::types::RemoteCommand::PlanApprove => {
                self.handle_plan_approve(&mut session).await?
            }
            super::types::RemoteCommand::TaskConfirmConfig => {
                self.handle_task_confirm_config(&mut session).await?
            }
            super::types::RemoteCommand::TaskGeneratePrd => {
                self.handle_task_generate_prd(&mut session).await?
            }
            super::types::RemoteCommand::TaskApprovePrd => {
                self.handle_task_approve_prd(&mut session).await?
            }
            super::types::RemoteCommand::DebugApprovePatch => {
                self.handle_debug_approve_patch(&mut session).await?
            }
            super::types::RemoteCommand::SetContextPreset { preset } => {
                self.set_context_preset(&mut session, preset).await?
            }
            super::types::RemoteCommand::ToggleContextSource { source } => {
                self.toggle_context_source(&mut session, source).await?
            }
            super::types::RemoteCommand::SetPermissionLevel { level } => {
                self.set_permission_level(&mut session, *level).await?
            }
            super::types::RemoteCommand::RespondPermission {
                request_id,
                allowed,
                always_allow,
            } => self
                .respond_permission(&mut session, request_id, *allowed, *always_allow)
                .await?,
            _ => return Err("Unsupported workflow action".to_string()),
        };
        Ok((session, message))
    }

    async fn set_context_preset(
        &self,
        session: &mut RemoteWorkflowSession,
        preset: &str,
    ) -> Result<RemoteUiMessage, String> {
        session.context_sources = Some(Self::build_context_preset(
            session
                .context_sources
                .as_ref()
                .and_then(|value| (!value.project_id.trim().is_empty()).then_some(value.project_id.clone()))
                .unwrap_or_else(|| "default".to_string()),
            preset,
        ));
        session.pending_interaction = Some(RemotePendingInteraction::ContextWizard);
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::context_action_card(
            session,
            Some(format!("Applied context preset: {preset}")),
        )))
    }

    async fn toggle_context_source(
        &self,
        session: &mut RemoteWorkflowSession,
        source: &str,
    ) -> Result<RemoteUiMessage, String> {
        let mut config = session.context_sources.clone().unwrap_or_else(|| ContextSourceConfig {
            project_id: "default".to_string(),
            knowledge: Some(KnowledgeSourceConfig {
                enabled: false,
                selected_collections: Vec::new(),
                selected_documents: Vec::new(),
            }),
            memory: Some(MemorySourceConfig {
                enabled: false,
                selected_categories: Vec::new(),
                selected_memory_ids: Vec::new(),
                excluded_memory_ids: Vec::new(),
                selected_scopes: Vec::new(),
                session_id: None,
                statuses: Vec::new(),
                review_mode: None,
                selection_mode: None,
            }),
            skills: Some(SkillsSourceConfig {
                enabled: false,
                selected_skill_ids: Vec::new(),
                invoked_skill_ids: Vec::new(),
                selection_mode: Default::default(),
                review_filter: None,
            }),
        });
        match source {
            "knowledge" => {
                let entry = config.knowledge.get_or_insert(KnowledgeSourceConfig {
                    enabled: false,
                    selected_collections: Vec::new(),
                    selected_documents: Vec::new(),
                });
                entry.enabled = !entry.enabled;
            }
            "memory" => {
                let entry = config.memory.get_or_insert(MemorySourceConfig {
                    enabled: false,
                    selected_categories: Vec::new(),
                    selected_memory_ids: Vec::new(),
                    excluded_memory_ids: Vec::new(),
                    selected_scopes: vec!["project".to_string(), "global".to_string()],
                    session_id: None,
                    statuses: Vec::new(),
                    review_mode: None,
                    selection_mode: None,
                });
                entry.enabled = !entry.enabled;
            }
            "skills" => {
                let entry = config.skills.get_or_insert(SkillsSourceConfig {
                    enabled: false,
                    selected_skill_ids: Vec::new(),
                    invoked_skill_ids: Vec::new(),
                    selection_mode: Default::default(),
                    review_filter: None,
                });
                entry.enabled = !entry.enabled;
            }
            _ => return Err(format!("Unsupported context source: {source}")),
        }
        session.context_sources = Some(config);
        session.pending_interaction = Some(RemotePendingInteraction::ContextWizard);
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::context_action_card(
            session,
            Some(format!("Toggled context source: {source}")),
        )))
    }

    async fn set_permission_level(
        &self,
        session: &mut RemoteWorkflowSession,
        level: PermissionLevel,
    ) -> Result<RemoteUiMessage, String> {
        session.permission_level = level;
        let mut session_ids = vec![session.kernel_session_id.clone()];
        session_ids.extend(session.linked_mode_sessions.values().cloned());
        for session_id in session_ids {
            if !session_id.trim().is_empty() {
                self.apply_permission_level(&session_id, level).await;
            }
        }
        session.pending_interaction = Some(RemotePendingInteraction::PermissionWizard);
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::permission_action_card(
            session,
            &[],
            Some(format!("Permission level set to {:?}.", level)),
        )))
    }

    async fn respond_permission(
        &self,
        session: &mut RemoteWorkflowSession,
        request_id: &str,
        allowed: bool,
        always_allow: bool,
    ) -> Result<RemoteUiMessage, String> {
        self.app
            .state::<PermissionState>()
            .gate
            .resolve(
                request_id,
                PermissionResponse {
                    request_id: request_id.to_string(),
                    allowed,
                    always_allow,
                },
            )
            .await;
        let pending = self.pending_permission_requests(session).await;
        session.pending_interaction = pending.first().map(|request| RemotePendingInteraction::ToolApproval {
            request_id: request.request_id.clone(),
            tool_name: request.tool_name.clone(),
            risk: request.risk.clone(),
        });
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::permission_action_card(
            session,
            &pending,
            Some(if allowed {
                if always_allow {
                    "Approved and saved for this session.".to_string()
                } else {
                    "Approved once.".to_string()
                }
            } else {
                "Denied.".to_string()
            }),
        )))
    }

    async fn current_mode_phase(&self, session: &RemoteWorkflowSession) -> Option<String> {
        match session.active_mode {
            WorkflowMode::Chat => Some("conversation".to_string()),
            WorkflowMode::Plan => {
                let session_id = session.linked_mode_sessions.get("plan")?;
                self.app
                    .state::<PlanModeState>()
                    .get_or_load_session_snapshot(session_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|snapshot| snapshot.phase.to_string())
            }
            WorkflowMode::Task => {
                let session_id = session.linked_mode_sessions.get("task")?;
                self.app
                    .state::<TaskModeState>()
                    .get_or_load_session_snapshot(session_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|snapshot| format!("{:?}", snapshot.status))
            }
            WorkflowMode::Debug => {
                let session_id = session.linked_mode_sessions.get("debug")?;
                self.app
                    .state::<DebugModeState>()
                    .get_or_load_session_snapshot(session_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|snapshot| snapshot.state.phase)
            }
        }
    }

    async fn collect_artifacts(
        &self,
        session: &RemoteWorkflowSession,
    ) -> (Vec<String>, Vec<RemoteAttachmentRef>) {
        let mut lines = Vec::new();
        let mut attachments = Vec::new();
        let mut seen_paths = HashSet::new();

        if !session.kernel_session_id.trim().is_empty() {
            if let Some(handoff) = self
                .kernel_state()
                .handoff_context_for_kernel_session(&session.kernel_session_id)
                .await
            {
                let count = handoff.artifact_refs.len()
                    + handoff
                        .summary_items
                        .iter()
                        .map(|item| item.artifact_refs.len())
                        .sum::<usize>();
                if count > 0 {
                    lines.push(format!("Workflow handoff artifacts: {}", count));
                }
                Self::collect_handoff_attachments(
                    &handoff,
                    session.project_path.as_deref(),
                    &mut seen_paths,
                    &mut attachments,
                );
            }

            if let Some(entry_handoff) = self
                .kernel_state()
                .mode_entry_handoff_for_kernel_session(&session.kernel_session_id, session.active_mode)
                .await
            {
                if !entry_handoff.artifact_refs.is_empty() {
                    lines.push(format!(
                        "{:?} entry handoff artifacts: {}",
                        session.active_mode,
                        entry_handoff.artifact_refs.len()
                    ));
                }
                Self::collect_handoff_attachments(
                    &entry_handoff,
                    session.project_path.as_deref(),
                    &mut seen_paths,
                    &mut attachments,
                );
            }
        }

        if let Some(task_session_id) = session.linked_mode_sessions.get("task") {
            if let Ok(Some(task_session)) = self
                .app
                .state::<TaskModeState>()
                .get_or_load_session_snapshot(task_session_id)
                .await
            {
                lines.push(format!("Task session: {:?}", task_session.status));
                if let Some(prd) = task_session.prd.as_ref() {
                    lines.push(format!(
                        "Task PRD ready: {} stories across {} batches",
                        prd.stories.len(),
                        prd.batches.len()
                    ));
                }
                if let Some(progress) = task_session.progress.as_ref() {
                    lines.push(format!(
                        "Task progress: batch {}/{} | stories {}/{} | failed {} | phase {}",
                        progress.current_batch.saturating_add(1),
                        progress.total_batches,
                        progress.stories_completed,
                        progress.total_stories,
                        progress.stories_failed,
                        progress.current_phase
                    ));
                }
            }
        }

        if let Some(debug_session_id) = session.linked_mode_sessions.get("debug") {
            if let Ok(Some(debug_session)) = self
                .app
                .state::<DebugModeState>()
                .get_or_load_session_snapshot(debug_session_id)
                .await
            {
                lines.push(format!("Debug phase: {}", debug_session.state.phase));
                if let Some(approval) = debug_session.state.pending_approval.as_ref() {
                    lines.push(format!("Debug patch awaiting approval: {}", approval.title));
                }
                if let Some(proposal) = debug_session
                    .state
                    .fix_proposal
                    .as_ref()
                    .and_then(|proposal| proposal.patch_preview_ref.as_ref())
                {
                    Self::push_attachment(
                        &mut attachments,
                        &mut seen_paths,
                        "Patch Preview".to_string(),
                        proposal.clone(),
                        session.project_path.as_deref(),
                    );
                }
                if let Some(report) = debug_session.state.verification_report.as_ref() {
                    lines.push(format!("Verification summary: {}", report.summary));
                    for artifact in &report.artifacts {
                        let label = Path::new(artifact)
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("Debug Artifact")
                            .to_string();
                        Self::push_attachment(
                            &mut attachments,
                            &mut seen_paths,
                            label,
                            artifact.clone(),
                            session.project_path.as_deref(),
                        );
                    }
                }
            }
        }

        if attachments.is_empty() {
            lines.push("No attachment-backed artifacts are available yet.".to_string());
        } else {
            lines.push(format!("Attachment-backed artifacts: {}", attachments.len()));
            for attachment in attachments.iter().take(8) {
                lines.push(format!("- {}: {}", attachment.label, attachment.path));
            }
        }

        (lines, attachments)
    }

    fn collect_handoff_attachments(
        handoff: &HandoffContextBundle,
        project_path: Option<&str>,
        seen_paths: &mut HashSet<String>,
        attachments: &mut Vec<RemoteAttachmentRef>,
    ) {
        for artifact in &handoff.artifact_refs {
            Self::push_attachment(
                attachments,
                seen_paths,
                Path::new(artifact)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Artifact")
                    .to_string(),
                artifact.clone(),
                project_path,
            );
        }
        for item in &handoff.summary_items {
            for artifact in &item.artifact_refs {
                Self::push_attachment(
                    attachments,
                    seen_paths,
                    format!(
                        "{}: {}",
                        item.title,
                        Path::new(artifact)
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("artifact")
                    ),
                    artifact.clone(),
                    project_path,
                );
            }
        }
    }

    fn push_attachment(
        attachments: &mut Vec<RemoteAttachmentRef>,
        seen_paths: &mut HashSet<String>,
        label: String,
        raw_path: String,
        project_path: Option<&str>,
    ) {
        let resolved_path = Self::resolve_artifact_path(project_path, &raw_path);
        if seen_paths.insert(resolved_path.clone()) {
            attachments.push(RemoteAttachmentRef {
                label,
                path: resolved_path,
            });
        }
    }

    fn resolve_artifact_path(project_path: Option<&str>, raw_path: &str) -> String {
        let path = Path::new(raw_path);
        if path.is_absolute() {
            return raw_path.to_string();
        }
        if let Some(base) = project_path {
            let candidate = Path::new(base).join(raw_path);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
        raw_path.to_string()
    }

    fn pending_interaction_label(interaction: &RemotePendingInteraction) -> String {
        match interaction {
            RemotePendingInteraction::SessionWizard => "session wizard".to_string(),
            RemotePendingInteraction::ContextWizard => "context wizard".to_string(),
            RemotePendingInteraction::PermissionWizard => "permission wizard".to_string(),
            RemotePendingInteraction::PlanClarification { session_id } => {
                format!("plan clarification ({session_id})")
            }
            RemotePendingInteraction::PlanReview { session_id } => {
                format!("plan review ({session_id})")
            }
            RemotePendingInteraction::TaskConfiguration { session_id } => {
                format!("task configuration ({session_id})")
            }
            RemotePendingInteraction::TaskPrdApproval { session_id } => {
                format!("task PRD approval ({session_id})")
            }
            RemotePendingInteraction::DebugPatchApproval { session_id } => {
                format!("debug patch approval ({session_id})")
            }
            RemotePendingInteraction::ToolApproval {
                request_id,
                tool_name,
                risk,
            } => format!("tool approval: {tool_name} [{risk}] ({request_id})"),
        }
    }

    async fn handle_plan_text_input(
        &self,
        session: &mut RemoteWorkflowSession,
        content: &str,
    ) -> Result<RemoteUiMessage, String> {
        let plan_state = self.app.state::<PlanModeState>();
        let plan_session_id = session.linked_mode_sessions.get("plan").cloned();
        let snapshot = if let Some(plan_session_id) = plan_session_id {
            if matches!(
                session.pending_interaction,
                Some(RemotePendingInteraction::PlanClarification { .. })
            ) {
                let existing = plan_state
                    .get_or_load_session_snapshot(&plan_session_id)
                    .await?
                    .ok_or_else(|| "Plan session not found".to_string())?;
                let current_question = existing
                    .current_question
                    .as_ref()
                    .ok_or_else(|| "No pending clarification question".to_string())?;
                let response = self
                    .expect_ok(submit_plan_clarification(
                        SubmitPlanClarificationRequest {
                            session_id: plan_session_id.clone(),
                            answer: crate::services::plan_mode::types::ClarificationAnswer {
                                question_id: current_question.question_id.clone(),
                                answer: content.to_string(),
                                skipped: false,
                                question_text: current_question.question.clone(),
                            },
                            provider: session.provider.clone(),
                            model: session.model.clone(),
                            base_url: session.base_url.clone(),
                            agent_ref: None,
                            agent_source: None,
                            project_path: session.project_path.clone(),
                            context_sources: session.context_sources.clone(),
                            conversation_context: None,
                            locale: None,
                        },
                        plan_state,
                        self.app.state::<AppState>(),
                        self.app.state::<KnowledgeState>(),
                        self.app.state::<WorkflowKernelState>(),
                        self.app.clone(),
                    )
                    .await)?;
                response
            } else {
                return Ok(RemoteUiMessage::ActionCard(Self::plan_action_card(
                    &plan_state
                        .get_or_load_session_snapshot(&plan_session_id)
                        .await?
                        .ok_or_else(|| "Plan session not found".to_string())?,
                    session,
                    Some("Plan mode is active. Use the buttons below or answer the clarification prompt.".to_string()),
                )));
            }
        } else {
            let response = self
                .expect_ok(enter_plan_mode(
                    EnterPlanModeRequest {
                        description: content.to_string(),
                        kernel_session_id: Some(session.kernel_session_id.clone()),
                        provider: session.provider.clone(),
                        model: session.model.clone(),
                        base_url: session.base_url.clone(),
                        agent_ref: None,
                        agent_source: None,
                        project_path: session.project_path.clone(),
                        context_sources: session.context_sources.clone(),
                        conversation_context: None,
                        locale: None,
                    },
                    self.app.state::<PlanModeState>(),
                    self.app.state::<AppState>(),
                    self.app.state::<KnowledgeState>(),
                    self.app.state::<WorkflowKernelState>(),
                    self.app.clone(),
                )
                .await)?;
            session
                .linked_mode_sessions
                .insert("plan".to_string(), response.session_id.clone());
            self.apply_permission_level(&response.session_id, session.permission_level)
                .await;
            response
        };

        session.pending_interaction = if snapshot.current_question.is_some() {
            Some(RemotePendingInteraction::PlanClarification {
                session_id: snapshot.session_id.clone(),
            })
        } else if snapshot.plan.is_some() && snapshot.phase == PlanModePhase::ReviewingPlan {
            Some(RemotePendingInteraction::PlanReview {
                session_id: snapshot.session_id.clone(),
            })
        } else {
            None
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(RemoteUiMessage::ActionCard(Self::plan_action_card(
            &snapshot,
            session,
            None,
        )))
    }

    async fn handle_plan_generate(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let plan_session_id = session
            .linked_mode_sessions
            .get("plan")
            .cloned()
            .ok_or_else(|| "No active plan session".to_string())?;
        self.expect_ok(generate_plan(
            GeneratePlanRequest {
                session_id: plan_session_id.clone(),
                provider: session.provider.clone(),
                model: session.model.clone(),
                base_url: session.base_url.clone(),
                agent_ref: None,
                agent_source: None,
                project_path: session.project_path.clone(),
                context_sources: session.context_sources.clone(),
                conversation_context: None,
                locale: None,
            },
            self.app.state::<PlanModeState>(),
            self.app.state::<AppState>(),
            self.app.state::<KnowledgeState>(),
            self.app.state::<WorkflowKernelState>(),
            self.app.clone(),
        )
        .await)?;

        let snapshot = self
            .app
            .state::<PlanModeState>()
            .get_or_load_session_snapshot(&plan_session_id)
            .await?
            .ok_or_else(|| "Plan session not found".to_string())?;
        session.pending_interaction = if snapshot.plan.is_some() {
            Some(RemotePendingInteraction::PlanReview {
                session_id: snapshot.session_id.clone(),
            })
        } else {
            None
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::plan_action_card(
            &snapshot,
            session,
            Some("Plan generated.".to_string()),
        )))
    }

    async fn handle_plan_approve(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let plan_state = self.app.state::<PlanModeState>();
        let plan_session_id = session
            .linked_mode_sessions
            .get("plan")
            .cloned()
            .ok_or_else(|| "No active plan session".to_string())?;
        let snapshot = plan_state
            .get_or_load_session_snapshot(&plan_session_id)
            .await?
            .ok_or_else(|| "Plan session not found".to_string())?;
        let plan = snapshot
            .plan
            .clone()
            .ok_or_else(|| "No generated plan to approve".to_string())?;
        self.expect_ok(approve_plan(
            ApprovePlanRequest {
                session_id: plan_session_id.clone(),
                plan,
                provider: session.provider.clone(),
                model: session.model.clone(),
                base_url: session.base_url.clone(),
                agent_ref: None,
                agent_source: None,
                project_path: session.project_path.clone(),
                context_sources: session.context_sources.clone(),
                conversation_context: None,
                locale: None,
            },
            plan_state,
            self.app.state::<FileChangesState>(),
            self.app.state::<AppState>(),
            self.app.state::<KnowledgeState>(),
            self.app.state::<StandaloneState>(),
            self.app.state::<PermissionState>(),
            self.app.state::<WorkflowKernelState>(),
            self.app.clone(),
        )
        .await)?;

        let updated = self
            .app
            .state::<PlanModeState>()
            .get_or_load_session_snapshot(&plan_session_id)
            .await?
            .ok_or_else(|| "Plan session not found".to_string())?;
        session.pending_interaction = None;
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::plan_action_card(
            &updated,
            session,
            Some("Plan approved. Execution has started.".to_string()),
        )))
    }

    async fn handle_task_text_input(
        &self,
        session: &mut RemoteWorkflowSession,
        content: &str,
    ) -> Result<RemoteUiMessage, String> {
        let task_session_id = session.linked_mode_sessions.get("task").cloned();
        let snapshot = if let Some(task_session_id) = task_session_id {
            self.app
                .state::<TaskModeState>()
                .get_or_load_session_snapshot(&task_session_id)
                .await?
                .ok_or_else(|| "Task session not found".to_string())?
        } else {
            let response = self
                .expect_ok(enter_task_mode(
                    EnterTaskModeRequest {
                        description: content.to_string(),
                        kernel_session_id: Some(session.kernel_session_id.clone()),
                        locale: None,
                        provider: session.provider.clone(),
                        model: session.model.clone(),
                        base_url: session.base_url.clone(),
                    },
                    self.app.state::<TaskModeState>(),
                    self.app.state::<WorkflowKernelState>(),
                    self.app.clone(),
                    self.app.state::<AppState>(),
                )
                .await)?;
            session
                .linked_mode_sessions
                .insert("task".to_string(), response.session_id.clone());
            self.apply_permission_level(&response.session_id, session.permission_level)
                .await;
            response
        };

        session.pending_interaction = match snapshot.status {
            TaskModeStatus::Initialized if snapshot.config_confirmation_state == TaskConfigConfirmationState::Pending => {
                Some(RemotePendingInteraction::TaskConfiguration {
                    session_id: snapshot.session_id.clone(),
                })
            }
            TaskModeStatus::ReviewingPrd if snapshot.prd.is_some() => {
                Some(RemotePendingInteraction::TaskPrdApproval {
                    session_id: snapshot.session_id.clone(),
                })
            }
            _ => None,
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(RemoteUiMessage::ActionCard(Self::task_action_card(
            &snapshot,
            session,
            None,
        )))
    }

    async fn handle_task_confirm_config(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let task_state = self.app.state::<TaskModeState>();
        let task_session_id = session
            .linked_mode_sessions
            .get("task")
            .cloned()
            .ok_or_else(|| "No active task session".to_string())?;
        let snapshot = task_state
            .get_or_load_session_snapshot(&task_session_id)
            .await?
            .ok_or_else(|| "Task session not found".to_string())?;
        let workflow_config = snapshot
            .strategy_recommendation
            .as_ref()
            .map(|value| Self::recommended_task_config(&value.recommended_config))
            .or_else(|| snapshot.confirmed_config.clone())
            .unwrap_or_default();
        let updated = self
            .expect_ok(confirm_task_configuration(
                ConfirmTaskConfigurationRequest {
                    session_id: task_session_id.clone(),
                    workflow_config,
                },
                task_state,
                self.app.state::<WorkflowKernelState>(),
                self.app.clone(),
            )
            .await)?;
        session.pending_interaction = Some(RemotePendingInteraction::TaskConfiguration {
            session_id: updated.session_id.clone(),
        });
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::task_action_card(
            &updated,
            session,
            Some("Task configuration confirmed.".to_string()),
        )))
    }

    async fn handle_task_generate_prd(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let task_session_id = session
            .linked_mode_sessions
            .get("task")
            .cloned()
            .ok_or_else(|| "No active task session".to_string())?;
        self.expect_ok(generate_task_prd(
            GenerateTaskPrdRequest {
                session_id: task_session_id.clone(),
                provider: session.provider.clone(),
                model: session.model.clone(),
                api_key: None,
                base_url: session.base_url.clone(),
                compiled_spec: None,
                conversation_history: None,
                max_context_tokens: None,
                locale: None,
                context_sources: session.context_sources.clone(),
                project_path: session.project_path.clone(),
            },
            self.app.state::<TaskModeState>(),
            self.app.state::<AppState>(),
            self.app.state::<KnowledgeState>(),
            self.app.state::<WorkflowKernelState>(),
            self.app.clone(),
        )
        .await)?;

        let snapshot = self
            .app
            .state::<TaskModeState>()
            .get_or_load_session_snapshot(&task_session_id)
            .await?
            .ok_or_else(|| "Task session not found".to_string())?;
        session.pending_interaction = if snapshot.prd.is_some() {
            Some(RemotePendingInteraction::TaskPrdApproval {
                session_id: snapshot.session_id.clone(),
            })
        } else {
            None
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::task_action_card(
            &snapshot,
            session,
            Some("PRD generated.".to_string()),
        )))
    }

    async fn handle_task_approve_prd(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let task_state = self.app.state::<TaskModeState>();
        let task_session_id = session
            .linked_mode_sessions
            .get("task")
            .cloned()
            .ok_or_else(|| "No active task session".to_string())?;
        let snapshot = task_state
            .get_or_load_session_snapshot(&task_session_id)
            .await?
            .ok_or_else(|| "Task session not found".to_string())?;
        let prd = snapshot
            .prd
            .clone()
            .ok_or_else(|| "No PRD available to approve".to_string())?;
        let workflow_config = snapshot.confirmed_config.clone().or_else(|| {
            snapshot
                .strategy_recommendation
                .as_ref()
                .map(|value| Self::recommended_task_config(&value.recommended_config))
        });
        self.expect_ok(approve_task_prd(
            self.app.clone(),
            ApproveTaskPrdRequest {
                session_id: task_session_id.clone(),
                prd,
                provider: session.provider.clone(),
                model: session.model.clone(),
                base_url: session.base_url.clone(),
                execution_mode: Some(StoryExecutionMode::Llm),
                workflow_config,
                global_default_agent: None,
                phase_configs: None,
                locale: None,
                context_sources: session.context_sources.clone(),
                project_path: session.project_path.clone(),
            },
            task_state,
            self.app.state::<FileChangesState>(),
            self.app.state::<WorkflowKernelState>(),
            self.app.state::<AppState>(),
            self.app.state::<PermissionState>(),
            self.app.state::<KnowledgeState>(),
            self.app.state::<PluginState>(),
        )
        .await)?;

        let updated = self
            .app
            .state::<TaskModeState>()
            .get_or_load_session_snapshot(&task_session_id)
            .await?
            .ok_or_else(|| "Task session not found".to_string())?;
        session.pending_interaction = None;
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::task_action_card(
            &updated,
            session,
            Some("PRD approved. Task execution has started.".to_string()),
        )))
    }

    async fn handle_debug_text_input(
        &self,
        session: &mut RemoteWorkflowSession,
        content: &str,
    ) -> Result<RemoteUiMessage, String> {
        let debug_session_id = session.linked_mode_sessions.get("debug").cloned();
        let snapshot = if let Some(debug_session_id) = debug_session_id {
            self.expect_ok(submit_debug_clarification(
                self.app.clone(),
                self.app.state::<DebugModeState>(),
                self.app.state::<WorkflowKernelState>(),
                SubmitDebugClarificationRequest {
                    session_id: debug_session_id,
                    answer: content.to_string(),
                    provider: session.provider.clone(),
                    model: session.model.clone(),
                    base_url: session.base_url.clone(),
                    project_path: session.project_path.clone(),
                    context_sources: session.context_sources.clone(),
                    locale: None,
                },
            )
            .await)?
        } else {
            let response = self
                .expect_ok(enter_debug_mode(
                    self.app.clone(),
                    self.app.state::<DebugModeState>(),
                    self.app.state::<WorkflowKernelState>(),
                    self.app.state::<PermissionState>(),
                    EnterDebugModeRequest {
                        description: content.to_string(),
                        environment: None,
                        kernel_session_id: Some(session.kernel_session_id.clone()),
                        provider: session.provider.clone(),
                        model: session.model.clone(),
                        base_url: session.base_url.clone(),
                        project_path: session.project_path.clone(),
                        context_sources: session.context_sources.clone(),
                        locale: None,
                    },
                )
                .await)?;
            session
                .linked_mode_sessions
                .insert("debug".to_string(), response.session_id.clone());
            self.apply_permission_level(&response.session_id, session.permission_level)
                .await;
            response
        };
        session.pending_interaction = if snapshot.state.pending_approval.is_some() {
            Some(RemotePendingInteraction::DebugPatchApproval {
                session_id: snapshot.session_id.clone(),
            })
        } else {
            None
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::debug_action_card(
            &snapshot,
            session,
            None,
        )))
    }

    async fn handle_debug_approve_patch(
        &self,
        session: &mut RemoteWorkflowSession,
    ) -> Result<RemoteUiMessage, String> {
        let debug_session_id = session
            .linked_mode_sessions
            .get("debug")
            .cloned()
            .ok_or_else(|| "No active debug session".to_string())?;
        let updated = self
            .expect_ok(approve_debug_patch(
                self.app.clone(),
                self.app.state::<DebugModeState>(),
                self.app.state::<WorkflowKernelState>(),
                ApproveDebugPatchRequest {
                    session_id: debug_session_id,
                    provider: session.provider.clone(),
                    model: session.model.clone(),
                    base_url: session.base_url.clone(),
                    project_path: session.project_path.clone(),
                    context_sources: session.context_sources.clone(),
                    locale: None,
                },
            )
            .await)?;
        session.pending_interaction = if updated.state.pending_approval.is_some() {
            Some(RemotePendingInteraction::DebugPatchApproval {
                session_id: updated.session_id.clone(),
            })
        } else {
            None
        };
        session.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(RemoteUiMessage::ActionCard(Self::debug_action_card(
            &updated,
            session,
            Some("Debug patch approval submitted.".to_string()),
        )))
    }

    async fn ensure_root_session(&self, session: &mut RemoteWorkflowSession) -> Result<(), String> {
        if !session.kernel_session_id.trim().is_empty() {
            return Ok(());
        }
        let initial_context = session.project_path.as_ref().map(|path| {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "workspacePath".to_string(),
                serde_json::Value::String(path.clone()),
            );
            if let Some(label) = session
                .workspace_label
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
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
        let kernel_state = self.kernel_state();
        let root = kernel_state
            .open_session(Some(WorkflowMode::Chat), initial_context)
            .await?;
        if let Some(label) = session
            .workspace_label
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            let _ = kernel_state.rename_session(&root.session_id, label).await;
        }
        session.kernel_session_id = root.session_id;
        session.updated_at = chrono::Utc::now().to_rfc3339();
        self.apply_permission_level(&session.kernel_session_id, session.permission_level)
            .await;
        Ok(())
    }

    async fn apply_permission_level(&self, session_id: &str, level: PermissionLevel) {
        self.app
            .state::<PermissionState>()
            .gate
            .set_session_level(session_id, level)
            .await;
    }

    fn kernel_state(&self) -> tauri::State<'_, WorkflowKernelState> {
        self.app.state::<WorkflowKernelState>()
    }

    fn expect_ok<T>(&self, result: Result<CommandResponse<T>, String>) -> Result<T, String> {
        let response = result?;
        if response.success {
            response
                .data
                .ok_or_else(|| "Command completed without data".to_string())
        } else {
            Err(response
                .error
                .unwrap_or_else(|| "Command failed".to_string()))
        }
    }

    fn plan_action_card(
        session: &PlanModeSession,
        remote_session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let mut lines: Vec<String> = Vec::new();
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.push(prefix);
        }
        lines.push(format!("Description: {}", session.description));
        lines.push(format!("Phase: {}", session.phase));
        if let Some(question) = session.current_question.as_ref() {
            lines.push(String::new());
            lines.push(format!("Clarification: {}", question.question));
            if let Some(hint) = question.hint.as_ref().filter(|value| !value.trim().is_empty()) {
                lines.push(format!("Hint: {}", hint));
            }
            lines.push("Reply directly in Telegram to answer.".to_string());
        }
        if let Some(plan) = session.plan.as_ref() {
            lines.push(String::new());
            lines.push(format!(
                "Plan: {} steps across {} batches",
                plan.steps.len(),
                plan.batches.len()
            ));
            if let Some(step) = plan.steps.first() {
                lines.push(format!("First step: {}", step.title));
            }
        }
        let actions = if session.current_question.is_some() {
            vec![Self::button("remote:status", "Status")]
        } else if session.plan.is_some() && session.phase == PlanModePhase::ReviewingPlan {
            vec![
                Self::button("remote:plan:approve", "Approve Plan"),
                Self::button("remote:status", "Status"),
            ]
        } else if session.plan.is_none()
            && matches!(
                session.phase,
                PlanModePhase::Analyzing | PlanModePhase::Clarifying | PlanModePhase::Planning
            )
        {
            vec![
                Self::button("remote:plan:generate", "Generate Plan"),
                Self::button("remote:status", "Status"),
            ]
        } else {
            vec![Self::button("remote:status", "Status")]
        };
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("mode".to_string(), format!("{:?}", remote_session.active_mode));
        metadata.insert("sessionId".to_string(), session.session_id.clone());
        RemoteActionCard {
            title: "Plan Mode".to_string(),
            body: lines.join("\n"),
            actions,
            metadata,
            attachment_refs: Vec::new(),
        }
    }

    fn task_action_card(
        session: &TaskModeSession,
        remote_session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let mut lines: Vec<String> = Vec::new();
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.push(prefix);
        }
        lines.push(format!("Description: {}", session.description));
        lines.push(format!("Status: {:?}", session.status));
        if let Some(recommendation) = session.strategy_recommendation.as_ref() {
            lines.push(format!(
                "Recommended mode: {:?}, stories: {}, risk: {:?}",
                recommendation.analysis.recommended_mode,
                recommendation.analysis.estimated_stories,
                recommendation.analysis.risk_level
            ));
            lines.push(format!(
                "Suggested workflow: flow={}, tdd={}, maxParallel={}",
                recommendation.recommended_config.flow_level,
                recommendation.recommended_config.tdd_mode,
                recommendation.recommended_config.max_parallel
            ));
        }
        if let Some(prd) = session.prd.as_ref() {
            lines.push(String::new());
            lines.push(format!(
                "PRD: {} stories across {} batches",
                prd.stories.len(),
                prd.batches.len()
            ));
            if let Some(story) = prd.stories.first() {
                lines.push(format!("First story: {}", story.title));
            }
        }
        if let Some(progress) = session.progress.as_ref() {
            lines.push(String::new());
            lines.push(format!(
                "Execution: batch {}/{} | stories {}/{} | failed {} | phase {}",
                progress.current_batch.saturating_add(1),
                progress.total_batches,
                progress.stories_completed,
                progress.total_stories,
                progress.stories_failed,
                progress.current_phase
            ));
        }
        let actions = match session.status {
            TaskModeStatus::Initialized
                if session.config_confirmation_state == TaskConfigConfirmationState::Pending =>
            {
                vec![
                    Self::button("remote:task:confirm-config", "Confirm Config"),
                    Self::button("remote:artifacts", "Artifacts"),
                    Self::button("remote:status", "Status"),
                ]
            }
            TaskModeStatus::Initialized if session.confirmed_config.is_some() => vec![
                Self::button("remote:task:generate-prd", "Generate PRD"),
                Self::button("remote:artifacts", "Artifacts"),
                Self::button("remote:status", "Status"),
            ],
            TaskModeStatus::ReviewingPrd if session.prd.is_some() => vec![
                Self::button("remote:task:approve-prd", "Approve PRD"),
                Self::button("remote:artifacts", "Artifacts"),
                Self::button("remote:status", "Status"),
            ],
            TaskModeStatus::Executing
            | TaskModeStatus::Completed
            | TaskModeStatus::Failed
            | TaskModeStatus::Cancelled => vec![
                Self::button("remote:artifacts", "Artifacts"),
                Self::button("remote:status", "Status"),
            ],
            _ => vec![Self::button("remote:status", "Status")],
        };
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("mode".to_string(), format!("{:?}", remote_session.active_mode));
        metadata.insert("sessionId".to_string(), session.session_id.clone());
        RemoteActionCard {
            title: "Task Mode".to_string(),
            body: lines.join("\n"),
            actions,
            metadata,
            attachment_refs: Vec::new(),
        }
    }

    fn debug_action_card(
        session: &DebugModeSession,
        remote_session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let mut lines: Vec<String> = Vec::new();
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.push(prefix);
        }
        lines.push(format!(
            "Case: {}",
            session
                .state
                .title
                .as_deref()
                .unwrap_or(session.state.symptom_summary.as_str())
        ));
        lines.push(format!("Phase: {}", session.state.phase));
        lines.push(format!("Environment: {:?}", session.state.environment));
        if let Some(prompt) = session
            .state
            .pending_prompt
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(String::new());
            lines.push(prompt.clone());
        }
        if let Some(root_cause) = session.state.selected_root_cause.as_ref() {
            lines.push(String::new());
            lines.push(format!("Root cause: {}", root_cause.conclusion));
        }
        if let Some(approval) = session.state.pending_approval.as_ref() {
            lines.push(String::new());
            lines.push(format!("Pending approval: {}", approval.title));
            lines.push(approval.description.clone());
        }
        if let Some(report) = session.state.verification_report.as_ref() {
            lines.push(String::new());
            lines.push(format!("Verification: {}", report.summary));
        }
        let actions = if session.state.pending_approval.is_some() {
            vec![
                Self::button("remote:debug:approve-patch", "Approve Patch"),
                Self::button("remote:artifacts", "Artifacts"),
            ]
        } else {
            vec![
                Self::button("remote:artifacts", "Artifacts"),
                Self::button("remote:status", "Status"),
            ]
        };
        let mut attachments: Vec<RemoteAttachmentRef> = Vec::new();
        if let Some(path) = session
            .state
            .fix_proposal
            .as_ref()
            .and_then(|proposal| proposal.patch_preview_ref.as_ref())
        {
            attachments.push(RemoteAttachmentRef {
                label: "Patch Preview".to_string(),
                path: path.clone(),
            });
        }
        if let Some(report) = session.state.verification_report.as_ref() {
            for path in &report.artifacts {
                attachments.push(RemoteAttachmentRef {
                    label: std::path::Path::new(path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("Artifact")
                        .to_string(),
                    path: path.clone(),
                });
            }
        }
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("mode".to_string(), format!("{:?}", remote_session.active_mode));
        metadata.insert("sessionId".to_string(), session.session_id.clone());
        RemoteActionCard {
            title: "Debug Mode".to_string(),
            body: lines.join("\n"),
            actions,
            metadata,
            attachment_refs: attachments,
        }
    }

    fn context_action_card(
        session: &RemoteWorkflowSession,
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let mut lines: Vec<String> = Vec::new();
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.push(prefix);
            lines.push(String::new());
        }
        let summary = session
            .context_sources
            .as_ref()
            .map(Self::summarize_context)
            .unwrap_or_else(|| "Default context preset".to_string());
        lines.push(summary);
        RemoteActionCard {
            title: "Context Sources".to_string(),
            body: lines.join("\n"),
            actions: vec![
                Self::button("remote:context:preset:default", "Default"),
                Self::button("remote:context:preset:focused", "Focused"),
                Self::button("remote:context:preset:knowledge", "Knowledge"),
                Self::button("remote:context:preset:memory", "Memory"),
                Self::button("remote:context:preset:skills", "Skills"),
                Self::button("remote:context:toggle:knowledge", "Toggle Knowledge"),
                Self::button("remote:context:toggle:memory", "Toggle Memory"),
                Self::button("remote:context:toggle:skills", "Toggle Skills"),
            ],
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn permission_action_card(
        session: &RemoteWorkflowSession,
        pending_requests: &[PendingPermissionRequestSnapshot],
        prefix: Option<String>,
    ) -> RemoteActionCard {
        let mut lines: Vec<String> = Vec::new();
        if let Some(prefix) = prefix.filter(|value| !value.trim().is_empty()) {
            lines.push(prefix);
            lines.push(String::new());
        }
        lines.push(format!("Current permission level: {:?}", session.permission_level));
        if pending_requests.is_empty() {
            lines.push("Pending approvals: none".to_string());
        } else {
            lines.push(format!("Pending approvals: {}", pending_requests.len()));
            for request in pending_requests.iter().take(3) {
                lines.push(format!(
                    "- [{}] {} ({})",
                    request.risk, request.tool_name, request.request_id
                ));
            }
        }
        let mut actions = vec![
            Self::button("remote:permission:set:strict", "Strict"),
            Self::button("remote:permission:set:standard", "Standard"),
            Self::button("remote:permission:set:permissive", "Permissive"),
        ];
        if let Some(request) = pending_requests.first() {
            actions.push(Self::button(
                &format!("remote:approval:allow-once:{}", request.request_id),
                "Approve Once",
            ));
            actions.push(Self::button(
                &format!("remote:approval:always-allow:{}", request.request_id),
                "Always Allow",
            ));
            actions.push(Self::button(
                &format!("remote:approval:deny:{}", request.request_id),
                "Deny",
            ));
        }
        RemoteActionCard {
            title: "Permission Control".to_string(),
            body: lines.join("\n"),
            actions,
            metadata: HashMap::new(),
            attachment_refs: Vec::new(),
        }
    }

    fn build_context_preset(project_id: String, preset: &str) -> ContextSourceConfig {
        let mut config = ContextSourceConfig {
            project_id,
            knowledge: Some(KnowledgeSourceConfig {
                enabled: false,
                selected_collections: Vec::new(),
                selected_documents: Vec::new(),
            }),
            memory: Some(MemorySourceConfig {
                enabled: false,
                selected_categories: Vec::new(),
                selected_memory_ids: Vec::new(),
                excluded_memory_ids: Vec::new(),
                selected_scopes: vec!["project".to_string(), "global".to_string()],
                session_id: None,
                statuses: Vec::new(),
                review_mode: None,
                selection_mode: None,
            }),
            skills: Some(SkillsSourceConfig {
                enabled: false,
                selected_skill_ids: Vec::new(),
                invoked_skill_ids: Vec::new(),
                selection_mode: Default::default(),
                review_filter: None,
            }),
        };
        match preset {
            "focused" => {}
            "knowledge" => {
                if let Some(knowledge) = config.knowledge.as_mut() {
                    knowledge.enabled = true;
                }
            }
            "memory" => {
                if let Some(memory) = config.memory.as_mut() {
                    memory.enabled = true;
                }
            }
            "skills" => {
                if let Some(skills) = config.skills.as_mut() {
                    skills.enabled = true;
                }
            }
            _ => {
                if let Some(knowledge) = config.knowledge.as_mut() {
                    knowledge.enabled = true;
                }
                if let Some(memory) = config.memory.as_mut() {
                    memory.enabled = true;
                    memory.session_id = None;
                }
                if let Some(skills) = config.skills.as_mut() {
                    skills.enabled = true;
                }
            }
        }
        config
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

    fn recommended_task_config(recommended: &RecommendedWorkflowConfig) -> TaskWorkflowConfig {
        TaskWorkflowConfig {
            flow_level: Some(recommended.flow_level.clone()),
            tdd_mode: Some(recommended.tdd_mode.clone()),
            enable_interview: recommended.spec_interview_enabled,
            quality_gates_enabled: recommended.quality_gates_enabled,
            max_parallel: Some(recommended.max_parallel),
            skip_verification: recommended.skip_verification,
            skip_review: recommended.skip_review,
            global_agent_override: recommended.global_agent_override.clone(),
            impl_agent_override: recommended.impl_agent_override.clone(),
        }
    }

    fn button(id: &str, label: &str) -> RemoteActionButton {
        RemoteActionButton {
            id: id.to_string(),
            label: label.to_string(),
            style: None,
        }
    }
}
