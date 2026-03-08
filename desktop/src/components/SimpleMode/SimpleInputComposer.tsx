import type { Ref } from 'react';
import type { FileAttachmentData } from '../../types/attachment';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import { InputBox, type InputBoxHandle } from './InputBox';
import { InterviewInputPanel } from './InterviewInputPanel';
import { PlanClarifyInputPanel } from './PlanClarifyInputPanel';
import type { QueuePriority, QueuedChatMessage } from './queuePersistence';

export interface SimpleInputComposerProps {
  t: (key: string, opts?: { defaultValue?: string; [key: string]: unknown }) => string;
  workflowMode: 'chat' | 'plan' | 'task';
  workflowPhase: string;
  isRunning: boolean;
  taskInterviewingPhase: boolean;
  planClarifyingPhase: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasStructuredPlanClarifyQuestion: boolean;
  hasTextInterviewQuestion: boolean;
  taskPendingQuestion: InterviewQuestionCardData | null;
  planPendingQuestion: PlanClarifyQuestionCardData | null;
  onStructuredInterviewSubmit: (answer: string) => void | Promise<void>;
  onStructuredPlanClarifySubmit: (answer: string) => void | Promise<void>;
  onSkipInterviewQuestion: () => void | Promise<void>;
  onSkipPlanClarifyQuestion: () => void | Promise<void>;
  onSkipPlanClarification: () => void | Promise<void>;
  isInterviewSubmitting: boolean;
  inputBoxRef: Ref<InputBoxHandle>;
  description: string;
  onDescriptionChange: (value: string) => void;
  onSubmit: () => void | Promise<void>;
  inputDisabled: boolean;
  canQueueWhileRunning: boolean;
  inputLoading: boolean;
  attachments: FileAttachmentData[];
  onAttach: (file: FileAttachmentData) => void;
  onRemoveAttachment: (id: string) => void;
  workspacePath: string | null;
  activeAgentName: string | null;
  onClearAgent: () => void;
  queuedChatMessages: QueuedChatMessage[];
  onRemoveQueuedChatMessage: (id: string) => void;
  onMoveQueuedChatMessage: (id: string, direction: 'up' | 'down' | 'top' | 'bottom') => void;
  onSetQueuedChatMessagePriority: (id: string, priority: QueuePriority) => void;
  onRetryQueuedChatMessage: (id: string) => void;
  onClearQueuedChatMessages: () => void;
  maxQueuedChatMessages: number;
}

