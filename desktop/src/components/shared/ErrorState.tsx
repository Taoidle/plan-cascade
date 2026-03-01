/**
 * ErrorState Component
 *
 * Compact error surface for right-side output panel:
 * - summary row with severity counts
 * - collapsed preview (latest error only)
 * - expandable, scrollable stacked list for multiple errors
 */

import { useMemo, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ExclamationTriangleIcon,
  CrossCircledIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  Cross2Icon,
  ReloadIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, type ExecutionError, type ErrorSeverity } from '../../store/execution';

interface ErrorStateProps {
  storyId?: string;
  className?: string;
  showDismissed?: boolean;
  maxErrors?: number;
}

interface CompactErrorItemProps {
  error: ExecutionError;
  onDismiss: (id: string) => void;
  onRetry?: (storyId: string) => void;
  isPrimary?: boolean;
}

const SEVERITY_ORDER: Record<ErrorSeverity, number> = { critical: 0, error: 1, warning: 2 };

function severityBadgeClass(severity: ErrorSeverity): string {
  if (severity === 'critical') {
    return 'bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-300';
  }
  if (severity === 'error') {
    return 'bg-red-50 dark:bg-red-900/30 text-red-600 dark:text-red-300';
  }
  return 'bg-amber-50 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300';
}

function severityTextClass(severity: ErrorSeverity): string {
  if (severity === 'critical') return 'text-red-700 dark:text-red-300';
  if (severity === 'error') return 'text-red-600 dark:text-red-300';
  return 'text-amber-700 dark:text-amber-300';
}

function SeverityIcon({ severity }: { severity: ErrorSeverity }) {
  if (severity === 'warning') return <ExclamationTriangleIcon className="w-3.5 h-3.5" />;
  return <CrossCircledIcon className="w-3.5 h-3.5" />;
}

