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
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useExecutionStore } from './execution';
import { useTaskModeStore, type TaskPrd, type StrategyAnalysis, type StoryQualityGateResults } from './taskMode';
import { useSpecInterviewStore, type InterviewQuestion } from './specInterview';
import { useSettingsStore } from './settings';
import type {
  WorkflowPhase,
  WorkflowConfig,
  CardPayload,
  InterviewQuestionCardData,
  StrategyCardData,
  ConfigCardData,
  PrdCardData,
  ExecutionUpdateCardData,
  GateResultCardData,
  CompletionReportCardData,
  WorkflowInfoData,
  WorkflowErrorData,
  InterviewAnswerCardData,
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

  /** Working copy of PRD during review phase */
  editablePrd: TaskPrd | null;

  /** Currently pending interview question */
  pendingQuestion: InterviewQuestionCardData | null;

  /** Error message */
  error: string | null;

  /** Event unsubscribe function */
  _unlistenFn: UnlistenFn | null;

  // Actions
  startWorkflow: (description: string) => Promise<void>;
  confirmConfig: (overrides?: Partial<WorkflowConfig>) => Promise<void>;
  updateConfig: (updates: Partial<WorkflowConfig>) => void;
  overrideConfigNatural: (text: string) => void;
  submitInterviewAnswer: (answer: string) => Promise<void>;
  skipInterviewQuestion: () => Promise<void>;
  approvePrd: (editedPrd?: TaskPrd) => Promise<void>;
  addPrdFeedback: (feedback: string) => void;
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
  editablePrd: null as TaskPrd | null,
  pendingQuestion: null as InterviewQuestionCardData | null,
  error: null as string | null,
  _unlistenFn: null as UnlistenFn | null,
};

// ============================================================================
// Helpers
// ============================================================================

let _cardCounter = 0;

function nextCardId(): string {
  return `card-${++_cardCounter}-${Date.now()}`;
}

/** Inject a card message into the chat transcript */
function injectCard<T extends CardPayload['cardType']>(
  cardType: T,
  data: CardPayload['data'],
  interactive = false
) {
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
  totalQuestions: number
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
      inputType = 'multi_select';
      break;
    case 'boolean':
      inputType = 'boolean';
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
    options: [],
    questionNumber,
    totalQuestions,
  };
}

/** Build strategy card data from analysis */
function buildStrategyCardData(analysis: StrategyAnalysis): StrategyCardData {
  const recommendations: string[] = [];
  if (analysis.parallelizationBenefit === 'significant') {
    recommendations.push('High parallelization potential — multiple stories can execute concurrently');
  }
  if (analysis.riskLevel === 'high') {
    recommendations.push('High risk — consider enabling TDD mode and full quality gates');
  }
  if (analysis.estimatedStories > 6) {
    recommendations.push(`${analysis.estimatedStories} estimated stories — consider increasing max parallel agents`);
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
    set({ phase: 'analyzing', taskDescription: description, error: null });
    injectInfo('Analyzing task complexity...', 'info');

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
      const analysis = taskModeState.strategyAnalysis;

      set({ sessionId, strategyAnalysis: analysis });

      // 2. Inject strategy card
      if (analysis) {
        injectCard('strategy_card', buildStrategyCardData(analysis));
      }

      // 3. Build recommended config from analysis
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
        if (
          config.flowLevel !== 'quick' &&
          (analysis.riskLevel === 'high' || analysis.estimatedStories > 8)
        ) {
          config.specInterviewEnabled = true;
        }
      }

      set({ config, phase: 'configuring' });

      // 4. Inject config card
      injectCard('config_card', buildConfigCardData(config, false), true);

      // 5. Auto-advance in Simple mode (no manual config step)
      await get().confirmConfig();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError('Workflow Failed', msg);
    }
  },

  /**
   * Confirm configuration and advance workflow.
   * If interview enabled: configuring → interviewing
   * Else: configuring → generating_prd
   */
  confirmConfig: async (overrides?: Partial<WorkflowConfig>) => {
    const state = get();
    const config = overrides ? { ...state.config, ...overrides } : state.config;
    set({ config });

    try {
      if (config.specInterviewEnabled) {
        // Start interview flow
        set({ phase: 'interviewing' });
        injectInfo('Starting requirements interview...', 'info');

        const workspacePath = useSettingsStore.getState().workspacePath;
        const session = await useSpecInterviewStore.getState().startInterview({
          description: state.taskDescription,
          flow_level: config.flowLevel,
          max_questions: config.flowLevel === 'quick' ? 5 : config.flowLevel === 'full' ? 15 : 10,
          first_principles: false,
          project_path: workspacePath,
        });

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
            session.max_questions
          );
          set({ pendingQuestion: questionData });
          injectCard('interview_question', questionData, true);
        }
      } else {
        // Skip interview, go straight to PRD generation
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
      injectInfo(`Config updated: ${Object.entries(updates).map(([k, v]) => `${k}=${v}`).join(', ')}`, 'success');
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
      injectInfo('Interview complete! Compiling requirements...', 'success');

      // Compile spec
      const { config, taskDescription } = get();
      const compiled = await useSpecInterviewStore.getState().compileSpec({
        description: taskDescription,
        flow_level: config.flowLevel,
        tdd_mode: config.tddMode === 'off' ? null : config.tddMode,
      });

      if (compiled) {
        // Advance to PRD generation with compiled spec
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
        updatedSession.max_questions
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
      injectInfo('Interview complete! Compiling requirements...', 'success');
      const { config, taskDescription } = get();
      const compiled = await useSpecInterviewStore.getState().compileSpec({
        description: taskDescription,
        flow_level: config.flowLevel,
        tdd_mode: config.tddMode === 'off' ? null : config.tddMode,
      });
      if (compiled) {
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
        updatedSession.max_questions
      );
      set({ pendingQuestion: questionData });
      injectCard('interview_question', questionData, true);
    }
  },

  /** Approve PRD and start execution */
  approvePrd: async (editedPrd?: TaskPrd) => {
    const state = get();
    const prd = editedPrd || state.editablePrd;
    if (!prd) {
      set({ error: 'No PRD to approve' });
      return;
    }

    set({ phase: 'executing', editablePrd: prd });
    injectInfo('PRD approved! Starting execution...', 'success');

    try {
      // Subscribe to progress events
      await subscribeToProgressEvents(set, get);

      // Approve PRD via taskMode store
      await useTaskModeStore.getState().approvePrd(prd);

      const taskModeError = useTaskModeStore.getState().error;
      if (taskModeError) {
        set({ phase: 'failed', error: taskModeError });
        injectError('Execution Failed', taskModeError);
        return;
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ phase: 'failed', error: msg });
      injectError('Execution Failed', msg);
    }
  },

  /** Add feedback to editable PRD (during reviewing_prd phase) */
  addPrdFeedback: (_feedback: string) => {
    // In the future, this could use LLM to apply NL edits to the PRD.
    // For now, inject as info message.
    injectInfo(`PRD feedback noted: "${_feedback}". Use the Edit button on the PRD card to make changes.`, 'info');
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
    injectInfo('Workflow cancelled.', 'warning');
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
}));

