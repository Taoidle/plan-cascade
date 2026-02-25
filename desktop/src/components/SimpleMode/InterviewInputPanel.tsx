/**
 * InterviewInputPanel
 *
 * Unified interview input panel that replaces the bottom InputBox during interviews.
 * Combines question display, input controls, and action buttons in one panel.
 * Supports text, textarea, boolean, single_select, multi_select with "Other" custom input.
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { InterviewQuestionCardData } from '../../types/workflowCard';

interface InterviewInputPanelProps {
  question: InterviewQuestionCardData;
  onSubmit: (answer: string) => void;
  onSkip: () => void;
  loading: boolean;
}

export function InterviewInputPanel({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');

  return (
    <div className="border-t border-violet-200 dark:border-violet-800 bg-violet-50/30 dark:bg-violet-900/10">
      {/* Header: phase + progress */}
      <div className="px-4 pt-3 pb-1 flex items-center justify-between">
        <span className="text-2xs font-medium text-violet-600 dark:text-violet-400 uppercase tracking-wide">
          {t('workflow.interview.questionTitle')}
        </span>
        <span className="text-2xs text-violet-500 dark:text-violet-400">
          {t('workflow.interview.questionNumber', {
            current: question.questionNumber,
            total: question.totalQuestions,
          })}
        </span>
      </div>

      {/* Question text + hint */}
      <div className="px-4 pb-2">
        <p className="text-sm font-medium text-violet-800 dark:text-violet-200">{question.question}</p>
        {question.hint && (
          <p className="text-xs text-violet-600/70 dark:text-violet-400/70 italic mt-0.5">{question.hint}</p>
        )}
      </div>

      {/* Input area - dispatch by inputType */}
      <div className="px-4 pb-3">
        <InterviewInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />
      </div>
    </div>
  );
}

/** Internal component that dispatches to the correct input type */
function InterviewInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  switch (question.inputType) {
    case 'boolean':
      return <BooleanInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />;
    case 'single_select':
      return <SingleSelectInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />;
    case 'multi_select':
      return <MultiSelectInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />;
    case 'textarea':
      return <TextareaInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />;
    default:
      return <TextInput question={question} onSubmit={onSubmit} onSkip={onSkip} loading={loading} />;
  }
}

/** Text input with Enter to submit */
function TextInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, [question.questionId]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey && value.trim()) {
        e.preventDefault();
        onSubmit(value.trim());
        setValue('');
      }
    },
    [value, onSubmit],
  );

  return (
    <div className="flex items-center gap-2">
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={loading}
        placeholder={question.hint || ''}
        className={clsx(
          'flex-1 px-3 py-2 rounded-lg text-sm border transition-colors',
          'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100',
          'border-violet-300 dark:border-violet-700',
          'focus:outline-none focus:ring-2 focus:ring-violet-500/40',
          'placeholder:text-gray-400 dark:placeholder:text-gray-500',
          'disabled:opacity-50',
        )}
      />
      <button
        onClick={() => {
          if (value.trim()) {
            onSubmit(value.trim());
            setValue('');
          }
        }}
        disabled={loading || !value.trim()}
        className="px-4 py-2 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {t('workflow.interview.submit')}
      </button>
      {!question.required && (
        <button
          onClick={onSkip}
          disabled={loading}
          className="px-3 py-2 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
        >
          {t('workflow.interview.skipBtn')}
        </button>
      )}
    </div>
  );
}

/** Textarea input with Ctrl+Enter to submit */
function TextareaInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, [question.questionId]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && value.trim()) {
        e.preventDefault();
        onSubmit(value.trim());
        setValue('');
      }
    },
    [value, onSubmit],
  );

  return (
    <div className="space-y-2">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={loading}
        placeholder={question.hint || ''}
        rows={3}
        className={clsx(
          'w-full px-3 py-2 rounded-lg text-sm border transition-colors resize-none',
          'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100',
          'border-violet-300 dark:border-violet-700',
          'focus:outline-none focus:ring-2 focus:ring-violet-500/40',
          'placeholder:text-gray-400 dark:placeholder:text-gray-500',
          'disabled:opacity-50',
        )}
      />
      <div className="flex items-center gap-2">
        <button
          onClick={() => {
            if (value.trim()) {
              onSubmit(value.trim());
              setValue('');
            }
          }}
          disabled={loading || !value.trim()}
          className="px-4 py-1.5 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t('workflow.interview.submit')}
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
          >
            {t('workflow.interview.skipBtn')}
          </button>
        )}
        <span className="ml-auto text-2xs text-gray-400 dark:text-gray-500">Ctrl+Enter</span>
      </div>
    </div>
  );
}

