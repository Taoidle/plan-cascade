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
 * - useExecutionStore: appendCard for structured card injection into chat transcript
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../i18n';
import { useExecutionStore } from './execution';
import {
  useTaskModeStore,
  type TaskPrd,
  type StrategyAnalysis,
  type GateResult,
  type ExecutionReport,
} from './taskMode';
import { useSpecInterviewStore, type InterviewQuestion, type InterviewSession } from './specInterview';
import { useSettingsStore } from './settings';
import { useWorkflowKernelStore } from './workflowKernel';
import { buildConversationHistory, synthesizePlanningTurn, synthesizeExecutionTurn } from '../lib/contextBridge';
import { getNextTurnId } from '../lib/conversationUtils';
import { deriveGateOverallStatus } from '../lib/gateStatus';
import { parseWorkflowConfigNatural } from '../lib/workflowConfigNaturalParser';
import type { CrossModeConversationTurn } from '../types/crossModeContext';
import { applyArchitectureModifications } from './workflowOrchestrator/architectureModifications';
import type {
  WorkflowPhase,
  WorkflowConfig,
  CardPayload,
  InterviewQuestionCardData,
  StrategyCardData,
  ConfigCardData,
  PrdCardData,
  DesignDocCardData,
  ExplorationCardData,
  ExecutionUpdateCardData,
  GateResultCardData,
  CompletionReportCardData,
  WorkflowInfoData,
  WorkflowErrorData,
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
  pendingQuestion: InterviewQuestionCardData | null;

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

  /** Conversation history extracted from Chat for Task context sharing */
  _conversationHistory: CrossModeConversationTurn[];

  /** Guards async continuations after cancel/reset */
  _runToken: number;

  /** Prevent duplicate completion-card injection for a single run token */
  _completionCardInjectedRunToken: number | null;

  // Actions
  startWorkflow: (description: string) => Promise<void>;
  confirmConfig: (overrides?: Partial<WorkflowConfig>) => Promise<void>;
  updateConfig: (updates: Partial<WorkflowConfig>) => void;
  overrideConfigNatural: (text: string) => void;
  submitInterviewAnswer: (answer: string) => Promise<void>;
  skipInterviewQuestion: () => Promise<void>;
  approvePrd: (editedPrd?: TaskPrd) => Promise<void>;
  updateEditableStory: (
    storyId: string,
    updates: Partial<{ title: string; description: string; priority: string; acceptanceCriteria: string[] }>,
  ) => void;
  addPrdFeedback: (feedback: string) => void;
  approveArchitecture: (
    acceptAsIs: boolean,
    selectedModifications: ArchitectureReviewCardData['prdModifications'],
  ) => Promise<void>;
  cancelWorkflow: () => Promise<void>;
  resetWorkflow: () => void;
  clearConversationHistory: () => void;
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
  pendingQuestion: null as InterviewQuestionCardData | null,
  requirementAnalysis: null as RequirementAnalysisCardData | null,
  architectureReview: null as ArchitectureReviewCardData | null,
  architectureReviewRound: 0,
  error: null as string | null,
  isCancelling: false,
  _unlistenFn: null as UnlistenFn | null,
  _conversationHistory: [] as CrossModeConversationTurn[],
  _runToken: 0,
  _completionCardInjectedRunToken: null as number | null,
};

// ============================================================================
// Helpers
// ============================================================================

let _cardCounter = 0;

function nextCardId(): string {
  return `card-${++_cardCounter}-${Date.now()}`;
}

function isRunActive(get: GetFn, runToken: number): boolean {
  return get()._runToken === runToken;
}

/** Inject a card message into the chat transcript */
function injectCard<T extends CardPayload['cardType']>(cardType: T, data: CardPayload['data'], interactive = false) {
  const payload: CardPayload = {
    cardType,
    cardId: nextCardId(),
    data,
    interactive,
  };
  useExecutionStore.getState().appendCard(payload);
}

/** Inject an info-level workflow message */
function injectInfo(message: string, level: WorkflowInfoData['level'] = 'info') {
  injectCard('workflow_info', { message, level } as WorkflowInfoData);
}

