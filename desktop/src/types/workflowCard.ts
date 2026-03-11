/**
 * Workflow Card Types
 *
 * Defines all structured message types for the Simple Mode Task workflow chat.
 * Cards are injected into the chat transcript via execution.appendStreamLine()
 * with type 'card', rendering as rich interactive elements inline.
 */

import type { GateResult, DimensionScore, GateStatus } from '../store/taskMode';
import type { HandoffSummaryItem, WorkflowMode } from './workflowKernel';
import type {
  DebugCapabilityClass,
  DebugEnvironment,
  DebugHypothesis,
  DebugPatchOperation,
  DebugSeverity,
  DebugToolCategory,
  FixProposal,
  RootCauseReport,
  VerificationReport,
} from './debugMode';

// ============================================================================
// Phase & Card Type Enums
// ============================================================================

/** Canonical workflow phases for the Task mode state machine */
export type WorkflowPhase =
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
  | 'exploration_card'
  | 'file_change'
  | 'turn_change_summary'
  | 'requirement_analysis_card'
  | 'architecture_review_card'
  | 'persona_indicator'
  | 'mode_handoff_card'
  | 'debug_intake_card'
  | 'signal_summary_card'
  | 'reproduction_status_card'
  | 'browser_runtime_card'
  | 'evidence_card'
  | 'console_error_card'
  | 'network_trace_card'
  | 'source_mapping_card'
  | 'performance_trace_card'
  | 'hypothesis_card'
  | 'root_cause_card'
  | 'fix_candidate_card'
  | 'patch_review_card'
  | 'verification_card'
  | 'incident_summary_card'
  // Plan Mode card types
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
// Card Payload (typed path + JSON compatibility replay path)
// ============================================================================

/** Top-level card payload serialized into StreamLine.content and optionally attached as StreamLine.cardPayload. */
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
  exploration_card: ExplorationCardData;
  file_change: FileChangeCardData;
  turn_change_summary: TurnChangeSummaryCardData;
  requirement_analysis_card: RequirementAnalysisCardData;
  architecture_review_card: ArchitectureReviewCardData;
  persona_indicator: PersonaIndicatorData;
  mode_handoff_card: ModeHandoffCardData;
  debug_intake_card: DebugIntakeCardData;
  signal_summary_card: SignalSummaryCardData;
  reproduction_status_card: ReproductionStatusCardData;
  browser_runtime_card: BrowserRuntimeCardData;
  evidence_card: EvidenceCardData;
  console_error_card: ConsoleErrorCardData;
  network_trace_card: NetworkTraceCardData;
  source_mapping_card: SourceMappingCardData;
  performance_trace_card: PerformanceTraceCardData;
  hypothesis_card: HypothesisCardData;
  root_cause_card: RootCauseCardData;
  fix_candidate_card: FixCandidateCardData;
  patch_review_card: PatchReviewCardData;
  verification_card: VerificationCardData;
  incident_summary_card: IncidentSummaryCardData;
  // Plan Mode card data (uses types from planModeCard.ts)
  plan_analysis_card: import('../types/planModeCard').PlanAnalysisCardData;
  plan_clarify_question: import('../types/planModeCard').PlanClarifyQuestionCardData;
  plan_clarify_answer: import('../types/planModeCard').PlanClarifyAnswerCardData;
  plan_clarification_resolution: import('../types/planModeCard').PlanClarificationResolutionCardData;
  plan_card: import('../types/planModeCard').PlanCardData;
  plan_step_update: import('../types/planModeCard').PlanStepUpdateCardData;
  plan_step_output: import('../types/planModeCard').PlanStepOutputCardData;
  plan_completion_card: import('../types/planModeCard').PlanCompletionCardData;
  plan_persona_indicator: import('../types/planModeCard').PlanPersonaIndicatorData;
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
  model?: string;
  recommendationSource?: 'deterministic' | 'llm_enhanced' | 'fallback_deterministic';
}

