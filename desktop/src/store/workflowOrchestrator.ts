/**
 * Workflow Orchestrator Store
 *
 * Orchestrates the full Task mode workflow lifecycle in Simple mode.
 * Drives the state machine: idle → analyzing → configuring → [interviewing]
 * → generating_prd → reviewing_prd → executing → completed/failed.
 *
 * Delegates to existing stores:
 * - useTaskModeStore: backend session lifecycle (enterTaskMode, generatePrd, approvePrd)
 * - useSpecInterviewStore: interview flow (startInterview, submitAnswer, compileSpec)
 * - mode transcript routing: append cards into rootSession + task transcript
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../i18n';
import {
  useTaskModeStore,
  type TaskPrd,
  type StrategyAnalysis,
  type GateResult,
  type ExecutionReport,
  type StoryQualityGateResults,
  type PrdFeedbackApplySummary,
} from './taskMode';
import { useSpecInterviewStore, type InterviewQuestion } from './specInterview';
import { useSettingsStore } from './settings';
import { useWorkflowKernelStore } from './workflowKernel';
import { selectKernelTaskRuntime } from './workflowKernelSelectors';
import { deriveGateOverallStatus } from '../lib/gateStatus';
import { parseWorkflowConfigNatural } from '../lib/workflowConfigNaturalParser';
import { failResult, okResult, type ActionResult } from '../types/actionResult';
import { applyArchitectureModifications } from './workflowOrchestrator/architectureModifications';
import {
  appendWorkflowUserMessage,
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from './workflowOrchestrator/cardInjection';
import { runExplorePhase } from './workflowOrchestrator/phases/explorePhase';
import { runRequirementPhase } from './workflowOrchestrator/phases/requirementPhase';
import { runArchitecturePhase } from './workflowOrchestrator/phases/architecturePhase';
import { runDesignDocAndExecutionPhase } from './workflowOrchestrator/phases/executionPhase';
import { runPrdPhase } from './workflowOrchestrator/phases/prdPhase';
import { runInterviewPhase } from './workflowOrchestrator/phases/interviewPhase';
import type { WorkflowPhaseRuntime } from './workflowOrchestrator/phases/runtime';
import type {
  WorkflowPhase,
  WorkflowConfig,
  InterviewQuestionCardData,
  StrategyCardData,
  ConfigCardData,
  PrdCardData,
  ExplorationCardData,
  ExecutionUpdateCardData,
  GateResultCardData,
  CompletionReportCardData,
  InterviewAnswerCardData,
  RequirementAnalysisCardData,
  ArchitectureReviewCardData,
} from '../types/workflowCard';

// ============================================================================
// Types
// ============================================================================

interface WorkflowOrchestratorState {
  /** Current workflow phase */
  phase: WorkflowPhase;

  /** Task mode session ID (from useTaskModeStore) */
  sessionId: string | null;

  /** Spec interview session ID */
  interviewId: string | null;

  /** Original task description */
  taskDescription: string;

  /** Workflow configuration */
  config: WorkflowConfig;

  /** Strategy analysis result (cached from taskMode) */
  strategyAnalysis: StrategyAnalysis | null;

  /** Project exploration result */
  explorationResult: ExplorationCardData | null;

  /** Working copy of PRD during review phase */
  editablePrd: TaskPrd | null;

  /** Currently pending interview question */
  pendingInterviewQuestion: InterviewQuestionCardData | null;

  /** Execution projection: current batch */
  currentBatch: number;

  /** Execution projection: total batches */
  totalBatches: number;

  /** Execution projection: per-story status */
  storyStatuses: Record<string, string>;

  /** Execution projection: per-story gate results */
  qualityGateResults: Record<string, StoryQualityGateResults>;

  /** Execution projection: completion report */
  report: ExecutionReport | null;

  /** Requirement analysis result (from PM persona) */
  requirementAnalysis: RequirementAnalysisCardData | null;

  /** Architecture review result (from Architect persona) */
  architectureReview: ArchitectureReviewCardData | null;

  /** Architecture review iteration counter (max 3 to prevent loops) */
  architectureReviewRound: number;

  /** Error message */
  error: string | null;

  /** True while waiting for backend cancel confirmation */
  isCancelling: boolean;

  /** Event unsubscribe function */
  _unlistenFn: UnlistenFn | null;

  /** Guards async continuations after cancel/reset */
  _runToken: number;

  /** Prevent duplicate completion-card injection for a single run token */
  _completionCardInjectedRunToken: number | null;

  // Actions
  startWorkflow: (description: string, kernelSessionId?: string | null) => Promise<{ modeSessionId: string | null }>;
  confirmConfig: (overrides?: Partial<WorkflowConfig>) => Promise<ActionResult>;
  updateConfig: (updates: Partial<WorkflowConfig>) => void;
  overrideConfigNatural: (text: string) => void;
  submitInterviewAnswer: (answer: string) => Promise<void>;
  skipInterviewQuestion: () => Promise<void>;
  approvePrd: (editedPrd?: TaskPrd) => Promise<ActionResult>;
  updateEditableStory: (
    storyId: string,
    updates: Partial<{ title: string; description: string; priority: string; acceptanceCriteria: string[] }>,
  ) => void;
  addPrdFeedback: (feedback: string) => Promise<ActionResult>;
  approveArchitecture: (
    acceptAsIs: boolean,
    selectedModifications: ArchitectureReviewCardData['prdModifications'],
  ) => Promise<ActionResult>;
  cancelWorkflow: () => Promise<void>;
  resetWorkflow: () => void;
}

// ============================================================================
// Default Config
// ============================================================================

