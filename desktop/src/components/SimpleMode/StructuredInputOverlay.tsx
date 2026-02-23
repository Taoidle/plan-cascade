/**
 * StructuredInputOverlay
 *
 * Renders structured input UI when a pending interview question requires
 * boolean, single_select, or multi_select input. Positioned above/replacing
 * the InputBox in the chat panel footer.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import type { InterviewQuestionCardData } from '../../types/workflowCard';

interface StructuredInputOverlayProps {
  question: InterviewQuestionCardData;
  onSubmit: (answer: string) => void;
  onSkip: () => void;
  loading: boolean;
}

export function StructuredInputOverlay({
  question,
  onSubmit,
  onSkip,
  loading,
}: StructuredInputOverlayProps) {
  switch (question.inputType) {
    case 'boolean':
      return (
        <BooleanInput
          question={question}
          onSubmit={onSubmit}
          onSkip={onSkip}
          loading={loading}
        />
      );
    case 'single_select':
      return (
        <SingleSelectInput
          question={question}
          onSubmit={onSubmit}
          onSkip={onSkip}
          loading={loading}
        />
      );
    case 'multi_select':
      return (
        <MultiSelectInput
          question={question}
          onSubmit={onSubmit}
          onSkip={onSkip}
          loading={loading}
        />
      );
    default:
      // text / textarea — falls through to normal InputBox (handled by parent)
      return null;
  }
}

function BooleanInput({
  question,
  onSubmit,
  onSkip,
  loading,
}: StructuredInputOverlayProps) {
  return (
    <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-700">
      <p className="text-xs text-gray-500 dark:text-gray-400 mb-2 truncate">
        Q{question.questionNumber}: {question.question}
      </p>
      <div className="flex items-center gap-2">
        <button
          onClick={() => onSubmit('yes')}
          disabled={loading}
          className={clsx(
            'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
            'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
            'hover:bg-green-200 dark:hover:bg-green-900/50',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          Yes
        </button>
        <button
          onClick={() => onSubmit('no')}
          disabled={loading}
          className={clsx(
            'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
            'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
            'hover:bg-red-200 dark:hover:bg-red-900/50',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          No
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-2 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
          >
            Skip
          </button>
        )}
      </div>
    </div>
  );
}

function SingleSelectInput({
  question,
  onSubmit,
  onSkip,
  loading,
}: StructuredInputOverlayProps) {
  const [selected, setSelected] = useState<string | null>(null);

  const handleSubmit = useCallback(() => {
    if (selected) onSubmit(selected);
  }, [selected, onSubmit]);

  return (
    <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-700">
      <p className="text-xs text-gray-500 dark:text-gray-400 mb-2 truncate">
        Q{question.questionNumber}: {question.question}
      </p>
      <div className="space-y-1 max-h-32 overflow-y-auto mb-2">
        {question.options.map((opt) => (
          <button
            key={opt}
            onClick={() => setSelected(opt)}
            className={clsx(
              'w-full text-left px-3 py-1.5 rounded-lg text-xs transition-colors',
              selected === opt
                ? 'bg-violet-100 dark:bg-violet-900/40 text-violet-700 dark:text-violet-300 ring-1 ring-violet-400 dark:ring-violet-600'
                : 'bg-gray-50 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
            )}
          >
            {opt}
          </button>
        ))}
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleSubmit}
          disabled={loading || !selected}
          className="px-4 py-1.5 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          Submit
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
          >
            Skip
          </button>
        )}
      </div>
    </div>
  );
}

function MultiSelectInput({
  question,
  onSubmit,
  onSkip,
  loading,
}: StructuredInputOverlayProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const toggleOption = useCallback((opt: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(opt)) {
        next.delete(opt);
      } else {
        next.add(opt);
      }
      return next;
    });
  }, []);

  const handleSubmit = useCallback(() => {
    if (selected.size > 0) {
      onSubmit(Array.from(selected).join(', '));
    }
  }, [selected, onSubmit]);

  return (
    <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-700">
      <p className="text-xs text-gray-500 dark:text-gray-400 mb-2 truncate">
        Q{question.questionNumber}: {question.question}
      </p>
      <div className="space-y-1 max-h-32 overflow-y-auto mb-2">
        {question.options.map((opt) => (
          <button
            key={opt}
            onClick={() => toggleOption(opt)}
            className={clsx(
              'w-full text-left px-3 py-1.5 rounded-lg text-xs transition-colors flex items-center gap-2',
              selected.has(opt)
                ? 'bg-violet-100 dark:bg-violet-900/40 text-violet-700 dark:text-violet-300'
                : 'bg-gray-50 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
            )}
          >
            <span
              className={clsx(
                'w-3.5 h-3.5 rounded border flex items-center justify-center shrink-0',
                selected.has(opt)
                  ? 'border-violet-500 bg-violet-500'
                  : 'border-gray-300 dark:border-gray-600'
              )}
            >
              {selected.has(opt) && (
                <svg className="w-2.5 h-2.5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                </svg>
              )}
            </span>
            {opt}
          </button>
        ))}
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleSubmit}
          disabled={loading || selected.size === 0}
          className="px-4 py-1.5 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          Submit ({selected.size})
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
          >
            Skip
          </button>
        )}
      </div>
    </div>
  );
}
