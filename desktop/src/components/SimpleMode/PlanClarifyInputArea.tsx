import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

/** Plan clarification input area — shown during plan clarifying phase */
export function PlanClarifyInputArea({
  question,
  onSubmit,
  onSkip,
  onSkipAll,
  loading,
}: {
  question: { questionId: string; question: string; hint: string | null; inputType: string };
  onSubmit: (text: string) => void;
  onSkip: () => void;
  onSkipAll: () => void;
  loading: boolean;
}) {
  const { t } = useTranslation('planMode');
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus on question change
  useEffect(() => {
    inputRef.current?.focus();
    setValue('');
  }, [question.questionId]);

  const handleSubmit = useCallback(() => {
    if (!value.trim() || loading) return;
    onSubmit(value.trim());
    setValue('');
  }, [value, loading, onSubmit]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit],
  );

  return (
    <div className="px-4 py-3 space-y-2">
      {/* Question display */}
      <div className="flex items-start gap-2">
        <span className="text-amber-500 dark:text-amber-400 mt-0.5">&#10067;</span>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-amber-700 dark:text-amber-300">{question.question}</p>
          {question.hint && <p className="text-xs text-amber-500/80 dark:text-amber-400/70 mt-0.5">{question.hint}</p>}
        </div>
      </div>

      {/* Input + buttons */}
      <div className="flex items-end gap-2">
        <textarea
          ref={inputRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          rows={question.inputType === 'textarea' ? 3 : 1}
          className="flex-1 min-w-0 resize-none rounded-lg border border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-amber-400 dark:focus:ring-amber-600 disabled:opacity-50"
          placeholder={t('clarify.inputPlaceholder', { defaultValue: 'Type your answer...' }) as string}
        />
        <button
          onClick={handleSubmit}
          disabled={!value.trim() || loading}
          className="shrink-0 px-3 py-2 rounded-lg bg-amber-500 hover:bg-amber-600 disabled:bg-gray-300 dark:disabled:bg-gray-700 text-white text-sm font-medium transition-colors disabled:cursor-not-allowed"
        >
          {t('clarify.submit', { defaultValue: 'Submit' })}
        </button>
        <button
          onClick={onSkip}
          disabled={loading}
          className="shrink-0 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm transition-colors disabled:opacity-50"
        >
          {t('clarify.skipQuestion', { defaultValue: 'Skip' })}
        </button>
        <button
          onClick={onSkipAll}
          disabled={loading}
          className="shrink-0 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm transition-colors disabled:opacity-50"
        >
          {t('clarify.skipAll', { defaultValue: 'Skip All' })}
        </button>
      </div>
    </div>
  );
}