/** Workflow configuration card data */
export interface ConfigCardData {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  maxParallel: number;
  qualityGatesEnabled: boolean;
  specInterviewEnabled: boolean;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
  isOverridden: boolean;
  recommendationSource?: 'deterministic' | 'llm_enhanced' | 'fallback_deterministic';
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
  allowCustom: boolean;
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
  primaryAction?: 'submit_architecture_review' | 'approve_and_execute';
  revisionSource?: 'architecture_updated';
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
  eventType: 'batch_start' | 'story_start' | 'story_complete' | 'story_failed';
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
  blockingStatus?: 'passed' | 'failed';
  softFailedGateCount?: number;
  gateSource?: 'llm' | 'fallback_heuristic' | 'skipped' | 'mixed';
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

export interface DebugIntakeCardData {
  title: string;
  symptomSummary: string;
  expectedBehavior: string;
  actualBehavior: string;
  reproSteps: string[];
  environment: DebugEnvironment;
  severity: DebugSeverity;
  affectedSurface: string[];
  recentChanges: string | null;
  targetUrlOrEntry: string | null;
}

export interface SignalSummaryCardData {
  summary: string;
  environment: DebugEnvironment;
  severity: DebugSeverity;
  toolCategories: DebugToolCategory[];
  evidenceCount: number;
  highlights: string[];
}

export interface ReproductionStatusCardData {
  status: 'pending' | 'partial' | 'confirmed' | 'failed';
  summary: string;
  reproductionSteps: string[];
  browserArtifacts: string[];
}

export interface BrowserRuntimeCardData {
  bridgeKind: 'devtools_mcp' | 'builtin_browser' | 'unavailable';
  targetUrl: string | null;
  serverName: string | null;
  capabilities: string[];
  notes: string[];
  recommendedCatalogItemId: string | null;
}

export interface EvidenceCardData {
  title: string;
  summary: string;
  source: string;
  collectedAt: string;
  tags: string[];
}

export interface ConsoleErrorCardData {
  currentUrl: string | null;
  collectedAt: string;
  entries: Array<{
    level: string;
    message: string;
    timestamp?: string | null;
  }>;
}

export interface NetworkTraceCardData {
  currentUrl: string | null;
  collectedAt: string;
  totalEvents: number;
  failedEvents: number;
  highlights: string[];
  slowestRequests?: string[];
  harCaptured?: boolean;
}

export interface SourceMappingCardData {
  source: string;
  summary: string;
  candidateFiles: string[];
  bundleScripts?: string[];
  sourceMapUrls?: string[];
  resolvedSources?: string[];
  stackFrames?: string[];
  matchedSourceMaps?: string[];
  originalPositionHints?: string[];
}

export interface PerformanceTraceCardData {
  currentUrl: string | null;
  collectedAt: string;
  summary: string;
  metrics: string[];
  longTasks?: string[];
}

export interface HypothesisCardData {
  hypotheses: DebugHypothesis[];
  recommendedNextChecks: string[];
}

export type RootCauseCardData = RootCauseReport;

export interface FixCandidateCardData extends FixProposal {
  requiresApproval: boolean;
}

export interface PatchReviewCardData {
  title: string;
  summary: string;
  riskLevel: DebugSeverity;
  filesOrSystemsTouched: string[];
  verificationPlan: string[];
  patchPreviewRef?: string | null;
  patchOperations?: DebugPatchOperation[];
  requiredCapabilityClass: DebugCapabilityClass;
  approvalDescription: string;
}

export type VerificationCardData = VerificationReport;

export interface IncidentSummaryCardData {
  title: string;
  environment: DebugEnvironment;
  severity: DebugSeverity;
  summary: string;
  rootCauseConclusion: string | null;
  fixApplied: boolean;
  verificationSummary: string | null;
  residualRisks: string[];
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
  systemOverview: string;
  dataFlow: string;
  infrastructure: {
    existingServices: string[];
    newServices: string[];
  };
  componentsCount: number;
  componentNames: string[];
  components: Array<{
    name: string;
    description: string;
    responsibilities: string[];
    dependencies: string[];
    features: string[];
  }>;
  patternsCount: number;
  patternNames: string[];
  patterns: Array<{
    name: string;
    description: string;
    rationale: string;
    appliesTo: string[];
  }>;
  decisionsCount: number;
  decisions: Array<{
    id: string;
    title: string;
    context: string;
    decision: string;
    rationale: string;
    alternatives: string[];
    status: string;
    appliesTo: string[];
  }>;
  featureMappingsCount: number;
  featureMappings: Array<{
    featureId: string;
    description: string;
    components: string[];
    patterns: string[];
    decisions: string[];
  }>;
  savedPath: string | null;
}

/** Project exploration result card data */
export interface ExplorationCardData {
  techStack: {
    languages: string[];
    frameworks: string[];
    buildTools: string[];
    testFrameworks: string[];
    packageManager: string | null;
  };
  keyFiles: Array<{ path: string; fileType: string; relevance: string }>;
  components: Array<{ name: string; path: string; description: string; fileCount: number }>;
  patterns: string[];
  llmSummary: string | null;
  summaryQuality: 'complete' | 'partial' | 'empty';
  summarySource: 'llm' | 'fallback_synthesized' | 'deterministic_only';
  summaryNotes: string | null;
  durationMs: number;
  usedLlmExploration: boolean;
}

/** Inline file change preview card data */
export interface FileChangeCardData {
  changeId: string;
  filePath: string;
  toolName: 'Write' | 'Edit' | 'Bash';
  changeType: 'new_file' | 'modified' | 'deleted';
  beforeHash: string | null;
  afterHash: string | null;
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
    toolName: 'Write' | 'Edit' | 'Bash';
    changeType: 'new_file' | 'modified' | 'deleted';
    linesAdded: number;
    linesRemoved: number;
  }>;
  totalLinesAdded: number;
  totalLinesRemoved: number;
}

