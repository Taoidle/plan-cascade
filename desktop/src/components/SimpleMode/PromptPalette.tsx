/**
 * PromptPalette Component
 *
 * Floating panel triggered by "/" in InputBox.
 * Shows filtered prompt templates with keyboard navigation.
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { usePromptsStore } from '../../store/prompts';
import { substituteVariables } from '../../types/prompt';

// ============================================================================
// Types
// ============================================================================

interface PromptPaletteProps {
  query: string;
  onSelect: (resolvedText: string) => void;
  onClose: () => void;
  onKeyboardNav?: (handler: (e: React.KeyboardEvent) => boolean) => void;
}

// ============================================================================
// PromptPalette
// ============================================================================

export function PromptPalette({ query, onSelect, onClose }: PromptPaletteProps) {
  const { t } = useTranslation('simpleMode');

  const prompts = usePromptsStore((s) => s.prompts);
  const fetchPrompts = usePromptsStore((s) => s.fetchPrompts);
  const recordUse = usePromptsStore((s) => s.recordUse);

  const [selectedIndex, setSelectedIndex] = useState(0);
  const [variableMode, setVariableMode] = useState(false);
  const [selectedPromptId, setSelectedPromptId] = useState<string | null>(null);
  const [variableValues, setVariableValues] = useState<Record<string, string>>({});
  const paletteRef = useRef<HTMLDivElement>(null);

  // Fetch prompts on mount
  useEffect(() => {
    if (prompts.length === 0) {
      fetchPrompts();
    }
  }, [prompts.length, fetchPrompts]);

  // Filter prompts by query
  const filteredPrompts = useMemo(() => {
    if (!query.trim()) {
      // Show pinned first, then by use_count
      return [...prompts]
        .sort((a, b) => {
          if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
          return b.use_count - a.use_count;
        })
        .slice(0, 8);
    }
    const q = query.toLowerCase();
    return prompts
      .filter(
        (p) =>
          p.title.toLowerCase().includes(q) ||
          p.category.toLowerCase().includes(q) ||
          (p.description && p.description.toLowerCase().includes(q))
      )
      .slice(0, 8);
  }, [prompts, query]);

  const selectedPrompt = useMemo(
    () => prompts.find((p) => p.id === selectedPromptId),
    [prompts, selectedPromptId]
  );

  // Reset selection when filtered list changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [filteredPrompts.length]);

  const handleSelectPrompt = useCallback(
    (promptId: string) => {
      const prompt = prompts.find((p) => p.id === promptId);
      if (!prompt) return;

      recordUse(prompt.id);

      if (prompt.variables.length > 0) {
        setSelectedPromptId(promptId);
        setVariableMode(true);
        setVariableValues({});
      } else {
        onSelect(prompt.content);
      }
    },
    [prompts, recordUse, onSelect]
  );

  const handleVariableSubmit = useCallback(() => {
    if (!selectedPrompt) return;
    const resolved = substituteVariables(selectedPrompt.content, variableValues);
    onSelect(resolved);
  }, [selectedPrompt, variableValues, onSelect]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (variableMode) {
        if (e.key === 'Escape') {
          e.preventDefault();
          setVariableMode(false);
          setSelectedPromptId(null);
        } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          handleVariableSubmit();
        }
        return;
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) =>
          prev < filteredPrompts.length - 1 ? prev + 1 : prev
        );
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
      } else if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const selected = filteredPrompts[selectedIndex];
        if (selected) {
          handleSelectPrompt(selected.id);
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    },
    [variableMode, filteredPrompts, selectedIndex, handleSelectPrompt, handleVariableSubmit, onClose]
  );

  // Expose keyboard handler via ref-like pattern
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      handleKeyDown(e as unknown as React.KeyboardEvent);
    };

    // Only listen when palette is mounted
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, [handleKeyDown]);

  const categoryBadgeColor: Record<string, string> = {
    coding: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
    writing: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
    analysis: 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300',
    custom: 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300',
  };

  if (filteredPrompts.length === 0 && !variableMode) return null;

  return (
    <div
      ref={paletteRef}
      className={clsx(
        'absolute z-50 left-4 right-16 max-h-72 overflow-auto',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'rounded-lg shadow-lg',
        'bottom-full mb-2'
      )}
    >
      {variableMode && selectedPrompt ? (
        /* Variable fill mode */
        <div className="p-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
              {selectedPrompt.title}
            </span>
            <button
              onClick={() => {
                setVariableMode(false);
                setSelectedPromptId(null);
              }}
              className="text-2xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              {t('promptPalette.back', { defaultValue: 'Back' })}
            </button>
          </div>
          {selectedPrompt.variables.map((varName) => (
            <div key={varName}>
              <label className="block text-2xs text-gray-500 dark:text-gray-400 mb-0.5">
                {`{{${varName}}}`}
              </label>
              <input
                type="text"
                value={variableValues[varName] || ''}
                onChange={(e) =>
                  setVariableValues((prev) => ({ ...prev, [varName]: e.target.value }))
                }
                placeholder={varName}
                className="w-full px-2 py-1 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                autoFocus={selectedPrompt.variables[0] === varName}
              />
            </div>
          ))}
          <button
            onClick={handleVariableSubmit}
            className={clsx(
              'w-full px-3 py-1.5 text-xs font-medium rounded-md',
              'bg-primary-600 text-white hover:bg-primary-700',
              'transition-colors'
            )}
          >
            {t('promptPalette.insertPrompt', { defaultValue: 'Insert Prompt' })}
          </button>
        </div>
      ) : (
        /* Prompt list mode */
        <>
          <div className="sticky top-0 px-3 py-1.5 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {t('promptPalette.title', { defaultValue: '/ Prompts' })}
              {query && ` — ${filteredPrompts.length} matches`}
            </span>
          </div>
          <div className="py-1">
            {filteredPrompts.map((prompt, index) => (
              <button
                key={prompt.id}
                onClick={() => handleSelectPrompt(prompt.id)}
                className={clsx(
                  'w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors',
                  index === selectedIndex
                    ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-900 dark:text-primary-100'
                    : 'hover:bg-gray-100 dark:hover:bg-gray-700'
                )}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span className="text-sm font-medium truncate">{prompt.title}</span>
                    <span
                      className={clsx(
                        'text-2xs px-1 py-0.5 rounded shrink-0',
                        categoryBadgeColor[prompt.category] || categoryBadgeColor.custom
                      )}
                    >
                      {prompt.category}
                    </span>
                  </div>
                  {prompt.description && (
                    <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
                      {prompt.description}
                    </div>
                  )}
                </div>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

export default PromptPalette;