/** Inject a workflow error card */
function injectError(title: string, description: string, suggestedFix: string | null = null) {
  injectCard('workflow_error', { title, description, suggestedFix } as WorkflowErrorData);
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

async function syncKernelTaskPhase(
  phase: Extract<WorkflowPhase, 'requirement_analysis' | 'architecture_review' | 'generating_design_doc'>,
  reasonCode: string,
): Promise<void> {
  const transitionAndSubmitInput = useWorkflowKernelStore.getState().transitionAndSubmitInput;
  const session = useWorkflowKernelStore.getState().session;
  if (!session || session.activeMode !== 'task') return;

  try {
    await transitionAndSubmitInput('task', {
      type: 'system_phase_update',
      content: `phase:${phase}`,
      metadata: {
        mode: 'task',
        phase,
        reasonCode,
      },
    });
  } catch {
    // best-effort kernel phase sync
  }
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
  startWorkflow: async (description: string) => {
    const runToken = get()._runToken + 1;
    // Add user message as 'info' StreamLine so it appears as a chat bubble in ChatTranscript
    const executionState = useExecutionStore.getState();
    const turnId = getNextTurnId(executionState.streamingOutput);
    executionState.appendStreamLine(description, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });

    set({ phase: 'analyzing', taskDescription: description, error: null, isCancelling: false, _runToken: runToken });
    injectInfo(i18n.t('workflow.orchestrator.analyzingTask', { ns: 'simpleMode' }), 'info');

    // Extract complete Chat conversation history for Task context sharing
    const conversationHistory = buildConversationHistory();
    if (!isRunActive(get, runToken)) return;
    set({ _conversationHistory: conversationHistory });

    try {
      // 1. Enter task mode (creates session + runs strategy analysis)
      await useTaskModeStore.getState().enterTaskMode(description);
      if (!isRunActive(get, runToken)) return;

      const taskModeState = useTaskModeStore.getState();
      if (taskModeState.error) {
        if (!isRunActive(get, runToken)) return;
        set({ phase: 'failed', error: taskModeState.error });
        injectError(i18n.t('workflow.orchestrator.strategyAnalysisFailed', { ns: 'simpleMode' }), taskModeState.error);
        return;
      }

      const sessionId = taskModeState.sessionId;
      let analysis = taskModeState.strategyAnalysis;

      // 2. Try LLM enhancement of strategy analysis
      const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
      if (!isRunActive(get, runToken)) return;
      const strategyResolved = resolvePhaseAgent('plan_strategy');
      if (analysis) {
        try {
          if (!isRunActive(get, runToken)) return;
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
          if (!isRunActive(get, runToken)) return;

          if (enhanced.success && enhanced.data) {
            analysis = enhanced.data;
            useTaskModeStore.setState({ strategyAnalysis: analysis });
          }
        } catch {
          // LLM enhancement failed — silently use keyword analysis
        }
      }

      if (!isRunActive(get, runToken)) return;
      set({ sessionId, strategyAnalysis: analysis });

      // 3. Inject strategy card (with LLM-enhanced or keyword result)
      if (analysis) {
        if (!isRunActive(get, runToken)) return;
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

      if (!isRunActive(get, runToken)) return;
      set({ config, phase: 'configuring' });

      // 5. Inject config card (user interacts with it to advance)
      if (!isRunActive(get, runToken)) return;
      injectCard('config_card', buildConfigCardData(config, false), true);
    } catch (e) {
      if (!isRunActive(get, runToken)) return;
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError(i18n.t('workflow.orchestrator.workflowFailed', { ns: 'simpleMode' }), msg);
    }
  },

  /**
   * Confirm configuration and advance workflow.
   * Phase flow: configuring → exploring → [interviewing] → requirement_analysis → generating_prd
   */
  confirmConfig: async (overrides?: Partial<WorkflowConfig>) => {
    const state = get();
    const runToken = state._runToken;
    const config = overrides ? { ...state.config, ...overrides } : state.config;
    if (!isRunActive(get, runToken)) return;
    set({ config, phase: 'exploring' });

    try {
      // Always explore first (exploration provides context for interview BA)
      await explorePhase(set, get, runToken);
      if (!isRunActive(get, runToken)) return;

      if (config.specInterviewEnabled) {
        // Start interview flow (BA now has exploration context)
        if (!isRunActive(get, runToken)) return;
        set({ phase: 'interviewing' });

        const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
        if (!isRunActive(get, runToken)) return;
        const interviewResolved = resolvePhaseAgent('plan_interview');

        injectCard('persona_indicator', {
          role: 'BusinessAnalyst',
          displayName: 'Business Analyst',
          phase: 'interviewing',
          model: formatModelDisplay(interviewResolved),
        });
        injectInfo(i18n.t('workflow.orchestrator.startingInterview', { ns: 'simpleMode' }), 'info');

        const settings = useSettingsStore.getState();
        const workspacePath = settings.workspacePath;
        const { explorationResult } = get();
        const interviewConfig = {
          description: state.taskDescription,
          flow_level: config.flowLevel,
          max_questions: config.flowLevel === 'quick' ? 10 : config.flowLevel === 'full' ? 25 : 18,
          first_principles: false,
          project_path: workspacePath,
          exploration_context: explorationResult ? JSON.stringify(explorationResult) : null,
          task_session_id: state.sessionId,
          locale: i18n.language,
        };

        // Set LLM provider settings so specInterview store passes them to backend
        if (interviewResolved.provider) {
          useSpecInterviewStore.getState().setProviderSettings({
            provider: interviewResolved.provider,
            model: interviewResolved.model || undefined,
            baseUrl: interviewResolved.baseUrl || undefined,
          });
        }

        // Retry with exponential backoff if backend not yet initialized (race with init_app)
        const maxRetries = 5;
        const baseDelay = 500;
        let session: InterviewSession | null = null;
        for (let attempt = 0; attempt < maxRetries; attempt++) {
          session = await useSpecInterviewStore.getState().startInterview(interviewConfig);
          if (!isRunActive(get, runToken)) return;
          if (session) break;
          const interviewError = useSpecInterviewStore.getState().error || '';
          if (interviewError.includes('not initialized') && attempt < maxRetries - 1) {
            await new Promise((r) => setTimeout(r, baseDelay * Math.pow(2, attempt)));
            if (!isRunActive(get, runToken)) return;
            useSpecInterviewStore.getState().clearError();
            continue;
          }
          break;
        }

        if (!session) {
          const interviewError = useSpecInterviewStore.getState().error;
          set({ phase: 'failed', error: interviewError || 'Failed to start interview' });
          injectError(
            i18n.t('workflow.orchestrator.interviewFailed', { ns: 'simpleMode' }),
            interviewError || i18n.t('workflow.orchestrator.interviewStartFailed', { ns: 'simpleMode' }),
          );
          return;
        }

        if (!isRunActive(get, runToken)) return;
        set({ interviewId: session.id });

        let interviewSession = session;
        if (!interviewSession.current_question && interviewSession.status !== 'finalized') {
          const recovered = await useSpecInterviewStore.getState().fetchState(interviewSession.id);
          if (!isRunActive(get, runToken)) return;
          if (recovered) interviewSession = recovered;
        }

        // Present first question
        if (interviewSession.current_question) {
          const questionData = mapInterviewQuestion(
            interviewSession.current_question,
            interviewSession.question_cursor + 1,
            interviewSession.max_questions,
          );
          set({ pendingQuestion: questionData });
          injectCard('interview_question', questionData, true);
        } else {
          injectInfo(
            i18n.t('workflow.orchestrator.interviewQuestionUnavailable', {
              ns: 'simpleMode',
              defaultValue: 'Interview question unavailable, continuing with requirement analysis.',
            }),
            'warning',
          );
          await requirementAnalysisPhase(set, get, runToken);
          if (!isRunActive(get, runToken)) return;
          await generatePrdPhase(set, get, runToken);
        }
      } else {
        // Skip interview, run requirement analysis then generate PRD
        await requirementAnalysisPhase(set, get, runToken);
        if (!isRunActive(get, runToken)) return;
        await generatePrdPhase(set, get, runToken);
      }
    } catch (e) {
      if (!isRunActive(get, runToken)) return;
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError(i18n.t('workflow.orchestrator.configurationFailed', { ns: 'simpleMode' }), msg);
    }
  },

  /** Update workflow config without advancing phase */
  updateConfig: (updates: Partial<WorkflowConfig>) => {
    set((state) => ({ config: { ...state.config, ...updates } }));
  },

  /** Parse natural language config override */
  overrideConfigNatural: (text: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    const executionState = useExecutionStore.getState();
    const turnId = getNextTurnId(executionState.streamingOutput);
    executionState.appendStreamLine(text, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });

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
    const { pendingQuestion, interviewId } = get();
    const kernelPendingInterview =
      useWorkflowKernelStore.getState().session?.modeSnapshots.task?.pendingInterview ?? null;
    const resolvedInterviewId = interviewId || kernelPendingInterview?.interviewId || null;
    if (!resolvedInterviewId) return;
    if (!interviewId && resolvedInterviewId) {
      set({ interviewId: resolvedInterviewId });
    }

    // Inject answer card
    const answerData: InterviewAnswerCardData = {
      questionId: pendingQuestion?.questionId ?? kernelPendingInterview?.questionId ?? 'kernel-pending-question',
      answer,
      skipped: false,
    };
    injectCard('interview_answer', answerData);
    set({ pendingQuestion: null });

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
        await requirementAnalysisPhase(set, get, runToken);
        if (!isRunActive(get, runToken)) return;
        await generatePrdPhase(set, get, runToken);
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
      set({ pendingQuestion: questionData });
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
    await requirementAnalysisPhase(set, get, runToken);
    if (!isRunActive(get, runToken)) return;
    await generatePrdPhase(set, get, runToken);
  },

  /** Skip current interview question */
  skipInterviewQuestion: async () => {
    const runToken = get()._runToken;
    const { pendingQuestion, interviewId } = get();
    const kernelPendingInterview =
      useWorkflowKernelStore.getState().session?.modeSnapshots.task?.pendingInterview ?? null;
    const resolvedInterviewId = interviewId || kernelPendingInterview?.interviewId || null;
    if (!resolvedInterviewId) return;
    if (!interviewId && resolvedInterviewId) {
      set({ interviewId: resolvedInterviewId });
    }

    const answerData: InterviewAnswerCardData = {
      questionId: pendingQuestion?.questionId ?? kernelPendingInterview?.questionId ?? 'kernel-pending-question',
      answer: '',
      skipped: true,
    };
    injectCard('interview_answer', answerData);
    set({ pendingQuestion: null });

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
        await requirementAnalysisPhase(set, get, runToken);
        if (!isRunActive(get, runToken)) return;
        await generatePrdPhase(set, get, runToken);
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
      set({ pendingQuestion: questionData });
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
    await requirementAnalysisPhase(set, get, runToken);
    if (!isRunActive(get, runToken)) return;
    await generatePrdPhase(set, get, runToken);
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
      set({ error: 'No PRD to approve' });
      return;
    }

    set({ editablePrd: prd });

    // Non-quick flow: run architecture review (interactive — returns after injecting card)
    if (state.config.flowLevel !== 'quick') {
      await architectureReviewPhase(set, get, prd, runToken);
      // Architecture review is interactive — user clicks Accept/Revise in the card.
      // The continuation happens in approveArchitecture() action.
      return;
    }

    // Quick flow: skip architecture review, go straight to design doc + execution
    await designDocAndExecutePhase(set, get, prd, runToken);
  },

  /** Add feedback to editable PRD (during reviewing_prd phase) */
  addPrdFeedback: (_feedback: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    const executionState = useExecutionStore.getState();
    const turnId = getNextTurnId(executionState.streamingOutput);
    executionState.appendStreamLine(_feedback, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });

    // In the future, this could use LLM to apply NL edits to the PRD.
    // For now, inject as info message.
    injectInfo(i18n.t('workflow.orchestrator.prdFeedbackNoted', { ns: 'simpleMode', feedback: _feedback }), 'info');
  },

  /** Approve or request changes to the architecture review */
  approveArchitecture: async (
    acceptAsIs: boolean,
    selectedModifications: ArchitectureReviewCardData['prdModifications'],
  ) => {
    const runToken = get()._runToken;
    const { phase, editablePrd, config } = get();
    if (phase !== 'architecture_review') return;

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
      if (!prd) return;

      await designDocAndExecutePhase(set, get, prd, runToken);
    } else {
      if (!editablePrd) return;

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
        }
      }
    }
  },

  /** Cancel the current workflow */
  cancelWorkflow: async () => {
    const { phase, sessionId, _runToken } = get();
    const linkedSessionId = useWorkflowKernelStore.getState().session?.linkedModeSessions?.task ?? null;
    const effectiveSessionId = sessionId || linkedSessionId || useTaskModeStore.getState().sessionId || null;
    if (!sessionId && effectiveSessionId) {
      set({ sessionId: effectiveSessionId });
      useTaskModeStore.setState({ sessionId: effectiveSessionId, isTaskMode: true });
    }

    if (phase === 'executing' && effectiveSessionId) {
      if (get().isCancelling) return;
      set({ isCancelling: true, error: null });
      await useTaskModeStore.getState().cancelExecution();
      const taskState = useTaskModeStore.getState();
      if (taskState.error) {
        set({ isCancelling: false, error: taskState.error });
        injectError(
          i18n.t('workflow.orchestrator.cancelFailed', {
            ns: 'simpleMode',
            defaultValue: 'Cancel Failed',
          }),
          taskState.error,
          i18n.t('workflow.orchestrator.cancelRetry', {
            ns: 'simpleMode',
            defaultValue: 'Please retry cancellation.',
          }),
        );
        throw new Error(taskState.error);
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
    await useTaskModeStore.getState().cancelOperation();

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

  /** Clear conversation history without resetting the entire workflow */
  clearConversationHistory: () => {
    set({ _conversationHistory: [] });
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

/**
 * Explore project codebase (Senior Engineer persona).
 *
 * For quick flow: skips exploration entirely.
 * For standard/full flow: runs project exploration, injects results as card.
 */
async function explorePhase(set: SetFn, get: GetFn, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  const { config, taskDescription, sessionId } = get();

  // Quick flow: skip exploration entirely
  if (config.flowLevel === 'quick') return;

  set({ phase: 'exploring' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const explorationResolved = resolvePhaseAgent('plan_exploration');

  injectCard('persona_indicator', {
    role: 'SeniorEngineer',
    displayName: 'Senior Engineer',
    phase: 'exploring',
    model: formatModelDisplay(explorationResolved),
  });
  injectInfo(i18n.t('workflow.orchestrator.exploringProject', { ns: 'simpleMode' }), 'info');

  try {
    const result = await invoke<{
      success: boolean;
      data: ExplorationCardData | null;
      error: string | null;
    }>('explore_project', {
      request: {
        sessionId,
        flowLevel: config.flowLevel,
        taskDescription,
        provider: explorationResolved.provider || null,
        model: explorationResolved.model || null,
        apiKey: null,
        baseUrl: explorationResolved.baseUrl || null,
        locale: i18n.language,
        contextSources: (await import('./contextSources')).useContextSourcesStore.getState().buildConfig() ?? null,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      const normalized = normalizeExplorationCardData(result.data);
      set({ explorationResult: normalized });
      injectCard('exploration_card', normalized);
    } else {
      injectInfo(i18n.t('workflow.orchestrator.explorationFailed', { ns: 'simpleMode' }), 'warning');
    }
  } catch {
    injectInfo(i18n.t('workflow.orchestrator.explorationFailed', { ns: 'simpleMode' }), 'warning');
  }
}

/**
 * Requirement analysis phase (Product Manager persona).
 *
 * Runs the PM expert-formatter pipeline to analyze requirements
 * from task description, interview results, and exploration context.
 */
async function requirementAnalysisPhase(set: SetFn, get: GetFn, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  const { config, taskDescription, explorationResult } = get();

  // Skip for quick flow
  if (config.flowLevel === 'quick') return;

  set({ phase: 'requirement_analysis' });
  await syncKernelTaskPhase('requirement_analysis', 'requirement_analysis_started');

  const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const reqResolved = resolvePhaseAgent('plan_requirements');

  injectCard('persona_indicator', {
    role: 'ProductManager',
    displayName: 'Product Manager',
    phase: 'requirement_analysis',
    model: formatModelDisplay(reqResolved),
  });
  injectInfo(
    i18n.t('workflow.orchestrator.analyzingRequirements', {
      ns: 'simpleMode',
      defaultValue: 'Analyzing requirements...',
    }),
    'info',
  );

  try {
    // Build exploration context string for the backend
    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;

    // Get compiled spec from interview (if any)
    const specStore = useSpecInterviewStore.getState();
    const interviewResult = specStore.compiledSpec ? JSON.stringify(specStore.compiledSpec) : null;

    const contextSources = (await import('./contextSources')).useContextSourcesStore.getState().buildConfig() ?? null;
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const result = await invoke<{
      success: boolean;
      data: RequirementAnalysisCardData | null;
      error: string | null;
    }>('run_requirement_analysis', {
      request: {
        sessionId: get().sessionId || '',
        taskDescription,
        interviewResult,
        explorationContext,
        provider: reqResolved.provider || null,
        model: reqResolved.model || null,
        apiKey: null,
        baseUrl: reqResolved.baseUrl || null,
        locale: i18n.language,
        contextSources,
        projectPath,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      set({ requirementAnalysis: result.data });
      injectCard('requirement_analysis_card', result.data);
    } else {
      // Non-blocking — warn and continue
      injectInfo(
        i18n.t('workflow.orchestrator.requirementAnalysisFailed', {
          ns: 'simpleMode',
          defaultValue: 'Requirement analysis could not be completed. Continuing...',
        }),
        'warning',
      );
    }
  } catch {
    injectInfo(
      i18n.t('workflow.orchestrator.requirementAnalysisFailed', {
        ns: 'simpleMode',
        defaultValue: 'Requirement analysis could not be completed. Continuing...',
      }),
      'warning',
    );
  }
}

/**
 * Architecture review phase (Software Architect persona).
 *
 * Reviews the approved PRD and injects an interactive card for
 * the user to accept or request revisions. Max 3 rounds.
 */
async function architectureReviewPhase(set: SetFn, get: GetFn, prd: TaskPrd, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  const { architectureReviewRound, explorationResult } = get();

  // Max 3 rounds to prevent infinite loops
  if (architectureReviewRound >= 3) {
    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewMaxRounds', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review limit reached (3 rounds). Proceeding with current PRD.',
      }),
      'warning',
    );
    return;
  }

  set({ phase: 'architecture_review', architectureReviewRound: architectureReviewRound + 1 });
  await syncKernelTaskPhase('architecture_review', 'architecture_review_started');

  const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const archResolved = resolvePhaseAgent('plan_architecture');

  injectCard('persona_indicator', {
    role: 'SoftwareArchitect',
    displayName: 'Software Architect',
    phase: 'architecture_review',
    model: formatModelDisplay(archResolved),
  });
  injectInfo(
    i18n.t('workflow.orchestrator.reviewingArchitecture', {
      ns: 'simpleMode',
      defaultValue: 'Reviewing architecture...',
    }),
    'info',
  );

  try {
    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;

    const archContextSources =
      (await import('./contextSources')).useContextSourcesStore.getState().buildConfig() ?? null;
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const result = await invoke<{
      success: boolean;
      data: ArchitectureReviewCardData | null;
      error: string | null;
    }>('run_architecture_review', {
      request: {
        sessionId: get().sessionId || '',
        prdJson: JSON.stringify(prd),
        explorationContext,
        provider: archResolved.provider || null,
        model: archResolved.model || null,
        apiKey: null,
        baseUrl: archResolved.baseUrl || null,
        locale: i18n.language,
        contextSources: archContextSources,
        projectPath,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      set({ architectureReview: result.data });
      injectCard('architecture_review_card', result.data, true);
      // Phase stays as 'architecture_review' — user interacts with the card
      // Continuation happens in approveArchitecture() action
    } else {
      // Architecture review failed — skip and continue
      injectInfo(
        i18n.t('workflow.orchestrator.architectureReviewFailed', {
          ns: 'simpleMode',
          defaultValue: 'Architecture review could not be completed. Continuing...',
        }),
        'warning',
      );
      // Continue to design doc + execution
      await designDocAndExecutePhase(set, get, prd, runToken);
    }
  } catch {
    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewFailed', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review could not be completed. Continuing...',
      }),
      'warning',
    );
    await designDocAndExecutePhase(set, get, prd, runToken);
  }
}

/**
 * Design doc generation + execution phase.
 *
 * Generates design doc from PRD, then starts story execution.
 * Extracted from approvePrd to share between approveArchitecture and quick flow.
 */
async function designDocAndExecutePhase(set: SetFn, get: GetFn, prd: TaskPrd, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  set({ phase: 'generating_design_doc', editablePrd: prd });
  await syncKernelTaskPhase('generating_design_doc', 'design_doc_generation_started');
  injectInfo(i18n.t('workflow.orchestrator.generatingDesignDoc', { ns: 'simpleMode' }), 'info');

  try {
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const designResult = await invoke<{
      success: boolean;
      data?: {
        design_doc: {
          overview: { title: string; summary: string };
          architecture: {
            system_overview: string;
            data_flow: string;
            infrastructure: { existing_services: string[]; new_services: string[] };
            components: {
              name: string;
              description: string;
              responsibilities: string[];
              dependencies: string[];
              features: string[];
            }[];
            patterns: {
              name: string;
              description: string;
              rationale: string;
              applies_to: string[];
            }[];
          };
          decisions: {
            id: string;
            title: string;
            context: string;
            decision: string;
            rationale: string;
            alternatives_considered: string[];
            status: string;
            applies_to: string[];
          }[];
          feature_mappings: Record<
            string,
            {
              description: string;
              components: string[];
              patterns: string[];
              decisions: string[];
            }
          >;
        };
        saved_path: string | null;
        generation_info: unknown;
      };
      error?: string;
    }>('prepare_design_doc_for_task', { sessionId: get().sessionId, prd, projectPath });
    if (!isRunActive(get, runToken)) return;
    if (designResult.success && designResult.data) {
      const doc = designResult.data.design_doc;
      const cardData: DesignDocCardData = {
        title: doc.overview.title,
        summary: doc.overview.summary,
        systemOverview: doc.architecture.system_overview,
        dataFlow: doc.architecture.data_flow,
        infrastructure: {
          existingServices: doc.architecture.infrastructure?.existing_services ?? [],
          newServices: doc.architecture.infrastructure?.new_services ?? [],
        },
        componentsCount: doc.architecture.components.length,
        componentNames: doc.architecture.components.map((c) => c.name),
        components: doc.architecture.components.map((c) => ({
          name: c.name,
          description: c.description,
          responsibilities: c.responsibilities ?? [],
          dependencies: c.dependencies ?? [],
          features: c.features ?? [],
        })),
        patternsCount: doc.architecture.patterns.length,
        patternNames: doc.architecture.patterns.map((p) => p.name),
        patterns: doc.architecture.patterns.map((p) => ({
          name: p.name,
          description: p.description,
          rationale: p.rationale,
          appliesTo: p.applies_to ?? [],
        })),
        decisionsCount: doc.decisions.length,
        decisions: doc.decisions.map((d) => ({
          id: d.id,
          title: d.title,
          context: d.context,
          decision: d.decision,
          rationale: d.rationale,
          alternatives: d.alternatives_considered ?? [],
          status: d.status,
          appliesTo: d.applies_to ?? [],
        })),
        featureMappingsCount: Object.keys(doc.feature_mappings).length,
        featureMappings: Object.entries(doc.feature_mappings).map(([featureId, mapping]) => ({
          featureId,
          description: mapping.description ?? '',
          components: mapping.components ?? [],
          patterns: mapping.patterns ?? [],
          decisions: mapping.decisions ?? [],
        })),
        savedPath: designResult.data.saved_path,
      };
      injectCard('design_doc_card', cardData);
    }
    if (!designResult.success) {
      injectInfo(i18n.t('workflow.orchestrator.designDocFailed', { ns: 'simpleMode' }), 'warning');
    }
  } catch {
    injectInfo(i18n.t('workflow.orchestrator.designDocFailed', { ns: 'simpleMode' }), 'warning');
  }

  // Start execution
  if (!isRunActive(get, runToken)) return;
  set({ phase: 'executing', isCancelling: false });
  injectInfo(i18n.t('workflow.orchestrator.prdApproved', { ns: 'simpleMode' }), 'success');

  try {
    await subscribeToProgressEvents(set, get, runToken);
    if (!isRunActive(get, runToken)) return;
    await useTaskModeStore.getState().approvePrd(prd);
    if (!isRunActive(get, runToken)) return;

    const taskModeError = useTaskModeStore.getState().error;
    if (taskModeError) {
      set({ phase: 'failed', error: taskModeError });
      injectError(i18n.t('workflow.orchestrator.executionFailed', { ns: 'simpleMode' }), taskModeError);
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError(i18n.t('workflow.orchestrator.executionFailed', { ns: 'simpleMode' }), msg);
  }
}

/**
 * Generate PRD phase (called after config confirmation or interview completion).
 */
async function generatePrdPhase(set: SetFn, get: GetFn, runToken: number) {
  if (!isRunActive(get, runToken)) return;
  set({ phase: 'generating_prd' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const prdResolved = resolvePhaseAgent('plan_prd');

  injectCard('persona_indicator', {
    role: 'TechLead',
    displayName: 'Tech Lead',
    phase: 'generating_prd',
    model: formatModelDisplay(prdResolved),
  });
  injectInfo(i18n.t('workflow.orchestrator.generatingPrd', { ns: 'simpleMode' }), 'info');

  try {
    // Pass conversation history and context budget to PRD generation
    const history = get()._conversationHistory || [];
    const settings = useSettingsStore.getState();
    const maxContextTokens = settings.maxTotalTokens ?? 200_000;
    await useTaskModeStore
      .getState()
      .generatePrd(
        history,
        maxContextTokens,
        prdResolved.provider || undefined,
        prdResolved.model || undefined,
        prdResolved.baseUrl,
      );
    if (!isRunActive(get, runToken)) return;

    const taskModeState = useTaskModeStore.getState();
    if (taskModeState.error) {
      set({ phase: 'failed', error: taskModeState.error });
      injectError(i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }), taskModeState.error);
      return;
    }

    const prd = taskModeState.prd;
    if (!prd) {
      set({ phase: 'failed', error: 'PRD generation returned empty result' });
      injectError(
        i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }),
        i18n.t('workflow.orchestrator.prdMissingData', { ns: 'simpleMode' }),
      );
      return;
    }

    // Synthesize planning turn into conversation history (Task → Chat writeback)
    const { taskDescription, strategyAnalysis } = get();
    synthesizePlanningTurn(taskDescription, strategyAnalysis, prd);

    set({ phase: 'reviewing_prd', editablePrd: prd });

    // Inject PRD card
    injectCard('prd_card', toPrdCardData(prd), true);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError(i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }), msg);
  }
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
      const taskModeState = useTaskModeStore.getState();
      const taskModePatch: {
        currentBatch: number;
        totalBatches: number;
        storyStatuses?: Record<string, string>;
        qualityGateResults?: typeof taskModeState.qualityGateResults;
        isCancelling?: boolean;
        error?: string | null;
      } = {
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
      };

      // Accumulate story status
      if (payload.storyId && payload.storyStatus) {
        accumulatedStatuses[payload.storyId] = payload.storyStatus;
        taskModePatch.storyStatuses = {
          ...taskModeState.storyStatuses,
          [payload.storyId]: payload.storyStatus,
        };
      }

      if (payload.storyId && payload.gateResults && payload.gateResults.length > 0) {
        taskModePatch.qualityGateResults = {
          ...taskModeState.qualityGateResults,
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
          break;
        }

        case 'execution_completed': {
          const fallbackTotalStories =
            Object.keys(accumulatedStatuses).length ||
            state.editablePrd?.stories.length ||
            taskModeState.prd?.stories.length ||
            0;
          const totalStories = fallbackTotalStories;
          const completedCount = Object.values(accumulatedStatuses).filter((s) => s === 'completed').length;
          const failedCount = Object.values(accumulatedStatuses).filter((s) => s === 'failed').length;
          const success = failedCount === 0;
          taskModePatch.isCancelling = false;
          useTaskModeStore.setState(taskModePatch);

          set({ phase: success ? 'completed' : 'failed', isCancelling: false });

          void (async () => {
            let report: ExecutionReport | null = null;
            try {
              report = await Promise.race<ExecutionReport | null>([
                (async () => {
                  await useTaskModeStore.getState().fetchReport();
                  const latestReport = useTaskModeStore.getState().report;
                  return latestReport && latestReport.sessionId === payload.sessionId ? latestReport : null;
                })(),
                new Promise<null>((resolve) => {
                  setTimeout(() => resolve(null), 1500);
                }),
              ]);
            } catch {
              report = null;
            }

            if (!isRunActive(get, runToken)) return;
            if (get()._completionCardInjectedRunToken === runToken) return;

            const completionData = report
              ? buildCompletionReportDataFromReport(report)
              : buildCompletionReportDataFallback({
                  success,
                  totalStories,
                  completed: completedCount,
                  failed: failedCount,
                });

            injectCard('completion_report', completionData);
            set({ _completionCardInjectedRunToken: runToken });

            // Synthesize execution result into conversation history (Task → Chat writeback)
            synthesizeExecutionTurn(completionData.completed, completionData.totalStories, completionData.success);
          })();
          break;
        }

        case 'execution_cancelled': {
          taskModePatch.isCancelling = false;
          useTaskModeStore.setState(taskModePatch);
          set({ phase: 'cancelled', isCancelling: false });
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
            taskModePatch.error = payload.error;
          }
          taskModePatch.isCancelling = false;
          useTaskModeStore.setState(taskModePatch);
          set({ isCancelling: false });
          if (payload.error) {
            injectError(i18n.t('workflow.orchestrator.executionError', { ns: 'simpleMode' }), payload.error);
          }
          break;
        }
        default: {
          if (payload.eventType === 'story_failed' && payload.error) {
            taskModePatch.error = payload.error;
          }
          useTaskModeStore.setState(taskModePatch);
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
