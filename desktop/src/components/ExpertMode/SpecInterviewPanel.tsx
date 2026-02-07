/**
 * SpecInterviewPanel Component
 *
 * Conversational interview panel for Expert mode that conducts
 * multi-turn LLM-driven conversations to elicit project requirements.
 * Renders interview questions with a progress indicator and conversation history.
 */

import { useState, useRef, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  useSpecInterviewStore,
  getPhaseLabel,
  getPhaseOrder,
} from '../../store/specInterview';
import type {
  InterviewPhase,
  InterviewConfig,
  InterviewQuestion,
  InterviewHistoryEntry,
} from '../../store/specInterview';

// ============================================================================
// Main Panel
// ============================================================================

export function SpecInterviewPanel() {
  const {
    session,
    compiledSpec,
    loading,
    error,
    startInterview,
    submitAnswer,
    compileSpec,
    reset,
    clearError,
  } = useSpecInterviewStore();

  // If no session, show the start form
  if (!session) {
    return <StartInterviewForm onStart={startInterview} loading={loading.starting} error={error} onClearError={clearError} />;
  }

  // If session is complete and compiled, show results
  if (session.status === 'finalized' && compiledSpec) {
    return <CompileResults compiledSpec={compiledSpec} onReset={reset} />;
  }

  // Active interview
  return (
    <div className="h-full flex flex-col">
      {/* Progress Header */}
      <InterviewProgressBar
        phase={session.phase}
        progress={session.progress}
        questionCursor={session.question_cursor}
        maxQuestions={session.max_questions}
      />

      {/* Error Banner */}
      {error && (
        <div className="mx-6 mt-2 px-4 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <div className="flex items-center justify-between">
            <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
            <button
              onClick={clearError}
              className="text-red-500 hover:text-red-700 text-sm font-medium"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Conversation Area */}
      <div className="flex-1 overflow-hidden flex flex-col">
        <ConversationHistory history={session.history} />

        {/* Current Question / Compile Action */}
        {session.status === 'finalized' ? (
          <CompileAction
            onCompile={() => compileSpec()}
            loading={loading.compiling}
          />
        ) : session.current_question ? (
          <QuestionInput
            question={session.current_question}
            onSubmit={submitAnswer}
            loading={loading.submitting}
          />
        ) : null}
      </div>

      {/* Footer Actions */}
      <div className="px-6 py-3 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <button
          onClick={reset}
          className={clsx(
            'text-sm text-gray-500 dark:text-gray-400',
            'hover:text-gray-700 dark:hover:text-gray-200',
            'transition-colors'
          )}
        >
          Cancel Interview
        </button>
        <div className="text-xs text-gray-400 dark:text-gray-500">
          {session.flow_level} flow | {session.question_cursor}/{session.max_questions} questions
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Start Interview Form
// ============================================================================

interface StartInterviewFormProps {
  onStart: (config: InterviewConfig) => Promise<unknown>;
  loading: boolean;
  error: string | null;
  onClearError: () => void;
}

function StartInterviewForm({ onStart, loading, error, onClearError }: StartInterviewFormProps) {
  const [description, setDescription] = useState('');
  const [flowLevel, setFlowLevel] = useState('standard');
  const [maxQuestions, setMaxQuestions] = useState(18);
  const [firstPrinciples, setFirstPrinciples] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!description.trim()) return;

    await onStart({
      description: description.trim(),
      flow_level: flowLevel,
      max_questions: maxQuestions,
      first_principles: firstPrinciples,
      project_path: null,
    });
  };

  return (
    <div className="h-full flex items-center justify-center p-6">
      <div className="max-w-lg w-full">
        <div className="mb-8 text-center">
          <h2 className="text-2xl font-semibold text-gray-900 dark:text-white mb-2">
            Spec Interview
          </h2>
          <p className="text-gray-600 dark:text-gray-400">
            Answer a series of questions to build a complete project specification.
            The interview will guide you through overview, scope, requirements, and stories.
          </p>
        </div>

        {error && (
          <div className="mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
            <div className="flex items-center justify-between">
              <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
              <button onClick={onClearError} className="text-red-500 hover:text-red-700 text-sm">
                Dismiss
              </button>
            </div>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-5">
          {/* Description */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Project Description
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe what you want to build..."
              className={clsx(
                'w-full px-4 py-3 rounded-lg border',
                'bg-white dark:bg-gray-800',
                'border-gray-300 dark:border-gray-600',
                'text-gray-900 dark:text-white',
                'placeholder-gray-400 dark:placeholder-gray-500',
                'focus:ring-2 focus:ring-primary-500 focus:border-primary-500',
                'resize-none'
              )}
              rows={4}
              required
            />
          </div>

          {/* Flow Level */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Flow Level
            </label>
            <div className="flex gap-3">
              {(['quick', 'standard', 'full'] as const).map((level) => (
                <button
                  key={level}
                  type="button"
                  onClick={() => setFlowLevel(level)}
                  className={clsx(
                    'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                    flowLevel === level
                      ? 'bg-primary-600 text-white'
                      : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                  )}
                >
                  {level.charAt(0).toUpperCase() + level.slice(1)}
                </button>
              ))}
            </div>
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {flowLevel === 'quick' && 'Fewer questions, faster completion'}
              {flowLevel === 'standard' && 'Balanced coverage of all spec areas'}
              {flowLevel === 'full' && 'Comprehensive with mandatory fields'}
            </p>
          </div>

          {/* Options Row */}
          <div className="flex items-center gap-6">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={firstPrinciples}
                onChange={(e) => setFirstPrinciples(e.target.checked)}
                className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">
                First principles mode
              </span>
            </label>

            <div className="flex items-center gap-2">
              <label className="text-sm text-gray-700 dark:text-gray-300">Max questions:</label>
              <input
                type="number"
                value={maxQuestions}
                onChange={(e) => setMaxQuestions(parseInt(e.target.value) || 18)}
                min={5}
                max={50}
                className={clsx(
                  'w-16 px-2 py-1 rounded border text-sm',
                  'bg-white dark:bg-gray-800',
                  'border-gray-300 dark:border-gray-600',
                  'text-gray-900 dark:text-white'
                )}
              />
            </div>
          </div>

          {/* Submit */}
          <button
            type="submit"
            disabled={loading || !description.trim()}
            className={clsx(
              'w-full px-6 py-3 rounded-lg font-medium text-white',
              'bg-primary-600 hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
          >
            {loading ? 'Starting Interview...' : 'Start Interview'}
          </button>
        </form>
      </div>
    </div>
  );
}

// ============================================================================
// Progress Bar
// ============================================================================

interface InterviewProgressBarProps {
  phase: InterviewPhase;
  progress: number;
  questionCursor: number;
  maxQuestions: number;
}

function InterviewProgressBar({ phase, progress, questionCursor, maxQuestions }: InterviewProgressBarProps) {
  const phases = getPhaseOrder().filter((p) => p !== 'complete');

  return (
    <div className="px-6 pt-4 pb-3 border-b border-gray-200 dark:border-gray-700">
      {/* Phase indicators */}
      <div className="flex items-center gap-1 mb-3">
        {phases.map((p, idx) => {
          const currentIdx = phases.indexOf(phase);
          const isActive = p === phase;
          const isComplete = idx < currentIdx || phase === 'complete';

          return (
            <div key={p} className="flex-1 flex flex-col items-center">
              <div
                className={clsx(
                  'w-full h-1.5 rounded-full transition-colors',
                  isComplete
                    ? 'bg-green-500'
                    : isActive
                    ? 'bg-primary-500'
                    : 'bg-gray-200 dark:bg-gray-700'
                )}
              />
              <span
                className={clsx(
                  'text-[10px] mt-1 font-medium',
                  isActive
                    ? 'text-primary-600 dark:text-primary-400'
                    : isComplete
                    ? 'text-green-600 dark:text-green-400'
                    : 'text-gray-400 dark:text-gray-500'
                )}
              >
                {getPhaseLabel(p)}
              </span>
            </div>
          );
        })}
      </div>

      {/* Overall progress */}
      <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>Phase: {getPhaseLabel(phase)}</span>
        <span>{Math.round(progress)}% complete</span>
      </div>
    </div>
  );
}

// ============================================================================
// Conversation History
// ============================================================================

interface ConversationHistoryProps {
  history: InterviewHistoryEntry[];
}

function ConversationHistory({ history }: ConversationHistoryProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [history.length]);

  if (history.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <p className="text-gray-400 dark:text-gray-500 text-sm">
          Interview conversation will appear here...
        </p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-auto p-6 space-y-4"
    >
      {history.map((entry, idx) => (
        <div key={idx} className="space-y-2">
          {/* Question (from system) */}
          <div className="flex gap-3">
            <div className={clsx(
              'flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center text-xs font-medium',
              'bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300'
            )}>
              Q
            </div>
            <div className={clsx(
              'flex-1 px-4 py-2.5 rounded-lg text-sm',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-800 dark:text-gray-200'
            )}>
              <div className="text-[10px] text-gray-400 dark:text-gray-500 mb-1">
                {getPhaseLabel(entry.phase as InterviewPhase)}
              </div>
              {entry.question}
            </div>
          </div>

          {/* Answer (from user) */}
          <div className="flex gap-3 justify-end">
            <div className={clsx(
              'max-w-[80%] px-4 py-2.5 rounded-lg text-sm',
              'bg-primary-50 dark:bg-primary-900/30',
              'text-gray-800 dark:text-gray-200',
              'border border-primary-200 dark:border-primary-800'
            )}>
              {entry.answer || <span className="text-gray-400 italic">(skipped)</span>}
            </div>
            <div className={clsx(
              'flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center text-xs font-medium',
              'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300'
            )}>
              A
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

// ============================================================================
// Question Input
// ============================================================================

interface QuestionInputProps {
  question: InterviewQuestion;
  onSubmit: (answer: string) => Promise<unknown>;
  loading: boolean;
}

function QuestionInput({ question, onSubmit, loading }: QuestionInputProps) {
  const [answer, setAnswer] = useState('');
  const inputRef = useRef<HTMLTextAreaElement | HTMLInputElement>(null);

  // Focus on input when question changes
  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.focus();
    }
    setAnswer('');
  }, [question.id]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (question.required && !answer.trim()) return;
    await onSubmit(answer.trim());
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && question.input_type !== 'textarea') {
      e.preventDefault();
      handleSubmit(e as unknown as React.FormEvent);
    }
  };

  return (
    <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Current question */}
      <div className="mb-3">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-[10px] uppercase tracking-wider text-primary-500 font-medium">
            {getPhaseLabel(question.phase)}
          </span>
          {question.required && (
            <span className="text-[10px] text-red-500 font-medium">Required</span>
          )}
        </div>
        <p className="text-sm font-medium text-gray-900 dark:text-white">
          {question.question}
        </p>
        {question.hint && (
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
            {question.hint}
          </p>
        )}
      </div>

      {/* Input */}
      <form onSubmit={handleSubmit} className="flex gap-2">
        {question.input_type === 'textarea' || question.input_type === 'list' ? (
          <textarea
            ref={inputRef as React.RefObject<HTMLTextAreaElement>}
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && e.ctrlKey) {
                e.preventDefault();
                handleSubmit(e as unknown as React.FormEvent);
              }
            }}
            placeholder={question.hint || (question.input_type === 'list' ? 'Enter items separated by commas or newlines' : 'Type your answer...')}
            className={clsx(
              'flex-1 px-4 py-2.5 rounded-lg border text-sm resize-none',
              'bg-gray-50 dark:bg-gray-800',
              'border-gray-300 dark:border-gray-600',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
            )}
            rows={question.input_type === 'list' ? 3 : 2}
            disabled={loading}
          />
        ) : (
          <input
            ref={inputRef as React.RefObject<HTMLInputElement>}
            type="text"
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={question.hint || 'Type your answer...'}
            className={clsx(
              'flex-1 px-4 py-2.5 rounded-lg border text-sm',
              'bg-gray-50 dark:bg-gray-800',
              'border-gray-300 dark:border-gray-600',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
            )}
            disabled={loading}
          />
        )}

        <div className="flex flex-col gap-1">
          <button
            type="submit"
            disabled={loading || (question.required && !answer.trim())}
            className={clsx(
              'px-4 py-2.5 rounded-lg text-sm font-medium text-white',
              'bg-primary-600 hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
          >
            {loading ? '...' : 'Submit'}
          </button>
          {!question.required && (
            <button
              type="button"
              onClick={() => onSubmit('next')}
              disabled={loading}
              className={clsx(
                'px-4 py-1.5 rounded-lg text-xs',
                'text-gray-500 dark:text-gray-400',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
                'disabled:opacity-50',
                'transition-colors'
              )}
            >
              Skip
            </button>
          )}
        </div>
      </form>
    </div>
  );
}

// ============================================================================
// Compile Action
// ============================================================================

interface CompileActionProps {
  onCompile: () => Promise<unknown>;
  loading: boolean;
}

function CompileAction({ onCompile, loading }: CompileActionProps) {
  return (
    <div className="px-6 py-6 border-t border-gray-200 dark:border-gray-700 bg-green-50 dark:bg-green-900/10">
      <div className="text-center">
        <div className="text-lg font-semibold text-green-700 dark:text-green-400 mb-2">
          Interview Complete
        </div>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
          All questions have been answered. Compile the interview into a specification.
        </p>
        <button
          onClick={onCompile}
          disabled={loading}
          className={clsx(
            'px-8 py-3 rounded-lg font-medium text-white',
            'bg-green-600 hover:bg-green-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors'
          )}
        >
          {loading ? 'Compiling Spec...' : 'Compile Specification'}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// Compile Results
// ============================================================================

interface CompileResultsProps {
  compiledSpec: {
    spec_json: Record<string, unknown>;
    spec_md: string;
    prd_json: Record<string, unknown>;
  };
  onReset: () => void;
}

function CompileResults({ compiledSpec, onReset }: CompileResultsProps) {
  const [activeTab, setActiveTab] = useState<'md' | 'json' | 'prd'>('md');

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            Compiled Specification
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            spec.json, spec.md, and prd.json have been generated
          </p>
        </div>
        <button
          onClick={onReset}
          className={clsx(
            'px-4 py-2 rounded-lg text-sm font-medium',
            'bg-gray-100 dark:bg-gray-700',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-600',
            'transition-colors'
          )}
        >
          New Interview
        </button>
      </div>

      {/* Tab switcher */}
      <div className="px-6 pt-2 border-b border-gray-200 dark:border-gray-700 flex gap-1">
        {([
          { id: 'md' as const, label: 'spec.md' },
          { id: 'json' as const, label: 'spec.json' },
          { id: 'prd' as const, label: 'prd.json' },
        ]).map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={clsx(
              'px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
              activeTab === tab.id
                ? 'bg-gray-100 dark:bg-gray-800 text-primary-600 dark:text-primary-400'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {activeTab === 'md' && (
          <pre className={clsx(
            'whitespace-pre-wrap text-sm font-mono',
            'text-gray-800 dark:text-gray-200'
          )}>
            {compiledSpec.spec_md}
          </pre>
        )}
        {activeTab === 'json' && (
          <pre className={clsx(
            'whitespace-pre-wrap text-sm font-mono',
            'text-gray-800 dark:text-gray-200'
          )}>
            {JSON.stringify(compiledSpec.spec_json, null, 2)}
          </pre>
        )}
        {activeTab === 'prd' && (
          <pre className={clsx(
            'whitespace-pre-wrap text-sm font-mono',
            'text-gray-800 dark:text-gray-200'
          )}>
            {JSON.stringify(compiledSpec.prd_json, null, 2)}
          </pre>
        )}
      </div>
    </div>
  );
}

export default SpecInterviewPanel;
