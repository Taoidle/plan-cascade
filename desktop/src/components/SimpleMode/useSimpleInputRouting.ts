import { useCallback } from 'react';
import { reportInteractiveActionFailure } from '../../lib/workflowObservability';
import { startModeWithCompensation, submitWorkflowInputWithTracking } from '../../store/simpleWorkflowCoordinator';
import { useContextSourcesStore } from '../../store/contextSources';
import { useSettingsStore } from '../../store/settings';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import type { HandoffContextBundle, UserInputIntent, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';
import type { ActionResult } from '../../types/actionResult';

interface SkillSlashCommandParseResult {
  skillId: string;
  remainingPrompt: string;
}

function parseSkillSlashCommand(prompt: string): SkillSlashCommandParseResult | null {
  const match = prompt.match(/^\/skill:([^\s]+)(?:\s+([\s\S]*))?$/);
  if (!match) return null;
  return {
    skillId: match[1].trim(),
    remainingPrompt: (match[2] ?? '').trim(),
  };
}

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
  startWorkflow: (description: string, kernelSessionId?: string | null) => Promise<{ modeSessionId: string | null }>;
  startPlanWorkflow: (
    description: string,
    kernelSessionId?: string | null,
  ) => Promise<{ modeSessionId: string | null }>;
  overrideConfigNatural: (text: string) => void;
  addPrdFeedback: (feedback: string) => Promise<ActionResult>;
  submitPlanClarification: (answer: {
    questionId: string;
    answer: string;
    skipped: boolean;
  }) => Promise<{ ok: boolean; errorCode?: string | null }>;
  submitInterviewAnswer: (answer: string) => Promise<void>;
  skipInterviewQuestion: () => Promise<void>;
  skipPlanClarification: () => Promise<void>;
  taskInterviewingPhase: boolean;
  taskPendingQuestion: InterviewQuestionCardData | null;
  planClarifyingPhase: boolean;
  planPendingQuestion: PlanClarifyQuestionCardData | null;
  hasStructuredInterviewQuestion: boolean;
  hasStructuredPlanClarifyQuestion: boolean;
  linkWorkflowKernelModeSession: (mode: WorkflowMode, modeSessionId: string) => Promise<WorkflowSession | null>;
  cancelWorkflowKernelOperation: (reason?: string) => Promise<WorkflowSession | null>;
  appendWorkflowKernelContextItems?: (
    targetMode: WorkflowMode,
    handoff: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
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
  handleStructuredPlanClarifySubmit: (answer: string) => Promise<void>;
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
  hasStructuredPlanClarifyQuestion,
  linkWorkflowKernelModeSession,
  cancelWorkflowKernelOperation,
  transitionAndSubmitWorkflowKernelInput,
}: UseSimpleInputRoutingParams): UseSimpleInputRoutingResult {
  const resolvePromptAfterSkillInvocation = useCallback(async (rawPrompt: string): Promise<string | null> => {
    const parsed = parseSkillSlashCommand(rawPrompt.trim());
    if (!parsed) {
      return rawPrompt.trim();
    }

    const projectPath = useSettingsStore.getState().workspacePath?.trim() || '';
    if (!projectPath) {
      return parsed.remainingPrompt || null;
    }

    const sessionId = useWorkflowKernelStore.getState().sessionId;
    const invoked = await useContextSourcesStore.getState().invokeSkillCommand(projectPath, parsed.skillId, sessionId);
    if (!invoked) {
      return null;
    }
    return parsed.remainingPrompt || null;
  }, []);

  const handleStart = useCallback(
    async (inputPrompt?: string) => {
      const rawPrompt = inputPrompt ?? description;
      const prompt = await resolvePromptAfterSkillInvocation(rawPrompt);
      if (!prompt || isSubmitting || isAnalyzingStrategy) {
        if (inputPrompt === undefined && parseSkillSlashCommand(rawPrompt.trim())) {
          setDescription('');
        }
        return;
      }
      if (inputPrompt === undefined) {
        setDescription('');
      }

      const handoff: HandoffContextBundle = {
        conversationContext: [],
        summaryItems: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {
          source: 'start',
          mode: workflowMode,
        },
      };

      await startModeWithCompensation({
        mode: workflowMode,
        prompt,
        source: inputPrompt === undefined ? 'composer' : 'queue_or_external',
        handoff,
        transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
        linkModeSession: linkWorkflowKernelModeSession,
        cancelKernelOperation: cancelWorkflowKernelOperation,
        startChat: start,
        startTaskWorkflow: startWorkflow,
        startPlanWorkflow: startPlanWorkflow,
      });
    },
    [
      cancelWorkflowKernelOperation,
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
      resolvePromptAfterSkillInvocation,
    ],
  );

  const handleFollowUp = useCallback(
    async (inputPrompt?: string) => {
      const rawPrompt = inputPrompt ?? description;
      const prompt = await resolvePromptAfterSkillInvocation(rawPrompt);
      if (!prompt || isSubmitting) {
        if (inputPrompt === undefined && parseSkillSlashCommand(rawPrompt.trim())) {
          setDescription('');
        }
        return;
      }
      if (inputPrompt === undefined) {
        setDescription('');
      }

      if (workflowMode === 'task') {
        if (workflowPhase === 'configuring') {
          const submitted = await submitWorkflowInputWithTracking({
            transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
            targetMode: workflowMode,
            intent: {
              type: 'task_configuration',
              content: prompt,
              metadata: { mode: workflowMode, phase: workflowPhase },
            },
          });
          if (!submitted) return;
          overrideConfigNatural(prompt);
          return;
        }
        if (workflowPhase === 'reviewing_prd') {
          const submitted = await submitWorkflowInputWithTracking({
            transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
            targetMode: workflowMode,
            intent: {
              type: 'task_prd_feedback',
              content: prompt,
              metadata: { mode: workflowMode, phase: workflowPhase },
            },
          });
          if (!submitted) return;
          const result = await addPrdFeedback(prompt);
          if (!result.ok) {
            await reportInteractiveActionFailure({
              card: 'input_router',
              action: 'add_prd_feedback',
              errorCode: result.errorCode || 'prd_feedback_apply_failed',
              message: result.message || 'Failed to apply PRD feedback',
              session: useWorkflowKernelStore.getState().session,
            });
          }
          return;
        }
        if (taskInterviewingPhase && taskPendingQuestion && !hasStructuredInterviewQuestion) {
          const submitted = await submitWorkflowInputWithTracking({
            transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
            targetMode: workflowMode,
            intent: {
              type: 'task_interview_answer',
              content: prompt,
              metadata: {
                mode: workflowMode,
                phase: workflowPhase,
                questionId: taskPendingQuestion.questionId,
              },
            },
          });
          if (!submitted) return;
          await submitInterviewAnswer(prompt);
          return;
        }
      }

      if (planClarifyingPhase && planPendingQuestion && !hasStructuredPlanClarifyQuestion) {
        const submitted = await submitWorkflowInputWithTracking({
          transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
          targetMode: workflowMode,
          intent: {
            type: 'plan_clarification',
            content: prompt,
            metadata: {
              mode: workflowMode,
              phase: planPhase,
              questionId: planPendingQuestion.questionId,
            },
          },
        });
        if (!submitted) return;
        await submitPlanClarification({
          questionId: planPendingQuestion.questionId,
          answer: prompt,
          skipped: false,
        });
        return;
      }

      const submitted = await submitWorkflowInputWithTracking({
        transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
        targetMode: workflowMode,
        intent: {
          type: 'chat_message',
          content: prompt,
          metadata: {
            mode: workflowMode,
          },
        },
      });
      if (!submitted) return;
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
      hasStructuredPlanClarifyQuestion,
      sendFollowUp,
      setDescription,
      submitInterviewAnswer,
      submitPlanClarification,
      taskInterviewingPhase,
      taskPendingQuestion,
      transitionAndSubmitWorkflowKernelInput,
      workflowMode,
      workflowPhase,
      resolvePromptAfterSkillInvocation,
    ],
  );

  const handleStructuredPlanClarifySubmit = useCallback(
    async (answer: string) => {
      const normalized = answer.trim();
      if (!normalized || !planPendingQuestion) return;
      const submitted = await submitWorkflowInputWithTracking({
        transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
        targetMode: 'plan',
        intent: {
          type: 'plan_clarification',
          content: normalized,
          metadata: {
            mode: 'plan',
            phase: planPhase,
            source: 'structured_plan_clarify_panel',
            questionId: planPendingQuestion.questionId,
          },
        },
      });
      if (!submitted) return;
      await submitPlanClarification({
        questionId: planPendingQuestion.questionId,
        answer: normalized,
        skipped: false,
      });
    },
    [planPendingQuestion, planPhase, submitPlanClarification, transitionAndSubmitWorkflowKernelInput],
  );

  const handleStructuredInterviewSubmit = useCallback(
    async (answer: string) => {
      const normalized = answer.trim();
      if (!normalized) return;
      const questionId = taskPendingQuestion?.questionId;
      const submitted = await submitWorkflowInputWithTracking({
        transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
        targetMode: 'task',
        intent: {
          type: 'task_interview_answer',
          content: normalized,
          metadata: {
            mode: 'task',
            phase: workflowPhase,
            source: 'structured_interview_panel',
            questionId: questionId ?? null,
          },
        },
      });
      if (!submitted) return;
      await submitInterviewAnswer(normalized);
    },
    [taskPendingQuestion?.questionId, submitInterviewAnswer, transitionAndSubmitWorkflowKernelInput, workflowPhase],
  );

  const handleSkipInterviewQuestion = useCallback(async () => {
    const questionId = taskPendingQuestion?.questionId;
    const submitted = await submitWorkflowInputWithTracking({
      transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
      targetMode: 'task',
      intent: {
        type: 'task_interview_answer',
        content: '[skip]',
        metadata: {
          mode: 'task',
          phase: workflowPhase,
          source: 'interview_skip',
          questionId: questionId ?? null,
          skipped: true,
        },
      },
    });
    if (!submitted) return;
    await skipInterviewQuestion();
  }, [taskPendingQuestion?.questionId, skipInterviewQuestion, transitionAndSubmitWorkflowKernelInput, workflowPhase]);

  const handleSkipPlanClarifyQuestion = useCallback(async () => {
    const questionId = planPendingQuestion?.questionId;
    const submitted = await submitWorkflowInputWithTracking({
      transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
      targetMode: 'plan',
      intent: {
        type: 'plan_clarification',
        content: '[skip]',
        metadata: {
          mode: 'plan',
          phase: planPhase,
          source: 'plan_clarify_skip_question',
          questionId: questionId ?? null,
          skipped: true,
        },
      },
    });
    if (!submitted) return;
    if (!planPendingQuestion) return;
    await submitPlanClarification({
      questionId: planPendingQuestion.questionId,
      answer: '',
      skipped: true,
    });
  }, [planPendingQuestion, planPhase, submitPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  const handleSkipPlanClarification = useCallback(async () => {
    const submitted = await submitWorkflowInputWithTracking({
      transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
      targetMode: 'plan',
      intent: {
        type: 'plan_clarification',
        content: '[skip_all]',
        metadata: {
          mode: 'plan',
          phase: planPhase,
          source: 'plan_clarify_skip_all',
          questionId: planPendingQuestion?.questionId ?? null,
          skippedAll: true,
        },
      },
    });
    if (!submitted) return;
    await skipPlanClarification();
  }, [planPendingQuestion?.questionId, planPhase, skipPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  return {
    handleStart,
    handleFollowUp,
    handleStructuredInterviewSubmit,
    handleStructuredPlanClarifySubmit,
    handleSkipInterviewQuestion,
    handleSkipPlanClarifyQuestion,
    handleSkipPlanClarification,
  };
}
