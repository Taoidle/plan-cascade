/**
 * AIReviewPanel Component
 *
 * Displays AI code review results inline within the staged changes section.
 * Parses the review text into structured notes with severity levels.
 * Each note is dismissible.
 *
 * Feature-005: LLM-Powered Git Assistance
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ReviewNote {
  id: number;
  text: string;
  severity: 'info' | 'warning' | 'error';
}

interface AIReviewPanelProps {
  /** Raw review text from LLM */
  reviewText: string;
  /** Callback when panel is dismissed */
  onDismiss: () => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Parse raw review text into structured notes.
 * The LLM returns bullet points; we classify severity by keywords.
 */
function parseReviewNotes(text: string): ReviewNote[] {
  const lines = text.split('\n').filter((line) => line.trim().length > 0);
  const notes: ReviewNote[] = [];
  let id = 0;

  for (const line of lines) {
    const trimmed = line.replace(/^[-*]\s*/, '').trim();
    if (!trimmed) continue;

    let severity: ReviewNote['severity'] = 'info';
    const lower = trimmed.toLowerCase();

    if (
      lower.includes('bug') ||
      lower.includes('security') ||
      lower.includes('vulnerability') ||
      lower.includes('critical') ||
      lower.includes('error') ||
      lower.includes('dangerous')
    ) {
      severity = 'error';
    } else if (
      lower.includes('warning') ||
      lower.includes('potential') ||
      lower.includes('consider') ||
      lower.includes('might') ||
      lower.includes('should') ||
      lower.includes('performance') ||
      lower.includes('improve')
    ) {
      severity = 'warning';
    }

    notes.push({ id: ++id, text: trimmed, severity });
  }

  return notes;
}

// ---------------------------------------------------------------------------
// Severity config
// ---------------------------------------------------------------------------

const SEVERITY_CONFIG = {
  error: {
    icon: (
      <svg className="w-3.5 h-3.5 text-red-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
    ),
    bg: 'bg-red-50 dark:bg-red-900/10 border-red-200 dark:border-red-800/50',
    text: 'text-red-700 dark:text-red-400',
    labelKey: 'aiReviewPanel.issue',
  },
  warning: {
    icon: (
      <svg className="w-3.5 h-3.5 text-amber-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
        />
      </svg>
    ),
    bg: 'bg-amber-50 dark:bg-amber-900/10 border-amber-200 dark:border-amber-800/50',
    text: 'text-amber-700 dark:text-amber-400',
    labelKey: 'aiReviewPanel.warning',
  },
  info: {
    icon: (
      <svg className="w-3.5 h-3.5 text-blue-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
    ),
    bg: 'bg-blue-50 dark:bg-blue-900/10 border-blue-200 dark:border-blue-800/50',
    text: 'text-blue-700 dark:text-blue-400',
    labelKey: 'aiReviewPanel.info',
  },
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function AIReviewPanel({ reviewText, onDismiss }: AIReviewPanelProps) {
  const { t } = useTranslation('git');
  const allNotes = parseReviewNotes(reviewText);
  const [dismissedIds, setDismissedIds] = useState<Set<number>>(new Set());

  const visibleNotes = allNotes.filter((n) => !dismissedIds.has(n.id));

  const handleDismissNote = useCallback((id: number) => {
    setDismissedIds((prev) => new Set(prev).add(id));
  }, []);

  // Summary counts
  const errorCount = visibleNotes.filter((n) => n.severity === 'error').length;
  const warningCount = visibleNotes.filter((n) => n.severity === 'warning').length;
  const infoCount = visibleNotes.filter((n) => n.severity === 'info').length;

  if (visibleNotes.length === 0 && allNotes.length > 0) {
    return (
      <div className="mx-3 my-2 p-2 rounded-lg border border-green-200 dark:border-green-800/50 bg-green-50 dark:bg-green-900/10">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5 text-xs text-green-700 dark:text-green-400">
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
            {t('aiReviewPanel.allDismissed')}
          </div>
          <button onClick={onDismiss} className="text-2xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
            {t('aiReviewPanel.close')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="mx-3 my-2 space-y-2">
      {/* Summary card */}
      <div className="flex items-center justify-between p-2 rounded-lg border border-purple-200 dark:border-purple-800/50 bg-purple-50 dark:bg-purple-900/10">
        <div className="flex items-center gap-3">
          <span className="text-xs font-medium text-purple-700 dark:text-purple-400 flex items-center gap-1">
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
              />
            </svg>
            {t('aiReviewPanel.title')}
          </span>
          <div className="flex items-center gap-2 text-2xs">
            {errorCount > 0 && (
              <span className="text-red-600 dark:text-red-400 font-medium">
                {t('aiReviewPanel.issues', { count: errorCount })}
              </span>
            )}
            {warningCount > 0 && (
              <span className="text-amber-600 dark:text-amber-400 font-medium">
                {t('aiReviewPanel.warnings', { count: warningCount })}
              </span>
            )}
            {infoCount > 0 && (
              <span className="text-blue-600 dark:text-blue-400 font-medium">
                {t('aiReviewPanel.notes', { count: infoCount })}
              </span>
            )}
          </div>
        </div>
        <button
          onClick={onDismiss}
          className="p-1 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
          title={t('aiReviewPanel.dismissReview')}
        >
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Review notes */}
      <div className="space-y-1">
        {visibleNotes.map((note) => {
          const config = SEVERITY_CONFIG[note.severity];
          return (
            <div
              key={note.id}
              className={clsx('flex items-start gap-2 px-2.5 py-1.5 rounded-md border text-xs', config.bg)}
            >
              {config.icon}
              <span className={clsx('flex-1', config.text)}>{note.text}</span>
              <button
                onClick={() => handleDismissNote(note.id)}
                className="p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 shrink-0"
                title={t('aiReviewPanel.dismiss')}
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default AIReviewPanel;
