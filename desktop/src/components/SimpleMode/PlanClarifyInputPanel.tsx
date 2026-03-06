import { useCallback, useEffect, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';

interface PlanClarifyInputPanelProps {
  question: PlanClarifyQuestionCardData;
  onSubmit: (answer: string) => void;
  onSkipQuestion: () => void;
  onSkipAll: () => void;
  loading: boolean;
}

export function PlanClarifyInputPanel({
  question,
  onSubmit,
  onSkipQuestion,
  onSkipAll,
  loading,
}: PlanClarifyInputPanelProps) {
  const { t } = useTranslation('planMode');

  return (
    <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50/40 dark:bg-amber-900/20 px-3 py-2 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-medium uppercase tracking-wide text-amber-600 dark:text-amber-400">
            {t('clarify.title', { defaultValue: 'Clarification Needed' })}
          </p>
          <p className="mt-1 text-sm font-medium text-amber-800 dark:text-amber-200">{question.question}</p>
          {question.hint && <p className="mt-1 text-xs text-amber-700/80 dark:text-amber-300/80">{question.hint}</p>}
        </div>
        <div className="shrink-0 flex items-center gap-1">
          <button
            onClick={onSkipQuestion}
            disabled={loading}
            className="px-2 py-1 rounded text-xs text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-800/50 disabled:opacity-50 transition-colors"
          >
            {t('clarify.skipQuestion', { defaultValue: 'Skip' })}
          </button>
          <button
            onClick={onSkipAll}
            disabled={loading}
            className="px-2 py-1 rounded text-xs text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-800/50 disabled:opacity-50 transition-colors"
          >
            {t('clarify.skipAll', { defaultValue: 'Skip All' })}
          </button>
        </div>
      </div>

      <PlanClarifyAnswerInput question={question} onSubmit={onSubmit} loading={loading} />
    </div>
  );
}

function PlanClarifyAnswerInput({
  question,
  onSubmit,
  loading,
}: Pick<PlanClarifyInputPanelProps, 'question' | 'onSubmit' | 'loading'>) {
  switch (question.inputType) {
    case 'boolean':
      return <BooleanAnswer onSubmit={onSubmit} loading={loading} />;
    case 'single_select':
      return <SingleSelectAnswer question={question} onSubmit={onSubmit} loading={loading} />;
    case 'multi_select':
      return <MultiSelectAnswer question={question} onSubmit={onSubmit} loading={loading} />;
    case 'textarea':
      return <TextareaAnswer question={question} onSubmit={onSubmit} loading={loading} />;
    default:
      return <TextAnswer question={question} onSubmit={onSubmit} loading={loading} />;
  }
}

function BooleanAnswer({ onSubmit, loading }: Pick<PlanClarifyInputPanelProps, 'onSubmit' | 'loading'>) {
  const { t } = useTranslation('planMode');
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={() => onSubmit('yes')}
        disabled={loading}
        className={clsx(
          'flex-1 px-3 py-2 rounded text-xs font-medium transition-colors',
          'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
          'hover:bg-green-200 dark:hover:bg-green-900/50 disabled:opacity-50 disabled:cursor-not-allowed',
        )}
      >
        {t('clarify.booleanYes', { defaultValue: 'Yes' })}
      </button>
      <button
        onClick={() => onSubmit('no')}
        disabled={loading}
        className={clsx(
          'flex-1 px-3 py-2 rounded text-xs font-medium transition-colors',
          'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
          'hover:bg-red-200 dark:hover:bg-red-900/50 disabled:opacity-50 disabled:cursor-not-allowed',
        )}
      >
        {t('clarify.booleanNo', { defaultValue: 'No' })}
      </button>
    </div>
  );
}

