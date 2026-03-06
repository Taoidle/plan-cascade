/**
 * Workflow Kernel v2 Types
 *
 * Mirrors Rust types from `services/workflow_kernel`.
 */

export type WorkflowMode = 'chat' | 'plan' | 'task';

export type WorkflowStatus = 'active' | 'completed' | 'failed' | 'cancelled';

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

export interface HandoffContextBundle {
  conversationContext: ConversationTurn[];
  artifactRefs: string[];
  contextSources: string[];
  metadata: Record<string, unknown>;
}

export interface ChatState {
  phase: string;
  draftInput: string;
  turnCount: number;
  lastUserMessage: string | null;
  lastAssistantMessage: string | null;
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
  | 'execution_control'
  | 'system_phase_update';

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

export interface WorkflowSession {
  sessionId: string;
  status: WorkflowStatus;
  activeMode: WorkflowMode;
  modeSnapshots: ModeSnapshots;
  handoffContext: HandoffContextBundle;
  linkedModeSessions: Partial<Record<WorkflowMode, string>>;
  lastError: string | null;
  createdAt: string;
  updatedAt: string;
  lastCheckpointId: string | null;
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