const DEFAULT_CONFIG: WorkflowConfig = {
  flowLevel: 'standard',
  tddMode: 'off',
  maxParallel: 4,
  qualityGatesEnabled: true,
  specInterviewEnabled: false,
  skipVerification: false,
  skipReview: false,
  globalAgentOverride: null,
  implAgentOverride: null,
};

const DEFAULT_STATE = {
  phase: 'idle' as WorkflowPhase,
  sessionId: null as string | null,
  interviewId: null as string | null,
  taskDescription: '',
  config: { ...DEFAULT_CONFIG },
  strategyAnalysis: null as StrategyAnalysis | null,
  explorationResult: null as ExplorationCardData | null,
  editablePrd: null as TaskPrd | null,
  pendingInterviewQuestion: null as InterviewQuestionCardData | null,
  currentBatch: 0,
  totalBatches: 0,
  storyStatuses: {} as Record<string, string>,
  qualityGateResults: {} as Record<string, StoryQualityGateResults>,
  report: null as ExecutionReport | null,
  requirementAnalysis: null as RequirementAnalysisCardData | null,
  architectureReview: null as ArchitectureReviewCardData | null,
  architectureReviewRound: 0,
  error: null as string | null,
  isCancelling: false,
  _unlistenFn: null as UnlistenFn | null,
  _runToken: 0,
  _completionCardInjectedRunToken: null as number | null,
};

function isRunActive(get: GetFn, runToken: number): boolean {
  return get()._runToken === runToken;
}

/** Map backend InterviewQuestion to card data */
function mapInterviewQuestion(
  q: InterviewQuestion,
  questionNumber: number,
  totalQuestions: number,
): InterviewQuestionCardData {
  let inputType: InterviewQuestionCardData['inputType'];
  switch (q.input_type) {
    case 'text':
      inputType = 'text';
      break;
    case 'textarea':
      inputType = 'textarea';
      break;
    case 'list':
      inputType = 'text';
      break;
    case 'boolean':
      inputType = 'boolean';
      break;
    case 'single_select':
      inputType = 'single_select';
      break;
    case 'multi_select':
      inputType = 'multi_select';
      break;
    default:
      inputType = 'text';
  }

  return {
    questionId: q.id,
    question: q.question,
    hint: q.hint,
    required: q.required,
    inputType,
    options: q.options ?? [],
    allowCustom: q.allow_custom ?? true,
    questionNumber,
    totalQuestions,
  };
}

/** Build strategy card data from analysis */
function buildStrategyCardData(analysis: StrategyAnalysis, model?: string): StrategyCardData {
  const recommendations: string[] = [];
  if (analysis.parallelizationBenefit === 'significant') {
    recommendations.push(i18n.t('workflow.strategy.highParallelization', { ns: 'simpleMode' }));
  }
  if (analysis.riskLevel === 'high') {
    recommendations.push(i18n.t('workflow.strategy.highRisk', { ns: 'simpleMode' }));
  }
  if (analysis.estimatedStories > 6) {
    recommendations.push(
      i18n.t('workflow.strategy.manyStories', { ns: 'simpleMode', count: analysis.estimatedStories }),
    );
  }

  return {
    strategy: analysis.strategyDecision?.strategy ?? analysis.recommendedMode,
    confidence: analysis.strategyDecision?.confidence ?? analysis.confidence,
    reasoning: analysis.strategyDecision?.reasoning ?? analysis.reasoning,
    riskLevel: analysis.riskLevel,
    estimatedStories: analysis.estimatedStories,
    parallelizationBenefit: analysis.parallelizationBenefit,
    functionalAreas: analysis.functionalAreas,
    recommendations,
    model,
  };
}

/** Build config card data from current config */
function buildConfigCardData(config: WorkflowConfig, isOverridden: boolean): ConfigCardData {
  return {
    flowLevel: config.flowLevel,
    tddMode: config.tddMode,
    maxParallel: config.maxParallel,
    qualityGatesEnabled: config.qualityGatesEnabled,
    specInterviewEnabled: config.specInterviewEnabled,
    isOverridden,
  };
}

function toPrdCardData(prd: TaskPrd): PrdCardData {
  return {
    title: prd.title,
    description: prd.description,
    stories: prd.stories.map((s) => ({
      id: s.id,
      title: s.title,
      description: s.description,
      priority: s.priority,
      dependencies: s.dependencies,
      acceptanceCriteria: s.acceptanceCriteria,
    })),
    batches: prd.batches.map((b) => ({
      index: b.index,
      storyIds: b.storyIds,
    })),
    isEditable: true,
  };
}

function normalizeExplorationCardData(data: ExplorationCardData): ExplorationCardData {
  const summary = data.llmSummary?.trim() || null;
  const quality = data.summaryQuality ?? (summary ? (summary.length < 220 ? 'partial' : 'complete') : 'empty');
  return {
    ...data,
    llmSummary: summary,
    summaryQuality: quality,
    summarySource: data.summarySource ?? (summary ? 'llm' : 'deterministic_only'),
    summaryNotes: data.summaryNotes ?? null,
  };
}

function synthesizePlanningTurnForPrdPhase(_taskDescription: string, _strategyAnalysis: unknown, _prd: TaskPrd) {
  // Cross-mode task handoff is published by the backend into the kernel ledger.
}

function buildCompletionReportDataFromReport(report: ExecutionReport): CompletionReportCardData {
  return {
    success: report.success,
    totalStories: report.totalStories,
    completed: report.storiesCompleted,
    failed: report.storiesFailed,
    duration: report.totalDurationMs,
    agentAssignments: report.agentAssignments,
  };
}

function buildCompletionReportDataFallback(params: {
  totalStories: number;
  completed: number;
  failed: number;
  success: boolean;
}): CompletionReportCardData {
  return {
    success: params.success,
    totalStories: params.totalStories,
    completed: params.completed,
    failed: params.failed,
    duration: 0,
    agentAssignments: {},
  };
}

