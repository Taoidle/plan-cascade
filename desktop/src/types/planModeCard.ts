/**
 * Plan Mode Card Types
 *
 * Defines all structured message types for the Plan Mode workflow chat.
 * Cards are injected into the chat transcript via execution.appendStreamLine()
 * with type 'card', rendering as rich interactive elements inline.
 */

// ============================================================================
// Phase & Domain Types
// ============================================================================

/** Plan Mode execution phases */
export type PlanModePhase =
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

/** Task domain classification */
export type TaskDomain =
  | 'general'
  | 'writing'
  | 'research'
  | 'marketing'
  | 'data_analysis'
  | 'project_management'
  | { custom: string };

/** Plan persona roles */
export type PlanPersonaRole = 'planner' | 'analyst' | 'executor' | 'reviewer';

// ============================================================================
// Plan Mode Card Types (added to CardType union)
// ============================================================================

export type PlanModeCardType =
  | 'plan_analysis_card'
  | 'plan_clarify_question'
  | 'plan_clarify_answer'
  | 'plan_clarification_resolution'
  | 'plan_card'
  | 'plan_step_update'
  | 'plan_step_output'
  | 'plan_completion_card'
  | 'plan_persona_indicator';

// ============================================================================
// Card Data Interfaces
// ============================================================================

/** Plan analysis card data (Analyzing phase) */
export interface PlanAnalysisCardData {
  domain: string;
  complexity: number;
  estimatedSteps: number;
  needsClarification: boolean;
  reasoning: string;
  adapterName: string;
  suggestedApproach: string;
}

/** Clarification question card data (Clarifying phase) */
export interface PlanClarifyQuestionCardData {
  questionId: string;
  question: string;
  hint: string | null;
  inputType: 'text' | 'textarea' | 'single_select' | 'multi_select' | 'boolean';
  options?: string[];
  allowCustom?: boolean;
}

/** Clarification answer card data (Clarifying phase) */
export interface PlanClarifyAnswerCardData {
  questionId: string;
  answer: string;
  skipped: boolean;
  questionText?: string;
}

/** Clarification recovery card data (Clarification error phase) */
export interface PlanClarificationResolutionCardData {
  title: string;
  message: string;
  reasonCode?: string | null;
  canRetry: boolean;
  canSkip: boolean;
  canCancel: boolean;
}

/** Plan card data (Reviewing Plan phase) */
export interface PlanCardData {
  title: string;
  description: string;
  domain: string;
  adapterName: string;
  steps: PlanStepData[];
  batches: PlanBatchData[];
  executionConfig?: PlanExecutionConfigData;
  editable: boolean;
}

export interface PlanExecutionConfigData {
  maxParallel: number;
  maxStepIterations?: number;
  retry?: PlanRetryPolicyData;
}

export interface PlanRetryPolicyData {
  enabled: boolean;
  maxAttempts: number;
  backoffMs: number;
  failBatchOnExhausted: boolean;
}

/** Plan step data */
export interface PlanStepData {
  id: string;
  title: string;
  description: string;
  priority: 'high' | 'medium' | 'low';
  dependencies: string[];
  completionCriteria: string[];
  expectedOutput: string;
}

/** Plan batch data */
export interface PlanBatchData {
  index: number;
  stepIds: string[];
}

/** Plan step update card data (Executing phase) */
export interface PlanStepUpdateCardData {
  eventType: 'batch_started' | 'step_started' | 'step_completed' | 'step_failed' | 'step_retrying' | 'batch_blocked';
  currentBatch: number;
  totalBatches: number;
  stepId?: string;
  stepTitle?: string;
  stepStatus?: string;
  progressPct: number;
  error?: string;
  attemptCount?: number;
  errorCode?: string;
  diagnostics?: PlanStepOutputDiagnosticData;
}

export interface PlanStepOutputDiagnosticData {
  summary?: string;
  content: string;
  fullContent: string;
  format: 'text' | 'markdown' | 'json' | 'html' | 'code';
  truncated: boolean;
  originalLength: number;
  shownLength: number;
  qualityState?: 'complete' | 'incomplete';
  incompleteReason?: string | null;
  attemptCount?: number;
  toolEvidence?: string[];
  iterations?: number;
  stopReason?: string | null;
  errorCode?: string | null;
}

/** Plan step output card data (Executing phase) */
export interface PlanStepOutputCardData {
  stepId: string;
  stepTitle: string;
  summary?: string;
  content: string;
  fullContent?: string;
  format: 'text' | 'markdown' | 'json' | 'html' | 'code';
  truncated?: boolean;
  originalLength?: number;
  shownLength?: number;
  artifacts?: string[];
  qualityState?: 'complete' | 'incomplete';
  incompleteReason?: string | null;
  attemptCount?: number;
  toolEvidence?: string[];
  iterations?: number;
  stopReason?: string | null;
  errorCode?: string | null;
  criteriaMet: CriterionResultData[];
}