// ============================================================================
// Persona & New Phase Card Data
// ============================================================================

/** Requirement analysis card data (ProductManager persona) */
export interface RequirementAnalysisCardData {
  personaRole: string;
  analysis: string;
  keyRequirements: string[];
  identifiedGaps: string[];
  suggestedScope: string;
}

/** Architecture review card data (SoftwareArchitect persona) */
export interface ArchitectureReviewCardData {
  personaRole: string;
  analysis: string;
  concerns: Array<{ severity: string; description: string }>;
  suggestions: string[];
  prdModifications: Array<{
    operationId: string;
    type: 'update_story' | 'add_story' | 'remove_story' | 'split_story' | 'merge_story';
    targetStoryId: string | null;
    payload: {
      title?: string;
      description?: string;
      priority?: string;
      dependencies?: string[];
      acceptanceCriteria?: string[];
      story?: {
        id?: string;
        title: string;
        description: string;
        priority: string;
        dependencies: string[];
        acceptanceCriteria: string[];
      };
      stories?: Array<{
        id?: string;
        title: string;
        description: string;
        priority: string;
        dependencies: string[];
        acceptanceCriteria: string[];
      }>;
      dependencyRemap?: Record<string, string[]>;
    };
    preview: string;
    reason: string;
    confidence: number;
  }>;
  approved: boolean;
}

/** Persona indicator badge data */
export interface PersonaIndicatorData {
  role: string;
  displayName?: string;
  phase: string;
  model?: string;
}

export interface ModeHandoffCardData {
  sourceMode: WorkflowMode;
  targetMode: WorkflowMode;
  conversationTurns: number;
  summaryItems: HandoffSummaryItem[];
  artifactRefs: string[];
  contextSources: string[];
  metadata: Record<string, unknown>;
}