function buildPrdFeedbackSummaryMessage(summary: PrdFeedbackApplySummary): string {
  const segments: string[] = [];
  if (summary.addedStoryIds.length > 0) {
    segments.push(`Added: ${summary.addedStoryIds.join(', ')}`);
  }
  if (summary.updatedStoryIds.length > 0) {
    segments.push(`Updated: ${summary.updatedStoryIds.join(', ')}`);
  }
  if (summary.removedStoryIds.length > 0) {
    segments.push(`Removed: ${summary.removedStoryIds.join(', ')}`);
  }
  if (summary.batchChanges.length > 0) {
    segments.push(`Batch changes: ${summary.batchChanges.join(' | ')}`);
  }
  if (summary.warnings.length > 0) {
    segments.push(`Warnings: ${summary.warnings.join(' | ')}`);
  }
  if (segments.length === 0) {
    return 'PRD updated without structural changes.';
  }
  return segments.join('\n');
}

function resolveTaskSessionId(
  get: () => WorkflowOrchestratorState,
  set: (
    partial:
      | Partial<WorkflowOrchestratorState>
      | ((state: WorkflowOrchestratorState) => Partial<WorkflowOrchestratorState>),
  ) => void,
): string | null {
  const localSessionId = get().sessionId?.trim() ?? '';
  if (localSessionId.length > 0) return localSessionId;

  const linkedSessionId =
    selectKernelTaskRuntime(useWorkflowKernelStore.getState().session).linkedSessionId?.trim() ?? '';
  if (linkedSessionId.length > 0) {
    set({ sessionId: linkedSessionId });
    return linkedSessionId;
  }
  return null;
}

// ============================================================================
// Store
// ============================================================================