/** Boolean yes/no buttons */
function BooleanInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={() => onSubmit('yes')}
        disabled={loading}
        className={clsx(
          'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
          'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
          'hover:bg-green-200 dark:hover:bg-green-900/50',
          'disabled:opacity-50 disabled:cursor-not-allowed',
        )}
      >
        {t('workflow.interview.yes')}
      </button>
      <button
        onClick={() => onSubmit('no')}
        disabled={loading}
        className={clsx(
          'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
          'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
          'hover:bg-red-200 dark:hover:bg-red-900/50',
          'disabled:opacity-50 disabled:cursor-not-allowed',
        )}
      >
        {t('workflow.interview.no')}
      </button>
      {!question.required && (
        <button
          onClick={onSkip}
          disabled={loading}
          className="px-3 py-2 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
        >
          {t('workflow.interview.skipBtn')}
        </button>
      )}
    </div>
  );
}

/** Single select with optional "Other" custom input */
function SingleSelectInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');
  const [selected, setSelected] = useState<string | null>(null);
  const [showCustom, setShowCustom] = useState(false);
  const [customValue, setCustomValue] = useState('');
  const customInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showCustom) customInputRef.current?.focus();
  }, [showCustom]);

  const handleSubmit = useCallback(() => {
    if (showCustom && customValue.trim()) {
      onSubmit(customValue.trim());
    } else if (selected) {
      onSubmit(selected);
    }
  }, [selected, showCustom, customValue, onSubmit]);

  const canSubmit = showCustom ? customValue.trim().length > 0 : selected !== null;

  return (
    <div className="space-y-2">
      <div className="space-y-1 max-h-40 overflow-y-auto">
        {question.options.map((opt) => (
          <button
            key={opt}
            onClick={() => {
              setSelected(opt);
              setShowCustom(false);
              setCustomValue('');
            }}
            disabled={loading}
            className={clsx(
              'w-full text-left px-3 py-1.5 rounded-lg text-xs transition-colors',
              !showCustom && selected === opt
                ? 'bg-violet-100 dark:bg-violet-900/40 text-violet-700 dark:text-violet-300 ring-1 ring-violet-400 dark:ring-violet-600'
                : 'bg-gray-50 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700',
              'disabled:opacity-50',
            )}
          >
            {opt}
          </button>
        ))}
        {question.allowCustom && (
          <>
            {!showCustom ? (
              <button
                onClick={() => {
                  setShowCustom(true);
                  setSelected(null);
                }}
                disabled={loading}
                className="w-full text-left px-3 py-1.5 rounded-lg text-xs text-violet-600 dark:text-violet-400 bg-gray-50 dark:bg-gray-800 hover:bg-violet-50 dark:hover:bg-violet-900/20 transition-colors border border-dashed border-violet-300 dark:border-violet-700 disabled:opacity-50"
              >
                {t('workflow.interview.other')}
              </button>
            ) : (
              <div className="space-y-1">
                <input
                  ref={customInputRef}
                  type="text"
                  value={customValue}
                  onChange={(e) => setCustomValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && customValue.trim()) {
                      e.preventDefault();
                      onSubmit(customValue.trim());
                    }
                  }}
                  disabled={loading}
                  placeholder={t('workflow.interview.customPlaceholder')}
                  className={clsx(
                    'w-full px-3 py-1.5 rounded-lg text-xs border transition-colors',
                    'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100',
                    'border-violet-400 dark:border-violet-600',
                    'focus:outline-none focus:ring-2 focus:ring-violet-500/40',
                    'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                    'disabled:opacity-50',
                  )}
                />
                <button
                  onClick={() => {
                    setShowCustom(false);
                    setCustomValue('');
                  }}
                  className="text-2xs text-violet-500 dark:text-violet-400 hover:underline"
                >
                  {t('workflow.interview.backToOptions')}
                </button>
              </div>
            )}
          </>
        )}
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleSubmit}
          disabled={loading || !canSubmit}
          className="px-4 py-1.5 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t('workflow.interview.submit')}
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
          >
            {t('workflow.interview.skipBtn')}
          </button>
        )}
      </div>
    </div>
  );
}

