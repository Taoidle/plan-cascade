//! Unified Workflow Kernel
//!
//! Provides a typed, recoverable workflow session core for Chat/Plan/Task.
//! The kernel owns:
//! - unified session lifecycle
//! - typed event stream v2
//! - mode handoff and context bundle merging
//! - lightweight checkpointing and recovery

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::utils::paths::ensure_plan_cascade_dir;

const MAX_HANDOFF_TURNS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    Chat,
    Plan,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurn {
    pub user: String,
    pub assistant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffContextBundle {
    #[serde(default)]
    pub conversation_context: Vec<ConversationTurn>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub context_sources: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,
}

impl Default for HandoffContextBundle {
    fn default() -> Self {
        Self {
            conversation_context: Vec::new(),
            artifact_refs: Vec::new(),
            context_sources: Vec::new(),
            metadata: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatState {
    pub phase: String,
    pub draft_input: String,
    pub turn_count: u64,
    pub last_user_message: Option<String>,
    pub last_assistant_message: Option<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            phase: "ready".to_string(),
            draft_input: String::new(),
            turn_count: 0,
            last_user_message: None,
            last_assistant_message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanState {
    pub phase: String,
    pub plan_id: Option<String>,
    pub running_step_id: Option<String>,
    pub pending_question: Option<String>,
    pub retryable_steps: Vec<String>,
    pub plan_revision: u64,
    pub last_edit_operation: Option<String>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            plan_id: None,
            running_step_id: None,
            pending_question: None,
            retryable_steps: Vec::new(),
            plan_revision: 0,
            last_edit_operation: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskState {
    pub phase: String,
    pub prd_id: Option<String>,
    pub current_story_id: Option<String>,
    pub completed_stories: u64,
    pub failed_stories: u64,
}

impl Default for TaskState {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            prd_id: None,
            current_story_id: None,
            completed_stories: 0,
            failed_stories: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "state")]
pub enum ModeState {
    Chat(ChatState),
    Plan(PlanState),
    Task(TaskState),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSnapshots {
    pub chat: Option<ChatState>,
    pub plan: Option<PlanState>,
    pub task: Option<TaskState>,
}

impl ModeSnapshots {
    fn ensure_mode(&mut self, mode: WorkflowMode) {
        match mode {
            WorkflowMode::Chat => {
                if self.chat.is_none() {
                    self.chat = Some(ChatState::default());
                }
            }
            WorkflowMode::Plan => {
                if self.plan.is_none() {
                    self.plan = Some(PlanState::default());
                }
            }
            WorkflowMode::Task => {
                if self.task.is_none() {
                    self.task = Some(TaskState::default());
                }
            }
        }
    }

    fn plan_mut(&mut self) -> &mut PlanState {
        if self.plan.is_none() {
            self.plan = Some(PlanState::default());
        }
        self.plan
            .as_mut()
            .expect("plan snapshot must exist after initialization")
    }

    fn chat_mut(&mut self) -> &mut ChatState {
        if self.chat.is_none() {
            self.chat = Some(ChatState::default());
        }
        self.chat
            .as_mut()
            .expect("chat snapshot must exist after initialization")
    }

    fn task_mut(&mut self) -> &mut TaskState {
        if self.task.is_none() {
            self.task = Some(TaskState::default());
        }
        self.task
            .as_mut()
            .expect("task snapshot must exist after initialization")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventKind {
    SessionOpened,
    ModeTransitioned,
    InputSubmitted,
    ContextAppended,
    PlanEdited,
    PlanExecutionStarted,
    PlanStepRetried,
    OperationCancelled,
    SessionRecovered,
    CheckpointCreated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEventV2 {
    pub event_id: String,
    pub session_id: String,
    pub kind: WorkflowEventKind,
    pub mode: WorkflowMode,
    pub created_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserInputIntentType {
    ChatMessage,
    ModeEntryPrompt,
    PlanClarification,
    PlanEditInstruction,
    PlanApproval,
    TaskConfiguration,
    TaskInterviewAnswer,
    TaskPrdFeedback,
    ExecutionControl,
    SystemPhaseUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInputIntent {
    #[serde(rename = "type")]
    pub intent_type: UserInputIntentType,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanEditOperationType {
    AddStep,
    UpdateStep,
    RemoveStep,
    ReorderStep,
    SetDependency,
    ClearDependency,
    SetParallelism,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEditOperation {
    #[serde(rename = "type")]
    pub operation_type: PlanEditOperationType,
    pub target_step_id: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSession {
    pub session_id: String,
    pub status: WorkflowStatus,
    pub active_mode: WorkflowMode,
    pub mode_snapshots: ModeSnapshots,
    pub handoff_context: HandoffContextBundle,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_checkpoint_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCheckpoint {
    pub checkpoint_id: String,
    pub session_id: String,
    pub created_at: String,
    pub reason: String,
    #[serde(default = "default_reason_code")]
    pub reason_code: String,
    pub event_count: usize,
    pub snapshot: WorkflowSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSessionState {
    pub session: WorkflowSession,
    pub events: Vec<WorkflowEventV2>,
    pub checkpoints: Vec<WorkflowCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSessionRecord {
    session: WorkflowSession,
    events: Vec<WorkflowEventV2>,
    checkpoints: Vec<WorkflowCheckpoint>,
}

/// In-memory kernel state with persisted event/checkpoint record per session.
pub struct WorkflowKernelState {
    sessions: Arc<RwLock<HashMap<String, WorkflowSession>>>,
    events: Arc<RwLock<HashMap<String, Vec<WorkflowEventV2>>>>,
    checkpoints: Arc<RwLock<HashMap<String, Vec<WorkflowCheckpoint>>>>,
    storage_root: Arc<PathBuf>,
}

impl WorkflowKernelState {
    pub fn new() -> Self {
        Self::new_with_storage_dir(resolve_storage_root())
    }

    pub fn new_with_storage_dir(storage_root: PathBuf) -> Self {
        let sessions_dir = storage_root.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            storage_root: Arc::new(storage_root),
        }
    }

    pub async fn open_session(
        &self,
        initial_mode: Option<WorkflowMode>,
        initial_context: Option<HandoffContextBundle>,
    ) -> Result<WorkflowSession, String> {
        let active_mode = initial_mode.unwrap_or(WorkflowMode::Chat);
        let now = now_rfc3339();
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut mode_snapshots = ModeSnapshots::default();
        mode_snapshots.ensure_mode(active_mode);

        let session = WorkflowSession {
            session_id: session_id.clone(),
            status: WorkflowStatus::Active,
            active_mode,
            mode_snapshots,
            handoff_context: initial_context.unwrap_or_default(),
            last_error: None,
            created_at: now.clone(),
            updated_at: now,
            last_checkpoint_id: None,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }

        self.append_event(
            &session_id,
            WorkflowEventKind::SessionOpened,
            active_mode,
            json!({
                "initialMode": active_mode,
            }),
        )
        .await;
        self.create_checkpoint(&session_id, "session_opened")
            .await?;
        self.persist_session_record(&session_id).await?;

        Ok(session)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<WorkflowSession, String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Workflow session not found: {session_id}"))
    }

    pub async fn get_session_state(
        &self,
        session_id: &str,
    ) -> Result<WorkflowSessionState, String> {
        let session = self.get_session(session_id).await?;
        let events = {
            let map = self.events.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };
        let checkpoints = {
            let map = self.checkpoints.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };
        Ok(WorkflowSessionState {
            session,
            events,
            checkpoints,
        })
    }

    pub async fn transition_mode(
        &self,
        session_id: &str,
        target_mode: WorkflowMode,
        handoff: Option<HandoffContextBundle>,
    ) -> Result<WorkflowSession, String> {
        let (source_mode, updated_session) = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            let source_mode = session.active_mode;
            session.mode_snapshots.ensure_mode(source_mode);
            session.mode_snapshots.ensure_mode(target_mode);
            if let Some(incoming) = handoff {
                merge_handoff_bundle(&mut session.handoff_context, incoming);
            }
            session.active_mode = target_mode;
            session.status = WorkflowStatus::Active;
            session.last_error = None;
            session.updated_at = now_rfc3339();

            (source_mode, session.clone())
        };

        self.append_event(
            session_id,
            WorkflowEventKind::ModeTransitioned,
            target_mode,
            json!({
                "sourceMode": source_mode,
                "targetMode": target_mode,
                "conversationTurns": updated_session.handoff_context.conversation_context.len(),
                "artifactRefs": updated_session.handoff_context.artifact_refs.len(),
                "contextSources": updated_session.handoff_context.context_sources.len()
            }),
        )
        .await;
        self.create_checkpoint(session_id, "mode_transitioned")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn submit_input(
        &self,
        session_id: &str,
        intent: UserInputIntent,
    ) -> Result<WorkflowSession, String> {
        if !intent_has_valid_content(&intent) {
            return Err("Input content cannot be empty".to_string());
        }

        let (active_mode, updated_session) = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            let active_mode = session.active_mode;
            apply_intent_to_mode_state(session, &intent);
            session.updated_at = now_rfc3339();
            (active_mode, session.clone())
        };
        let event_mode = resolve_intent_event_mode(active_mode, &intent);

        self.append_event(
            session_id,
            WorkflowEventKind::InputSubmitted,
            event_mode,
            serde_json::to_value(&intent).unwrap_or_else(|_| {
                json!({
                    "type": "serialization_failed",
                    "content": intent.content
                })
            }),
        )
        .await;
        self.create_checkpoint(session_id, "input_submitted")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn transition_and_submit_input(
        &self,
        session_id: &str,
        target_mode: WorkflowMode,
        handoff: Option<HandoffContextBundle>,
        intent: UserInputIntent,
    ) -> Result<WorkflowSession, String> {
        if !intent_has_valid_content(&intent) {
            return Err("Input content cannot be empty".to_string());
        }

        let (source_mode, updated_session) = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            let source_mode = session.active_mode;
            session.mode_snapshots.ensure_mode(source_mode);
            session.mode_snapshots.ensure_mode(target_mode);
            if let Some(incoming) = handoff {
                merge_handoff_bundle(&mut session.handoff_context, incoming);
            }

            session.active_mode = target_mode;
            session.status = WorkflowStatus::Active;
            session.last_error = None;
            apply_intent_to_mode_state(session, &intent);
            session.updated_at = now_rfc3339();

            (source_mode, session.clone())
        };

        self.append_event(
            session_id,
            WorkflowEventKind::ModeTransitioned,
            target_mode,
            json!({
                "sourceMode": source_mode,
                "targetMode": target_mode,
                "conversationTurns": updated_session.handoff_context.conversation_context.len(),
                "artifactRefs": updated_session.handoff_context.artifact_refs.len(),
                "contextSources": updated_session.handoff_context.context_sources.len(),
                "atomicSubmit": true
            }),
        )
        .await;
        let input_event_mode = resolve_intent_event_mode(target_mode, &intent);
        self.append_event(
            session_id,
            WorkflowEventKind::InputSubmitted,
            input_event_mode,
            serde_json::to_value(&intent).unwrap_or_else(|_| {
                json!({
                    "type": "serialization_failed",
                    "content": intent.content
                })
            }),
        )
        .await;
        self.create_checkpoint(session_id, "mode_transitioned_with_input")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn append_context_items(
        &self,
        session_id: &str,
        target_mode: WorkflowMode,
        handoff: HandoffContextBundle,
    ) -> Result<WorkflowSession, String> {
        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            session.mode_snapshots.ensure_mode(target_mode);
            merge_handoff_bundle(&mut session.handoff_context, handoff);
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::ContextAppended,
            target_mode,
            json!({
                "targetMode": target_mode,
                "conversationTurns": updated_session.handoff_context.conversation_context.len(),
                "artifactRefs": updated_session.handoff_context.artifact_refs.len(),
                "contextSources": updated_session.handoff_context.context_sources.len()
            }),
        )
        .await;
        self.create_checkpoint(session_id, "context_appended")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn apply_plan_edit(
        &self,
        session_id: &str,
        operation: PlanEditOperation,
    ) -> Result<WorkflowSession, String> {
        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
            session.active_mode = WorkflowMode::Plan;
            let plan = session.mode_snapshots.plan_mut();
            plan.phase = "reviewing_plan".to_string();
            plan.plan_revision = plan.plan_revision.saturating_add(1);
            plan.last_edit_operation = Some(format!("{:?}", operation.operation_type));
            if let Some(step_id) = &operation.target_step_id {
                if !plan.retryable_steps.contains(step_id) {
                    plan.retryable_steps.push(step_id.clone());
                }
            }
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::PlanEdited,
            WorkflowMode::Plan,
            serde_json::to_value(&operation).unwrap_or_else(|_| {
                json!({
                    "type": "serialization_failed"
                })
            }),
        )
        .await;
        self.create_checkpoint(session_id, "plan_edited").await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn execute_plan(&self, session_id: &str) -> Result<WorkflowSession, String> {
        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
            session.active_mode = WorkflowMode::Plan;
            session.status = WorkflowStatus::Active;
            let plan = session.mode_snapshots.plan_mut();
            plan.phase = "executing".to_string();
            if plan.plan_id.is_none() {
                plan.plan_id = Some(uuid::Uuid::new_v4().to_string());
            }
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::PlanExecutionStarted,
            WorkflowMode::Plan,
            json!({
                "planId": updated_session.mode_snapshots.plan.as_ref().and_then(|p| p.plan_id.clone())
            }),
        )
        .await;
        self.create_checkpoint(session_id, "plan_execution_started")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn retry_step(
        &self,
        session_id: &str,
        step_id: &str,
    ) -> Result<WorkflowSession, String> {
        if step_id.trim().is_empty() {
            return Err("Step id cannot be empty".to_string());
        }

        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
            session.active_mode = WorkflowMode::Plan;
            let plan = session.mode_snapshots.plan_mut();
            plan.phase = "executing".to_string();
            plan.running_step_id = Some(step_id.to_string());
            if !plan.retryable_steps.iter().any(|id| id == step_id) {
                plan.retryable_steps.push(step_id.to_string());
            }
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::PlanStepRetried,
            WorkflowMode::Plan,
            json!({
                "stepId": step_id
            }),
        )
        .await;
        self.create_checkpoint(session_id, "plan_step_retried")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn cancel_operation(
        &self,
        session_id: &str,
        reason: Option<String>,
    ) -> Result<WorkflowSession, String> {
        let cancel_reason = reason.unwrap_or_else(|| "cancelled_by_user".to_string());
        let cancel_reason_code = normalize_reason_code(&cancel_reason);
        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            session.status = WorkflowStatus::Cancelled;
            session.last_error = Some(cancel_reason.clone());
            session.updated_at = now_rfc3339();
            match session.active_mode {
                WorkflowMode::Chat => {
                    session.mode_snapshots.chat_mut().phase = "cancelled".to_string()
                }
                WorkflowMode::Plan => {
                    session.mode_snapshots.plan_mut().phase = "cancelled".to_string()
                }
                WorkflowMode::Task => {
                    session.mode_snapshots.task_mut().phase = "cancelled".to_string()
                }
            }
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::OperationCancelled,
            updated_session.active_mode,
            json!({
                "reason": cancel_reason,
                "reasonCode": cancel_reason_code
            }),
        )
        .await;
        self.create_checkpoint(session_id, "operation_cancelled")
            .await?;
        self.persist_session_record(session_id).await?;

        Ok(updated_session)
    }

    pub async fn recover_session(&self, session_id: &str) -> Result<WorkflowSession, String> {
        if let Ok(session) = self.get_session(session_id).await {
            return Ok(session);
        }

        let record = self.read_persisted_record(session_id).await?;
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), record.session.clone());
        }
        {
            let mut events = self.events.write().await;
            events.insert(session_id.to_string(), record.events);
        }
        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints.insert(session_id.to_string(), record.checkpoints);
        }

        self.append_event(
            session_id,
            WorkflowEventKind::SessionRecovered,
            record.session.active_mode,
            json!({
                "fromPersistence": true
            }),
        )
        .await;
        self.create_checkpoint(session_id, "session_recovered")
            .await?;
        self.persist_session_record(session_id).await?;

        self.get_session(session_id).await
    }

    async fn append_event(
        &self,
        session_id: &str,
        kind: WorkflowEventKind,
        mode: WorkflowMode,
        payload: Value,
    ) {
        let event = WorkflowEventV2 {
            event_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            kind,
            mode,
            created_at: now_rfc3339(),
            payload,
        };

        let mut map = self.events.write().await;
        map.entry(session_id.to_string()).or_default().push(event);
    }

    async fn create_checkpoint(
        &self,
        session_id: &str,
        reason: &str,
    ) -> Result<WorkflowCheckpoint, String> {
        let session_snapshot = self.get_session(session_id).await?;
        let event_count = {
            let map = self.events.read().await;
            map.get(session_id).map(|events| events.len()).unwrap_or(0)
        };
        let created_at = now_rfc3339();
        let checkpoint = WorkflowCheckpoint {
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            created_at: created_at.clone(),
            reason: reason.to_string(),
            reason_code: normalize_reason_code(reason),
            event_count,
            snapshot: session_snapshot.clone(),
        };

        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints
                .entry(session_id.to_string())
                .or_default()
                .push(checkpoint.clone());
        }
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.last_checkpoint_id = Some(checkpoint.checkpoint_id.clone());
                session.updated_at = created_at;
            }
        }

        self.append_event(
            session_id,
            WorkflowEventKind::CheckpointCreated,
            session_snapshot.active_mode,
            json!({
                "checkpointId": checkpoint.checkpoint_id,
                "reason": reason,
                "reasonCode": checkpoint.reason_code,
                "eventCount": event_count
            }),
        )
        .await;

        Ok(checkpoint)
    }

    async fn persist_session_record(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id).await?;
        let events = {
            let map = self.events.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };
        let checkpoints = {
            let map = self.checkpoints.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };

        let record = PersistedSessionRecord {
            session,
            events,
            checkpoints,
        };
        let encoded = serde_json::to_vec_pretty(&record)
            .map_err(|e| format!("Failed to serialize workflow session record: {e}"))?;
        let path = self.session_file_path(session_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create workflow session directory: {e}"))?;
        }
        fs::write(path, encoded).map_err(|e| format!("Failed to persist workflow session: {e}"))
    }

    async fn read_persisted_record(
        &self,
        session_id: &str,
    ) -> Result<PersistedSessionRecord, String> {
        let path = self.session_file_path(session_id);
        let content = fs::read_to_string(&path).map_err(|e| {
            format!("Failed to read persisted workflow session '{session_id}': {e}")
        })?;
        let mut record = serde_json::from_str::<PersistedSessionRecord>(&content).map_err(|e| {
            format!("Failed to decode persisted workflow session '{session_id}': {e}")
        })?;

        for checkpoint in &mut record.checkpoints {
            hydrate_checkpoint_reason_code(checkpoint);
        }

        Ok(record)
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.storage_root
            .join("sessions")
            .join(format!("{session_id}.json"))
    }
}

fn apply_intent_to_mode_state(session: &mut WorkflowSession, intent: &UserInputIntent) {
    if intent.intent_type == UserInputIntentType::SystemPhaseUpdate {
        if let Some(phase) = extract_phase_hint(intent) {
            match resolve_intent_mode_hint(intent).unwrap_or(session.active_mode) {
                WorkflowMode::Chat => session.mode_snapshots.chat_mut().phase = phase,
                WorkflowMode::Plan => session.mode_snapshots.plan_mut().phase = phase,
                WorkflowMode::Task => session.mode_snapshots.task_mut().phase = phase,
            }
        }
        return;
    }

    match session.active_mode {
        WorkflowMode::Chat => {
            let chat = session.mode_snapshots.chat_mut();
            chat.turn_count = chat.turn_count.saturating_add(1);
            chat.last_user_message = Some(intent.content.clone());
            chat.phase = "ready".to_string();
            chat.draft_input.clear();
        }
        WorkflowMode::Plan => {
            let plan = session.mode_snapshots.plan_mut();
            match intent.intent_type {
                UserInputIntentType::PlanClarification => {
                    plan.phase = "planning".to_string();
                    plan.pending_question = None;
                }
                UserInputIntentType::PlanApproval => {
                    plan.phase = "executing".to_string();
                }
                UserInputIntentType::PlanEditInstruction => {
                    plan.phase = "reviewing_plan".to_string();
                }
                UserInputIntentType::ModeEntryPrompt => {
                    plan.phase = "analyzing".to_string();
                }
                _ => {}
            }
        }
        WorkflowMode::Task => {
            let task = session.mode_snapshots.task_mut();
            match intent.intent_type {
                UserInputIntentType::TaskConfiguration => {
                    task.phase = "configuring".to_string();
                }
                UserInputIntentType::TaskInterviewAnswer => {
                    task.phase = "interviewing".to_string();
                }
                UserInputIntentType::TaskPrdFeedback => {
                    task.phase = "reviewing_prd".to_string();
                }
                UserInputIntentType::ExecutionControl => {
                    task.phase = "executing".to_string();
                }
                UserInputIntentType::ModeEntryPrompt => {
                    task.phase = "analyzing".to_string();
                }
                _ => {}
            }
        }
    }
}

fn extract_phase_hint(intent: &UserInputIntent) -> Option<String> {
    if let Some(phase) = intent
        .metadata
        .as_object()
        .and_then(|obj| obj.get("phase"))
        .and_then(|value| value.as_str())
    {
        let normalized = phase.trim();
        if !normalized.is_empty() {
            return Some(normalized.to_string());
        }
    }

    let content = intent.content.trim();
    if content.is_empty() {
        return None;
    }

    if let Some(rest) = content.strip_prefix("phase:") {
        let normalized = rest.trim();
        if !normalized.is_empty() {
            return Some(normalized.to_string());
        }
    }

    None
}

fn resolve_intent_event_mode(active_mode: WorkflowMode, intent: &UserInputIntent) -> WorkflowMode {
    if intent.intent_type == UserInputIntentType::SystemPhaseUpdate {
        return resolve_intent_mode_hint(intent).unwrap_or(active_mode);
    }
    active_mode
}

fn resolve_intent_mode_hint(intent: &UserInputIntent) -> Option<WorkflowMode> {
    let mode_hint = intent
        .metadata
        .as_object()
        .and_then(|obj| obj.get("mode"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .and_then(parse_workflow_mode);
    if mode_hint.is_some() {
        return mode_hint;
    }

    let content = intent.content.trim();
    if let Some(rest) = content.strip_prefix("mode:") {
        return parse_workflow_mode(rest.trim());
    }
    None
}

fn parse_workflow_mode(value: &str) -> Option<WorkflowMode> {
    match value {
        "chat" => Some(WorkflowMode::Chat),
        "plan" => Some(WorkflowMode::Plan),
        "task" => Some(WorkflowMode::Task),
        _ => None,
    }
}

fn intent_has_valid_content(intent: &UserInputIntent) -> bool {
    if !intent.content.trim().is_empty() {
        return true;
    }
    if intent.intent_type == UserInputIntentType::SystemPhaseUpdate {
        return extract_phase_hint(intent).is_some();
    }
    false
}

fn merge_handoff_bundle(current: &mut HandoffContextBundle, incoming: HandoffContextBundle) {
    current
        .conversation_context
        .extend(incoming.conversation_context);
    if current.conversation_context.len() > MAX_HANDOFF_TURNS {
        let remove_count = current.conversation_context.len() - MAX_HANDOFF_TURNS;
        current.conversation_context.drain(0..remove_count);
    }

    current.artifact_refs.extend(incoming.artifact_refs);
    dedupe_strings(&mut current.artifact_refs);

    current.context_sources.extend(incoming.context_sources);
    dedupe_strings(&mut current.context_sources);

    for (key, value) in incoming.metadata {
        current.metadata.insert(key, value);
    }
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut deduped: Vec<String> = Vec::with_capacity(values.len());
    for value in values.iter() {
        if !deduped.iter().any(|existing| existing == value) {
            deduped.push(value.clone());
        }
    }
    *values = deduped;
}

fn default_reason_code() -> String {
    "unknown_reason".to_string()
}

fn hydrate_checkpoint_reason_code(checkpoint: &mut WorkflowCheckpoint) {
    let normalized_reason_code = normalize_reason_code(&checkpoint.reason_code);
    if normalized_reason_code != "unknown_reason" {
        checkpoint.reason_code = normalized_reason_code;
        return;
    }

    checkpoint.reason_code = normalize_reason_code(&checkpoint.reason);
}

fn normalize_reason_code(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut prev_underscore = false;

    for ch in value.trim().chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            prev_underscore = false;
        } else if !prev_underscore {
            normalized.push('_');
            prev_underscore = true;
        }
    }

    let normalized = normalized.trim_matches('_').to_string();
    if normalized.is_empty() {
        "unknown_reason".to_string()
    } else {
        normalized
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn resolve_storage_root() -> PathBuf {
    if let Ok(root) = ensure_plan_cascade_dir() {
        let path = root.join("workflow-kernel");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    let fallback = std::env::temp_dir().join("plan-cascade-workflow-kernel");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn workflow_session_roundtrip_recovery() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        kernel
            .submit_input(
                &session_id,
                UserInputIntent {
                    intent_type: UserInputIntentType::ChatMessage,
                    content: "hello".to_string(),
                    metadata: Value::Null,
                },
            )
            .await
            .expect("submit input");

        kernel
            .transition_mode(
                &session_id,
                WorkflowMode::Plan,
                Some(HandoffContextBundle {
                    conversation_context: vec![ConversationTurn {
                        user: "hello".to_string(),
                        assistant: "hi".to_string(),
                    }],
                    artifact_refs: vec!["artifact.md".to_string()],
                    context_sources: vec!["chat_history".to_string()],
                    metadata: serde_json::Map::new(),
                }),
            )
            .await
            .expect("transition");

        let snapshot = kernel
            .get_session_state(&session_id)
            .await
            .expect("get session state");
        assert_eq!(snapshot.session.active_mode, WorkflowMode::Plan);
        assert!(snapshot.events.len() >= 3);
        assert!(!snapshot.checkpoints.is_empty());
        assert_eq!(
            snapshot.session.handoff_context.conversation_context.len(),
            1
        );

        let recovered_kernel =
            WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let recovered = recovered_kernel
            .recover_session(&session_id)
            .await
            .expect("recover");
        assert_eq!(recovered.active_mode, WorkflowMode::Plan);

        let recovered_state = recovered_kernel
            .get_session_state(&session_id)
            .await
            .expect("recovered state");
        assert!(!recovered_state.events.is_empty());
        assert!(!recovered_state.checkpoints.is_empty());
    }

    #[tokio::test]
    async fn system_phase_update_respects_explicit_mode_hint() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let updated = kernel
            .submit_input(
                &session_id,
                UserInputIntent {
                    intent_type: UserInputIntentType::SystemPhaseUpdate,
                    content: String::new(),
                    metadata: json!({
                        "mode": "plan",
                        "phase": "executing"
                    }),
                },
            )
            .await
            .expect("submit system phase");

        assert_eq!(updated.active_mode, WorkflowMode::Chat);
        assert_eq!(
            updated
                .mode_snapshots
                .plan
                .as_ref()
                .map(|plan| plan.phase.as_str()),
            Some("executing")
        );
        assert_eq!(
            updated
                .mode_snapshots
                .chat
                .as_ref()
                .map(|chat| chat.phase.as_str()),
            Some("ready")
        );
    }

    #[tokio::test]
    async fn transition_and_submit_input_is_atomic() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let updated = kernel
            .transition_and_submit_input(
                &session_id,
                WorkflowMode::Plan,
                None,
                UserInputIntent {
                    intent_type: UserInputIntentType::ModeEntryPrompt,
                    content: "Ship workflow v2".to_string(),
                    metadata: Value::Null,
                },
            )
            .await
            .expect("atomic transition+submit");

        assert_eq!(updated.active_mode, WorkflowMode::Plan);
        assert_eq!(
            updated
                .mode_snapshots
                .plan
                .as_ref()
                .map(|plan| plan.phase.as_str()),
            Some("analyzing")
        );

        let session_state = kernel
            .get_session_state(&session_id)
            .await
            .expect("session state");
        assert!(session_state.events.len() >= 3);
        let transition_index = session_state
            .events
            .iter()
            .rposition(|event| event.kind == WorkflowEventKind::ModeTransitioned)
            .expect("mode transition event");
        let input_index = session_state
            .events
            .iter()
            .rposition(|event| event.kind == WorkflowEventKind::InputSubmitted)
            .expect("input submitted event");
        assert!(transition_index < input_index);
    }

    #[tokio::test]
    async fn cancel_operation_emits_localizable_reason_code() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Task), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        kernel
            .cancel_operation(&session_id, Some("Cancelled By User".to_string()))
            .await
            .expect("cancel operation");

        let state = kernel
            .get_session_state(&session_id)
            .await
            .expect("session state");

        let cancel_event = state
            .events
            .iter()
            .rev()
            .find(|event| event.kind == WorkflowEventKind::OperationCancelled)
            .expect("operation_cancelled event");
        let reason_code = cancel_event
            .payload
            .get("reasonCode")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert_eq!(reason_code, "cancelled_by_user");

        let checkpoint_event = state
            .events
            .iter()
            .rev()
            .find(|event| event.kind == WorkflowEventKind::CheckpointCreated)
            .expect("checkpoint event");
        let checkpoint_reason_code = checkpoint_event
            .payload
            .get("reasonCode")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert_eq!(checkpoint_reason_code, "operation_cancelled");

        let checkpoint = state.checkpoints.last().expect("checkpoint snapshot");
        assert_eq!(checkpoint.reason_code, "operation_cancelled");
    }
}
