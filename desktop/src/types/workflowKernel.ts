/**
 * Workflow Kernel v2 Types
 *
 * Mirrors Rust types from `services/workflow_kernel`.
 */

import type {
  DebugCapabilityProfile,
  DebugEnvironment,
  DebugPendingApproval,
  DebugRuntimeCapabilities,
  DebugSeverity,
  DebugState,
} from './debugMode';
import type { ModeQualitySnapshot } from './workflowQuality';

export type WorkflowMode = 'chat' | 'plan' | 'task' | 'debug';

export type WorkflowStatus = 'active' | 'completed' | 'failed' | 'cancelled' | 'archived';

export type WorkflowSessionKind = 'simple_root';

export type WorkflowBackgroundState = 'foreground' | 'background_idle' | 'background_running' | 'interrupted';
export type WorkflowRuntimeKind = 'main' | 'managed_worktree' | 'legacy_worktree';

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
  quality?: ModeQualitySnapshot | null;
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
  quality?: ModeQualitySnapshot | null;
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

export type StrategyRecommendationSource = 'deterministic' | 'llm_enhanced' | 'fallback_deterministic';

export interface TaskStrategyAnalysisSnapshot {
  functionalAreas: string[];
  estimatedStories: number;
  hasDependencies: boolean;
  riskLevel: 'low' | 'medium' | 'high';
  parallelizationBenefit: 'none' | 'moderate' | 'significant';
  recommendedMode: 'chat' | 'task';
  confidence: number;
  reasoning: string;
  strategyDecision: {
    strategy: string;
    confidence: number;
    reasoning: string;
  };
}

export interface TaskRecommendedWorkflowConfigSnapshot {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  specInterviewEnabled: boolean;
  qualityGatesEnabled: boolean;
  maxParallel: number;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
}

export interface TaskStrategyRecommendationSnapshot {
  analysis: TaskStrategyAnalysisSnapshot;
  recommendedConfig: TaskRecommendedWorkflowConfigSnapshot;
  recommendationSource: StrategyRecommendationSource;
  reasoning: string;
  confidence: number;
  configRationale: string[];
}

export interface TaskWorkflowConfigSnapshot {
  flowLevel: string | null;
  tddMode: string | null;
  enableInterview: boolean;
  qualityGatesEnabled: boolean;
  selectedQualityGateIds: string[];
  qualityRetryMaxAttempts: number | null;
  customQualityGates: import('./workflowQuality').QualityCustomGate[];
  maxParallel: number | null;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
}

export type TaskConfigConfirmationState = 'pending' | 'confirmed';

export interface TaskState {
  phase: TaskLifecyclePhase;
  prdId: string | null;
  currentStoryId: string | null;
  interviewSessionId: string | null;
  pendingInterview: TaskInterviewSnapshot | null;
  completedStories: number;
  failedStories: number;
  strategyRecommendation?: TaskStrategyRecommendationSnapshot | null;
  configConfirmationState?: TaskConfigConfirmationState;
  confirmedConfig?: TaskWorkflowConfigSnapshot | null;
  runId?: string | null;
  backgroundStatus?: string | null;
  resumableFromCheckpoint?: boolean;
  lastCheckpointId?: string | null;
  entryHandoff?: HandoffContextBundle;
  quality?: ModeQualitySnapshot | null;
}

export type ModeState =
  | { kind: 'chat'; state: ChatState }
  | { kind: 'plan'; state: PlanState }
  | { kind: 'task'; state: TaskState }
  | { kind: 'debug'; state: DebugState };

export interface ModeSnapshots {
  chat: ChatState | null;
  plan: PlanState | null;
  task: TaskState | null;
  debug?: DebugState | null;
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
  | 'checkpoint_created'
  | 'quality_run_started'
  | 'quality_gate_updated'
  | 'quality_run_completed'
  | 'quality_decision_required'
  | 'quality_decision_applied';

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
  | 'debug_intake'
  | 'debug_clarification'
  | 'debug_hypothesis_feedback'
  | 'debug_patch_approval'
  | 'debug_verification_control'
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
  runtime?: SessionRuntimeInfo;
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
  debugCapabilityProfile?: DebugCapabilityProfile | null;
  debugRuntimeCapabilities?: DebugRuntimeCapabilities | null;
  debugEnvironment?: DebugEnvironment | null;
  debugSeverity?: DebugSeverity | null;
  pendingApproval?: DebugPendingApproval | null;
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
  runtime?: SessionRuntimeInfo;
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

export interface SessionRuntimeInfo {
  rootPath: string | null;
  runtimePath: string | null;
  runtimeKind: WorkflowRuntimeKind;
  displayLabel?: string | null;
  branch?: string | null;
  targetBranch?: string | null;
  managedWorktreeId?: string | null;
  legacy: boolean;
  runtimeStatus?: import('./git').WorktreeStatus | null;
  prStatus?: import('./git').PullRequestInfo | null;
}