export const useWorkflowOrchestratorStore = create<WorkflowOrchestratorState>()((set, get) => ({
  ...DEFAULT_STATE,

  /**
   * Start the full workflow from a task description.
   * Phase transitions: idle → analyzing → configuring
   */
  startWorkflow: async (description: string, kernelSessionId?: string | null) => {
    const runToken = get()._runToken + 1;
    let modeSessionId: string | null = null;
    // Add user message as 'info' StreamLine so it appears as a chat bubble in ChatTranscript
    appendWorkflowUserMessage(description);

    set({ phase: 'analyzing', taskDescription: description, error: null, isCancelling: false, _runToken: runToken });
    injectInfo(i18n.t('workflow.orchestrator.analyzingTask', { ns: 'simpleMode' }), 'info');

    try {
      // 1. Enter task mode (creates session + runs strategy analysis)
      const enteredSession = await useTaskModeStore.getState().enterTaskMode(description, kernelSessionId);
      if (!isRunActive(get, runToken)) return { modeSessionId: null };

      const taskModeError = useTaskModeStore.getState().error;
      if (taskModeError || !enteredSession) {
        if (!isRunActive(get, runToken)) return { modeSessionId: null };
        set({ phase: 'failed', error: taskModeError || 'Failed to enter task mode' });
        injectError(
          i18n.t('workflow.orchestrator.strategyAnalysisFailed', { ns: 'simpleMode' }),
          taskModeError || 'Failed to enter task mode',
        );
        return { modeSessionId: null };
      }

      const sessionId = enteredSession.sessionId;
      modeSessionId = sessionId;
      let analysis = enteredSession.strategyAnalysis;

      // 2. Try LLM enhancement of strategy analysis
      const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
      if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
      const strategyResolved = resolvePhaseAgent('plan_strategy');
      if (analysis) {
        try {
          if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
          injectInfo(i18n.t('workflow.orchestrator.enhancingAnalysis', { ns: 'simpleMode' }), 'info');

          const enhanced = await invoke<{ success: boolean; data: StrategyAnalysis | null; error: string | null }>(
            'enhance_strategy_with_llm',
            {
              description,
              keywordAnalysis: analysis,
              provider: strategyResolved.provider || null,
              model: strategyResolved.model || null,
              apiKey: null,
              baseUrl: strategyResolved.baseUrl || null,
              locale: i18n.language,
            },
          );
          if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };

          if (enhanced.success && enhanced.data) {
            analysis = enhanced.data;
          }
        } catch {
          // LLM enhancement failed — silently use keyword analysis
        }
      }

      if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
      set({ sessionId, strategyAnalysis: analysis });

      // 3. Inject strategy card (with LLM-enhanced or keyword result)
      if (analysis) {
        if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
        injectCard('strategy_card', buildStrategyCardData(analysis, formatModelDisplay(strategyResolved)));
      }

      // 4. Build recommended config from analysis
      const config: WorkflowConfig = { ...DEFAULT_CONFIG };
      if (analysis) {
        if (analysis.riskLevel === 'high') {
          config.tddMode = 'flexible';
          config.qualityGatesEnabled = true;
        }
        if (analysis.estimatedStories > 6) {
          config.maxParallel = 6;
        }
        if (analysis.parallelizationBenefit === 'none') {
          config.maxParallel = 2;
        }
        // Enable interview for high-risk or high-story-count tasks in standard/full flow
        if (config.flowLevel !== 'quick' && (analysis.riskLevel === 'high' || analysis.estimatedStories > 8)) {
          config.specInterviewEnabled = true;
        }
      }

      if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
      set({ config, phase: 'configuring' });

      // 5. Inject config card (user interacts with it to advance)
      if (!isRunActive(get, runToken)) return { modeSessionId: modeSessionId ?? null };
      injectCard('config_card', buildConfigCardData(config, false), true);
      return { modeSessionId: modeSessionId ?? null };
    } catch (e) {
      if (!isRunActive(get, runToken)) return { modeSessionId: null };
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError(i18n.t('workflow.orchestrator.workflowFailed', { ns: 'simpleMode' }), msg);
      return { modeSessionId: null };
    }
  },

  /**
   * Confirm configuration and advance workflow.
   * Phase flow: configuring → exploring → [interviewing] → requirement_analysis → generating_prd
   */
  confirmConfig: async (overrides?: Partial<WorkflowConfig>) => {
    const state = get();
    const runToken = state._runToken;
    const phaseRuntime = buildPhaseRuntime(set, get, runToken);
    const config = overrides ? { ...state.config, ...overrides } : state.config;
    if (!isRunActive(get, runToken)) {
      return failResult('stale_run_token', 'Configuration request was superseded');
    }
    set({ config, phase: 'exploring' });

    try {
      // Always explore first (exploration provides context for interview BA)
      await runExplorePhase(phaseRuntime, { normalizeExplorationCardData });
      if (!isRunActive(get, runToken)) {
        return failResult('stale_run_token', 'Configuration request was superseded');
      }

      if (config.specInterviewEnabled) {
        const interviewResult = await runInterviewPhase(
          phaseRuntime,
          { flowLevel: config.flowLevel },
          {
            mapInterviewQuestion,
            runRequirementPhase: async (runtime) => {
              await runRequirementPhase(runtime, {});
            },
            runPrdPhase: async (runtime) =>
              runPrdPhase(runtime, {
                toPrdCardData,
                synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
              }),
          },
        );
        if (!interviewResult.ok) {
          return interviewResult;
        }
      } else {
        // Skip interview, run requirement analysis then generate PRD
        await runRequirementPhase(phaseRuntime, {});
        if (!isRunActive(get, runToken)) {
          return failResult('stale_run_token', 'Configuration request was superseded');
        }
        const prdResult = await runPrdPhase(phaseRuntime, {
          toPrdCardData,
          synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
        });
        if (!prdResult.ok) {
          return prdResult;
        }
      }
    } catch (e) {
      if (!isRunActive(get, runToken)) {
        return failResult('stale_run_token', 'Configuration request was superseded');
      }
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError(i18n.t('workflow.orchestrator.configurationFailed', { ns: 'simpleMode' }), msg);
      return failResult('config_confirm_failed', msg);
    }

    if (!isRunActive(get, runToken)) {
      return failResult('stale_run_token', 'Configuration request was superseded');
    }
    if (get().phase === 'failed') {
      return failResult('config_confirm_failed', get().error || 'Configuration failed');
    }
    return okResult();
  },

  /** Update workflow config without advancing phase */
  updateConfig: (updates: Partial<WorkflowConfig>) => {
    set((state) => ({ config: { ...state.config, ...updates } }));
  },

  /** Parse natural language config override */
  overrideConfigNatural: (text: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    appendWorkflowUserMessage(text);

    const { updates, matched, unmatched } = parseWorkflowConfigNatural(text, i18n.language);

    if (Object.keys(updates).length > 0) {
      set((state) => ({ config: { ...state.config, ...updates } }));
      injectInfo(
        i18n.t('workflow.orchestrator.configUpdated', {
          ns: 'simpleMode',
          details: matched.join(', '),
        }),
        'success',
      );
      return;
    }

    if (unmatched.length > 0) {
      injectInfo(
        i18n.t('workflow.orchestrator.configNoRecognizedOverrides', {
          ns: 'simpleMode',
          defaultValue: 'No recognizable config overrides found in: {{input}}',
          input: unmatched[0],
        }),
        'warning',
      );
    }
  },

  /** Submit answer to current interview question */
  submitInterviewAnswer: async (answer: string) => {
    const runToken = get()._runToken;
    const { pendingInterviewQuestion, interviewId } = get();
    const kernelPendingInterview = selectKernelTaskRuntime(useWorkflowKernelStore.getState().session).pendingInterview;
    const resolvedInterviewId = interviewId || kernelPendingInterview?.interviewId || null;
    if (!resolvedInterviewId) return;
    if (!interviewId && resolvedInterviewId) {
      set({ interviewId: resolvedInterviewId });
    }

    // Inject answer card
    const answerData: InterviewAnswerCardData = {
      questionId:
        pendingInterviewQuestion?.questionId ?? kernelPendingInterview?.questionId ?? 'kernel-pending-question',
      answer,
      skipped: false,
    };
    injectCard('interview_answer', answerData);
    set({ pendingInterviewQuestion: null });

    // Submit to backend
    const updatedSession = await useSpecInterviewStore.getState().submitAnswer(answer, resolvedInterviewId);
    if (!isRunActive(get, runToken)) return;

    if (!updatedSession) {
      const error = useSpecInterviewStore.getState().error;
      set({ phase: 'failed', error: error || 'Failed to submit answer' });
      injectError(
        i18n.t('workflow.orchestrator.interviewError', { ns: 'simpleMode' }),
        error || i18n.t('workflow.orchestrator.submitAnswerFailed', { ns: 'simpleMode' }),
      );
      return;
    }

    // Check if interview is complete
    if (updatedSession.status === 'finalized') {
      injectInfo(i18n.t('workflow.orchestrator.interviewComplete', { ns: 'simpleMode' }), 'success');

      // Compile spec
      const { config, taskDescription } = get();
      const compiled = await useSpecInterviewStore.getState().compileSpec({
        description: taskDescription,
        flow_level: config.flowLevel,
        tdd_mode: config.tddMode === 'off' ? null : config.tddMode,
      });
      if (!isRunActive(get, runToken)) return;

      if (compiled) {
        // Advance to requirement analysis then PRD generation
        const phaseRuntime = buildPhaseRuntime(set, get, runToken);
        await runRequirementPhase(phaseRuntime, {});
        if (!isRunActive(get, runToken)) return;
        await runPrdPhase(phaseRuntime, {
          toPrdCardData,
          synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
        });
      } else {
        const error = useSpecInterviewStore.getState().error;
        set({ phase: 'failed', error: error || 'Failed to compile spec' });
        injectError(
          i18n.t('workflow.orchestrator.specCompilationFailed', { ns: 'simpleMode' }),
          error || i18n.t('workflow.orchestrator.compileSpecFailed', { ns: 'simpleMode' }),
        );
      }
      return;
    }

    let nextSession = updatedSession;
    if (!nextSession.current_question && nextSession.status !== 'finalized') {
      const recovered = await useSpecInterviewStore.getState().fetchState(nextSession.id);
      if (!isRunActive(get, runToken)) return;
      if (recovered) nextSession = recovered;
    }

    // Present next question
    if (nextSession.current_question) {
      const questionData = mapInterviewQuestion(
        nextSession.current_question,
        nextSession.question_cursor + 1,
        nextSession.max_questions,
      );
      set({ pendingInterviewQuestion: questionData });
      injectCard('interview_question', questionData, true);
      return;
    }

    injectInfo(
      i18n.t('workflow.orchestrator.interviewQuestionUnavailable', {
        ns: 'simpleMode',
        defaultValue: 'Interview question unavailable, continuing with requirement analysis.',
      }),
      'warning',
    );
    const phaseRuntime = buildPhaseRuntime(set, get, runToken);
    await runRequirementPhase(phaseRuntime, {});
    if (!isRunActive(get, runToken)) return;
    await runPrdPhase(phaseRuntime, {
      toPrdCardData,
      synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
    });
  },

  /** Skip current interview question */
  skipInterviewQuestion: async () => {
    const runToken = get()._runToken;
    const { pendingInterviewQuestion, interviewId } = get();
    const kernelPendingInterview = selectKernelTaskRuntime(useWorkflowKernelStore.getState().session).pendingInterview;
    const resolvedInterviewId = interviewId || kernelPendingInterview?.interviewId || null;
    if (!resolvedInterviewId) return;
    if (!interviewId && resolvedInterviewId) {
      set({ interviewId: resolvedInterviewId });
    }

    const answerData: InterviewAnswerCardData = {
      questionId:
        pendingInterviewQuestion?.questionId ?? kernelPendingInterview?.questionId ?? 'kernel-pending-question',
      answer: '',
      skipped: true,
    };
    injectCard('interview_answer', answerData);
    set({ pendingInterviewQuestion: null });

    // Submit skip (empty answer)
    const updatedSession = await useSpecInterviewStore.getState().submitAnswer('', resolvedInterviewId);
    if (!isRunActive(get, runToken)) return;

    if (!updatedSession) {
      const error = useSpecInterviewStore.getState().error;
      set({ phase: 'failed', error: error || 'Failed to skip question' });
      return;
    }

    if (updatedSession.status === 'finalized') {
      injectInfo(i18n.t('workflow.orchestrator.interviewComplete', { ns: 'simpleMode' }), 'success');
      const { config, taskDescription } = get();
      const compiled = await useSpecInterviewStore.getState().compileSpec({
        description: taskDescription,
        flow_level: config.flowLevel,
        tdd_mode: config.tddMode === 'off' ? null : config.tddMode,
      });
      if (!isRunActive(get, runToken)) return;
      if (compiled) {
        const phaseRuntime = buildPhaseRuntime(set, get, runToken);
        await runRequirementPhase(phaseRuntime, {});
        if (!isRunActive(get, runToken)) return;
        await runPrdPhase(phaseRuntime, {
          toPrdCardData,
          synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
        });
      } else {
        const error = useSpecInterviewStore.getState().error;
        set({ phase: 'failed', error: error || 'Failed to compile spec' });
        injectError(
          i18n.t('workflow.orchestrator.specCompilationFailed', { ns: 'simpleMode' }),
          error || i18n.t('workflow.orchestrator.compileSpecFailed', { ns: 'simpleMode' }),
        );
      }
      return;
    }

    let nextSession = updatedSession;
    if (!nextSession.current_question && nextSession.status !== 'finalized') {
      const recovered = await useSpecInterviewStore.getState().fetchState(nextSession.id);
      if (!isRunActive(get, runToken)) return;
      if (recovered) nextSession = recovered;
    }

    if (nextSession.current_question) {
      const questionData = mapInterviewQuestion(
        nextSession.current_question,
        nextSession.question_cursor + 1,
        nextSession.max_questions,
      );
      set({ pendingInterviewQuestion: questionData });
      injectCard('interview_question', questionData, true);
      return;
    }

    injectInfo(
      i18n.t('workflow.orchestrator.interviewQuestionUnavailable', {
        ns: 'simpleMode',
        defaultValue: 'Interview question unavailable, continuing with requirement analysis.',
      }),
      'warning',
    );
    const phaseRuntime = buildPhaseRuntime(set, get, runToken);
    await runRequirementPhase(phaseRuntime, {});
    if (!isRunActive(get, runToken)) return;
    await runPrdPhase(phaseRuntime, {
      toPrdCardData,
      synthesizePlanningTurn: synthesizePlanningTurnForPrdPhase,
    });
  },

  /** Update a story field in the editable PRD */
  updateEditableStory: (storyId, updates) => {
    const { editablePrd } = get();
    if (!editablePrd) return;
    set({
      editablePrd: {
        ...editablePrd,
        stories: editablePrd.stories.map((s) => (s.id === storyId ? { ...s, ...updates } : s)),
      },
    });
  },

  /** Approve PRD and run architecture review before execution */
  approvePrd: async (editedPrd?: TaskPrd) => {
    const state = get();
    const runToken = state._runToken;
    const prd = editedPrd || state.editablePrd;
    if (!prd) {
      const message = 'No PRD to approve';
      set({ error: message });
      return failResult('missing_prd', message);
    }

    set({ editablePrd: prd });

    // Non-quick flow: run architecture review (interactive — returns after injecting card)
    if (state.config.flowLevel !== 'quick') {
      const phaseRuntime = buildPhaseRuntime(set, get, runToken);
      await runArchitecturePhase(phaseRuntime, prd, {
        runDesignDocAndExecutionPhase: async (runtime, runtimePrd) =>
          runDesignDocAndExecutionPhase(runtime, runtimePrd, {
            subscribeToProgressEvents: subscribeToTaskProgressFromRuntime,
          }),
      });
      // Architecture review is interactive — user clicks Accept/Revise in the card.
      // The continuation happens in approveArchitecture() action.
      if (get().phase === 'failed') {
        return failResult('architecture_review_failed', get().error || 'Architecture review failed');
      }
      return okResult();
    }

    // Quick flow: skip architecture review, go straight to design doc + execution
    const phaseRuntime = buildPhaseRuntime(set, get, runToken);
    await runDesignDocAndExecutionPhase(phaseRuntime, prd, {
      subscribeToProgressEvents: subscribeToTaskProgressFromRuntime,
    });
    if (get().phase === 'failed') {
      return failResult('execution_start_failed', get().error || 'Task execution could not be started');
    }
    return okResult();
  },

  /** Add feedback to editable PRD (during reviewing_prd phase) */
  addPrdFeedback: async (_feedback: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    appendWorkflowUserMessage(_feedback);
    const normalizedFeedback = _feedback.trim();
    if (!normalizedFeedback) {
      return failResult('empty_feedback', 'Feedback cannot be empty');
    }

    const { sessionId, phase } = get();
    const effectiveSessionId = sessionId || resolveTaskSessionId(get, set);
    if (!effectiveSessionId) {
      const message = 'No active task session';
      set({ error: message });
      return failResult('missing_session', message);
    }
    if (phase !== 'reviewing_prd') {
      return failResult('invalid_phase', `Cannot apply PRD feedback in phase '${phase}'`);
    }

    const { resolvePhaseAgent } = await import('../lib/phaseAgentResolver');
    const prdResolved = resolvePhaseAgent('plan_prd');
    const settings = useSettingsStore.getState();
    const maxContextTokens = settings.maxTotalTokens ?? 200_000;
    const result = await useTaskModeStore
      .getState()
      .applyPrdFeedback(
        normalizedFeedback,
        undefined,
        maxContextTokens,
        prdResolved.provider || undefined,
        prdResolved.model || undefined,
        prdResolved.baseUrl,
        effectiveSessionId,
      );
    if (!result) {
      const message = useTaskModeStore.getState().error || 'Failed to apply PRD feedback';
      set({ error: message });
      injectError(
        i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }),
        message,
        i18n.t('workflow.orchestrator.prdMissingData', { ns: 'simpleMode' }),
      );
      return failResult('prd_feedback_apply_failed', message);
    }

    set({ editablePrd: result.prd, phase: 'reviewing_prd', error: null });
    injectCard('prd_card', toPrdCardData(result.prd), true);
    injectInfo(buildPrdFeedbackSummaryMessage(result.summary), 'info');
    return okResult();
  },

  /** Approve or request changes to the architecture review */
  approveArchitecture: async (
    acceptAsIs: boolean,
    selectedModifications: ArchitectureReviewCardData['prdModifications'],
  ) => {
    const runToken = get()._runToken;
    const { phase, editablePrd, config } = get();
    if (phase !== 'architecture_review') {
      return failResult('invalid_phase', `Cannot approve architecture in phase '${phase}'`);
    }

    if (acceptAsIs || selectedModifications.length === 0) {
      // Accept architecture as-is — proceed to design doc + execution
      injectInfo(
        i18n.t('workflow.orchestrator.architectureApproved', {
          ns: 'simpleMode',
          defaultValue: 'Architecture review accepted. Generating design document...',
        }),
        'success',
      );

      const prd = editablePrd;
      if (!prd) {
        return failResult('missing_prd', 'No PRD to execute after architecture approval');
      }

      const phaseRuntime = buildPhaseRuntime(set, get, runToken);
      await runDesignDocAndExecutionPhase(phaseRuntime, prd, {
        subscribeToProgressEvents: subscribeToTaskProgressFromRuntime,
      });
      if (get().phase === 'failed') {
        return failResult('execution_start_failed', get().error || 'Task execution could not be started');
      }
      return okResult();
    } else {
      if (!editablePrd) {
        return failResult('missing_prd', 'No PRD available for architecture revision');
      }

      injectInfo(
        i18n.t('workflow.orchestrator.architectureRevisionRequested', {
          ns: 'simpleMode',
          count: selectedModifications.length,
          defaultValue: 'Applying {{count}} architectural suggestions. Returning to PRD review...',
        }),
        'warning',
      );

      try {
        const patchedPrd = applyArchitectureModifications(editablePrd, selectedModifications, config.maxParallel);
        set({ phase: 'reviewing_prd', editablePrd: patchedPrd });
        injectCard('prd_card', toPrdCardData(patchedPrd), true);
        return okResult();
      } catch (e) {
        injectInfo(
          i18n.t('workflow.orchestrator.architectureApplyFailed', {
            ns: 'simpleMode',
            defaultValue: 'Could not apply architecture suggestions automatically. Please edit PRD manually.',
          }),
          'warning',
        );
        set({ phase: 'reviewing_prd' });
        injectCard('prd_card', toPrdCardData(editablePrd), true);
        if (e instanceof Error) {
          set({ error: e.message });
          return failResult('architecture_apply_failed', e.message);
        }
        return failResult('architecture_apply_failed', 'Failed to apply architecture modifications');
      }
    }
  },

  /** Cancel the current workflow */
  cancelWorkflow: async () => {
    const { phase, sessionId, _runToken } = get();
    const effectiveSessionId = resolveTaskSessionId(get, set);
    if (!sessionId && effectiveSessionId) {
      set({ sessionId: effectiveSessionId });
    }

    if (phase === 'executing' && effectiveSessionId) {
      if (get().isCancelling) return;
      set({ isCancelling: true, error: null });
      const cancelled = await useTaskModeStore.getState().cancelExecution(effectiveSessionId);
      const taskModeError = useTaskModeStore.getState().error;
      if (!cancelled && taskModeError) {
        set({ isCancelling: false, error: taskModeError });
        injectError(
          i18n.t('workflow.orchestrator.cancelFailed', {
            ns: 'simpleMode',
            defaultValue: 'Cancel Failed',
          }),
          taskModeError,
          i18n.t('workflow.orchestrator.cancelRetry', {
            ns: 'simpleMode',
            defaultValue: 'Please retry cancellation.',
          }),
        );
        throw new Error(taskModeError);
      }
      injectInfo(
        i18n.t('workflow.orchestrator.cancelling', {
          ns: 'simpleMode',
          defaultValue: 'Cancelling workflow...',
        }),
        'warning',
      );
      return;
    }

    const nextRunToken = _runToken + 1;
    set({ _runToken: nextRunToken, isCancelling: false });
    await useTaskModeStore.getState().cancelOperation(effectiveSessionId);

    // Unsubscribe from events
    const { _unlistenFn } = get();
    if (_unlistenFn) {
      _unlistenFn();
    }

    set({ phase: 'cancelled' });
    injectInfo(i18n.t('workflow.orchestrator.workflowCancelled', { ns: 'simpleMode' }), 'warning');
  },

  /** Reset the orchestrator to idle state */
  resetWorkflow: () => {
    const { _unlistenFn } = get();
    if (_unlistenFn) {
      _unlistenFn();
    }

    // Reset delegate stores
    useTaskModeStore.getState().reset();
    useSpecInterviewStore.getState().reset();

    set((state) => ({ ...DEFAULT_STATE, _runToken: state._runToken + 1 }));
  },
}));