/** Criterion result */
export interface CriterionResultData {
  criterion: string;
  met: boolean;
  explanation: string;
}

/** Plan completion card data (Completed phase) */
export interface PlanCompletionCardData {
  success: boolean;
  terminalState?: 'completed' | 'failed' | 'cancelled';
  planTitle: string;
  totalSteps: number;
  stepsCompleted: number;
  stepsFailed: number;
  stepsCancelled?: number;
  stepsAttempted?: number;
  stepsFailedBeforeCancel?: number;
  totalDurationMs: number;
  stepSummaries: Record<string, string>;
  failureReasons?: Record<string, string>;
  cancelledBy?: string | null;
  runId?: string;
  finalConclusionMarkdown?: string;
  highlights?: string[];
  nextActions?: string[];
  retryStats?: PlanRetryStatsData;
}

export interface PlanRetryStatsData {
  totalRetries: number;
  stepsRetried: number;
  exhaustedFailures: number;
}

/** Plan persona indicator data (all phases) */
export interface PlanPersonaIndicatorData {
  role: PlanPersonaRole;
  displayName: string;
  phase: PlanModePhase;
}

// ============================================================================
// Plan Mode Card Data Map
// ============================================================================

/** Type-safe mapping from PlanModeCardType to its data interface */
export interface PlanModeCardDataMap {
  plan_analysis_card: PlanAnalysisCardData;
  plan_clarify_question: PlanClarifyQuestionCardData;
  plan_clarify_answer: PlanClarifyAnswerCardData;
  plan_clarification_resolution: PlanClarificationResolutionCardData;
  plan_card: PlanCardData;
  plan_step_update: PlanStepUpdateCardData;
  plan_step_output: PlanStepOutputCardData;
  plan_completion_card: PlanCompletionCardData;
  plan_persona_indicator: PlanPersonaIndicatorData;
}

// ============================================================================
// Session & Progress Types
// ============================================================================

/** Plan mode session from backend */
export interface PlanModeSession {
  sessionId: string;
  kernelSessionId?: string | null;
  description: string;
  phase: PlanModePhase;
  analysis: PlanAnalysisCardData | null;
  clarifications: PlanClarifyAnswerCardData[];
  currentQuestion: PlanClarifyQuestionCardData | null;
  plan: PlanCardData | null;
  stepOutputs: Record<string, StepOutputData>;
  stepStates: Record<string, StepExecutionState>;
  progress: PlanExecutionProgress | null;
  createdAt: string;
}

/** Step output from backend */
export interface StepOutputData {
  stepId: string;
  summary?: string;
  content: string;
  fullContent?: string;
  format: string;
  criteriaMet: CriterionResultData[];
  artifacts: string[];
  truncated?: boolean;
  originalLength?: number;
  shownLength?: number;
  qualityState?: 'complete' | 'incomplete';
  incompleteReason?: string | null;
  attemptCount?: number;
  toolEvidence?: string[];
  iterations?: number;
  stopReason?: string | null;
  errorCode?: string | null;
}

/** Step execution state */
export type StepExecutionState =
  | 'pending'
  | 'running'
  | { completed: { durationMs: number } }
  | { failed: { reason: string } }
  | 'cancelled';

/** Execution progress */
export interface PlanExecutionProgress {
  currentBatch: number;
  totalBatches: number;
  stepsCompleted: number;
  stepsFailed: number;
  totalSteps: number;
  progressPct: number;
}

/** Execution report */
export interface PlanExecutionReport {
  sessionId: string;
  planTitle: string;
  success: boolean;
  terminalState: string;
  totalSteps: number;
  stepsCompleted: number;
  stepsFailed: number;
  stepsCancelled?: number;
  stepsAttempted?: number;
  stepsFailedBeforeCancel?: number;
  totalDurationMs: number;
  stepSummaries: Record<string, string>;
  failureReasons: Record<string, string>;
  cancelledBy: string | null;
  runId: string;
  finalConclusionMarkdown: string;
  highlights: string[];
  nextActions: string[];
  retryStats: PlanRetryStatsData;
}

/** Adapter info */
export interface AdapterInfo {
  id: string;
  displayName: string;
  supportedDomains: string[];
}

/** Plan mode progress event from Tauri */
export interface PlanModeProgressPayload {
  sessionId: string;
  eventType: string;
  currentBatch: number;
  totalBatches: number;
  stepId?: string;
  stepStatus?: string;
  error?: string;
  attemptCount?: number;
  errorCode?: string;
  stepOutput?: StepOutputData;
  terminalReport?: PlanExecutionReport;
  progressPct: number;
  runId?: string;
  eventSeq?: number;
  source?: string;
  dropReason?: string | null;
}

/** Execution status response */
export interface PlanExecutionStatusResponse {
  sessionId: string;
  phase: PlanModePhase;
  totalSteps: number;
  stepsCompleted: number;
  stepsFailed: number;
  totalBatches: number;
  progressPct: number;
}