function CompactErrorItem({ error, onDismiss, onRetry, isPrimary = false }: CompactErrorItemProps) {
  const { t } = useTranslation('simpleMode');
  const [showDetails, setShowDetails] = useState(false);
  const hasStackTrace = Boolean(error.stackTrace);
  const hasSuggestedFix = Boolean(error.suggestedFix);
  const canRetry = Boolean(error.storyId && onRetry);
  const timestamp = new Date(error.timestamp).toLocaleTimeString();

  const handleRetry = useCallback(() => {
    if (error.storyId && onRetry) {
      onRetry(error.storyId);
    }
  }, [error.storyId, onRetry]);

  return (
    <div
      className={clsx(
        'rounded-md border p-2',
        isPrimary
          ? 'border-red-200 dark:border-red-800/60 bg-red-50/60 dark:bg-red-900/20'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900/40',
      )}
    >
      <div className="flex items-start gap-2">
        <div className={clsx('mt-0.5 shrink-0', severityTextClass(error.severity))}>
          <SeverityIcon severity={error.severity} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 min-w-0">
            <span className={clsx('text-[11px] px-1.5 py-0.5 rounded font-medium', severityBadgeClass(error.severity))}>
              {t(`rightPanel.errors.severity.${error.severity}`)}
            </span>
            <span className="text-[11px] text-gray-500 dark:text-gray-400 shrink-0">{timestamp}</span>
            <p className="text-xs font-medium text-gray-800 dark:text-gray-200 truncate">{error.title}</p>
          </div>
          <p className="text-xs text-gray-600 dark:text-gray-300 mt-1 break-words line-clamp-2">{error.description}</p>
        </div>
        <button
          onClick={() => onDismiss(error.id)}
          className="p-1 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
          title={t('rightPanel.errors.dismiss')}
        >
          <Cross2Icon className="w-3.5 h-3.5" />
        </button>
      </div>

      {(canRetry || hasStackTrace || hasSuggestedFix) && (
        <div className="mt-2 flex items-center gap-1 flex-wrap">
          {canRetry && (
            <button
              onClick={handleRetry}
              className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs bg-primary-600 text-white hover:bg-primary-700"
            >
              <ReloadIcon className="w-3 h-3" />
              {t('rightPanel.errors.retryStory')}
            </button>
          )}
          {(hasStackTrace || hasSuggestedFix) && (
            <button
              onClick={() => setShowDetails((prev) => !prev)}
              className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-gray-600 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
            >
              {showDetails ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
              {showDetails ? t('rightPanel.errors.hideDetails') : t('rightPanel.errors.showDetails')}
            </button>
          )}
        </div>
      )}

      {showDetails && (hasSuggestedFix || hasStackTrace) && (
        <div className="mt-2 space-y-2">
          {hasSuggestedFix && (
            <div className="rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 px-2 py-1.5">
              <p className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                {t('rightPanel.errors.suggestedFix')}
              </p>
              <p className="text-xs text-gray-700 dark:text-gray-300 mt-0.5 break-words">{error.suggestedFix}</p>
            </div>
          )}
          {hasStackTrace && (
            <pre className="rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 p-2 text-[11px] text-gray-700 dark:text-gray-300 max-h-32 overflow-y-auto whitespace-pre-wrap break-all">
              {error.stackTrace}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

export function ErrorState({ storyId, className, showDismissed = false, maxErrors = 10 }: ErrorStateProps) {
  const { t } = useTranslation('simpleMode');
  const { executionErrors, dismissError, retryStory, clearExecutionErrors } = useExecutionStore();
  const [expanded, setExpanded] = useState(false);

  const sortedErrors = useMemo(() => {
    let errors = executionErrors;
    if (storyId) {
      errors = errors.filter((e) => e.storyId === storyId);
    }
    if (!showDismissed) {
      errors = errors.filter((e) => !e.dismissed);
    }

    return [...errors].sort((a, b) => {
      const severityDiff = SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity];
      if (severityDiff !== 0) return severityDiff;
      return b.timestamp - a.timestamp;
    });
  }, [executionErrors, showDismissed, storyId]);

  if (sortedErrors.length === 0) return null;

  const visibleErrors = sortedErrors.slice(0, maxErrors);
  const overflowCount = Math.max(0, sortedErrors.length - visibleErrors.length);
  const latest = visibleErrors[0];
  const criticalCount = visibleErrors.filter((e) => e.severity === 'critical').length;
  const errorCount = visibleErrors.filter((e) => e.severity === 'error').length;
  const warningCount = visibleErrors.filter((e) => e.severity === 'warning').length;

  return (
    <div
      className={clsx(
        'rounded-lg border border-red-200/70 dark:border-red-800/60 bg-red-50/40 dark:bg-red-900/15',
        className,
      )}
    >
      <div className="px-2.5 py-2 flex items-center gap-2">
        <CrossCircledIcon className="w-4 h-4 text-red-500 dark:text-red-400 shrink-0" />
        <p className="text-xs font-medium text-red-700 dark:text-red-300 truncate">
          {t('rightPanel.errors.summary', { count: sortedErrors.length })}
        </p>
        <div className="ml-auto flex items-center gap-1 text-[11px]">
          {criticalCount > 0 && <span className="text-red-700 dark:text-red-300">{criticalCount} C</span>}
          {errorCount > 0 && <span className="text-red-600 dark:text-red-300">{errorCount} E</span>}
          {warningCount > 0 && <span className="text-amber-700 dark:text-amber-300">{warningCount} W</span>}
        </div>
        <button
          onClick={() => setExpanded((prev) => !prev)}
          className="inline-flex items-center gap-1 text-[11px] px-1.5 py-1 rounded text-red-700 dark:text-red-300 hover:bg-red-100 dark:hover:bg-red-900/30"
        >
          {expanded ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
          {expanded ? t('rightPanel.errors.hideList') : t('rightPanel.errors.viewAll')}
        </button>
      </div>

      <div className="border-t border-red-200/70 dark:border-red-800/60 p-2">
        {expanded ? (
          <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
            {visibleErrors.map((error, idx) => (
              <CompactErrorItem
                key={error.id}
                error={error}
                onDismiss={dismissError}
                onRetry={retryStory}
                isPrimary={idx === 0}
              />
            ))}
            {overflowCount > 0 && (
              <p className="text-[11px] text-gray-500 dark:text-gray-400 text-center">
                {t('rightPanel.errors.moreErrors', { count: overflowCount })}
              </p>
            )}
            <div className="pt-1">
              <button
                onClick={clearExecutionErrors}
                className="text-xs px-2 py-1 rounded text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
              >
                {t('rightPanel.errors.dismissAll')}
              </button>
            </div>
          </div>
        ) : (
          <CompactErrorItem error={latest} onDismiss={dismissError} onRetry={retryStory} isPrimary />
        )}
      </div>
    </div>
  );
}

export default ErrorState;