// ============================================================================
// Internal Phase Transitions
// ============================================================================

type SetFn = (
  partial:
    | Partial<WorkflowOrchestratorState>
    | ((state: WorkflowOrchestratorState) => Partial<WorkflowOrchestratorState>),
) => void;
type GetFn = () => WorkflowOrchestratorState;

function buildPhaseRuntime(set: SetFn, get: GetFn, runToken: number): WorkflowPhaseRuntime {
  return {
    set: set as unknown as WorkflowPhaseRuntime['set'],
    get: get as unknown as WorkflowPhaseRuntime['get'],
    runToken,
    isRunActive: isRunActive as WorkflowPhaseRuntime['isRunActive'],
    resolveTaskSessionId: resolveTaskSessionId as WorkflowPhaseRuntime['resolveTaskSessionId'],
  };
}

function subscribeToTaskProgressFromRuntime(setter: unknown, getter: unknown, runToken: number) {
  return subscribeToProgressEvents(setter as SetFn, getter as GetFn, runToken);
}

/**
 * Subscribe to task-mode-progress events for execution tracking.
 *
 * Rust emits individual per-story events (batch_started, story_started,
 * story_completed, story_failed, execution_completed, execution_cancelled, error).
 * We accumulate story statuses locally and inject appropriate UI cards.
 */