export function SimpleInputComposer({
  t,
  workflowMode,
  workflowPhase,
  isRunning,
  taskInterviewingPhase,
  planClarifyingPhase,
  hasStructuredInterviewQuestion,
  hasStructuredPlanClarifyQuestion,
  hasTextInterviewQuestion,
  taskPendingQuestion,
  planPendingQuestion,
  onStructuredInterviewSubmit,
  onStructuredPlanClarifySubmit,
  onSkipInterviewQuestion,
  onSkipPlanClarifyQuestion,
  onSkipPlanClarification,
  isInterviewSubmitting,
  inputBoxRef,
  description,
  onDescriptionChange,
  onSubmit,
  inputDisabled,
  canQueueWhileRunning,
  inputLoading,
  attachments,
  onAttach,
  onRemoveAttachment,
  workspacePath,
  activeAgentName,
  onClearAgent,
  queuedChatMessages,
  onRemoveQueuedChatMessage,
  onMoveQueuedChatMessage,
  onSetQueuedChatMessagePriority,
  onRetryQueuedChatMessage,
  onClearQueuedChatMessages,
  maxQueuedChatMessages,
}: SimpleInputComposerProps) {
  return (
    <div className="p-4 space-y-3">
      {hasStructuredInterviewQuestion && taskPendingQuestion && (
        <InterviewInputPanel
          key={taskPendingQuestion.questionId}
          question={taskPendingQuestion}
          onSubmit={onStructuredInterviewSubmit}
          onSkip={onSkipInterviewQuestion}
          loading={isInterviewSubmitting}
        />
      )}

      {hasTextInterviewQuestion && taskPendingQuestion && (
        <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50/40 dark:bg-violet-900/20 px-3 py-2">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <p className="text-xs font-medium uppercase tracking-wide text-violet-600 dark:text-violet-400">
                {t('workflow.interview.questionTitle', { defaultValue: 'Interview Question' })}
              </p>
              <p className="mt-1 text-sm font-medium text-violet-800 dark:text-violet-200">
                {taskPendingQuestion.question}
              </p>
              {taskPendingQuestion.hint && (
                <p className="mt-1 text-xs text-violet-600/80 dark:text-violet-300/80">{taskPendingQuestion.hint}</p>
              )}
            </div>
            {!taskPendingQuestion.required && (
              <button
                onClick={() => {
                  void onSkipInterviewQuestion();
                }}
                className="shrink-0 px-2 py-1 rounded text-xs text-violet-600 dark:text-violet-300 hover:bg-violet-100 dark:hover:bg-violet-800/50 transition-colors"
              >
                {t('workflow.interview.skipBtn', { defaultValue: 'Skip' })}
              </button>
            )}
          </div>
        </div>
      )}

      {hasStructuredPlanClarifyQuestion && planPendingQuestion && (
        <PlanClarifyInputPanel
          question={planPendingQuestion}
          onSubmit={onStructuredPlanClarifySubmit}
          onSkipQuestion={onSkipPlanClarifyQuestion}
          onSkipAll={onSkipPlanClarification}
          loading={inputLoading}
        />
      )}

      {taskInterviewingPhase && !taskPendingQuestion && (
        <div className="px-3 py-2 flex items-center gap-2 text-sm text-violet-600 dark:text-violet-400">
          <svg className="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            />
          </svg>
          <span>{t('workflow.interview.generating', { defaultValue: 'Generating next question...' })}</span>
        </div>
      )}

      {planClarifyingPhase && !planPendingQuestion && (
        <div className="px-3 py-2 flex items-center gap-2 text-sm text-amber-600 dark:text-amber-400">
          <svg className="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            />
          </svg>
          <span>
            {t('planMode:clarify.generatingQuestion', {
              defaultValue: 'Generating clarification question...',
            })}
          </span>
        </div>
      )}

      <InputBox
        ref={inputBoxRef}
        value={description}
        onChange={onDescriptionChange}
        onSubmit={onSubmit}
        disabled={inputDisabled}
        enterSubmits={false}
        placeholder={
          inputDisabled && !canQueueWhileRunning
            ? t('workflow.input.waitingPlaceholder', { defaultValue: 'Waiting for response...' })
            : workflowMode === 'task' && workflowPhase === 'configuring'
              ? t('workflow.input.configuringPlaceholder', {
                  defaultValue: 'Type config overrides (e.g. "6 parallel, enable TDD") or click Continue above...',
                })
              : workflowMode === 'task' && workflowPhase === 'reviewing_prd'
                ? t('workflow.input.prdFeedbackPlaceholder', {
                    defaultValue: 'Add feedback or press Approve on the PRD card...',
                  })
                : hasTextInterviewQuestion
                  ? t('workflow.input.interviewPlaceholder', {
                      defaultValue: 'Type your answer to the interview question...',
                    })
                  : workflowMode === 'task'
                    ? t('workflow.input.taskPlaceholder', {
                        defaultValue: 'Describe a task (implementation / analysis / refactor)...',
                      })
                    : workflowMode === 'plan'
                      ? t('workflow.input.planPlaceholder', {
                          defaultValue: 'Describe a task to decompose and execute (writing, research, etc.)...',
                        })
                      : isRunning
                        ? t('workflow.queue.placeholder', {
                            defaultValue: 'Execution in progress. Your message will be queued...',
                          })
                        : t('input.followUpPlaceholder', {
                            defaultValue: 'Type a normal chat message...',
                          })
        }
        isLoading={inputLoading}
        allowSubmitWhileLoading={canQueueWhileRunning}
        attachments={attachments}
        onAttach={onAttach}
        onRemoveAttachment={onRemoveAttachment}
        workspacePath={workspacePath}
        activeAgentName={activeAgentName}
        onClearAgent={onClearAgent}
      />

      {queuedChatMessages.length > 0 && (
        <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/60 px-3 py-2">
          <div className="flex items-center justify-between gap-2">
            <p className="text-xs font-medium text-gray-600 dark:text-gray-300">
              {t('workflow.queue.title', {
                count: queuedChatMessages.length,
                max: maxQueuedChatMessages,
                defaultValue: `Queued messages (${queuedChatMessages.length}/${maxQueuedChatMessages})`,
              })}
            </p>
            <button
              onClick={onClearQueuedChatMessages}
              className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
            >
              {t('workflow.queue.clearAll', { defaultValue: 'Clear All' })}
            </button>
          </div>
          <div className="mt-2 space-y-1">
            {queuedChatMessages.map((message, index) => (
              <div
                key={message.id}
                className="flex items-center gap-2 rounded bg-white dark:bg-gray-900 px-2 py-1 border border-gray-200 dark:border-gray-700"
              >
                <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">#{index + 1}</span>
                <span className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-300 shrink-0">
                  {t(`workflowMode.${message.mode}`, { defaultValue: message.mode })}
                </span>
                <span className="text-2xs px-1 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300 shrink-0">
                  {t(`workflow.queue.priority.${message.priority}`, { defaultValue: message.priority })}
                </span>
                <span className="text-2xs px-1 py-0.5 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 shrink-0">
                  {t(`workflow.queue.status.${message.status}`, { defaultValue: message.status })}
                </span>
                <span className="text-xs text-gray-700 dark:text-gray-200 truncate flex-1">{message.prompt}</span>
                {message.attachments.length > 0 && (
                  <span className="text-2xs px-1 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 shrink-0">
                    {t('workflow.queue.attachmentCount', {
                      count: message.attachments.length,
                      defaultValue: `${message.attachments.length} attachment(s)`,
                    })}
                  </span>
                )}
                {message.attempts > 0 && (
                  <span className="text-2xs text-amber-600 dark:text-amber-300 shrink-0">
                    {t('workflow.queue.retryCount', {
                      count: message.attempts,
                      defaultValue: `retry ${message.attempts}`,
                    })}
                  </span>
                )}
                {message.lastError && (message.status === 'failed' || message.status === 'blocked') && (
                  <span
                    className="text-2xs text-rose-600 dark:text-rose-300 truncate max-w-44 shrink-0"
                    title={message.lastError}
                  >
                    {t('workflow.queue.blockedReason', {
                      reason: message.lastError,
                      defaultValue: `reason: ${message.lastError}`,
                    })}
                  </span>
                )}
                <div className="shrink-0 flex items-center gap-1">
                  <button
                    onClick={() => onMoveQueuedChatMessage(message.id, 'top')}
                    className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
                    title={t('workflow.queue.moveTop', { defaultValue: 'Move to top' })}
                  >
                    ⇡
                  </button>
                  <button
                    onClick={() => onMoveQueuedChatMessage(message.id, 'up')}
                    className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
                    title={t('workflow.queue.moveUp', { defaultValue: 'Move up' })}
                  >
                    ↑
                  </button>
                  <button
                    onClick={() => onMoveQueuedChatMessage(message.id, 'down')}
                    className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
                    title={t('workflow.queue.moveDown', { defaultValue: 'Move down' })}
                  >
                    ↓
                  </button>
                  <button
                    onClick={() => onMoveQueuedChatMessage(message.id, 'bottom')}
                    className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
                    title={t('workflow.queue.moveBottom', { defaultValue: 'Move to bottom' })}
                  >
                    ⇣
                  </button>
                  <button
                    onClick={() => onSetQueuedChatMessagePriority(message.id, 'high')}
                    className="text-2xs text-red-500 hover:text-red-600 dark:text-red-400 dark:hover:text-red-300 transition-colors"
                    title={t('workflow.queue.priority.high', { defaultValue: 'high' })}
                  >
                    H
                  </button>
                  <button
                    onClick={() => onSetQueuedChatMessagePriority(message.id, 'normal')}
                    className="text-2xs text-sky-600 hover:text-sky-700 dark:text-sky-400 dark:hover:text-sky-300 transition-colors"
                    title={t('workflow.queue.priority.normal', { defaultValue: 'normal' })}
                  >
                    N
                  </button>
                  <button
                    onClick={() => onSetQueuedChatMessagePriority(message.id, 'low')}
                    className="text-2xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
                    title={t('workflow.queue.priority.low', { defaultValue: 'low' })}
                  >
                    L
                  </button>
                  {(message.status === 'failed' || message.status === 'blocked') && (
                    <button
                      onClick={() => onRetryQueuedChatMessage(message.id)}
                      className="text-2xs text-amber-600 hover:text-amber-700 dark:text-amber-300 dark:hover:text-amber-200 transition-colors"
                      title={t('workflow.queue.retryNow', { defaultValue: 'Retry' })}
                    >
                      {t('workflow.queue.retryNowShort', { defaultValue: 'Retry' })}
                    </button>
                  )}
                </div>
                <button
                  onClick={() => onRemoveQueuedChatMessage(message.id)}
                  className="text-2xs text-red-500 hover:text-red-600 dark:text-red-400 dark:hover:text-red-300 transition-colors"
                  title={t('workflow.queue.remove', { defaultValue: 'Remove queued message' })}
                >
                  {t('workflow.queue.removeShort', { defaultValue: 'Remove' })}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default SimpleInputComposer;
