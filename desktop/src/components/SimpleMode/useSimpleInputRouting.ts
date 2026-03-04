import { useCallback } from 'react';
import { buildConversationHistory } from '../../lib/contextBridge';
import { usePlanOrchestratorStore } from '../../store/planOrchestrator';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import type { HandoffContextBundle, UserInputIntent, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';

interface UseSimpleInputRoutingParams {
  description: string;
  setDescription: (next: string) => void;
  workflowMode: WorkflowMode;
  workflowPhase: string;
  planPhase: string;
  isSubmitting: boolean;
  isAnalyzingStrategy: boolean;
  start: (prompt: string, source: 'simple') => Promise<void>;
  sendFollowUp: (prompt: string) => Promise<void>;
  startWorkflow: (description: string) => Promise<void>;
  startPlanWorkflow: (description: string) => Promise<void>;
  overrideConfigNatural: (text: string) => void;
  addPrdFeedback: (feedback: string) => void;
  submitPlanClarification: (answer: { questionId: string; answer: string; skipped: boolean }) => Promise<void>;
  submitInterviewAnswer: (answer: string) => Promise<void>;
  skipInterviewQuestion: () => Promise<void>;
  skipPlanClarification: () => Promise<void>;
  taskInterviewingPhase: boolean;
  taskPendingQuestion: InterviewQuestionCardData | null;
  planClarifyingPhase: boolean;
  planPendingQuestion: PlanClarifyQuestionCardData | null;
  hasStructuredInterviewQuestion: boolean;
  linkWorkflowKernelModeSession: (mode: WorkflowMode, modeSessionId: string) => Promise<WorkflowSession | null>;
  transitionAndSubmitWorkflowKernelInput: (
    targetMode: WorkflowMode,
    intent: UserInputIntent,
    handoff?: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
}

interface UseSimpleInputRoutingResult {
  handleStart: (inputPrompt?: string) => Promise<void>;
  handleFollowUp: (inputPrompt?: string) => Promise<void>;
  handleStructuredInterviewSubmit: (answer: string) => Promise<void>;
  handleSkipInterviewQuestion: () => Promise<void>;
  handleSkipPlanClarifyQuestion: () => Promise<void>;
  handleSkipPlanClarification: () => Promise<void>;
}

export function useSimpleInputRouting({
  description,
  setDescription,
  workflowMode,
  workflowPhase,
  planPhase,
  isSubmitting,
  isAnalyzingStrategy,
  start,
  sendFollowUp,
  startWorkflow,
  startPlanWorkflow,
  overrideConfigNatural,
  addPrdFeedback,
  submitPlanClarification,
  submitInterviewAnswer,
  skipInterviewQuestion,
  skipPlanClarification,
  taskInterviewingPhase,
  taskPendingQuestion,
  planClarifyingPhase,
  planPendingQuestion,
  hasStructuredInterviewQuestion,
  linkWorkflowKernelModeSession,
  transitionAndSubmitWorkflowKernelInput,
}: UseSimpleInputRoutingParams): UseSimpleInputRoutingResult {
  const handleStart = useCallback(
    async (inputPrompt?: string) => {
      const prompt = (inputPrompt ?? description).trim();
      if (!prompt || isSubmitting || isAnalyzingStrategy) return;
      if (inputPrompt === undefined) {
        setDescription('');
      }

      const conversationContext = buildConversationHistory().map((turn) => ({
        user: turn.user,
        assistant: turn.assistant,
      }));
      await transitionAndSubmitWorkflowKernelInput(
        workflowMode,
        {
          type: 'mode_entry_prompt',
          content: prompt,
          metadata: {
            mode: workflowMode,
            source: inputPrompt === undefined ? 'composer' : 'queue_or_external',
          },
        },
        {
          conversationContext,
          artifactRefs: [],
          contextSources: ['simple_mode'],
          metadata: {
            source: 'start',
            mode: workflowMode,
          },
        },
      );

      if (workflowMode === 'task') {
        await startWorkflow(prompt);
        const taskModeSessionId = useWorkflowOrchestratorStore.getState().sessionId;
        if (taskModeSessionId) {
          await linkWorkflowKernelModeSession('task', taskModeSessionId);
        }
        return;
      }

      if (workflowMode === 'plan') {
        await startPlanWorkflow(prompt);
        const planModeSessionId = usePlanOrchestratorStore.getState().sessionId;
        if (planModeSessionId) {
          await linkWorkflowKernelModeSession('plan', planModeSessionId);
        }
        return;
      }

      await start(prompt, 'simple');
    },
    [
      description,
      isAnalyzingStrategy,
      isSubmitting,
      linkWorkflowKernelModeSession,
      setDescription,
      start,
      startPlanWorkflow,
      startWorkflow,
      transitionAndSubmitWorkflowKernelInput,
      workflowMode,
    ],
  );

  const handleFollowUp = useCallback(
    async (inputPrompt?: string) => {
      const prompt = (inputPrompt ?? description).trim();
      if (!prompt || isSubmitting) return;
      if (inputPrompt === undefined) {
        setDescription('');
      }

      if (workflowMode === 'task') {
        if (workflowPhase === 'configuring') {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_configuration',
            content: prompt,
            metadata: { mode: workflowMode, phase: workflowPhase },
          });
          overrideConfigNatural(prompt);
          return;
        }
        if (workflowPhase === 'reviewing_prd') {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_prd_feedback',
            content: prompt,
            metadata: { mode: workflowMode, phase: workflowPhase },
          });
          addPrdFeedback(prompt);
          return;
        }
        if (taskInterviewingPhase && taskPendingQuestion && !hasStructuredInterviewQuestion) {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_interview_answer',
            content: prompt,
            metadata: {
              mode: workflowMode,
              phase: workflowPhase,
              questionId: taskPendingQuestion.questionId,
            },
          });
          await submitInterviewAnswer(prompt);
          return;
        }
      }

      if (planClarifyingPhase && planPendingQuestion) {
        await transitionAndSubmitWorkflowKernelInput(workflowMode, {
          type: 'plan_clarification',
          content: prompt,
          metadata: {
            mode: workflowMode,
            phase: planPhase,
            questionId: planPendingQuestion.questionId,
          },
        });
        await submitPlanClarification({
          questionId: planPendingQuestion.questionId,
          answer: prompt,
          skipped: false,
        });
        return;
      }

      await transitionAndSubmitWorkflowKernelInput(workflowMode, {
        type: 'chat_message',
        content: prompt,
        metadata: {
          mode: workflowMode,
        },
      });
      await sendFollowUp(prompt);
    },
    [
      addPrdFeedback,
      description,
      hasStructuredInterviewQuestion,
      isSubmitting,
      overrideConfigNatural,
      planClarifyingPhase,
      planPendingQuestion,
      planPhase,
      sendFollowUp,
      setDescription,
      submitInterviewAnswer,
      submitPlanClarification,
      taskInterviewingPhase,
      taskPendingQuestion,
      transitionAndSubmitWorkflowKernelInput,
      workflowMode,
      workflowPhase,
    ],
  );

  const handleStructuredInterviewSubmit = useCallback(
    async (answer: string) => {
      const normalized = answer.trim();
      if (!normalized) return;
      const questionId = taskPendingQuestion?.questionId;
      await transitionAndSubmitWorkflowKernelInput('task', {
        type: 'task_interview_answer',
        content: normalized,
        metadata: {
          mode: 'task',
          phase: workflowPhase,
          source: 'structured_interview_panel',
          questionId: questionId ?? null,
        },
      });
      await submitInterviewAnswer(normalized);
    },
    [taskPendingQuestion?.questionId, submitInterviewAnswer, transitionAndSubmitWorkflowKernelInput, workflowPhase],
  );

  const handleSkipInterviewQuestion = useCallback(async () => {
    const questionId = taskPendingQuestion?.questionId;
    await transitionAndSubmitWorkflowKernelInput('task', {
      type: 'task_interview_answer',
      content: '[skip]',
      metadata: {
        mode: 'task',
        phase: workflowPhase,
        source: 'interview_skip',
        questionId: questionId ?? null,
        skipped: true,
      },
    });
    await skipInterviewQuestion();
  }, [taskPendingQuestion?.questionId, skipInterviewQuestion, transitionAndSubmitWorkflowKernelInput, workflowPhase]);

  const handleSkipPlanClarifyQuestion = useCallback(async () => {
    const questionId = planPendingQuestion?.questionId;
    await transitionAndSubmitWorkflowKernelInput('plan', {
      type: 'plan_clarification',
      content: '[skip]',
      metadata: {
        mode: 'plan',
        phase: planPhase,
        source: 'plan_clarify_skip_question',
        questionId: questionId ?? null,
        skipped: true,
      },
    });
    if (!planPendingQuestion) return;
    await submitPlanClarification({
      questionId: planPendingQuestion.questionId,
      answer: '',
      skipped: true,
    });
  }, [planPendingQuestion, planPhase, submitPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  const handleSkipPlanClarification = useCallback(async () => {
    await transitionAndSubmitWorkflowKernelInput('plan', {
      type: 'plan_clarification',
      content: '[skip_all]',
      metadata: {
        mode: 'plan',
        phase: planPhase,
        source: 'plan_clarify_skip_all',
        questionId: planPendingQuestion?.questionId ?? null,
        skippedAll: true,
      },
    });
    await skipPlanClarification();
  }, [planPendingQuestion?.questionId, planPhase, skipPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  return {
    handleStart,
    handleFollowUp,
    handleStructuredInterviewSubmit,
    handleSkipInterviewQuestion,
    handleSkipPlanClarifyQuestion,
    handleSkipPlanClarification,
  };
}