function SingleSelectAnswer({
  question,
  onSubmit,
  loading,
}: Pick<PlanClarifyInputPanelProps, 'question' | 'onSubmit' | 'loading'>) {
  const { t } = useTranslation('planMode');
  const options = question.options ?? [];
  const hasOptions = options.length > 0;
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
      return;
    }
    if (selected) {
      onSubmit(selected);
    }
  }, [showCustom, customValue, selected, onSubmit]);

  const canSubmit = showCustom ? customValue.trim().length > 0 : selected !== null;
  if (!hasOptions) return null;

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {options.map((option) => (
          <button
            key={option}
            onClick={() => {
              setSelected(option);
              setShowCustom(false);
              setCustomValue('');
            }}
            disabled={loading}
            className={clsx(
              'px-3 py-2 rounded text-xs text-left border transition-colors',
              !showCustom && selected === option
                ? 'border-amber-500 dark:border-amber-500 bg-amber-100 dark:bg-amber-900/40'
                : 'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900',
              'text-amber-800 dark:text-amber-200 hover:bg-amber-50 dark:hover:bg-amber-900/30',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {option}
          </button>
        ))}
      </div>
      {question.allowCustom && (
        <>
          {!showCustom ? (
            <button
              onClick={() => {
                setShowCustom(true);
                setSelected(null);
              }}
              disabled={loading}
              className="w-full text-left px-3 py-1.5 rounded text-xs text-amber-700 dark:text-amber-300 bg-white dark:bg-gray-900 hover:bg-amber-50 dark:hover:bg-amber-900/20 transition-colors border border-dashed border-amber-300 dark:border-amber-700 disabled:opacity-50"
            >
              {t('clarify.otherOption', { defaultValue: 'Other (type custom answer)' })}
            </button>
          ) : (
            <input
              ref={customInputRef}
              value={customValue}
              onChange={(event) => setCustomValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && customValue.trim()) {
                  event.preventDefault();
                  onSubmit(customValue.trim());
                }
              }}
              disabled={loading}
              placeholder={t('clarify.customPlaceholder', { defaultValue: 'Type custom answer...' })}
              className={clsx(
                'w-full px-3 py-2 rounded text-sm border transition-colors',
                'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900',
                'text-amber-900 dark:text-amber-100 placeholder:text-amber-500/70 dark:placeholder:text-amber-400/60',
                'focus:outline-none focus:ring-2 focus:ring-amber-500/30 disabled:opacity-50',
              )}
            />
          )}
        </>
      )}
      <button
        onClick={handleSubmit}
        disabled={loading || !canSubmit}
        className="px-3 py-1.5 rounded text-xs font-medium bg-amber-600 text-white hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {t('clarify.submitStructured', { defaultValue: 'Submit' })}
      </button>
    </div>
  );
}

function MultiSelectAnswer({
  question,
  onSubmit,
  loading,
}: Pick<PlanClarifyInputPanelProps, 'question' | 'onSubmit' | 'loading'>) {
  const { t } = useTranslation('planMode');
  const options = question.options ?? [];
  const hasOptions = options.length > 0;
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [showCustom, setShowCustom] = useState(false);
  const [customValue, setCustomValue] = useState('');
  const customInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showCustom) customInputRef.current?.focus();
  }, [showCustom]);

  const toggle = useCallback((option: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(option)) {
        next.delete(option);
      } else {
        next.add(option);
      }
      return next;
    });
  }, []);

  const handleSubmit = useCallback(() => {
    const values = Array.from(selected);
    if (showCustom && customValue.trim()) {
      values.push(customValue.trim());
    }
    if (values.length > 0) {
      onSubmit(values.join(', '));
    }
  }, [selected, showCustom, customValue, onSubmit]);

  const hasAnySelection = selected.size > 0 || (showCustom && customValue.trim().length > 0);
  if (!hasOptions) return null;

  return (
    <div className="space-y-2">
      <div className="space-y-1 max-h-40 overflow-y-auto">
        {options.map((option) => (
          <button
            key={option}
            onClick={() => toggle(option)}
            disabled={loading}
            className={clsx(
              'w-full text-left px-3 py-1.5 rounded text-xs transition-colors border',
              selected.has(option)
                ? 'border-amber-500 dark:border-amber-500 bg-amber-100 dark:bg-amber-900/40 text-amber-800 dark:text-amber-200'
                : 'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900 text-amber-800 dark:text-amber-200 hover:bg-amber-50 dark:hover:bg-amber-900/30',
              'disabled:opacity-50',
            )}
          >
            {option}
          </button>
        ))}
      </div>
      {question.allowCustom && (
        <>
          {!showCustom ? (
            <button
              onClick={() => setShowCustom(true)}
              disabled={loading}
              className="w-full text-left px-3 py-1.5 rounded text-xs text-amber-700 dark:text-amber-300 bg-white dark:bg-gray-900 hover:bg-amber-50 dark:hover:bg-amber-900/20 transition-colors border border-dashed border-amber-300 dark:border-amber-700 disabled:opacity-50"
            >
              {t('clarify.otherOption', { defaultValue: 'Other (type custom answer)' })}
            </button>
          ) : (
            <input
              ref={customInputRef}
              value={customValue}
              onChange={(event) => setCustomValue(event.target.value)}
              disabled={loading}
              placeholder={t('clarify.customPlaceholder', { defaultValue: 'Type custom answer...' })}
              className={clsx(
                'w-full px-3 py-2 rounded text-sm border transition-colors',
                'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900',
                'text-amber-900 dark:text-amber-100 placeholder:text-amber-500/70 dark:placeholder:text-amber-400/60',
                'focus:outline-none focus:ring-2 focus:ring-amber-500/30 disabled:opacity-50',
              )}
            />
          )}
        </>
      )}
      <button
        onClick={handleSubmit}
        disabled={loading || !hasAnySelection}
        className="px-3 py-1.5 rounded text-xs font-medium bg-amber-600 text-white hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {t('clarify.submitStructured', { defaultValue: 'Submit' })}
      </button>
    </div>
  );
}

