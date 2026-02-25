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
 * - useExecutionStore: appendStreamLine for card injection into chat transcript
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../i18n';
import { useExecutionStore } from './execution';
import { useTaskModeStore, type TaskPrd, type StrategyAnalysis, type GateResult } from './taskMode';
import { useSpecInterviewStore, type InterviewQuestion, type InterviewSession } from './specInterview';
import { useSettingsStore } from './settings';
import { buildConversationHistory, synthesizePlanningTurn, synthesizeExecutionTurn } from '../lib/contextBridge';
import type { CrossModeConversationTurn } from '../types/crossModeContext';
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

  /** Event unsubscribe function */
  _unlistenFn: UnlistenFn | null;

  /** Conversation history extracted from Chat for Task context sharing */
  _conversationHistory: CrossModeConversationTurn[];

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
    selectedModifications: Array<{ storyId: string; action: string; reason: string }>,
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
  _unlistenFn: null as UnlistenFn | null,
  _conversationHistory: [] as CrossModeConversationTurn[],
};

// ============================================================================
// Helpers
// ============================================================================

let _cardCounter = 0;

function nextCardId(): string {
  return `card-${++_cardCounter}-${Date.now()}`;
}

/** Inject a card message into the chat transcript */
function injectCard<T extends CardPayload['cardType']>(cardType: T, data: CardPayload['data'], interactive = false) {
  const payload: CardPayload = {
    cardType,
    cardId: nextCardId(),
    data,
    interactive,
  };
  useExecutionStore.getState().appendStreamLine(JSON.stringify(payload), 'card');
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
function buildStrategyCardData(analysis: StrategyAnalysis): StrategyCardData {
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
    // Add user message as 'info' StreamLine so it appears as a chat bubble in ChatTranscript
    useExecutionStore.getState().appendStreamLine(description, 'info');

    set({ phase: 'analyzing', taskDescription: description, error: null });
    injectInfo(i18n.t('workflow.orchestrator.analyzingTask', { ns: 'simpleMode' }), 'info');

    // Extract complete Chat conversation history for Task context sharing
    const conversationHistory = buildConversationHistory();
    set({ _conversationHistory: conversationHistory });

    try {
      // 1. Enter task mode (creates session + runs strategy analysis)
      await useTaskModeStore.getState().enterTaskMode(description);

      const taskModeState = useTaskModeStore.getState();
      if (taskModeState.error) {
        set({ phase: 'failed', error: taskModeState.error });
        injectError('Strategy Analysis Failed', taskModeState.error);
        return;
      }

      const sessionId = taskModeState.sessionId;
      let analysis = taskModeState.strategyAnalysis;

      // 2. Try LLM enhancement of strategy analysis
      if (analysis) {
        try {
          injectInfo(i18n.t('workflow.orchestrator.enhancingAnalysis', { ns: 'simpleMode' }), 'info');

          const settings = useSettingsStore.getState();
          const { resolveProviderBaseUrl } = await import('../lib/providers');
          const baseUrl = settings.provider ? resolveProviderBaseUrl(settings.provider, settings) : undefined;

          const enhanced = await invoke<{ success: boolean; data: StrategyAnalysis | null; error: string | null }>(
            'enhance_strategy_with_llm',
            {
              description,
              keywordAnalysis: analysis,
              provider: settings.provider || null,
              model: settings.model || null,
              apiKey: null,
              baseUrl: baseUrl || null,
              locale: i18n.language,
            },
          );

          if (enhanced.success && enhanced.data) {
            analysis = enhanced.data;
            useTaskModeStore.setState({ strategyAnalysis: analysis });
          }
        } catch {
          // LLM enhancement failed — silently use keyword analysis
        }
      }

      set({ sessionId, strategyAnalysis: analysis });

      // 3. Inject strategy card (with LLM-enhanced or keyword result)
      if (analysis) {
        injectCard('strategy_card', buildStrategyCardData(analysis));
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

      set({ config, phase: 'configuring' });

      // 5. Inject config card (user interacts with it to advance)
      injectCard('config_card', buildConfigCardData(config, false), true);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError('Workflow Failed', msg);
    }
  },

  /**
   * Confirm configuration and advance workflow.
   * Phase flow: configuring → exploring → [interviewing] → requirement_analysis → generating_prd
   */
  confirmConfig: async (overrides?: Partial<WorkflowConfig>) => {
    const state = get();
    const config = overrides ? { ...state.config, ...overrides } : state.config;
    set({ config, phase: 'exploring' });

    try {
      // Always explore first (exploration provides context for interview BA)
      await explorePhase(set, get);

      if (config.specInterviewEnabled) {
        // Start interview flow (BA now has exploration context)
        set({ phase: 'interviewing' });
        injectCard('persona_indicator', {
          role: 'BusinessAnalyst',
          displayName: 'Business Analyst',
          phase: 'interviewing',
        });
        injectInfo(i18n.t('workflow.orchestrator.startingInterview', { ns: 'simpleMode' }), 'info');

        const settings = useSettingsStore.getState();
        const workspacePath = settings.workspacePath;
        const { explorationResult } = get();
        const interviewConfig = {
          description: state.taskDescription,
          flow_level: config.flowLevel,
          max_questions: config.flowLevel === 'quick' ? 5 : config.flowLevel === 'full' ? 15 : 10,
          first_principles: false,
          project_path: workspacePath,
          exploration_context: explorationResult ? JSON.stringify(explorationResult) : null,
          locale: i18n.language,
        };

        // Set LLM provider settings so specInterview store passes them to backend
        if (settings.provider) {
          const { resolveProviderBaseUrl } = await import('../lib/providers');
          const baseUrl = resolveProviderBaseUrl(settings.provider, settings);
          useSpecInterviewStore.getState().setProviderSettings({
            provider: settings.provider,
            model: settings.model || undefined,
            baseUrl: baseUrl || undefined,
          });
        }

        // Retry with exponential backoff if backend not yet initialized (race with init_app)
        const maxRetries = 5;
        const baseDelay = 500;
        let session: InterviewSession | null = null;
        for (let attempt = 0; attempt < maxRetries; attempt++) {
          session = await useSpecInterviewStore.getState().startInterview(interviewConfig);
          if (session) break;
          const interviewError = useSpecInterviewStore.getState().error || '';
          if (interviewError.includes('not initialized') && attempt < maxRetries - 1) {
            await new Promise((r) => setTimeout(r, baseDelay * Math.pow(2, attempt)));
            useSpecInterviewStore.getState().clearError();
            continue;
          }
          break;
        }

        if (!session) {
          const interviewError = useSpecInterviewStore.getState().error;
          set({ phase: 'failed', error: interviewError || 'Failed to start interview' });
          injectError('Interview Failed', interviewError || 'Failed to start interview');
          return;
        }

        set({ interviewId: session.id });

        // Present first question
        if (session.current_question) {
          const questionData = mapInterviewQuestion(
            session.current_question,
            session.question_cursor + 1,
            session.max_questions,
          );
          set({ pendingQuestion: questionData });
          injectCard('interview_question', questionData, true);
        }
      } else {
        // Skip interview, run requirement analysis then generate PRD
        await requirementAnalysisPhase(set, get);
        await generatePrdPhase(set, get);
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError('Configuration Failed', msg);
    }
  },

  /** Update workflow config without advancing phase */
  updateConfig: (updates: Partial<WorkflowConfig>) => {
    set((state) => ({ config: { ...state.config, ...updates } }));
  },

  /** Parse natural language config override */
  overrideConfigNatural: (text: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    useExecutionStore.getState().appendStreamLine(text, 'info');

    const updates: Partial<WorkflowConfig> = {};
    const lower = text.toLowerCase();

    // Parse max parallel
    const parallelMatch = lower.match(/(\d+)\s*parallel/);
    if (parallelMatch) {
      updates.maxParallel = Math.min(Math.max(parseInt(parallelMatch[1], 10), 1), 16);
    }

    // Parse TDD mode
    if (lower.includes('tdd') || lower.includes('test-driven')) {
      if (lower.includes('strict')) {
        updates.tddMode = 'strict';
      } else if (lower.includes('off') || lower.includes('disable') || lower.includes('no tdd')) {
        updates.tddMode = 'off';
      } else {
        updates.tddMode = 'flexible';
      }
    }

    // Parse flow level
    if (lower.includes('quick') || lower.includes('fast')) {
      updates.flowLevel = 'quick';
    } else if (lower.includes('full') || lower.includes('thorough')) {
      updates.flowLevel = 'full';
    }

    // Parse quality gates
    if (lower.includes('no quality') || lower.includes('skip quality') || lower.includes('disable quality')) {
      updates.qualityGatesEnabled = false;
    }
    if (lower.includes('enable quality')) {
      updates.qualityGatesEnabled = true;
    }

    // Parse interview
    if (lower.includes('interview') || lower.includes('spec')) {
      if (lower.includes('skip') || lower.includes('no') || lower.includes('disable')) {
        updates.specInterviewEnabled = false;
      } else {
        updates.specInterviewEnabled = true;
      }
    }

    if (Object.keys(updates).length > 0) {
      set((state) => ({ config: { ...state.config, ...updates } }));
      injectInfo(
        i18n.t('workflow.orchestrator.configUpdated', {
          ns: 'simpleMode',
          details: Object.entries(updates)
            .map(([k, v]) => `${k}=${v}`)
            .join(', '),
        }),
        'success',
      );
    }
  },

  /** Submit answer to current interview question */
  submitInterviewAnswer: async (answer: string) => {
    const { pendingQuestion } = get();
    if (!pendingQuestion) return;

    // Inject answer card
    const answerData: InterviewAnswerCardData = {
      questionId: pendingQuestion.questionId,
      answer,
      skipped: false,
    };
    injectCard('interview_answer', answerData);
    set({ pendingQuestion: null });

    // Submit to backend
    const updatedSession = await useSpecInterviewStore.getState().submitAnswer(answer);

    if (!updatedSession) {
      const error = useSpecInterviewStore.getState().error;
      set({ phase: 'failed', error: error || 'Failed to submit answer' });
      injectError('Interview Error', error || 'Failed to submit answer');
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

      if (compiled) {
        // Advance to requirement analysis then PRD generation
        await requirementAnalysisPhase(set, get);
        await generatePrdPhase(set, get);
      } else {
        const error = useSpecInterviewStore.getState().error;
        set({ phase: 'failed', error: error || 'Failed to compile spec' });
        injectError('Spec Compilation Failed', error || 'Failed to compile spec');
      }
      return;
    }

    // Present next question
    if (updatedSession.current_question) {
      const questionData = mapInterviewQuestion(
        updatedSession.current_question,
        updatedSession.question_cursor + 1,
        updatedSession.max_questions,
      );
      set({ pendingQuestion: questionData });
      injectCard('interview_question', questionData, true);
    }
  },

  /** Skip current interview question */
  skipInterviewQuestion: async () => {
    const { pendingQuestion } = get();
    if (!pendingQuestion) return;

    const answerData: InterviewAnswerCardData = {
      questionId: pendingQuestion.questionId,
      answer: '',
      skipped: true,
    };
    injectCard('interview_answer', answerData);
    set({ pendingQuestion: null });

    // Submit skip (empty answer)
    const updatedSession = await useSpecInterviewStore.getState().submitAnswer('');

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
      if (compiled) {
        await requirementAnalysisPhase(set, get);
        await generatePrdPhase(set, get);
      } else {
        const error = useSpecInterviewStore.getState().error;
        set({ phase: 'failed', error: error || 'Failed to compile spec' });
        injectError('Spec Compilation Failed', error || 'Failed to compile spec');
      }
      return;
    }

    if (updatedSession.current_question) {
      const questionData = mapInterviewQuestion(
        updatedSession.current_question,
        updatedSession.question_cursor + 1,
        updatedSession.max_questions,
      );
      set({ pendingQuestion: questionData });
      injectCard('interview_question', questionData, true);
    }
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
    const prd = editedPrd || state.editablePrd;
    if (!prd) {
      set({ error: 'No PRD to approve' });
      return;
    }

    set({ editablePrd: prd });

    // Non-quick flow: run architecture review (interactive — returns after injecting card)
    if (state.config.flowLevel !== 'quick') {
      await architectureReviewPhase(set, get, prd);
      // Architecture review is interactive — user clicks Accept/Revise in the card.
      // The continuation happens in approveArchitecture() action.
      return;
    }

    // Quick flow: skip architecture review, go straight to design doc + execution
    await designDocAndExecutePhase(set, get, prd);
  },

  /** Add feedback to editable PRD (during reviewing_prd phase) */
  addPrdFeedback: (_feedback: string) => {
    // Add user message as 'info' StreamLine so it appears as a chat bubble
    useExecutionStore.getState().appendStreamLine(_feedback, 'info');

    // In the future, this could use LLM to apply NL edits to the PRD.
    // For now, inject as info message.
    injectInfo(i18n.t('workflow.orchestrator.prdFeedbackNoted', { ns: 'simpleMode', feedback: _feedback }), 'info');
  },

  /** Approve or request changes to the architecture review */
  approveArchitecture: async (
    acceptAsIs: boolean,
    selectedModifications: Array<{ storyId: string; action: string; reason: string }>,
  ) => {
    const { phase, editablePrd } = get();
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

      await designDocAndExecutePhase(set, get, prd);
    } else {
      // Apply selected modifications to the editable PRD and return to review
      if (editablePrd) {
        injectInfo(
          i18n.t('workflow.orchestrator.architectureRevisionRequested', {
            ns: 'simpleMode',
            count: selectedModifications.length,
            defaultValue: 'Applying {{count}} architectural suggestions. Returning to PRD review...',
          }),
          'warning',
        );
      }
      set({ phase: 'reviewing_prd' });

      // Re-inject PRD card for user to review modifications
      if (editablePrd) {
        const prdData: PrdCardData = {
          title: editablePrd.title,
          description: editablePrd.description,
          stories: editablePrd.stories.map((s) => ({
            id: s.id,
            title: s.title,
            description: s.description,
            priority: s.priority,
            dependencies: s.dependencies,
            acceptanceCriteria: s.acceptanceCriteria,
          })),
          batches: editablePrd.batches.map((b) => ({
            index: b.index,
            storyIds: b.storyIds,
          })),
          isEditable: true,
        };
        injectCard('prd_card', prdData, true);
      }
    }
  },

  /** Cancel the current workflow */
  cancelWorkflow: async () => {
    const { phase, sessionId } = get();

    if (phase === 'executing' && sessionId) {
      await useTaskModeStore.getState().cancelExecution();
    }

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

    set({ ...DEFAULT_STATE });
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
async function explorePhase(set: SetFn, get: GetFn) {
  const { config, taskDescription, sessionId } = get();

  // Quick flow: skip exploration entirely
  if (config.flowLevel === 'quick') return;

  set({ phase: 'exploring' });
  injectCard('persona_indicator', { role: 'SeniorEngineer', displayName: 'Senior Engineer', phase: 'exploring' });
  injectInfo(i18n.t('workflow.orchestrator.exploringProject', { ns: 'simpleMode' }), 'info');

  try {
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');
    const baseUrl = settings.provider ? resolveProviderBaseUrl(settings.provider, settings) : undefined;

    const result = await invoke<{
      success: boolean;
      data: ExplorationCardData | null;
      error: string | null;
    }>('explore_project', {
      sessionId,
      flowLevel: config.flowLevel,
      taskDescription,
      provider: settings.provider || null,
      model: settings.model || null,
      apiKey: null,
      baseUrl: baseUrl || null,
    });

    if (result.success && result.data) {
      set({ explorationResult: result.data });
      injectCard('exploration_card', result.data);
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
async function requirementAnalysisPhase(set: SetFn, get: GetFn) {
  const { config, taskDescription, explorationResult } = get();

  // Skip for quick flow
  if (config.flowLevel === 'quick') return;

  set({ phase: 'requirement_analysis' });
  injectCard('persona_indicator', {
    role: 'ProductManager',
    displayName: 'Product Manager',
    phase: 'requirement_analysis',
  });
  injectInfo(
    i18n.t('workflow.orchestrator.analyzingRequirements', {
      ns: 'simpleMode',
      defaultValue: 'Analyzing requirements...',
    }),
    'info',
  );

  try {
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');
    const baseUrl = settings.provider ? resolveProviderBaseUrl(settings.provider, settings) : undefined;

    // Build exploration context string for the backend
    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;

    // Get compiled spec from interview (if any)
    const specStore = useSpecInterviewStore.getState();
    const interviewResult = specStore.compiledSpec ? JSON.stringify(specStore.compiledSpec) : null;

    const result = await invoke<{
      success: boolean;
      data: RequirementAnalysisCardData | null;
      error: string | null;
    }>('run_requirement_analysis', {
      sessionId: get().sessionId || '',
      taskDescription,
      interviewResult,
      explorationContext,
      provider: settings.provider || null,
      model: settings.model || null,
      apiKey: null,
      baseUrl: baseUrl || null,
    });

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
async function architectureReviewPhase(set: SetFn, get: GetFn, prd: TaskPrd) {
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
  injectCard('persona_indicator', {
    role: 'SoftwareArchitect',
    displayName: 'Software Architect',
    phase: 'architecture_review',
  });
  injectInfo(
    i18n.t('workflow.orchestrator.reviewingArchitecture', {
      ns: 'simpleMode',
      defaultValue: 'Reviewing architecture...',
    }),
    'info',
  );

  try {
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');
    const baseUrl = settings.provider ? resolveProviderBaseUrl(settings.provider, settings) : undefined;

    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;

    const result = await invoke<{
      success: boolean;
      data: ArchitectureReviewCardData | null;
      error: string | null;
    }>('run_architecture_review', {
      sessionId: get().sessionId || '',
      prdJson: JSON.stringify(prd),
      explorationContext,
      provider: settings.provider || null,
      model: settings.model || null,
      apiKey: null,
      baseUrl: baseUrl || null,
    });

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
      await designDocAndExecutePhase(set, get, prd);
    }
  } catch {
    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewFailed', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review could not be completed. Continuing...',
      }),
      'warning',
    );
    await designDocAndExecutePhase(set, get, prd);
  }
}

/**
 * Design doc generation + execution phase.
 *
 * Generates design doc from PRD, then starts story execution.
 * Extracted from approvePrd to share between approveArchitecture and quick flow.
 */
async function designDocAndExecutePhase(set: SetFn, get: GetFn, prd: TaskPrd) {
  set({ phase: 'generating_design_doc', editablePrd: prd });
  injectInfo(i18n.t('workflow.orchestrator.generatingDesignDoc', { ns: 'simpleMode' }), 'info');

  try {
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const designResult = await invoke<{
      success: boolean;
      data?: {
        design_doc: {
          overview: { title: string; summary: string };
          architecture: { components: { name: string }[]; patterns: { name: string }[] };
          decisions: unknown[];
          feature_mappings: Record<string, unknown>;
        };
        saved_path: string | null;
        generation_info: unknown;
      };
      error?: string;
    }>('prepare_design_doc_for_task', { prd, projectPath });
    if (designResult.success && designResult.data) {
      const doc = designResult.data.design_doc;
      const cardData: DesignDocCardData = {
        title: doc.overview.title,
        summary: doc.overview.summary,
        componentsCount: doc.architecture.components.length,
        componentNames: doc.architecture.components.map((c) => c.name),
        patternsCount: doc.architecture.patterns.length,
        patternNames: doc.architecture.patterns.map((p) => p.name),
        decisionsCount: doc.decisions.length,
        featureMappingsCount: Object.keys(doc.feature_mappings).length,
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
  set({ phase: 'executing' });
  injectInfo(i18n.t('workflow.orchestrator.prdApproved', { ns: 'simpleMode' }), 'success');

  try {
    await subscribeToProgressEvents(set, get);
    await useTaskModeStore.getState().approvePrd(prd);

    const taskModeError = useTaskModeStore.getState().error;
    if (taskModeError) {
      set({ phase: 'failed', error: taskModeError });
      injectError('Execution Failed', taskModeError);
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError('Execution Failed', msg);
  }
}

/**
 * Generate PRD phase (called after config confirmation or interview completion).
 */
async function generatePrdPhase(set: SetFn, get: GetFn) {
  set({ phase: 'generating_prd' });
  injectInfo(i18n.t('workflow.orchestrator.generatingPrd', { ns: 'simpleMode' }), 'info');

  try {
    // Pass conversation history and context budget to PRD generation
    const history = get()._conversationHistory || [];
    const settings = useSettingsStore.getState();
    const maxContextTokens = settings.maxTotalTokens ?? 200_000;
    await useTaskModeStore.getState().generatePrd(history, maxContextTokens);

    const taskModeState = useTaskModeStore.getState();
    if (taskModeState.error) {
      set({ phase: 'failed', error: taskModeState.error });
      injectError('PRD Generation Failed', taskModeState.error);
      return;
    }

    const prd = taskModeState.prd;
    if (!prd) {
      set({ phase: 'failed', error: 'PRD generation returned empty result' });
      injectError('PRD Generation Failed', 'No PRD data returned');
      return;
    }

    // Synthesize planning turn into conversation history (Task → Chat writeback)
    const { taskDescription, strategyAnalysis } = get();
    synthesizePlanningTurn(taskDescription, strategyAnalysis, prd);

    set({ phase: 'reviewing_prd', editablePrd: prd });

    // Inject PRD card
    const prdData: PrdCardData = {
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
    injectCard('prd_card', prdData, true);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError('PRD Generation Failed', msg);
  }
}

/**
 * Subscribe to task-mode-progress events for execution tracking.
 *
 * Rust emits individual per-story events (batch_started, story_started,
 * story_completed, story_failed, execution_completed, execution_cancelled, error).
 * We accumulate story statuses locally and inject appropriate UI cards.
 */
async function subscribeToProgressEvents(set: SetFn, get: GetFn) {
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
      const payload = event.payload;
      const state = get();
      if (state.sessionId && payload.sessionId !== state.sessionId) return;

      // Accumulate story status
      if (payload.storyId && payload.storyStatus) {
        accumulatedStatuses[payload.storyId] = payload.storyStatus;
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
              overallStatus: payload.gateResults.every((g) => g.status === 'passed') ? 'passed' : 'failed',
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
              overallStatus: 'failed',
              gates: payload.gateResults,
              codeReviewScores: [],
            } as GateResultCardData);
          }
          break;
        }

        case 'execution_completed': {
          const totalStories = Object.keys(accumulatedStatuses).length;
          const completedCount = Object.values(accumulatedStatuses).filter((s) => s === 'completed').length;
          const failedCount = Object.values(accumulatedStatuses).filter((s) => s === 'failed').length;
          const success = failedCount === 0;

          injectCard('completion_report', {
            success,
            totalStories,
            completed: completedCount,
            failed: failedCount,
            duration: 0,
            agentAssignments: {},
          } as CompletionReportCardData);

          set({ phase: success ? 'completed' : 'failed' });

          // Synthesize execution result into conversation history (Task → Chat writeback)
          synthesizeExecutionTurn(completedCount, totalStories, success);

          // Fetch full report for duration/agent data
          useTaskModeStore
            .getState()
            .fetchReport()
            .then(() => {
              const report = useTaskModeStore.getState().report;
              if (report) {
                injectCard('completion_report', {
                  success: report.success,
                  totalStories: report.totalStories,
                  completed: report.storiesCompleted,
                  failed: report.storiesFailed,
                  duration: report.totalDurationMs,
                  agentAssignments: report.agentAssignments,
                } as CompletionReportCardData);
              }
            });
          break;
        }

        case 'execution_cancelled': {
          set({ phase: 'cancelled' });
          break;
        }

        case 'error': {
          if (payload.error) {
            injectError('Execution Error', payload.error);
          }
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
