//! Unified Workflow Kernel
//!
//! Provides a typed, recoverable workflow session core for Chat/Plan/Task.
//! The kernel owns:
//! - unified session lifecycle
//! - typed event stream v2
//! - mode handoff and context bundle merging
//! - lightweight checkpointing and recovery

pub mod observability;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::utils::paths::ensure_plan_cascade_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSessionKind {
    SimpleRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBackgroundState {
    Foreground,
    BackgroundIdle,
    BackgroundRunning,
    Interrupted,
}

impl Default for WorkflowBackgroundState {
    fn default() -> Self {
        Self::Foreground
    }
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
pub struct PlanClarificationSnapshot {
    pub question_id: String,
    pub question: String,
    pub hint: Option<String>,
    pub input_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_true")]
    pub allow_custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanState {
    pub phase: String,
    pub plan_id: Option<String>,
    pub running_step_id: Option<String>,
    #[serde(default)]
    pub pending_clarification: Option<PlanClarificationSnapshot>,
    pub retryable_steps: Vec<String>,
    pub plan_revision: u64,
    pub last_edit_operation: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub background_status: Option<String>,
    #[serde(default)]
    pub resumable_from_checkpoint: bool,
    #[serde(default)]
    pub last_checkpoint_id: Option<String>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            plan_id: None,
            running_step_id: None,
            pending_clarification: None,
            retryable_steps: Vec::new(),
            plan_revision: 0,
            last_edit_operation: None,
            run_id: None,
            background_status: None,
            resumable_from_checkpoint: false,
            last_checkpoint_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInterviewSnapshot {
    pub interview_id: String,
    pub question_id: String,
    pub question: String,
    pub hint: Option<String>,
    pub required: bool,
    pub input_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub allow_custom: bool,
    pub question_number: u32,
    pub total_questions: u32,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskState {
    pub phase: String,
    pub prd_id: Option<String>,
    pub current_story_id: Option<String>,
    pub interview_session_id: Option<String>,
    #[serde(default)]
    pub pending_interview: Option<TaskInterviewSnapshot>,
    pub completed_stories: u64,
    pub failed_stories: u64,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub background_status: Option<String>,
    #[serde(default)]
    pub resumable_from_checkpoint: bool,
    #[serde(default)]
    pub last_checkpoint_id: Option<String>,
}

impl Default for TaskState {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            prd_id: None,
            current_story_id: None,
            interview_session_id: None,
            pending_interview: None,
            completed_stories: 0,
            failed_stories: 0,
            run_id: None,
            background_status: None,
            resumable_from_checkpoint: false,
            last_checkpoint_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowContextLedgerSummary {
    pub conversation_turn_count: usize,
    pub artifact_ref_count: usize,
    #[serde(default)]
    pub context_source_kinds: Vec<String>,
    pub last_compaction_at: Option<String>,
    pub ledger_version: u64,
}

impl Default for WorkflowContextLedgerSummary {
    fn default() -> Self {
        Self {
            conversation_turn_count: 0,
            artifact_ref_count: 0,
            context_source_kinds: Vec::new(),
            last_compaction_at: None,
            ledger_version: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorkflowContextLedgerEntryKind {
    ConversationTurn,
    ArtifactRef,
    ContextSource,
    MetadataPatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowContextLedgerEntry {
    entry_id: String,
    created_at: String,
    mode: Option<WorkflowMode>,
    kind: WorkflowContextLedgerEntryKind,
    value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeRuntimeMeta {
    pub mode: WorkflowMode,
    pub run_id: Option<String>,
    pub binding_session_id: Option<String>,
    pub is_foreground: bool,
    pub is_background_running: bool,
    pub is_interrupted: bool,
    pub resume_policy: String,
    pub last_heartbeat_at: Option<String>,
    pub last_checkpoint_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSessionCatalogItem {
    pub session_id: String,
    pub session_kind: WorkflowSessionKind,
    pub display_title: String,
    pub workspace_path: Option<String>,
    pub active_mode: WorkflowMode,
    pub status: WorkflowStatus,
    pub background_state: WorkflowBackgroundState,
    pub updated_at: String,
    pub created_at: String,
    pub last_error: Option<String>,
    pub context_ledger: WorkflowContextLedgerSummary,
    pub mode_snapshots: ModeSnapshots,
    pub mode_runtime_meta: HashMap<WorkflowMode, ModeRuntimeMeta>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSessionCatalogState {
    pub active_session_id: Option<String>,
    #[serde(default)]
    pub sessions: Vec<WorkflowSessionCatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeResult {
    pub session_id: String,
    pub mode: WorkflowMode,
    pub resumed: bool,
    pub reason: String,
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
    ModeSessionLinked,
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
    FollowUpIntent,
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
    #[serde(default = "default_session_kind")]
    pub session_kind: WorkflowSessionKind,
    #[serde(default = "default_display_title")]
    pub display_title: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    pub status: WorkflowStatus,
    pub active_mode: WorkflowMode,
    pub mode_snapshots: ModeSnapshots,
    pub handoff_context: HandoffContextBundle,
    #[serde(default)]
    pub linked_mode_sessions: HashMap<WorkflowMode, String>,
    #[serde(default)]
    pub background_state: WorkflowBackgroundState,
    #[serde(default)]
    pub context_ledger: WorkflowContextLedgerSummary,
    #[serde(default)]
    pub mode_runtime_meta: HashMap<WorkflowMode, ModeRuntimeMeta>,
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

pub const WORKFLOW_KERNEL_UPDATED_CHANNEL: &str = "workflow-kernel-updated";
pub const WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL: &str = "workflow-session-catalog-updated";
pub const WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL: &str = "workflow-mode-transcript-updated";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowKernelUpdatedEvent {
    pub session_state: WorkflowSessionState,
    pub revision: u64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSessionCatalogUpdatedEvent {
    pub active_session_id: Option<String>,
    pub sessions: Vec<WorkflowSessionCatalogItem>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeTranscriptPayload {
    pub session_id: String,
    pub mode: WorkflowMode,
    pub revision: u64,
    #[serde(default)]
    pub lines: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowModeTranscriptUpdatedEvent {
    pub session_id: String,
    pub mode: WorkflowMode,
    pub revision: u64,
    #[serde(default)]
    pub appended_lines: Vec<Value>,
    pub replace_from_line_id: Option<u64>,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct PlanSnapshotRehydrate {
    pub phase: Option<String>,
    pub running_step_id: Option<String>,
    pub pending_clarification: Option<PlanClarificationSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskSnapshotRehydrate {
    pub phase: Option<String>,
    pub current_story_id: Option<String>,
    pub completed_stories: Option<u64>,
    pub failed_stories: Option<u64>,
    pub interview_session_id: Option<String>,
    pub pending_interview: Option<TaskInterviewSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSessionRecord {
    session: WorkflowSession,
    events: Vec<WorkflowEventV2>,
    checkpoints: Vec<WorkflowCheckpoint>,
    #[serde(default)]
    context_ledger_entries: Vec<WorkflowContextLedgerEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedCatalogRecord {
    active_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedModeTranscriptRecord {
    session_id: String,
    mode: WorkflowMode,
    revision: u64,
    #[serde(default)]
    lines: Vec<Value>,
}

/// In-memory kernel state with persisted event/checkpoint record per session.
#[derive(Clone)]
pub struct WorkflowKernelState {
    sessions: Arc<RwLock<HashMap<String, WorkflowSession>>>,
    events: Arc<RwLock<HashMap<String, Vec<WorkflowEventV2>>>>,
    checkpoints: Arc<RwLock<HashMap<String, Vec<WorkflowCheckpoint>>>>,
    context_ledger_entries: Arc<RwLock<HashMap<String, Vec<WorkflowContextLedgerEntry>>>>,
    active_session_id: Arc<RwLock<Option<String>>>,
    attached_mode_runtimes: Arc<RwLock<HashMap<String, Vec<WorkflowMode>>>>,
    storage_root: Arc<PathBuf>,
}

impl WorkflowKernelState {
    pub fn new() -> Self {
        Self::new_with_storage_dir(resolve_storage_root())
    }

    pub fn new_with_storage_dir(storage_root: PathBuf) -> Self {
        let sessions_dir = storage_root.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);
        let active_session_id = read_persisted_active_session_id(&storage_root);

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            context_ledger_entries: Arc::new(RwLock::new(HashMap::new())),
            active_session_id: Arc::new(RwLock::new(active_session_id)),
            attached_mode_runtimes: Arc::new(RwLock::new(HashMap::new())),
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
        mode_snapshots.ensure_mode(WorkflowMode::Chat);
        mode_snapshots.ensure_mode(WorkflowMode::Plan);
        mode_snapshots.ensure_mode(WorkflowMode::Task);

        let mut ledger_entries = Vec::new();
        if let Some(context) = initial_context.as_ref() {
            append_handoff_to_context_ledger(&mut ledger_entries, active_mode, context.clone());
        }
        let mut session = WorkflowSession {
            session_id: session_id.clone(),
            session_kind: WorkflowSessionKind::SimpleRoot,
            display_title: derive_initial_display_title(initial_mode, initial_context.as_ref()),
            workspace_path: derive_workspace_path(initial_context.as_ref()),
            status: WorkflowStatus::Active,
            active_mode,
            mode_snapshots,
            handoff_context: build_handoff_context_from_ledger(&ledger_entries),
            linked_mode_sessions: HashMap::new(),
            background_state: WorkflowBackgroundState::Foreground,
            context_ledger: build_context_ledger_summary(&build_handoff_context_from_ledger(&ledger_entries)),
            mode_runtime_meta: HashMap::new(),
            last_error: None,
            created_at: now.clone(),
            updated_at: now,
            last_checkpoint_id: None,
        };
        self.refresh_session_derived_fields(&mut session, Some(session_id.as_str()));

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        {
            let mut context_entries = self.context_ledger_entries.write().await;
            context_entries.insert(session_id.clone(), ledger_entries);
        }
        {
            let mut active_session_id = self.active_session_id.write().await;
            *active_session_id = Some(session_id.clone());
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
        self.persist_catalog_record().await?;
        self.refresh_all_session_runtime_meta().await?;
        self.persist_session_record(&session_id).await?;

        Ok(session)
    }

    pub async fn active_session_id(&self) -> Option<String> {
        self.active_session_id.read().await.clone()
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

    pub async fn list_sessions(&self) -> Result<Vec<WorkflowSessionCatalogItem>, String> {
        let mut sessions = self.load_all_sessions().await?;
        let active_session_id = self.active_session_id().await;
        for session in sessions.values_mut() {
            self.refresh_session_derived_fields(session, active_session_id.as_deref());
        }
        let mut items = sessions
            .into_values()
            .map(|session| WorkflowSessionCatalogItem {
                session_id: session.session_id,
                session_kind: session.session_kind,
                display_title: session.display_title,
                workspace_path: session.workspace_path,
                active_mode: session.active_mode,
                status: session.status,
                background_state: session.background_state,
                updated_at: session.updated_at,
                created_at: session.created_at,
                last_error: session.last_error,
                context_ledger: session.context_ledger,
                mode_snapshots: session.mode_snapshots,
                mode_runtime_meta: session.mode_runtime_meta,
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(items)
    }

    pub async fn get_session_catalog_state(&self) -> Result<WorkflowSessionCatalogState, String> {
        Ok(WorkflowSessionCatalogState {
            active_session_id: self.active_session_id().await,
            sessions: self.list_sessions().await?,
        })
    }

    pub async fn activate_session(&self, session_id: &str) -> Result<WorkflowSession, String> {
        self.recover_session(session_id).await?;
        {
            let mut active_session_id = self.active_session_id.write().await;
            *active_session_id = Some(session_id.to_string());
        }
        self.persist_catalog_record().await?;
        self.refresh_all_session_runtime_meta().await?;
        Ok(self.get_session(session_id).await?)
    }

    pub async fn rename_session(
        &self,
        session_id: &str,
        display_title: &str,
    ) -> Result<WorkflowSession, String> {
        let normalized_title = display_title.trim();
        if normalized_title.is_empty() {
            return Err("Display title cannot be empty".to_string());
        }

        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.display_title = normalized_title.to_string();
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.persist_session_record(session_id).await?;
        Ok(updated_session)
    }

    pub async fn archive_session(
        &self,
        session_id: &str,
    ) -> Result<WorkflowSessionCatalogState, String> {
        let next_active = {
            let mut active_session_id = self.active_session_id.write().await;
            if active_session_id.as_deref() == Some(session_id) {
                let remaining = self
                    .sessions
                    .read()
                    .await
                    .iter()
                    .filter(|(candidate_id, session)| {
                        candidate_id.as_str() != session_id
                            && session.status != WorkflowStatus::Archived
                    })
                    .map(|(candidate_id, _)| candidate_id.clone())
                    .collect::<Vec<_>>();
                let next = remaining.first().cloned();
                *active_session_id = next.clone();
                next
            } else {
                active_session_id.clone()
            }
        };

        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.status = WorkflowStatus::Archived;
            session.background_state = WorkflowBackgroundState::BackgroundIdle;
            session.updated_at = now_rfc3339();
        }

        self.persist_session_record(session_id).await?;
        self.persist_catalog_record().await?;
        if next_active.is_some() {
            self.refresh_all_session_runtime_meta().await?;
        }
        self.get_session_catalog_state().await
    }

    pub async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<WorkflowSessionState, String> {
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.status = WorkflowStatus::Active;
            session.updated_at = now_rfc3339();
        }

        {
            let mut active_session_id = self.active_session_id.write().await;
            *active_session_id = Some(session_id.to_string());
        }

        self.persist_session_record(session_id).await?;
        self.persist_catalog_record().await?;
        self.refresh_all_session_runtime_meta().await?;
        self.get_session_state(session_id).await
    }

    pub async fn delete_session(
        &self,
        session_id: &str,
    ) -> Result<WorkflowSessionCatalogState, String> {
        let removed = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id)
        };
        if removed.is_none() {
            if self.session_file_path(session_id).exists() {
                self.delete_session_files(session_id).await?;
                let recovered = self.load_all_sessions().await?;
                let next_active = self.select_fallback_active_session_id(&recovered);
                {
                    let mut active_session_id = self.active_session_id.write().await;
                    *active_session_id = next_active;
                }
                self.refresh_all_session_runtime_meta().await?;
                self.persist_catalog_record().await?;
                return self.get_session_catalog_state().await;
            }
            return Err(format!("Workflow session not found: {session_id}"));
        }

        {
            let mut events = self.events.write().await;
            events.remove(session_id);
        }
        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints.remove(session_id);
        }
        {
            let mut ledger_entries = self.context_ledger_entries.write().await;
            ledger_entries.remove(session_id);
        }
        {
            let mut attached_runtimes = self.attached_mode_runtimes.write().await;
            attached_runtimes.remove(session_id);
        }

        let next_active = {
            let mut active_session_id = self.active_session_id.write().await;
            if active_session_id.as_deref() == Some(session_id) {
                let remaining = self.sessions.read().await.clone();
                let next = self.select_fallback_active_session_id(&remaining);
                *active_session_id = next.clone();
                next
            } else {
                active_session_id.clone()
            }
        };

        self.delete_session_files(session_id).await?;
        self.persist_catalog_record().await?;
        if next_active.is_some() {
            self.refresh_all_session_runtime_meta().await?;
        }
        self.get_session_catalog_state().await
    }

    pub async fn resume_background_runs(
        &self,
        session_id: Option<&str>,
    ) -> Result<Vec<ResumeResult>, String> {
        let sessions = self.load_all_sessions().await?;
        let filtered = sessions.values().filter(|session| {
            session_id
                .map(|value| value == session.session_id)
                .unwrap_or(true)
        });
        let mut results = Vec::new();
        for session in filtered {
            for mode in [WorkflowMode::Chat, WorkflowMode::Plan, WorkflowMode::Task] {
                if self
                    .is_mode_runtime_attached(&session.session_id, mode)
                    .await
                {
                    continue;
                }
                let phase = phase_for_mode(session, mode);
                let resumable = match mode {
                    WorkflowMode::Chat => {
                        matches!(phase, Some("submitting" | "streaming" | "paused"))
                    }
                    WorkflowMode::Plan | WorkflowMode::Task => {
                        is_background_resume_candidate(phase.unwrap_or("idle"))
                    }
                };
                if resumable {
                    results.push(ResumeResult {
                        session_id: session.session_id.clone(),
                        mode,
                        resumed: false,
                        reason: "resume_requires_mode_runtime_rebind".to_string(),
                    });
                }
            }
        }
        Ok(results)
    }

    pub async fn mark_mode_runtime_attached(&self, session_id: &str, mode: WorkflowMode) {
        let mut attached = self.attached_mode_runtimes.write().await;
        let modes = attached.entry(session_id.to_string()).or_default();
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }

    pub async fn is_mode_runtime_attached(&self, session_id: &str, mode: WorkflowMode) -> bool {
        self.attached_mode_runtimes
            .read()
            .await
            .get(session_id)
            .map(|modes| modes.contains(&mode))
            .unwrap_or(false)
    }

    pub async fn transition_mode(
        &self,
        session_id: &str,
        target_mode: WorkflowMode,
        handoff: Option<HandoffContextBundle>,
    ) -> Result<WorkflowSession, String> {
        let (source_mode, updated_session) = {
            let mut sessions = self.sessions.write().await;
            let mut ledger_map = self.context_ledger_entries.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            let source_mode = session.active_mode;
            session.mode_snapshots.ensure_mode(source_mode);
            session.mode_snapshots.ensure_mode(target_mode);
            if let Some(incoming) = handoff {
                let ledger = ledger_map.entry(session_id.to_string()).or_default();
                append_handoff_to_context_ledger(ledger, target_mode, incoming);
                session.handoff_context = build_handoff_context_from_ledger(ledger);
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
            let mut ledger_map = self.context_ledger_entries.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            let source_mode = session.active_mode;
            session.mode_snapshots.ensure_mode(source_mode);
            session.mode_snapshots.ensure_mode(target_mode);
            if let Some(incoming) = handoff {
                let ledger = ledger_map.entry(session_id.to_string()).or_default();
                append_handoff_to_context_ledger(ledger, target_mode, incoming);
                session.handoff_context = build_handoff_context_from_ledger(ledger);
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
            let mut ledger_map = self.context_ledger_entries.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            session.mode_snapshots.ensure_mode(target_mode);
            let ledger = ledger_map.entry(session_id.to_string()).or_default();
            append_handoff_to_context_ledger(ledger, target_mode, handoff);
            session.handoff_context = build_handoff_context_from_ledger(ledger);
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

    pub async fn link_mode_session(
        &self,
        session_id: &str,
        mode: WorkflowMode,
        mode_session_id: &str,
    ) -> Result<WorkflowSession, String> {
        let normalized_mode_session_id = mode_session_id.trim();
        if normalized_mode_session_id.is_empty() {
            return Err("Mode session id cannot be empty".to_string());
        }

        let updated_session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;

            session
                .linked_mode_sessions
                .insert(mode, normalized_mode_session_id.to_string());
            session.mode_snapshots.ensure_mode(mode);
            session.updated_at = now_rfc3339();
            session.clone()
        };

        self.append_event(
            session_id,
            WorkflowEventKind::ModeSessionLinked,
            mode,
            json!({
                "mode": mode,
                "modeSessionId": normalized_mode_session_id
            }),
        )
        .await;
        self.create_checkpoint(session_id, "mode_session_linked")
            .await?;
        self.persist_session_record(session_id).await?;
        self.mark_mode_runtime_attached(session_id, mode).await;

        Ok(updated_session)
    }

    async fn kernel_sessions_linked_to_mode_session(
        &self,
        mode: WorkflowMode,
        mode_session_id: &str,
    ) -> Vec<String> {
        let normalized_mode_session_id = mode_session_id.trim();
        if normalized_mode_session_id.is_empty() {
            return Vec::new();
        }

        let sessions = self.sessions.read().await;
        sessions
            .iter()
            .filter_map(|(kernel_session_id, session)| {
                let matches = session
                    .linked_mode_sessions
                    .get(&mode)
                    .map(|linked| linked == normalized_mode_session_id)
                    .unwrap_or(false);
                if matches {
                    Some(kernel_session_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn linked_kernel_sessions_for_mode_session(
        &self,
        mode: WorkflowMode,
        mode_session_id: &str,
    ) -> Vec<String> {
        self.kernel_sessions_linked_to_mode_session(mode, mode_session_id)
            .await
    }

    pub async fn handoff_context_for_mode_session(
        &self,
        mode: WorkflowMode,
        mode_session_id: &str,
    ) -> Option<HandoffContextBundle> {
        let linked_kernel_sessions = self
            .kernel_sessions_linked_to_mode_session(mode, mode_session_id)
            .await;
        let kernel_session_id = linked_kernel_sessions.first()?.clone();

        let sessions = self.sessions.read().await;
        sessions
            .get(&kernel_session_id)
            .map(|session| session.handoff_context.clone())
    }

    pub async fn find_linked_task_session_by_interview_session(
        &self,
        interview_session_id: &str,
    ) -> Option<String> {
        let normalized_interview_session_id = interview_session_id.trim();
        if normalized_interview_session_id.is_empty() {
            return None;
        }

        let sessions = self.sessions.read().await;
        sessions.values().find_map(|session| {
            let task_snapshot = session.mode_snapshots.task.as_ref()?;
            let interview_id_matches = task_snapshot
                .interview_session_id
                .as_ref()
                .map(|value| value == normalized_interview_session_id)
                .unwrap_or(false);
            if !interview_id_matches {
                return None;
            }
            session
                .linked_mode_sessions
                .get(&WorkflowMode::Task)
                .cloned()
        })
    }

    pub async fn sync_plan_snapshot_by_linked_session(
        &self,
        plan_session_id: &str,
        phase: Option<String>,
        pending_clarification: Option<PlanClarificationSnapshot>,
        running_step_id: Option<String>,
        status: Option<WorkflowStatus>,
    ) -> Result<Vec<String>, String> {
        let linked_kernel_sessions = self
            .kernel_sessions_linked_to_mode_session(WorkflowMode::Plan, plan_session_id)
            .await;
        if linked_kernel_sessions.is_empty() {
            return Ok(Vec::new());
        }

        {
            let mut sessions = self.sessions.write().await;
            for kernel_session_id in &linked_kernel_sessions {
                if let Some(session) = sessions.get_mut(kernel_session_id) {
                    session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
                    let plan = session.mode_snapshots.plan_mut();
                    if let Some(next_phase) = phase
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        plan.phase = next_phase.to_string();
                    }
                    if let Some(next_running_step_id) = running_step_id
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        plan.running_step_id = Some(next_running_step_id.to_string());
                    } else if phase.as_deref() != Some("executing") {
                        plan.running_step_id = None;
                    }
                    plan.pending_clarification = pending_clarification.clone();
                    plan.background_status = Some(
                        if phase
                            .as_deref()
                            .map(is_background_resume_candidate)
                            .unwrap_or(false)
                        {
                            "running".to_string()
                        } else {
                            "idle".to_string()
                        },
                    );
                    plan.resumable_from_checkpoint = phase
                        .as_deref()
                        .map(is_background_resume_candidate)
                        .unwrap_or(false);
                    plan.last_checkpoint_id = session.last_checkpoint_id.clone();
                    if let Some(next_status) = status {
                        session.status = next_status;
                    }
                    session.updated_at = now_rfc3339();
                }
            }
        }

        for kernel_session_id in &linked_kernel_sessions {
            self.persist_session_record(kernel_session_id).await?;
        }

        Ok(linked_kernel_sessions)
    }

    pub async fn sync_task_snapshot_by_linked_session(
        &self,
        task_session_id: &str,
        phase: Option<String>,
        current_story_id: Option<String>,
        completed_stories: Option<u64>,
        failed_stories: Option<u64>,
        status: Option<WorkflowStatus>,
    ) -> Result<Vec<String>, String> {
        let linked_kernel_sessions = self
            .kernel_sessions_linked_to_mode_session(WorkflowMode::Task, task_session_id)
            .await;
        if linked_kernel_sessions.is_empty() {
            return Ok(Vec::new());
        }

        {
            let mut sessions = self.sessions.write().await;
            for kernel_session_id in &linked_kernel_sessions {
                if let Some(session) = sessions.get_mut(kernel_session_id) {
                    session.mode_snapshots.ensure_mode(WorkflowMode::Task);
                    let task = session.mode_snapshots.task_mut();
                    if let Some(next_phase) = phase
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        task.phase = next_phase.to_string();
                    }
                    if let Some(story_id) = current_story_id
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        task.current_story_id = Some(story_id.to_string());
                    }
                    if let Some(completed) = completed_stories {
                        task.completed_stories = completed;
                    }
                    if let Some(failed) = failed_stories {
                        task.failed_stories = failed;
                    }
                    task.background_status = Some(
                        if phase
                            .as_deref()
                            .map(is_background_resume_candidate)
                            .unwrap_or(false)
                        {
                            "running".to_string()
                        } else {
                            "idle".to_string()
                        },
                    );
                    task.resumable_from_checkpoint = phase
                        .as_deref()
                        .map(is_background_resume_candidate)
                        .unwrap_or(false);
                    task.last_checkpoint_id = session.last_checkpoint_id.clone();
                    if let Some(next_status) = status {
                        session.status = next_status;
                    }
                    if phase.as_deref() != Some("interviewing") {
                        task.pending_interview = None;
                    }
                    session.updated_at = now_rfc3339();
                }
            }
        }

        for kernel_session_id in &linked_kernel_sessions {
            self.persist_session_record(kernel_session_id).await?;
        }

        Ok(linked_kernel_sessions)
    }

    pub async fn sync_task_interview_snapshot_by_linked_session(
        &self,
        task_session_id: &str,
        interview_session_id: Option<String>,
        phase: Option<String>,
        pending_interview: Option<TaskInterviewSnapshot>,
    ) -> Result<Vec<String>, String> {
        let linked_kernel_sessions = self
            .kernel_sessions_linked_to_mode_session(WorkflowMode::Task, task_session_id)
            .await;
        if linked_kernel_sessions.is_empty() {
            return Ok(Vec::new());
        }

        {
            let mut sessions = self.sessions.write().await;
            for kernel_session_id in &linked_kernel_sessions {
                if let Some(session) = sessions.get_mut(kernel_session_id) {
                    session.mode_snapshots.ensure_mode(WorkflowMode::Task);
                    let task = session.mode_snapshots.task_mut();
                    if let Some(next_phase) = phase
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        task.phase = next_phase.to_string();
                    }
                    if let Some(interview_session_id) = interview_session_id
                        .as_ref()
                        .map(|value| value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        task.interview_session_id = Some(interview_session_id.to_string());
                    }
                    task.pending_interview = pending_interview.clone();
                    session.updated_at = now_rfc3339();
                }
            }
        }

        for kernel_session_id in &linked_kernel_sessions {
            self.persist_session_record(kernel_session_id).await?;
        }

        Ok(linked_kernel_sessions)
    }

    pub async fn rehydrate_from_linked_sessions(
        &self,
        kernel_session_id: &str,
        plan_snapshot: Option<PlanSnapshotRehydrate>,
        task_snapshot: Option<TaskSnapshotRehydrate>,
    ) -> Result<WorkflowSession, String> {
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(kernel_session_id)
                .ok_or_else(|| format!("Workflow session not found: {kernel_session_id}"))?;

            if let Some(plan_snapshot) = plan_snapshot {
                session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
                let plan = session.mode_snapshots.plan_mut();
                if let Some(next_phase) = plan_snapshot
                    .phase
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    plan.phase = next_phase.to_string();
                }
                if let Some(running_step_id) = plan_snapshot
                    .running_step_id
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    plan.running_step_id = Some(running_step_id.to_string());
                }
                plan.pending_clarification = plan_snapshot.pending_clarification.clone();
                plan.background_status = Some(
                    if plan_snapshot
                        .phase
                        .as_deref()
                        .map(is_background_resume_candidate)
                        .unwrap_or(false)
                    {
                        "running".to_string()
                    } else {
                        "idle".to_string()
                    },
                );
                plan.resumable_from_checkpoint = plan_snapshot
                    .phase
                    .as_deref()
                    .map(is_background_resume_candidate)
                    .unwrap_or(false);
                plan.last_checkpoint_id = session.last_checkpoint_id.clone();
            }

            if let Some(task_snapshot) = task_snapshot {
                session.mode_snapshots.ensure_mode(WorkflowMode::Task);
                let task = session.mode_snapshots.task_mut();
                if let Some(next_phase) = task_snapshot
                    .phase
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    task.phase = next_phase.to_string();
                }
                if let Some(current_story_id) = task_snapshot
                    .current_story_id
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    task.current_story_id = Some(current_story_id.to_string());
                }
                if let Some(completed_stories) = task_snapshot.completed_stories {
                    task.completed_stories = completed_stories;
                }
                if let Some(failed_stories) = task_snapshot.failed_stories {
                    task.failed_stories = failed_stories;
                }
                if let Some(interview_session_id) = task_snapshot
                    .interview_session_id
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    task.interview_session_id = Some(interview_session_id.to_string());
                }
                task.pending_interview = task_snapshot.pending_interview;
                task.background_status = Some(
                    if task_snapshot
                        .phase
                        .as_deref()
                        .map(is_background_resume_candidate)
                        .unwrap_or(false)
                    {
                        "running".to_string()
                    } else {
                        "idle".to_string()
                    },
                );
                task.resumable_from_checkpoint = task_snapshot
                    .phase
                    .as_deref()
                    .map(is_background_resume_candidate)
                    .unwrap_or(false);
                task.last_checkpoint_id = session.last_checkpoint_id.clone();
            }

            session.updated_at = now_rfc3339();
        }

        self.persist_session_record(kernel_session_id).await?;
        self.get_session(kernel_session_id).await
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
            plan.run_id = Some(uuid::Uuid::new_v4().to_string());
            plan.background_status = Some("running".to_string());
            plan.resumable_from_checkpoint = true;
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
            if plan.run_id.is_none() {
                plan.run_id = Some(uuid::Uuid::new_v4().to_string());
            }
            plan.background_status = Some("running".to_string());
            plan.resumable_from_checkpoint = true;
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
        let mut recovered_session = record.session.clone();
        let repaired_fields = Self::repair_snapshot_integrity(&mut recovered_session);
        let chat_interrupted = mark_chat_runtime_interrupted(&mut recovered_session);
        recovered_session.handoff_context =
            build_handoff_context_from_ledger(&record.context_ledger_entries);
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), recovered_session.clone());
        }
        {
            let mut events = self.events.write().await;
            events.insert(session_id.to_string(), record.events);
        }
        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints.insert(session_id.to_string(), record.checkpoints);
        }
        {
            let mut context_ledger_entries = self.context_ledger_entries.write().await;
            context_ledger_entries.insert(session_id.to_string(), record.context_ledger_entries);
        }

        self.append_event(
            session_id,
            WorkflowEventKind::SessionRecovered,
            recovered_session.active_mode,
            json!({
                "fromPersistence": true,
                "snapshotIntegrity": if repaired_fields.is_empty() { "ok" } else { "repaired" },
                "repairedFields": repaired_fields,
                "chatInterrupted": chat_interrupted,
            }),
        )
        .await;
        self.create_checkpoint(session_id, "session_recovered")
            .await?;
        self.persist_session_record(session_id).await?;

        self.get_session(session_id).await
    }

    fn repair_snapshot_integrity(session: &mut WorkflowSession) -> Vec<String> {
        let mut repaired = Vec::new();

        if session.display_title.trim().is_empty() {
            session.display_title = default_display_title();
            repaired.push("display_title".to_string());
        }

        if session.mode_snapshots.chat.is_none() {
            session.mode_snapshots.chat = Some(ChatState::default());
            repaired.push("mode_snapshots.chat".to_string());
        }
        if session.mode_snapshots.plan.is_none() {
            session.mode_snapshots.plan = Some(PlanState::default());
            repaired.push("mode_snapshots.plan".to_string());
        }
        if session.mode_snapshots.task.is_none() {
            session.mode_snapshots.task = Some(TaskState::default());
            repaired.push("mode_snapshots.task".to_string());
        }
        if session.linked_mode_sessions.is_empty() {
            session.linked_mode_sessions = HashMap::new();
        }
        if session.context_ledger.ledger_version == 0 {
            session.context_ledger = build_context_ledger_summary(&session.handoff_context);
            repaired.push("context_ledger".to_string());
        }

        session.mode_snapshots.ensure_mode(session.active_mode);

        if let Some(chat) = session.mode_snapshots.chat.as_mut() {
            if chat.phase.trim().is_empty() {
                chat.phase = "ready".to_string();
                repaired.push("mode_snapshots.chat.phase".to_string());
            }
        }
        if let Some(plan) = session.mode_snapshots.plan.as_mut() {
            if plan.phase.trim().is_empty() {
                plan.phase = "idle".to_string();
                repaired.push("mode_snapshots.plan.phase".to_string());
            }
            if plan.background_status.is_none() {
                plan.background_status = Some("idle".to_string());
            }
            if plan.last_checkpoint_id.is_none() {
                plan.last_checkpoint_id = session.last_checkpoint_id.clone();
            }
        }
        if let Some(task) = session.mode_snapshots.task.as_mut() {
            if task.phase.trim().is_empty() {
                task.phase = "idle".to_string();
                repaired.push("mode_snapshots.task.phase".to_string());
            }
            if task
                .interview_session_id
                .as_ref()
                .map(|value| value.trim())
                .unwrap_or("")
                .is_empty()
            {
                task.interview_session_id = None;
            }
            if task.background_status.is_none() {
                task.background_status = Some("idle".to_string());
            }
            if task.last_checkpoint_id.is_none() {
                task.last_checkpoint_id = session.last_checkpoint_id.clone();
            }
        }

        repaired
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

    async fn load_all_sessions(&self) -> Result<HashMap<String, WorkflowSession>, String> {
        let mut merged = {
            let sessions = self.sessions.read().await;
            sessions.clone()
        };
        let sessions_dir = self.storage_root.join("sessions");
        let entries = fs::read_dir(&sessions_dir)
            .map_err(|error| format!("Failed to read workflow sessions directory: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("Failed to enumerate workflow sessions directory entry: {error}")
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if merged.contains_key(stem) {
                continue;
            }
            if let Ok(record) = self.read_persisted_record(stem).await {
                let mut session = record.session;
                Self::repair_snapshot_integrity(&mut session);
                merged.insert(stem.to_string(), session);
            }
        }
        Ok(merged)
    }

    async fn refresh_all_session_runtime_meta(&self) -> Result<(), String> {
        let active_session_id = self.active_session_id().await;
        let session_ids = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect::<Vec<_>>()
        };
        {
            let mut sessions = self.sessions.write().await;
            for session_id in &session_ids {
                if let Some(session) = sessions.get_mut(session_id) {
                    self.refresh_session_derived_fields(session, active_session_id.as_deref());
                }
            }
        }
        for session_id in session_ids {
            self.persist_session_record(&session_id).await?;
        }
        Ok(())
    }

    fn refresh_session_derived_fields(
        &self,
        session: &mut WorkflowSession,
        active_session_id: Option<&str>,
    ) {
        session.context_ledger = build_context_ledger_summary(&session.handoff_context);
        session.mode_snapshots.ensure_mode(WorkflowMode::Chat);
        session.mode_snapshots.ensure_mode(WorkflowMode::Plan);
        session.mode_snapshots.ensure_mode(WorkflowMode::Task);
        session.mode_runtime_meta = build_mode_runtime_meta_map(
            session,
            active_session_id,
            session.last_checkpoint_id.clone(),
        );
        session.background_state = derive_background_state(
            session,
            active_session_id == Some(session.session_id.as_str()),
        );
    }

    async fn persist_catalog_record(&self) -> Result<(), String> {
        let record = PersistedCatalogRecord {
            active_session_id: self.active_session_id().await,
        };
        let encoded = serde_json::to_vec_pretty(&record)
            .map_err(|error| format!("Failed to serialize workflow session catalog: {error}"))?;
        let path = self.catalog_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create workflow catalog directory: {error}"))?;
        }
        fs::write(path, encoded)
            .map_err(|error| format!("Failed to persist workflow catalog: {error}"))
    }

    async fn persist_session_record(&self, session_id: &str) -> Result<(), String> {
        let active_session_id = self.active_session_id().await;
        let session = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            self.refresh_session_derived_fields(session, active_session_id.as_deref());
            session.clone()
        };
        let events = {
            let map = self.events.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };
        let checkpoints = {
            let map = self.checkpoints.read().await;
            map.get(session_id).cloned().unwrap_or_default()
        };
        let context_ledger_entries = {
            let map = self.context_ledger_entries.read().await;
            map.get(session_id)
                .cloned()
                .unwrap_or_else(|| {
                    build_context_ledger_entries_from_handoff(
                        Some(session.active_mode),
                        &session.handoff_context,
                    )
                })
        };

        let record = PersistedSessionRecord {
            session,
            events,
            checkpoints,
            context_ledger_entries,
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
        if record.context_ledger_entries.is_empty() {
            record.context_ledger_entries = build_context_ledger_entries_from_handoff(
                Some(record.session.active_mode),
                &record.session.handoff_context,
            );
        }
        record.session.handoff_context =
            build_handoff_context_from_ledger(&record.context_ledger_entries);

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

    fn catalog_file_path(&self) -> PathBuf {
        self.storage_root.join("catalog.json")
    }

    async fn delete_session_files(&self, session_id: &str) -> Result<(), String> {
        let session_path = self.session_file_path(session_id);
        if session_path.exists() {
            fs::remove_file(&session_path)
                .map_err(|error| format!("Failed to remove workflow session record: {error}"))?;
        }

        let transcript_dir = self.storage_root.join("transcripts").join(session_id);
        if transcript_dir.exists() {
            fs::remove_dir_all(&transcript_dir)
                .map_err(|error| format!("Failed to remove workflow transcript directory: {error}"))?;
        }

        Ok(())
    }

    fn select_fallback_active_session_id(
        &self,
        sessions: &HashMap<String, WorkflowSession>,
    ) -> Option<String> {
        let mut items = sessions.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        items.first().map(|session| session.session_id.clone())
    }

    pub async fn get_mode_transcript(
        &self,
        session_id: &str,
        mode: WorkflowMode,
    ) -> Result<ModeTranscriptPayload, String> {
        let _ = self.recover_session(session_id).await?;
        let record = self
            .read_persisted_mode_transcript(session_id, mode)
            .await
            .unwrap_or_else(|_| PersistedModeTranscriptRecord {
                session_id: session_id.to_string(),
                mode,
                revision: 0,
                lines: Vec::new(),
            });
        Ok(ModeTranscriptPayload {
            session_id: record.session_id,
            mode: record.mode,
            revision: record.revision,
            lines: record.lines,
        })
    }

    pub async fn store_mode_transcript(
        &self,
        session_id: &str,
        mode: WorkflowMode,
        lines: Vec<Value>,
    ) -> Result<ModeTranscriptPayload, String> {
        let _ = self.recover_session(session_id).await?;
        let active_session_id = self.active_session_id().await;
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.mode_snapshots.ensure_mode(mode);
            session.updated_at = now_rfc3339();
            self.refresh_session_derived_fields(session, active_session_id.as_deref());
        }

        let previous = self
            .read_persisted_mode_transcript(session_id, mode)
            .await
            .unwrap_or_else(|_| PersistedModeTranscriptRecord {
                session_id: session_id.to_string(),
                mode,
                revision: 0,
                lines: Vec::new(),
            });
        let record = PersistedModeTranscriptRecord {
            session_id: session_id.to_string(),
            mode,
            revision: previous.revision.saturating_add(1),
            lines,
        };
        self.persist_mode_transcript_record(&record).await?;
        self.persist_session_record(session_id).await?;

        Ok(ModeTranscriptPayload {
            session_id: record.session_id,
            mode: record.mode,
            revision: record.revision,
            lines: record.lines,
        })
    }

    pub async fn append_mode_transcript(
        &self,
        session_id: &str,
        mode: WorkflowMode,
        appended_lines: Vec<Value>,
    ) -> Result<ModeTranscriptPayload, String> {
        let _ = self.recover_session(session_id).await?;
        let active_session_id = self.active_session_id().await;
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Workflow session not found: {session_id}"))?;
            session.mode_snapshots.ensure_mode(mode);
            session.updated_at = now_rfc3339();
            self.refresh_session_derived_fields(session, active_session_id.as_deref());
        }

        let mut record = self
            .read_persisted_mode_transcript(session_id, mode)
            .await
            .unwrap_or_else(|_| PersistedModeTranscriptRecord {
                session_id: session_id.to_string(),
                mode,
                revision: 0,
                lines: Vec::new(),
            });
        record.revision = record.revision.saturating_add(1);
        record.lines.extend(appended_lines);
        self.persist_mode_transcript_record(&record).await?;
        self.persist_session_record(session_id).await?;

        Ok(ModeTranscriptPayload {
            session_id: record.session_id,
            mode: record.mode,
            revision: record.revision,
            lines: record.lines,
        })
    }

    async fn persist_mode_transcript_record(
        &self,
        record: &PersistedModeTranscriptRecord,
    ) -> Result<(), String> {
        let encoded = serde_json::to_vec_pretty(record)
            .map_err(|error| format!("Failed to serialize workflow transcript record: {error}"))?;
        let path = self.mode_transcript_file_path(&record.session_id, record.mode);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create workflow transcript directory: {error}")
            })?;
        }
        fs::write(path, encoded)
            .map_err(|error| format!("Failed to persist workflow transcript: {error}"))
    }

    async fn read_persisted_mode_transcript(
        &self,
        session_id: &str,
        mode: WorkflowMode,
    ) -> Result<PersistedModeTranscriptRecord, String> {
        let path = self.mode_transcript_file_path(session_id, mode);
        let content = fs::read_to_string(&path).map_err(|error| {
            format!(
                "Failed to read persisted workflow transcript '{}:{}': {error}",
                session_id,
                mode_storage_name(mode)
            )
        })?;
        serde_json::from_str::<PersistedModeTranscriptRecord>(&content).map_err(|error| {
            format!(
                "Failed to decode persisted workflow transcript '{}:{}': {error}",
                session_id,
                mode_storage_name(mode)
            )
        })
    }

    fn mode_transcript_file_path(&self, session_id: &str, mode: WorkflowMode) -> PathBuf {
        self.storage_root
            .join("transcripts")
            .join(session_id)
            .join(format!("{}.json", mode_storage_name(mode)))
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
            if chat.turn_count == 1 {
                session.display_title = summarize_display_title(&intent.content);
            }
        }
        WorkflowMode::Plan => {
            let plan = session.mode_snapshots.plan_mut();
            match intent.intent_type {
                UserInputIntentType::PlanClarification => {
                    plan.phase = "planning".to_string();
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

fn default_session_kind() -> WorkflowSessionKind {
    WorkflowSessionKind::SimpleRoot
}

fn default_display_title() -> String {
    "New session".to_string()
}

fn read_persisted_active_session_id(storage_root: &PathBuf) -> Option<String> {
    let path = storage_root.join("catalog.json");
    let content = fs::read_to_string(path).ok()?;
    let record = serde_json::from_str::<PersistedCatalogRecord>(&content).ok()?;
    record
        .active_session_id
        .filter(|value| !value.trim().is_empty())
}

fn derive_initial_display_title(
    initial_mode: Option<WorkflowMode>,
    initial_context: Option<&HandoffContextBundle>,
) -> String {
    let metadata = initial_context.map(|context| &context.metadata);
    let metadata_title = metadata
        .and_then(|map| map.get("displayTitle"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(title) = metadata_title {
        return title;
    }

    match initial_mode.unwrap_or(WorkflowMode::Chat) {
        WorkflowMode::Chat => "New chat".to_string(),
        WorkflowMode::Plan => "New plan".to_string(),
        WorkflowMode::Task => "New task".to_string(),
    }
}

fn derive_workspace_path(initial_context: Option<&HandoffContextBundle>) -> Option<String> {
    initial_context
        .and_then(|context| context.metadata.get("workspacePath"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn summarize_display_title(content: &str) -> String {
    let normalized = content.trim();
    if normalized.is_empty() {
        return "New session".to_string();
    }
    let title = normalized.lines().next().unwrap_or(normalized).trim();
    let mut truncated = title.chars().take(80).collect::<String>();
    if title.chars().count() > 80 {
        truncated.push_str("...");
    }
    truncated
}

fn build_context_ledger_summary(
    handoff_context: &HandoffContextBundle,
) -> WorkflowContextLedgerSummary {
    WorkflowContextLedgerSummary {
        conversation_turn_count: handoff_context.conversation_context.len(),
        artifact_ref_count: handoff_context.artifact_refs.len(),
        context_source_kinds: handoff_context.context_sources.clone(),
        last_compaction_at: None,
        ledger_version: 2,
    }
}

fn build_context_ledger_entries_from_handoff(
    mode: Option<WorkflowMode>,
    handoff_context: &HandoffContextBundle,
) -> Vec<WorkflowContextLedgerEntry> {
    let mut entries = Vec::new();
    append_handoff_to_context_ledger(&mut entries, mode.unwrap_or(WorkflowMode::Chat), handoff_context.clone());
    entries
}

fn append_handoff_to_context_ledger(
    entries: &mut Vec<WorkflowContextLedgerEntry>,
    mode: WorkflowMode,
    handoff: HandoffContextBundle,
) {
    for turn in handoff.conversation_context {
        let value = serde_json::to_value(&turn).unwrap_or_else(|_| json!({
            "user": turn.user,
            "assistant": turn.assistant,
        }));
        if entries.iter().any(|entry| {
            entry.kind == WorkflowContextLedgerEntryKind::ConversationTurn && entry.value == value
        }) {
            continue;
        }
        entries.push(WorkflowContextLedgerEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            created_at: now_rfc3339(),
            mode: Some(mode),
            kind: WorkflowContextLedgerEntryKind::ConversationTurn,
            value,
        });
    }

    for artifact_ref in handoff.artifact_refs {
        let normalized = artifact_ref.trim();
        if normalized.is_empty() {
            continue;
        }
        let value = Value::String(normalized.to_string());
        if entries.iter().any(|entry| {
            entry.kind == WorkflowContextLedgerEntryKind::ArtifactRef && entry.value == value
        }) {
            continue;
        }
        entries.push(WorkflowContextLedgerEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            created_at: now_rfc3339(),
            mode: Some(mode),
            kind: WorkflowContextLedgerEntryKind::ArtifactRef,
            value,
        });
    }

    for context_source in handoff.context_sources {
        let normalized = context_source.trim();
        if normalized.is_empty() {
            continue;
        }
        let value = Value::String(normalized.to_string());
        if entries.iter().any(|entry| {
            entry.kind == WorkflowContextLedgerEntryKind::ContextSource && entry.value == value
        }) {
            continue;
        }
        entries.push(WorkflowContextLedgerEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            created_at: now_rfc3339(),
            mode: Some(mode),
            kind: WorkflowContextLedgerEntryKind::ContextSource,
            value,
        });
    }

    let metadata_value = Value::Object(handoff.metadata);
    if metadata_value
        .as_object()
        .map(|value| value.is_empty())
        .unwrap_or(true)
    {
        return;
    }
    if entries.iter().any(|entry| {
        entry.kind == WorkflowContextLedgerEntryKind::MetadataPatch
            && entry.mode == Some(mode)
            && entry.value == metadata_value
    }) {
        return;
    }
    entries.push(WorkflowContextLedgerEntry {
        entry_id: uuid::Uuid::new_v4().to_string(),
        created_at: now_rfc3339(),
        mode: Some(mode),
        kind: WorkflowContextLedgerEntryKind::MetadataPatch,
        value: metadata_value,
    });
}

fn build_handoff_context_from_ledger(entries: &[WorkflowContextLedgerEntry]) -> HandoffContextBundle {
    let mut conversation_context = Vec::new();
    let mut artifact_refs = Vec::new();
    let mut context_sources = Vec::new();
    let mut metadata = serde_json::Map::new();

    for entry in entries {
        match entry.kind {
            WorkflowContextLedgerEntryKind::ConversationTurn => {
                if let Ok(turn) = serde_json::from_value::<ConversationTurn>(entry.value.clone()) {
                    conversation_context.push(turn);
                }
            }
            WorkflowContextLedgerEntryKind::ArtifactRef => {
                if let Some(value) = entry.value.as_str() {
                    artifact_refs.push(value.to_string());
                }
            }
            WorkflowContextLedgerEntryKind::ContextSource => {
                if let Some(value) = entry.value.as_str() {
                    context_sources.push(value.to_string());
                }
            }
            WorkflowContextLedgerEntryKind::MetadataPatch => {
                if let Some(object) = entry.value.as_object() {
                    for (key, value) in object {
                        metadata.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    HandoffContextBundle {
        conversation_context,
        artifact_refs,
        context_sources,
        metadata,
    }
}

fn phase_for_mode(session: &WorkflowSession, mode: WorkflowMode) -> Option<&str> {
    match mode {
        WorkflowMode::Chat => session
            .mode_snapshots
            .chat
            .as_ref()
            .map(|state| state.phase.as_str()),
        WorkflowMode::Plan => session
            .mode_snapshots
            .plan
            .as_ref()
            .map(|state| state.phase.as_str()),
        WorkflowMode::Task => session
            .mode_snapshots
            .task
            .as_ref()
            .map(|state| state.phase.as_str()),
    }
}

fn is_terminal_phase(phase: &str) -> bool {
    matches!(
        phase,
        "idle" | "ready" | "completed" | "failed" | "cancelled"
    )
}

fn is_background_resume_candidate(phase: &str) -> bool {
    !is_terminal_phase(phase)
}

fn is_interrupted_phase(mode: WorkflowMode, phase: &str) -> bool {
    match mode {
        WorkflowMode::Chat => phase == "interrupted",
        WorkflowMode::Plan | WorkflowMode::Task => phase == "interrupted",
    }
}

fn mark_chat_runtime_interrupted(session: &mut WorkflowSession) -> bool {
    let Some(chat) = session.mode_snapshots.chat.as_mut() else {
        return false;
    };
    if matches!(chat.phase.as_str(), "submitting" | "streaming" | "paused") {
        chat.phase = "interrupted".to_string();
        session.last_error = Some("interrupted_by_restart".to_string());
        return true;
    }
    false
}

fn resume_policy_for_mode(mode: WorkflowMode) -> &'static str {
    match mode {
        WorkflowMode::Chat => "mark_interrupted_after_restart",
        WorkflowMode::Plan | WorkflowMode::Task => "resume_from_checkpoint",
    }
}

fn build_mode_runtime_meta_map(
    session: &WorkflowSession,
    active_session_id: Option<&str>,
    session_last_checkpoint_id: Option<String>,
) -> HashMap<WorkflowMode, ModeRuntimeMeta> {
    let is_foreground_session = active_session_id == Some(session.session_id.as_str());
    [WorkflowMode::Chat, WorkflowMode::Plan, WorkflowMode::Task]
        .into_iter()
        .map(|mode| {
            let phase = phase_for_mode(session, mode).unwrap_or(match mode {
                WorkflowMode::Chat => "ready",
                WorkflowMode::Plan | WorkflowMode::Task => "idle",
            });
            let binding_session_id = session.linked_mode_sessions.get(&mode).cloned();
            let run_id = match mode {
                WorkflowMode::Chat => None,
                WorkflowMode::Plan => session
                    .mode_snapshots
                    .plan
                    .as_ref()
                    .and_then(|state| state.run_id.clone()),
                WorkflowMode::Task => session
                    .mode_snapshots
                    .task
                    .as_ref()
                    .and_then(|state| state.run_id.clone()),
            };
            (
                mode,
                ModeRuntimeMeta {
                    mode,
                    run_id,
                    binding_session_id,
                    is_foreground: is_foreground_session && session.active_mode == mode,
                    is_background_running: !is_foreground_session
                        && is_background_resume_candidate(phase),
                    is_interrupted: is_interrupted_phase(mode, phase),
                    resume_policy: resume_policy_for_mode(mode).to_string(),
                    last_heartbeat_at: Some(session.updated_at.clone()),
                    last_checkpoint_id: session_last_checkpoint_id.clone(),
                    last_error: session.last_error.clone(),
                },
            )
        })
        .collect()
}

fn derive_background_state(
    session: &WorkflowSession,
    is_foreground_session: bool,
) -> WorkflowBackgroundState {
    if is_foreground_session {
        return WorkflowBackgroundState::Foreground;
    }
    let has_interrupted_mode = [WorkflowMode::Chat, WorkflowMode::Plan, WorkflowMode::Task]
        .into_iter()
        .any(|mode| {
            phase_for_mode(session, mode)
                .map(|phase| is_interrupted_phase(mode, phase))
                .unwrap_or(false)
        });
    if has_interrupted_mode {
        return WorkflowBackgroundState::Interrupted;
    }
    let is_running = [WorkflowMode::Chat, WorkflowMode::Plan, WorkflowMode::Task]
        .into_iter()
        .any(|mode| {
            phase_for_mode(session, mode)
                .map(is_background_resume_candidate)
                .unwrap_or(false)
        });
    if is_running {
        WorkflowBackgroundState::BackgroundRunning
    } else {
        WorkflowBackgroundState::BackgroundIdle
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

fn mode_storage_name(mode: WorkflowMode) -> &'static str {
    match mode {
        WorkflowMode::Chat => "chat",
        WorkflowMode::Plan => "plan",
        WorkflowMode::Task => "task",
    }
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
    async fn context_ledger_persists_deduped_handoff_context() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let bundle = HandoffContextBundle {
            conversation_context: vec![ConversationTurn {
                user: "Need auth".to_string(),
                assistant: "Let's design it".to_string(),
            }],
            artifact_refs: vec!["spec.md".to_string()],
            context_sources: vec!["chat_transcript_sync".to_string()],
            metadata: serde_json::Map::from_iter([(
                "source".to_string(),
                Value::String("simple_mode_chat_ledger_sync".to_string()),
            )]),
        };

        kernel
            .append_context_items(&session_id, WorkflowMode::Chat, bundle.clone())
            .await
            .expect("append first bundle");
        kernel
            .append_context_items(&session_id, WorkflowMode::Task, bundle)
            .await
            .expect("append duplicate bundle");

        let state = kernel
            .get_session_state(&session_id)
            .await
            .expect("state after append");
        assert_eq!(state.session.handoff_context.conversation_context.len(), 1);
        assert_eq!(state.session.handoff_context.artifact_refs, vec!["spec.md"]);
        assert_eq!(
            state.session.handoff_context.context_sources,
            vec!["chat_transcript_sync"]
        );
        assert_eq!(state.session.context_ledger.ledger_version, 2);

        let recovered_kernel =
            WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let recovered = recovered_kernel
            .recover_session(&session_id)
            .await
            .expect("recover deduped session");
        assert_eq!(recovered.handoff_context.conversation_context.len(), 1);
        assert_eq!(recovered.handoff_context.artifact_refs, vec!["spec.md"]);
        assert_eq!(
            recovered.handoff_context.context_sources,
            vec!["chat_transcript_sync"]
        );
        assert_eq!(recovered.context_ledger.ledger_version, 2);
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
    async fn link_mode_session_persists_linked_session_id() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let updated = kernel
            .link_mode_session(&session_id, WorkflowMode::Task, "task-session-123")
            .await
            .expect("link mode session");

        assert_eq!(
            updated
                .linked_mode_sessions
                .get(&WorkflowMode::Task)
                .map(String::as_str),
            Some("task-session-123")
        );
        assert!(
            kernel
                .is_mode_runtime_attached(&session_id, WorkflowMode::Task)
                .await
        );
    }

    #[tokio::test]
    async fn rehydrate_from_linked_sessions_restores_pending_interactions() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Task), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let plan_snapshot = PlanSnapshotRehydrate {
            phase: Some("clarifying".to_string()),
            running_step_id: None,
            pending_clarification: Some(PlanClarificationSnapshot {
                question_id: "plan-q1".to_string(),
                question: "Which scope should be prioritized?".to_string(),
                hint: Some("Choose one primary objective".to_string()),
                input_type: "single_select".to_string(),
                options: vec!["Performance".to_string(), "UX".to_string()],
                required: true,
                allow_custom: false,
            }),
        };
        let task_snapshot = TaskSnapshotRehydrate {
            phase: Some("interviewing".to_string()),
            current_story_id: Some("S001".to_string()),
            completed_stories: Some(2),
            failed_stories: Some(1),
            interview_session_id: Some("interview-session-1".to_string()),
            pending_interview: Some(TaskInterviewSnapshot {
                interview_id: "interview-session-1".to_string(),
                question_id: "task-q1".to_string(),
                question: "Should we keep backward compatibility?".to_string(),
                hint: Some("Answer yes/no with rationale".to_string()),
                required: true,
                input_type: "boolean".to_string(),
                options: vec!["yes".to_string(), "no".to_string()],
                allow_custom: false,
                question_number: 1,
                total_questions: 4,
            }),
        };

        let updated = kernel
            .rehydrate_from_linked_sessions(&session_id, Some(plan_snapshot), Some(task_snapshot))
            .await
            .expect("rehydrate linked sessions");

        let plan = updated.mode_snapshots.plan.as_ref().expect("plan snapshot");
        assert_eq!(plan.phase, "clarifying");
        assert_eq!(
            plan.pending_clarification
                .as_ref()
                .map(|item| item.question_id.as_str()),
            Some("plan-q1")
        );

        let task = updated.mode_snapshots.task.as_ref().expect("task snapshot");
        assert_eq!(task.phase, "interviewing");
        assert_eq!(task.current_story_id.as_deref(), Some("S001"));
        assert_eq!(task.completed_stories, 2);
        assert_eq!(task.failed_stories, 1);
        assert_eq!(
            task.pending_interview
                .as_ref()
                .map(|item| item.question_id.as_str()),
            Some("task-q1")
        );
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

    #[tokio::test]
    async fn recover_session_marks_streaming_chat_as_interrupted() {
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
                    intent_type: UserInputIntentType::SystemPhaseUpdate,
                    content: "phase:streaming".to_string(),
                    metadata: json!({
                        "mode": "chat",
                        "phase": "streaming",
                    }),
                },
            )
            .await
            .expect("set chat streaming phase");

        let recovered_kernel =
            WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let recovered = recovered_kernel
            .recover_session(&session_id)
            .await
            .expect("recover session");

        assert_eq!(
            recovered
                .mode_snapshots
                .chat
                .as_ref()
                .map(|chat| chat.phase.as_str()),
            Some("interrupted")
        );
        assert_eq!(
            recovered.last_error.as_deref(),
            Some("interrupted_by_restart")
        );
    }

    #[tokio::test]
    async fn resume_background_runs_skips_modes_already_attached_in_process() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        kernel
            .rehydrate_from_linked_sessions(
                &session_id,
                Some(PlanSnapshotRehydrate {
                    phase: Some("executing".to_string()),
                    running_step_id: Some("step-1".to_string()),
                    pending_clarification: None,
                }),
                None,
            )
            .await
            .expect("rehydrate plan snapshot");

        let recovered_kernel =
            WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let candidates = recovered_kernel
            .resume_background_runs(Some(&session_id))
            .await
            .expect("list resume candidates");
        assert!(candidates
            .iter()
            .any(|item| { item.session_id == session_id && item.mode == WorkflowMode::Plan }));

        recovered_kernel
            .mark_mode_runtime_attached(&session_id, WorkflowMode::Plan)
            .await;

        let skipped = recovered_kernel
            .resume_background_runs(Some(&session_id))
            .await
            .expect("skip already attached runtime");
        assert!(!skipped
            .iter()
            .any(|item| { item.session_id == session_id && item.mode == WorkflowMode::Plan }));
    }

    #[tokio::test]
    async fn mode_transcript_roundtrip_persists_by_session_and_mode() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let session = kernel
            .open_session(Some(WorkflowMode::Chat), None)
            .await
            .expect("open session");
        let session_id = session.session_id.clone();

        let stored = kernel
            .store_mode_transcript(
                &session_id,
                WorkflowMode::Task,
                vec![
                    json!({"id": 1, "type": "info", "content": "hello"}),
                    json!({"id": 2, "type": "card", "content": "", "cardPayload": {"type": "prd"}}),
                ],
            )
            .await
            .expect("store task transcript");
        assert_eq!(stored.revision, 1);
        assert_eq!(stored.lines.len(), 2);

        let reloaded_kernel =
            WorkflowKernelState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let recovered = reloaded_kernel
            .recover_session(&session_id)
            .await
            .expect("recover session");
        assert_eq!(recovered.session_id, session_id);

        let transcript = reloaded_kernel
            .get_mode_transcript(&session_id, WorkflowMode::Task)
            .await
            .expect("get stored task transcript");
        assert_eq!(transcript.revision, 1);
        assert_eq!(transcript.lines.len(), 2);
        assert_eq!(
            transcript.lines[0]
                .get("content")
                .and_then(|value| value.as_str()),
            Some("hello")
        );
    }
}
