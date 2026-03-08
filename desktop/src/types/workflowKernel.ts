/**
 * Workflow Kernel v2 Types
 *
 * Mirrors Rust types from `services/workflow_kernel`.
 */

export type WorkflowMode = 'chat' | 'plan' | 'task';

export type WorkflowStatus = 'active' | 'completed' | 'failed' | 'cancelled' | 'archived';

export type WorkflowSessionKind = 'simple_root';

export type WorkflowBackgroundState = 'foreground' | 'background_idle' | 'background_running' | 'interrupted';

export type ChatLifecyclePhase =
  | 'ready'
  | 'submitting'
  | 'streaming'
  | 'paused'
  | 'failed'
  | 'cancelled'
  | 'interrupted';

export type TaskLifecyclePhase =
  | 'idle'
  | 'analyzing'
  | 'configuring'
  | 'interviewing'
  | 'exploring'
  | 'requirement_analysis'
  | 'generating_prd'
  | 'reviewing_prd'
  | 'architecture_review'
  | 'generating_design_doc'
  | 'executing'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type PlanLifecyclePhase =
  | 'idle'
  | 'analyzing'
  | 'clarifying'
  | 'clarification_error'
  | 'planning'
  | 'reviewing_plan'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface ConversationTurn {
  user: string;
  assistant: string;
}

export interface HandoffSummaryItem {
  id: string;
  sourceMode: WorkflowMode;
  kind: string;
  title: string;
  body: string;
  artifactRefs: string[];
  metadata: Record<string, unknown>;
  createdAt: string;
}

export interface HandoffContextBundle {
  conversationContext: ConversationTurn[];
  summaryItems?: HandoffSummaryItem[];
  artifactRefs: string[];
  contextSources: string[];
  metadata: Record<string, unknown>;
}

export interface ChatState {
  phase: ChatLifecyclePhase | string;
  pendingInput: string;
  activeTurnId: string | null;
  turnCount: number;
  lastUserMessage: string | null;
  lastAssistantMessage: string | null;
  entryHandoff?: HandoffContextBundle;
}

export interface PlanClarificationSnapshot {
  questionId: string;
  question: string;
  hint: string | null;
  inputType: string;
  options: string[];
  required: boolean;
  allowCustom?: boolean;
}

export interface PlanState {
  phase: PlanLifecyclePhase;
  planId: string | null;
  runningStepId: string | null;
  pendingClarification: PlanClarificationSnapshot | null;
  retryableSteps: string[];
  planRevision: number;
  lastEditOperation: string | null;
  runId?: string | null;
  backgroundStatus?: string | null;
  resumableFromCheckpoint?: boolean;
  lastCheckpointId?: string | null;
  entryHandoff?: HandoffContextBundle;
}

export interface TaskInterviewSnapshot {
  interviewId: string;
  questionId: string;
  question: string;
  hint: string | null;
  required: boolean;
  inputType: string;
  options: string[];
  allowCustom: boolean;
  questionNumber: number;
  totalQuestions: number;
}

export interface TaskState {
  phase: TaskLifecyclePhase;
  prdId: string | null;
  currentStoryId: string | null;
  interviewSessionId: string | null;
  pendingInterview: TaskInterviewSnapshot | null;
  completedStories: number;
  failedStories: number;
  runId?: string | null;
  backgroundStatus?: string | null;
  resumableFromCheckpoint?: boolean;
  lastCheckpointId?: string | null;
  entryHandoff?: HandoffContextBundle;
}

export type ModeState =
  | { kind: 'chat'; state: ChatState }
  | { kind: 'plan'; state: PlanState }
  | { kind: 'task'; state: TaskState };

export interface ModeSnapshots {
  chat: ChatState | null;
  plan: PlanState | null;
  task: TaskState | null;
}

export type WorkflowEventKind =
  | 'session_opened'
  | 'mode_transitioned'
  | 'mode_session_linked'
  | 'input_submitted'
  | 'context_appended'
  | 'plan_edited'
  | 'plan_execution_started'
  | 'plan_step_retried'
  | 'operation_cancelled'
  | 'session_recovered'
  | 'checkpoint_created';

export interface WorkflowEventV2 {
  eventId: string;
  sessionId: string;
  kind: WorkflowEventKind;
  mode: WorkflowMode;
  createdAt: string;
  payload: Record<string, unknown>;
}

