/**
 * PromptPanel Component
 *
 * Collapsible sidebar panel showing prompt templates with quick insert.
 * Includes a "Manage All..." button to open the PromptDialog.
 */

import { useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { GearIcon, DrawingPinIcon, DrawingPinFilledIcon } from '@radix-ui/react-icons';
import { usePromptsStore } from '../../store/prompts';
import { Collapsible } from './Collapsible';

// ============================================================================
// PromptPanel
// ============================================================================

export function PromptPanel() {
  const { t } = useTranslation('simpleMode');

  const prompts = usePromptsStore((s) => s.prompts);
  const loading = usePromptsStore((s) => s.loading);
  const panelOpen = usePromptsStore((s) => s.panelOpen);
  const fetchPrompts = usePromptsStore((s) => s.fetchPrompts);
  const openDialog = usePromptsStore((s) => s.openDialog);
  const togglePin = usePromptsStore((s) => s.togglePin);
  const recordUse = usePromptsStore((s) => s.recordUse);
  const setPendingInsert = usePromptsStore((s) => s.setPendingInsert);

  // Load data when panel opens
  useEffect(() => {
    if (panelOpen && prompts.length === 0) {
      fetchPrompts();
    }
  }, [panelOpen, prompts.length, fetchPrompts]);

  const pinnedPrompts = useMemo(() => prompts.filter((p) => p.is_pinned), [prompts]);

  const recentPrompts = useMemo(
    () =>
      prompts
        .filter((p) => !p.is_pinned && p.use_count > 0)
        .sort((a, b) => b.use_count - a.use_count)
        .slice(0, 5),
    [prompts],
  );

  const handleInsert = useCallback(
    (prompt: (typeof prompts)[0]) => {
      recordUse(prompt.id);
      setPendingInsert(prompt.content);
    },
    [recordUse, setPendingInsert],
  );

  const handleManageAll = useCallback(() => {
    openDialog();
  }, [openDialog]);

  const categoryBadgeColor: Record<string, string> = {
    coding: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
    writing: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
    analysis: 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300',
    custom: 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300',
  };

  return (
    <Collapsible open={panelOpen}>
      <div data-testid="prompt-panel" className="border-t border-gray-200 dark:border-gray-700">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('promptPanel.title', { defaultValue: 'Prompts' })}
          </span>
          <button
            onClick={handleManageAll}
            className={clsx(
              'p-1 rounded-md transition-colors',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title={t('promptPanel.manageAll', { defaultValue: 'Manage All...' })}
          >
            <GearIcon className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Content */}
        <div className="px-2 pb-2 space-y-1 max-h-[300px] overflow-y-auto">
          {/* Loading state */}
          {loading && prompts.length === 0 && (
            <div className="text-center py-4">
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {t('promptPanel.loading', { defaultValue: 'Loading prompts...' })}
              </span>
            </div>
          )}

          {/* Empty state */}
          {!loading && prompts.length === 0 && (
            <div className="text-center py-4">
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {t('promptPanel.noPrompts', { defaultValue: 'No prompts yet' })}
              </span>
            </div>
          )}

          {/* Pinned section */}
          {pinnedPrompts.length > 0 && (
            <>
              {pinnedPrompts.map((prompt) => (
                <PromptRow
                  key={prompt.id}
                  prompt={prompt}
                  categoryBadgeColor={categoryBadgeColor}
                  onInsert={() => handleInsert(prompt)}
                  onTogglePin={() => togglePin(prompt.id)}
                />
              ))}
              {recentPrompts.length > 0 && <div className="border-t border-gray-100 dark:border-gray-800 my-1" />}
            </>
          )}

          {/* Recent section */}
          {recentPrompts.map((prompt) => (
            <PromptRow
              key={prompt.id}
              prompt={prompt}
              categoryBadgeColor={categoryBadgeColor}
              onInsert={() => handleInsert(prompt)}
              onTogglePin={() => togglePin(prompt.id)}
            />
          ))}

          {/* Show all if no pinned and no recent but have prompts */}
          {pinnedPrompts.length === 0 &&
            recentPrompts.length === 0 &&
            prompts.length > 0 &&
            prompts
              .slice(0, 8)
              .map((prompt) => (
                <PromptRow
                  key={prompt.id}
                  prompt={prompt}
                  categoryBadgeColor={categoryBadgeColor}
                  onInsert={() => handleInsert(prompt)}
                  onTogglePin={() => togglePin(prompt.id)}
                />
              ))}
        </div>

        {/* Manage All button */}
        <div className="px-3 pb-2">
          <button
            onClick={handleManageAll}
            className={clsx(
              'w-full px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
              'text-primary-600 dark:text-primary-400',
              'hover:bg-primary-50 dark:hover:bg-primary-900/20',
              'border border-primary-200 dark:border-primary-800',
            )}
          >
            {t('promptPanel.manageAll', { defaultValue: 'Manage All...' })}
          </button>
        </div>
      </div>
    </Collapsible>
  );
}

// ============================================================================
// PromptRow
// ============================================================================

function PromptRow({
  prompt,
  categoryBadgeColor,
  onInsert,
  onTogglePin,
}: {
  prompt: { id: string; title: string; category: string; is_pinned: boolean; content: string };
  categoryBadgeColor: Record<string, string>;
  onInsert: () => void;
  onTogglePin: () => void;
}) {
  const { t } = useTranslation('simpleMode');

  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-2 py-1.5 rounded-md',
        'hover:bg-gray-50 dark:hover:bg-gray-800',
        'transition-colors group',
      )}
    >
      {/* Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="text-xs text-gray-900 dark:text-white truncate">{prompt.title}</span>
          <span
            className={clsx(
              'text-2xs px-1 py-0.5 rounded shrink-0',
              categoryBadgeColor[prompt.category] || categoryBadgeColor.custom,
            )}
          >
            {prompt.category}
          </span>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-0.5 shrink-0">
        <button
          onClick={onInsert}
          className={clsx(
            'px-1.5 py-0.5 text-2xs rounded-md transition-colors',
            'text-primary-600 dark:text-primary-400',
            'hover:bg-primary-50 dark:hover:bg-primary-900/20',
            'opacity-0 group-hover:opacity-100',
          )}
        >
          {t('promptPanel.insert', { defaultValue: 'Insert' })}
        </button>
        <button
          onClick={onTogglePin}
          className={clsx(
            'p-0.5 rounded-md transition-colors',
            prompt.is_pinned ? 'text-amber-500' : 'text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
        >
          {prompt.is_pinned ? <DrawingPinFilledIcon className="w-3 h-3" /> : <DrawingPinIcon className="w-3 h-3" />}
        </button>
      </div>
    </div>
  );
}

export default PromptPanel;
