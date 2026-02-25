/**
 * ErrorState Component
 *
 * Actionable error display component with error description, suggested fix,
 * retry button, and collapsible stack trace. Supports warning, error, and
 * critical severity levels.
 *
 * Story 008: Real-time Execution Feedback
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  ExclamationTriangleIcon,
  CrossCircledIcon,
  InfoCircledIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  Cross2Icon,
  ReloadIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, ExecutionError, ErrorSeverity } from '../../store/execution';

// ============================================================================
// Types
// ============================================================================

interface ErrorStateProps {
  /** Filter errors by story ID */
  storyId?: string;
  /** Additional class names */
  className?: string;
  /** Show dismissed errors */
  showDismissed?: boolean;
  /** Maximum number of errors to display */
  maxErrors?: number;
}

interface SingleErrorProps {
  error: ExecutionError;
  onDismiss: (id: string) => void;
  onRetry?: (storyId: string) => void;
}

// ============================================================================
// Severity Configuration
// ============================================================================

const SEVERITY_CONFIG: Record<
  ErrorSeverity,
  {
    containerBg: string;
    containerBorder: string;
    iconColor: string;
    titleColor: string;
    icon: React.ReactNode;
    label: string;
  }
> = {
  warning: {
    containerBg: 'bg-warning-50 dark:bg-warning-950',
    containerBorder: 'border-warning-200 dark:border-warning-800',
    iconColor: 'text-warning-500',
    titleColor: 'text-warning-800 dark:text-warning-200',
    icon: <ExclamationTriangleIcon className="w-5 h-5" />,
    label: 'Warning',
  },
  error: {
    containerBg: 'bg-error-50 dark:bg-error-950',
    containerBorder: 'border-error-200 dark:border-error-800',
    iconColor: 'text-error-500',
    titleColor: 'text-error-800 dark:text-error-200',
    icon: <CrossCircledIcon className="w-5 h-5" />,
    label: 'Error',
  },
  critical: {
    containerBg: 'bg-error-100 dark:bg-error-900',
    containerBorder: 'border-error-300 dark:border-error-700',
    iconColor: 'text-error-600 dark:text-error-400',
    titleColor: 'text-error-900 dark:text-error-100',
    icon: <CrossCircledIcon className="w-5 h-5" />,
    label: 'Critical',
  },
};

// ============================================================================
// SingleError Component
// ============================================================================

