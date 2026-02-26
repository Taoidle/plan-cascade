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
  inputType: 'text' | 'textarea' | 'single_select' | 'boolean';
  options?: string[];
}

/** Clarification answer card data (Clarifying phase) */
export interface PlanClarifyAnswerCardData {
  questionId: string;
  answer: string;
  skipped: boolean;
  questionText?: string;
}

/** Plan card data (Reviewing Plan phase) */
export interface PlanCardData {
  title: string;
  description: string;
  domain: string;
  adapterName: string;
  steps: PlanStepData[];
  batches: PlanBatchData[];
  editable: boolean;
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
  eventType: 'batch_started' | 'step_started' | 'step_completed' | 'step_failed' | 'batch_completed';
  currentBatch: number;
  totalBatches: number;
  stepId?: string;
  stepTitle?: string;
  stepStatus?: string;
  progressPct: number;
  error?: string;
}

/** Plan step output card data (Executing phase) */
export interface PlanStepOutputCardData {
  stepId: string;
  stepTitle: string;
  content: string;
  format: 'text' | 'markdown' | 'json' | 'html' | 'code';
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
  planTitle: string;
  totalSteps: number;
  stepsCompleted: number;
  stepsFailed: number;
  totalDurationMs: number;
  stepSummaries: Record<string, string>;
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
  content: string;
  format: string;
  criteriaMet: CriterionResultData[];
  artifacts: string[];
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
  totalSteps: number;
  stepsCompleted: number;
  stepsFailed: number;
  totalDurationMs: number;
  stepSummaries: Record<string, string>;
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
  progressPct: number;
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
