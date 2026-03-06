import { useMemo } from 'react';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { PlanClarificationSnapshot, TaskInterviewSnapshot } from '../../types/workflowKernel';

export interface WorkflowQuestionSpecs {
  taskPendingQuestion: InterviewQuestionCardData | null;
  planPendingQuestion: PlanClarifyQuestionCardData | null;
}

function mapInterviewQuestion(snapshot: TaskInterviewSnapshot): InterviewQuestionCardData {
  const inputType: InterviewQuestionCardData['inputType'] = (() => {
    switch (snapshot.inputType) {
      case 'boolean':
        return 'boolean';
      case 'single_select':
        return 'single_select';
      case 'multi_select':
        return 'multi_select';
      case 'textarea':
        return 'textarea';
      case 'text':
      case 'list':
      default:
        return 'text';
    }
  })();

  return {
    questionId: snapshot.questionId,
    question: snapshot.question,
    hint: snapshot.hint,
    required: snapshot.required,
    inputType,
    options: snapshot.options ?? [],
    allowCustom: snapshot.allowCustom ?? true,
    questionNumber: snapshot.questionNumber ?? 1,
    totalQuestions: snapshot.totalQuestions ?? 1,
  };
}

function mapPlanQuestion(snapshot: PlanClarificationSnapshot): PlanClarifyQuestionCardData {
  const inputType: PlanClarifyQuestionCardData['inputType'] = (() => {
    switch (snapshot.inputType) {
      case 'boolean':
        return 'boolean';
      case 'single_select':
        return 'single_select';
      case 'multi_select':
        return 'multi_select';
      case 'textarea':
        return 'textarea';
      case 'text':
      default:
        return 'text';
    }
  })();

  return {
    questionId: snapshot.questionId,
    question: snapshot.question,
    hint: snapshot.hint,
    inputType,
    options: snapshot.options ?? [],
    allowCustom: snapshot.allowCustom ?? true,
  };
}

export function useWorkflowQuestionSpecs(
  pendingInterview: TaskInterviewSnapshot | null,
  pendingClarification: PlanClarificationSnapshot | null,
): WorkflowQuestionSpecs {
  const taskPendingQuestion = useMemo<InterviewQuestionCardData | null>(() => {
    if (!pendingInterview) return null;
    return mapInterviewQuestion(pendingInterview);
  }, [pendingInterview]);

  const planPendingQuestion = useMemo<PlanClarifyQuestionCardData | null>(() => {
    if (!pendingClarification) return null;
    return mapPlanQuestion(pendingClarification);
  }, [pendingClarification]);

  return {
    taskPendingQuestion,
    planPendingQuestion,
  };
}