export type UserInputIntentType =
  | 'chat_message'
  | 'mode_entry_prompt'
  | 'follow_up_intent'
  | 'plan_clarification'
  | 'plan_edit_instruction'
  | 'plan_approval'
  | 'task_configuration'
  | 'task_interview_answer'
  | 'task_prd_feedback'
  | 'execution_control';

export interface UserInputIntent {
  type: UserInputIntentType;
  content: string;
  metadata?: Record<string, unknown> | null;
}

export type PlanEditOperationType =
  | 'add_step'
  | 'update_step'
  | 'remove_step'
  | 'reorder_step'
  | 'set_dependency'
  | 'clear_dependency'
  | 'set_parallelism';

export interface PlanEditOperation {
  type: PlanEditOperationType;
  targetStepId?: string | null;
  payload?: Record<string, unknown> | null;
}

export interface ChatControlCapabilities {
  canPause: boolean;
  canResume: boolean;
  canCancel: boolean;
}

export interface WorkflowSession {
  sessionId: string;
  sessionKind: WorkflowSessionKind;
  displayTitle: string;
  workspacePath: string | null;
  status: WorkflowStatus;
  activeMode: WorkflowMode;
  modeSnapshots: ModeSnapshots;
  handoffContext: HandoffContextBundle;
  linkedModeSessions: Partial<Record<WorkflowMode, string>>;
  backgroundState: WorkflowBackgroundState;
  contextLedger: WorkflowContextLedgerSummary;
  modeRuntimeMeta: Partial<Record<WorkflowMode, ModeRuntimeMeta>>;
  lastError: string | null;
  createdAt: string;
  updatedAt: string;
  lastCheckpointId: string | null;
}

export interface ModeRuntimeMeta {
  mode: WorkflowMode;
  runId: string | null;
  bindingSessionId: string | null;
  isForeground: boolean;
  isBackgroundRunning: boolean;
  isInterrupted: boolean;
  resumePolicy: string;
  lastHeartbeatAt: string | null;
  lastCheckpointId: string | null;
  lastError: string | null;
  backendKind?: string | null;
  controlCapabilities?: ChatControlCapabilities | null;
  blockReason?: string | null;
}

export interface WorkflowContextLedgerSummary {
  conversationTurnCount: number;
  artifactRefCount: number;
  contextSourceKinds: string[];
  lastCompactionAt: string | null;
  ledgerVersion: number;
}

export interface WorkflowSessionCatalogItem {
  sessionId: string;
  sessionKind: WorkflowSessionKind;
  displayTitle: string;
  workspacePath: string | null;
  activeMode: WorkflowMode;
  status: WorkflowStatus;
  backgroundState: WorkflowBackgroundState;
  updatedAt: string;
  createdAt: string;
  lastError: string | null;
  contextLedger: WorkflowContextLedgerSummary;
  modeSnapshots: ModeSnapshots;
  modeRuntimeMeta: Partial<Record<WorkflowMode, ModeRuntimeMeta>>;
}

export interface WorkflowSessionCatalogState {
  activeSessionId: string | null;
  sessions: WorkflowSessionCatalogItem[];
}

export interface ModeTranscriptPayload {
  sessionId: string;
  mode: WorkflowMode;
  revision: number;
  lines: unknown[];
}

export interface ModeTranscriptPatch {
  replaceFromLineId?: number | null;
  appendedLines: unknown[];
}

export interface ModeTranscriptState {
  revision: number;
  lines: unknown[];
  loaded: boolean;
  unread: boolean;
}

export interface WorkflowSessionCatalogUpdatedEvent {
  activeSessionId: string | null;
  sessions: WorkflowSessionCatalogItem[];
  source: string;
}

export interface WorkflowModeTranscriptUpdatedEvent {
  sessionId: string;
  mode: WorkflowMode;
  revision: number;
  appendedLines: unknown[];
  replaceFromLineId?: number | null;
  lines?: unknown[];
  source: string;
}

export interface ResumeResult {
  sessionId: string;
  mode: WorkflowMode;
  resumed: boolean;
  reason: string;
}

export interface WorkflowCheckpoint {
  checkpointId: string;
  sessionId: string;
  createdAt: string;
  reason: string;
  reasonCode: string;
  eventCount: number;
  snapshot: WorkflowSession;
}

export interface WorkflowSessionState {
  session: WorkflowSession;
  events: WorkflowEventV2[];
  checkpoints: WorkflowCheckpoint[];
}

export interface WorkflowKernelUpdatedEvent {
  sessionState: WorkflowSessionState;
  revision: number;
  source: string;
}