async function subscribeToProgressEvents(set: SetFn, get: GetFn, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  // Unsubscribe from existing
  const existing = get()._unlistenFn;
  if (existing) existing();

  // Accumulated story statuses from individual events
  const accumulatedStatuses: Record<string, string> = {};

  try {
    const unlisten = await listen<{
      sessionId: string;
      eventType: string;
      currentBatch: number;
      totalBatches: number;
      storyId: string | null;
      storyStatus: string | null;
      agentName: string | null;
      gateResults: GateResult[] | null;
      error: string | null;
      progressPct: number;
    }>('task-mode-progress', (event) => {
      if (!isRunActive(get, runToken)) return;
      const payload = event.payload;
      const state = get();
      if (state.sessionId && payload.sessionId !== state.sessionId) return;
      const orchestratorPatch: Partial<WorkflowOrchestratorState> = {
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
      };

      // Accumulate story status
      if (payload.storyId && payload.storyStatus) {
        accumulatedStatuses[payload.storyId] = payload.storyStatus;
        orchestratorPatch.storyStatuses = {
          ...state.storyStatuses,
          [payload.storyId]: payload.storyStatus,
        };
      }

      if (payload.storyId && payload.gateResults && payload.gateResults.length > 0) {
        orchestratorPatch.qualityGateResults = {
          ...state.qualityGateResults,
          [payload.storyId]: {
            storyId: payload.storyId,
            overallStatus: deriveGateOverallStatus(payload.gateResults),
            gates: payload.gateResults,
          },
        };
      }

      // Resolve story title from editablePrd
      const storyTitle = payload.storyId
        ? (state.editablePrd?.stories.find((s) => s.id === payload.storyId)?.title ?? payload.storyId)
        : null;

      switch (payload.eventType) {
        case 'batch_started': {
          injectCard('execution_update', {
            eventType: 'batch_start',
            currentBatch: payload.currentBatch,
            totalBatches: payload.totalBatches,
            storyId: null,
            storyTitle: null,
            status: i18n.t('workflow.execution.batchLabel', {
              ns: 'simpleMode',
              current: payload.currentBatch + 1,
              total: payload.totalBatches,
            }),
            agent: null,
            progressPct: payload.progressPct,
          } as ExecutionUpdateCardData);
          set(orchestratorPatch as Partial<WorkflowOrchestratorState>);
          break;
        }

        case 'story_started': {
          injectCard('execution_update', {
            eventType: 'story_start',
            currentBatch: payload.currentBatch,
            totalBatches: payload.totalBatches,
            storyId: payload.storyId,
            storyTitle,
            status: 'running',
            agent: payload.agentName,
            progressPct: payload.progressPct,
          } as ExecutionUpdateCardData);
          set(orchestratorPatch as Partial<WorkflowOrchestratorState>);
          break;
        }

        case 'story_completed': {
          injectCard('execution_update', {
            eventType: 'story_complete',
            currentBatch: payload.currentBatch,
            totalBatches: payload.totalBatches,
            storyId: payload.storyId,
            storyTitle,
            status: 'completed',
            agent: payload.agentName,
            progressPct: payload.progressPct,
          } as ExecutionUpdateCardData);

          // Inject gate results if present
          if (payload.gateResults && payload.gateResults.length > 0 && payload.storyId) {
            injectCard('gate_result', {
              storyId: payload.storyId,
              storyTitle: storyTitle ?? payload.storyId,
              overallStatus: deriveGateOverallStatus(payload.gateResults),
              gates: payload.gateResults,
              codeReviewScores: [],
            } as GateResultCardData);
          }
          set(orchestratorPatch as Partial<WorkflowOrchestratorState>);
          break;
        }

        case 'story_failed': {
          injectCard('execution_update', {
            eventType: 'story_failed',
            currentBatch: payload.currentBatch,
            totalBatches: payload.totalBatches,
            storyId: payload.storyId,
            storyTitle,
            status: payload.error ?? 'failed',
            agent: payload.agentName,
            progressPct: payload.progressPct,
          } as ExecutionUpdateCardData);

          // Inject gate results if present
          if (payload.gateResults && payload.gateResults.length > 0 && payload.storyId) {
            injectCard('gate_result', {
              storyId: payload.storyId,
              storyTitle: storyTitle ?? payload.storyId,
              overallStatus: deriveGateOverallStatus(payload.gateResults),
              gates: payload.gateResults,
              codeReviewScores: [],
            } as GateResultCardData);
          }
          set(orchestratorPatch as Partial<WorkflowOrchestratorState>);
          break;
        }

        case 'execution_completed': {
          const fallbackTotalStories =
            Object.keys(accumulatedStatuses).length || state.editablePrd?.stories.length || 0;
          const totalStories = fallbackTotalStories;
          const completedCount = Object.values(accumulatedStatuses).filter((s) => s === 'completed').length;
          const failedCount = Object.values(accumulatedStatuses).filter((s) => s === 'failed').length;
          const success = failedCount === 0;
          orchestratorPatch.isCancelling = false;

          set({
            ...(orchestratorPatch as Partial<WorkflowOrchestratorState>),
            phase: success ? 'completed' : 'failed',
            isCancelling: false,
          });

          void (async () => {
            let report: ExecutionReport | null = null;
            try {
              report = await Promise.race<ExecutionReport | null>([
                useTaskModeStore.getState().fetchReport(payload.sessionId),
                new Promise<null>((resolve) => {
                  setTimeout(() => resolve(null), 1500);
                }),
              ]);
            } catch {
              report = null;
            }

            if (!isRunActive(get, runToken)) return;
            if (get()._completionCardInjectedRunToken === runToken) return;
            const latestReport = get().report;
            const effectiveReport = report && report.sessionId === payload.sessionId ? report : latestReport;

            const completionData = effectiveReport
              ? buildCompletionReportDataFromReport(effectiveReport)
              : buildCompletionReportDataFallback({
                  success,
                  totalStories,
                  completed: completedCount,
                  failed: failedCount,
                });

            injectCard('completion_report', completionData);
            set({ _completionCardInjectedRunToken: runToken, report: effectiveReport ?? null });
          })();
          break;
        }

        case 'execution_cancelled': {
          orchestratorPatch.isCancelling = false;
          set({
            ...(orchestratorPatch as Partial<WorkflowOrchestratorState>),
            phase: 'cancelled',
            isCancelling: false,
          });
          injectInfo(
            i18n.t('workflow.orchestrator.workflowCancelled', {
              ns: 'simpleMode',
              defaultValue: 'Workflow cancelled.',
            }),
            'warning',
          );
          break;
        }

        case 'error': {
          if (payload.error) {
            orchestratorPatch.error = payload.error;
          }
          orchestratorPatch.isCancelling = false;
          set({
            ...(orchestratorPatch as Partial<WorkflowOrchestratorState>),
            isCancelling: false,
          });
          if (payload.error) {
            injectError(i18n.t('workflow.orchestrator.executionError', { ns: 'simpleMode' }), payload.error);
          }
          break;
        }
        default: {
          if (payload.eventType === 'story_failed' && payload.error) {
            orchestratorPatch.error = payload.error;
          }
          set(orchestratorPatch as Partial<WorkflowOrchestratorState>);
          break;
        }
      }
    });

    set({ _unlistenFn: unlisten });
  } catch {
    // Non-fatal: event subscription failure
  }
}

export default useWorkflowOrchestratorStore;