/** Multi select with optional "Other" custom input */
function MultiSelectInput({ question, onSubmit, onSkip, loading }: InterviewInputPanelProps) {
  const { t } = useTranslation('simpleMode');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [showCustom, setShowCustom] = useState(false);
  const [customValue, setCustomValue] = useState('');
  const customInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showCustom) customInputRef.current?.focus();
  }, [showCustom]);

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
    const items = Array.from(selected);
    if (showCustom && customValue.trim()) {
      items.push(customValue.trim());
    }
    if (items.length > 0) {
      onSubmit(items.join(', '));
    }
  }, [selected, showCustom, customValue, onSubmit]);

  const totalCount = selected.size + (showCustom && customValue.trim() ? 1 : 0);

  return (
    <div className="space-y-2">
      <div className="space-y-1 max-h-40 overflow-y-auto">
        {question.options.map((opt) => (
          <button
            key={opt}
            onClick={() => toggleOption(opt)}
            disabled={loading}
            className={clsx(
              'w-full text-left px-3 py-1.5 rounded-lg text-xs transition-colors flex items-center gap-2',
              selected.has(opt)
                ? 'bg-violet-100 dark:bg-violet-900/40 text-violet-700 dark:text-violet-300'
                : 'bg-gray-50 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700',
              'disabled:opacity-50',
            )}
          >
            <span
              className={clsx(
                'w-3.5 h-3.5 rounded border flex items-center justify-center shrink-0',
                selected.has(opt) ? 'border-violet-500 bg-violet-500' : 'border-gray-300 dark:border-gray-600',
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
        {question.allowCustom && (
          <>
            {!showCustom ? (
              <button
                onClick={() => setShowCustom(true)}
                disabled={loading}
                className="w-full text-left px-3 py-1.5 rounded-lg text-xs text-violet-600 dark:text-violet-400 bg-gray-50 dark:bg-gray-800 hover:bg-violet-50 dark:hover:bg-violet-900/20 transition-colors border border-dashed border-violet-300 dark:border-violet-700 disabled:opacity-50"
              >
                {t('workflow.interview.other')}
              </button>
            ) : (
              <div className="space-y-1">
                <input
                  ref={customInputRef}
                  type="text"
                  value={customValue}
                  onChange={(e) => setCustomValue(e.target.value)}
                  disabled={loading}
                  placeholder={t('workflow.interview.customPlaceholder')}
                  className={clsx(
                    'w-full px-3 py-1.5 rounded-lg text-xs border transition-colors',
                    'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100',
                    'border-violet-400 dark:border-violet-600',
                    'focus:outline-none focus:ring-2 focus:ring-violet-500/40',
                    'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                    'disabled:opacity-50',
                  )}
                />
                <button
                  onClick={() => {
                    setShowCustom(false);
                    setCustomValue('');
                  }}
                  className="text-2xs text-violet-500 dark:text-violet-400 hover:underline"
                >
                  {t('workflow.interview.backToOptions')}
                </button>
              </div>
            )}
          </>
        )}
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleSubmit}
          disabled={loading || totalCount === 0}
          className="px-4 py-1.5 rounded-lg text-xs font-medium bg-violet-600 text-white hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t('workflow.interview.submitCount', { count: totalCount })}
        </button>
        {!question.required && (
          <button
            onClick={onSkip}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
          >
            {t('workflow.interview.skipBtn')}
          </button>
        )}
      </div>
    </div>
  );
}