// ============================================================================
// Internal Phase Transitions
// ============================================================================

type SetFn = (partial: Partial<WorkflowOrchestratorState> | ((state: WorkflowOrchestratorState) => Partial<WorkflowOrchestratorState>)) => void;
type GetFn = () => WorkflowOrchestratorState;

/**
 * Generate PRD phase (called after config confirmation or interview completion).
 */
async function generatePrdPhase(set: SetFn, _get: GetFn) {
  set({ phase: 'generating_prd' });
  injectInfo('Generating PRD from task description...', 'info');

  try {
    await useTaskModeStore.getState().generatePrd();

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
        batchIndex: b.batchIndex,
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
 */
async function subscribeToProgressEvents(set: SetFn, get: GetFn) {
  // Unsubscribe from existing
  const existing = get()._unlistenFn;
  if (existing) existing();

  try {
    const unlisten = await listen<{
      sessionId: string;
      currentBatch: number;
      totalBatches: number;
      storyStatuses: Record<string, string>;
      storiesCompleted: number;
      storiesFailed: number;
      qualityGateResults?: Record<string, StoryQualityGateResults>;
    }>('task-mode-progress', (event) => {
      const payload = event.payload;
      const state = get();
      if (state.sessionId && payload.sessionId !== state.sessionId) return;

      // Calculate progress percentage
      const totalStories = Object.keys(payload.storyStatuses).length;
      const completedCount = payload.storiesCompleted + payload.storiesFailed;
      const progressPct = totalStories > 0 ? (completedCount / totalStories) * 100 : 0;

      // Inject execution update card for meaningful events
      const updateData: ExecutionUpdateCardData = {
        eventType: 'batch_complete',
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
        storyId: null,
        storyTitle: null,
        status: `${payload.storiesCompleted} completed, ${payload.storiesFailed} failed`,
        agent: null,
        progressPct,
      };
      injectCard('execution_update', updateData);

      // Inject quality gate results if present
      if (payload.qualityGateResults) {
        for (const [storyId, result] of Object.entries(payload.qualityGateResults)) {
          const gateData: GateResultCardData = {
            storyId,
            storyTitle: storyId,
            overallStatus: result.overallStatus,
            gates: result.gates,
            codeReviewScores: result.codeReviewScores || [],
          };
          injectCard('gate_result', gateData);
        }
      }

      // Check for completion
      if (totalStories > 0 && completedCount >= totalStories) {
        const success = payload.storiesFailed === 0;
        const reportData: CompletionReportCardData = {
          success,
          totalStories,
          completed: payload.storiesCompleted,
          failed: payload.storiesFailed,
          duration: 0, // Will be fetched from report
          agentAssignments: {},
        };
        injectCard('completion_report', reportData);

        set({ phase: success ? 'completed' : 'failed' });

        // Fetch full report for duration/agent data
        useTaskModeStore.getState().fetchReport().then(() => {
          const report = useTaskModeStore.getState().report;
          if (report) {
            const fullReport: CompletionReportCardData = {
              success: report.success,
              totalStories: report.totalStories,
              completed: report.storiesCompleted,
              failed: report.storiesFailed,
              duration: report.totalDurationMs,
              agentAssignments: report.agentAssignments,
            };
            injectCard('completion_report', fullReport);
          }
        });
      }
    });

    set({ _unlistenFn: unlisten });
  } catch {
    // Non-fatal: event subscription failure
  }
}

export default useWorkflowOrchestratorStore;
