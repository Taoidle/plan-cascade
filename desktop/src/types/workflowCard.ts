/**
 * Workflow Card Types
 *
 * Defines all structured message types for the Simple Mode Task workflow chat.
 * Cards are injected into the chat transcript via execution.appendStreamLine()
 * with type 'card', rendering as rich interactive elements inline.
 */

import type { GateResult, DimensionScore, GateStatus } from '../store/taskMode';

// ============================================================================
// Phase & Card Type Enums
// ============================================================================

/** Canonical workflow phases for the Task mode state machine */
export type WorkflowPhase =
  | 'idle'
  | 'analyzing'
  | 'configuring'
  | 'interviewing'
  | 'generating_prd'
  | 'reviewing_prd'
  | 'generating_design_doc'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'cancelled';

/** Card types rendered in the chat transcript */
export type CardType =
  | 'strategy_card'
  | 'config_card'
  | 'interview_question'
  | 'interview_answer'
  | 'prd_card'
  | 'design_doc_card'
  | 'execution_update'
  | 'gate_result'
  | 'completion_report'
  | 'workflow_info'
  | 'workflow_error'
  | 'file_change'
  | 'turn_change_summary';

// ============================================================================
// Card Payload (stored in StreamLine.content as JSON)
// ============================================================================

/** Top-level card payload serialized into StreamLine.content */
export interface CardPayload {
  cardType: CardType;
  cardId: string;
  data: CardDataMap[CardType];
  interactive: boolean;
}

/** Type-safe mapping from CardType to its data interface */
export interface CardDataMap {
  strategy_card: StrategyCardData;
  config_card: ConfigCardData;
  interview_question: InterviewQuestionCardData;
  interview_answer: InterviewAnswerCardData;
  prd_card: PrdCardData;
  design_doc_card: DesignDocCardData;
  execution_update: ExecutionUpdateCardData;
  gate_result: GateResultCardData;
  completion_report: CompletionReportCardData;
  workflow_info: WorkflowInfoData;
  workflow_error: WorkflowErrorData;
  file_change: FileChangeCardData;
  turn_change_summary: TurnChangeSummaryCardData;
}

// ============================================================================
// Card Data Interfaces
// ============================================================================

/** Strategy analysis card data */
export interface StrategyCardData {
  strategy: string;
  confidence: number;
  reasoning: string;
  riskLevel: string;
  estimatedStories: number;
  parallelizationBenefit: string;
  functionalAreas: string[];
  recommendations: string[];
}

/** Workflow configuration card data */
export interface ConfigCardData {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  maxParallel: number;
  qualityGatesEnabled: boolean;
  specInterviewEnabled: boolean;
  isOverridden: boolean;
}

/** Workflow configuration values (used by orchestrator) */
export interface WorkflowConfig {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  maxParallel: number;
  qualityGatesEnabled: boolean;
  specInterviewEnabled: boolean;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
}

/** Interview question card data */
export interface InterviewQuestionCardData {
  questionId: string;
  question: string;
  hint: string | null;
  required: boolean;
  inputType: 'text' | 'textarea' | 'single_select' | 'multi_select' | 'boolean';
  options: string[];
  questionNumber: number;
  totalQuestions: number;
}

/** Interview answer card data */
export interface InterviewAnswerCardData {
  questionId: string;
  answer: string;
  skipped: boolean;
}

/** PRD card data */
export interface PrdCardData {
  title: string;
  description: string;
  stories: PrdStoryData[];
  batches: PrdBatchData[];
  isEditable: boolean;
}

export interface PrdStoryData {
  id: string;
  title: string;
  description: string;
  priority: string;
  dependencies: string[];
  acceptanceCriteria: string[];
}

export interface PrdBatchData {
  index: number;
  storyIds: string[];
}

/** Execution update card data */
export interface ExecutionUpdateCardData {
  eventType: 'batch_start' | 'story_start' | 'story_complete' | 'story_failed' | 'batch_complete';
  currentBatch: number;
  totalBatches: number;
  storyId: string | null;
  storyTitle: string | null;
  status: string;
  agent: string | null;
  progressPct: number;
}

/** Quality gate result card data */
export interface GateResultCardData {
  storyId: string;
  storyTitle: string;
  overallStatus: GateStatus;
  gates: GateResult[];
  codeReviewScores: DimensionScore[];
}

/** Completion report card data */
export interface CompletionReportCardData {
  success: boolean;
  totalStories: number;
  completed: number;
  failed: number;
  duration: number;
  agentAssignments: Record<string, string>;
}

/** Informational workflow message */
export interface WorkflowInfoData {
  message: string;
  level: 'info' | 'success' | 'warning';
}

/** Workflow error message */
export interface WorkflowErrorData {
  title: string;
  description: string;
  suggestedFix: string | null;
}

/** Design document summary card data */
export interface DesignDocCardData {
  title: string;
  summary: string;
  componentsCount: number;
  componentNames: string[];
  patternsCount: number;
  patternNames: string[];
  decisionsCount: number;
  featureMappingsCount: number;
  savedPath: string | null;
}

/** Inline file change preview card data */
export interface FileChangeCardData {
  changeId: string;
  filePath: string;
  toolName: 'Write' | 'Edit';
  changeType: 'new_file' | 'modified';
  beforeHash: string | null;
  afterHash: string;
  diffPreview: string | null;
  linesAdded: number;
  linesRemoved: number;
  sessionId: string;
  turnIndex: number;
  description: string;
}

/** Turn change summary card data */
export interface TurnChangeSummaryCardData {
  turnIndex: number;
  sessionId: string;
  totalFiles: number;
  files: Array<{
    filePath: string;
    toolName: 'Write' | 'Edit';
    changeType: 'new_file' | 'modified';
    linesAdded: number;
    linesRemoved: number;
  }>;
  totalLinesAdded: number;
  totalLinesRemoved: number;
}