function SingleError({ error, onDismiss, onRetry }: SingleErrorProps) {
  const [showStackTrace, setShowStackTrace] = useState(false);
  const config = SEVERITY_CONFIG[error.severity];
  const hasStackTrace = !!error.stackTrace;
  const hasRetry = !!error.storyId && !!onRetry;

  const handleRetry = useCallback(() => {
    if (error.storyId && onRetry) {
      onRetry(error.storyId);
    }
  }, [error.storyId, onRetry]);

  const timestamp = new Date(error.timestamp).toLocaleTimeString();

  return (
    <div
      className={clsx(
        'rounded-lg border overflow-hidden',
        'transition-all duration-300',
        'animate-[slideDown_0.3s_ease-out]',
        config.containerBg,
        config.containerBorder,
        error.severity === 'critical' && 'ring-2 ring-error-400/50 dark:ring-error-600/50',
      )}
    >
      {/* Header */}
      <div className="flex items-start gap-3 p-4">
        {/* Severity icon */}
        <div className={clsx('mt-0.5 shrink-0', config.iconColor)}>{config.icon}</div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Title row */}
          <div className="flex items-start justify-between gap-2">
            <div>
              <div className="flex items-center gap-2">
                <span
                  className={clsx(
                    'text-2xs font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded',
                    error.severity === 'warning' &&
                      'bg-warning-200 dark:bg-warning-800 text-warning-800 dark:text-warning-200',
                    error.severity === 'error' && 'bg-error-200 dark:bg-error-800 text-error-800 dark:text-error-200',
                    error.severity === 'critical' &&
                      'bg-error-300 dark:bg-error-700 text-error-900 dark:text-error-100',
                  )}
                >
                  {config.label}
                </span>
                <span className="text-2xs text-gray-500 dark:text-gray-400">{timestamp}</span>
              </div>
              <h4 className={clsx('font-semibold mt-1', config.titleColor)}>{error.title}</h4>
            </div>

            {/* Dismiss button */}
            <button
              onClick={() => onDismiss(error.id)}
              className={clsx(
                'p-1 rounded shrink-0',
                'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                'hover:bg-gray-200/50 dark:hover:bg-gray-700/50',
                'transition-colors',
              )}
              title="Dismiss"
            >
              <Cross2Icon className="w-4 h-4" />
            </button>
          </div>

          {/* Description */}
          <p className="text-sm text-gray-700 dark:text-gray-300 mt-1.5">{error.description}</p>

          {/* Suggested fix */}
          {error.suggestedFix && (
            <div
              className={clsx(
                'flex items-start gap-2 mt-3 p-2.5 rounded-md',
                'bg-white/60 dark:bg-gray-800/60',
                'border border-gray-200/50 dark:border-gray-700/50',
              )}
            >
              <InfoCircledIcon className="w-4 h-4 text-primary-500 mt-0.5 shrink-0" />
              <div>
                <span className="text-xs font-medium text-gray-600 dark:text-gray-400">Suggested Fix</span>
                <p className="text-sm text-gray-800 dark:text-gray-200 mt-0.5">{error.suggestedFix}</p>
              </div>
            </div>
          )}

          {/* Actions row */}
          <div className="flex items-center gap-2 mt-3">
            {/* Retry button */}
            {hasRetry && (
              <button
                onClick={handleRetry}
                className={clsx(
                  'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg',
                  'text-sm font-medium',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'transition-colors',
                  'shadow-sm',
                )}
              >
                <ReloadIcon className="w-3.5 h-3.5" />
                Retry Story
              </button>
            )}

            {/* Stack trace toggle */}
            {hasStackTrace && (
              <button
                onClick={() => setShowStackTrace((prev) => !prev)}
                className={clsx(
                  'inline-flex items-center gap-1 px-2 py-1 rounded-md',
                  'text-xs font-medium',
                  'bg-gray-200/50 dark:bg-gray-700/50',
                  'text-gray-600 dark:text-gray-400',
                  'hover:bg-gray-300/50 dark:hover:bg-gray-600/50',
                  'transition-colors',
                )}
              >
                {showStackTrace ? (
                  <ChevronDownIcon className="w-3.5 h-3.5" />
                ) : (
                  <ChevronRightIcon className="w-3.5 h-3.5" />
                )}
                {showStackTrace ? 'Hide' : 'Show'} Details
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Collapsible stack trace */}
      {showStackTrace && hasStackTrace && (
        <div className={clsx('border-t', config.containerBorder)}>
          <pre
            className={clsx(
              'p-4 text-xs font-mono',
              'text-gray-700 dark:text-gray-300',
              'whitespace-pre-wrap break-all',
              'max-h-48 overflow-y-auto',
              'bg-white/40 dark:bg-gray-900/40',
            )}
          >
            {error.stackTrace}
          </pre>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// ErrorState Component
// ============================================================================

export function ErrorState({ storyId, className, showDismissed = false, maxErrors = 10 }: ErrorStateProps) {
  const { executionErrors, dismissError, retryStory, clearExecutionErrors } = useExecutionStore();

  const filteredErrors = (() => {
    let errors = executionErrors;

    // Filter by story
    if (storyId) {
      errors = errors.filter((e) => e.storyId === storyId);
    }

    // Filter dismissed
    if (!showDismissed) {
      errors = errors.filter((e) => !e.dismissed);
    }

    // Sort by severity (critical > error > warning) then by timestamp
    errors = [...errors].sort((a, b) => {
      const severityOrder: Record<ErrorSeverity, number> = { critical: 0, error: 1, warning: 2 };
      const severityDiff = severityOrder[a.severity] - severityOrder[b.severity];
      if (severityDiff !== 0) return severityDiff;
      return b.timestamp - a.timestamp;
    });

    return errors.slice(0, maxErrors);
  })();

  if (filteredErrors.length === 0) return null;

  const criticalCount = filteredErrors.filter((e) => e.severity === 'critical').length;
  const errorCount = filteredErrors.filter((e) => e.severity === 'error').length;
  const warningCount = filteredErrors.filter((e) => e.severity === 'warning').length;

  return (
    <div className={clsx('space-y-3', className)}>
      {/* Summary header */}
      {filteredErrors.length > 1 && (
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs">
            {criticalCount > 0 && (
              <span className="text-error-600 dark:text-error-400 font-medium">{criticalCount} critical</span>
            )}
            {errorCount > 0 && (
              <span className="text-error-500 font-medium">
                {errorCount} error{errorCount > 1 ? 's' : ''}
              </span>
            )}
            {warningCount > 0 && (
              <span className="text-warning-600 dark:text-warning-400 font-medium">
                {warningCount} warning{warningCount > 1 ? 's' : ''}
              </span>
            )}
          </div>
          <button
            onClick={clearExecutionErrors}
            className={clsx(
              'text-xs px-2 py-1 rounded',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'transition-colors',
            )}
          >
            Dismiss All
          </button>
        </div>
      )}

      {/* Error list */}
      {filteredErrors.map((error) => (
        <SingleError key={error.id} error={error} onDismiss={dismissError} onRetry={retryStory} />
      ))}
    </div>
  );
}

export default ErrorState;
