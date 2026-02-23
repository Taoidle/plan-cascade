/**
 * InterviewQuestionCard
 *
 * Displays an interview question in the chat. Read-only after answered.
 * Structured input (boolean/select) is handled by StructuredInputOverlay.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { InterviewQuestionCardData } from '../../../types/workflowCard';

export function InterviewQuestionCard({ data }: { data: InterviewQuestionCardData }) {
  const { t } = useTranslation('simpleMode');

  const formatInputType = (type: InterviewQuestionCardData['inputType']): string => {
    switch (type) {
      case 'text': return t('workflow.interview.inputText');
      case 'textarea': return t('workflow.interview.inputTextarea');
      case 'single_select': return t('workflow.interview.inputSingleSelect');
      case 'multi_select': return t('workflow.interview.inputMultiSelect');
      case 'boolean': return t('workflow.interview.inputBoolean');
      default: return type;
    }
  };

  return (
    <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50 dark:bg-violet-900/20 px-3 py-2">
      <div className="flex items-center justify-between">
        <span className="text-2xs font-medium text-violet-600 dark:text-violet-400 uppercase tracking-wide">
          {t('workflow.interview.questionTitle')}
        </span>
        <span className="text-2xs text-violet-500 dark:text-violet-400">
          {t('workflow.interview.questionNumber', { current: data.questionNumber, total: data.totalQuestions })}
        </span>
      </div>

      <p className="text-sm text-violet-800 dark:text-violet-200 mt-1 font-medium">
        {data.question}
      </p>

      {data.hint && (
        <p className="text-xs text-violet-600/70 dark:text-violet-400/70 mt-1 italic">
          {data.hint}
        </p>
      )}

      <div className="mt-1 flex items-center gap-2">
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded',
            data.required
              ? 'bg-violet-200 dark:bg-violet-800 text-violet-600 dark:text-violet-400'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
          )}
        >
          {data.required ? t('workflow.interview.required') : t('workflow.interview.optional')}
        </span>
        <span className="text-2xs text-violet-500/60 dark:text-violet-400/60">
          {formatInputType(data.inputType)}
        </span>
      </div>
    </div>
  );
}