function TextAnswer({
  question,
  onSubmit,
  loading,
}: Pick<PlanClarifyInputPanelProps, 'question' | 'onSubmit' | 'loading'>) {
  const { t } = useTranslation('planMode');
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setValue('');
    setError(null);
    inputRef.current?.focus();
  }, [question.questionId]);

  const handleSubmit = useCallback(() => {
    const normalized = value.trim();
    if (!normalized) {
      setError(t('clarify.validation.emptyAnswer', { defaultValue: 'Please provide an answer.' }));
      return;
    }
    setError(null);
    onSubmit(normalized);
    setValue('');
  }, [onSubmit, t, value]);

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2">
        <input
          ref={inputRef}
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
            if (error) setError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              handleSubmit();
            }
          }}
          disabled={loading}
          placeholder={t('clarify.inputPlaceholder', { defaultValue: 'Type your answer...' })}
          className={clsx(
            'flex-1 px-3 py-2 rounded text-sm border transition-colors',
            'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900',
            'text-amber-900 dark:text-amber-100 placeholder:text-amber-500/70 dark:placeholder:text-amber-400/60',
            'focus:outline-none focus:ring-2 focus:ring-amber-500/30 disabled:opacity-50',
          )}
        />
        <button
          onClick={handleSubmit}
          disabled={loading}
          className="px-3 py-2 rounded text-xs font-medium bg-amber-600 text-white hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t('clarify.submitStructured', { defaultValue: 'Submit' })}
        </button>
      </div>
      {error && <p className="text-xs text-rose-600 dark:text-rose-300">{error}</p>}
    </div>
  );
}

function TextareaAnswer({
  question,
  onSubmit,
  loading,
}: Pick<PlanClarifyInputPanelProps, 'question' | 'onSubmit' | 'loading'>) {
  const { t } = useTranslation('planMode');
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setValue('');
    setError(null);
    textareaRef.current?.focus();
  }, [question.questionId]);

  const handleSubmit = useCallback(() => {
    const normalized = value.trim();
    if (!normalized) {
      setError(t('clarify.validation.emptyAnswer', { defaultValue: 'Please provide an answer.' }));
      return;
    }
    setError(null);
    onSubmit(normalized);
    setValue('');
  }, [onSubmit, t, value]);

  return (
    <div className="space-y-1">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => {
          setValue(event.target.value);
          if (error) setError(null);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            handleSubmit();
          }
        }}
        disabled={loading}
        rows={3}
        placeholder={t('clarify.inputPlaceholder', { defaultValue: 'Type your answer...' })}
        className={clsx(
          'w-full px-3 py-2 rounded text-sm border transition-colors resize-none',
          'border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-900',
          'text-amber-900 dark:text-amber-100 placeholder:text-amber-500/70 dark:placeholder:text-amber-400/60',
          'focus:outline-none focus:ring-2 focus:ring-amber-500/30 disabled:opacity-50',
        )}
      />
      <div className="flex items-center gap-2">
        <button
          onClick={handleSubmit}
          disabled={loading}
          className="px-3 py-1.5 rounded text-xs font-medium bg-amber-600 text-white hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t('clarify.submitStructured', { defaultValue: 'Submit' })}
        </button>
        <span className="ml-auto text-2xs text-amber-600/80 dark:text-amber-400/80">
          {t('clarify.shortcutHint', { defaultValue: 'Ctrl/Cmd + Enter' })}
        </span>
      </div>
      {error && <p className="text-xs text-rose-600 dark:text-rose-300">{error}</p>}
    </div>
  );
}
