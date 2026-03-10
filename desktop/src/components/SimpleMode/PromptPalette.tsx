/**
 * PromptPalette Component
 *
 * Floating panel triggered by "/" in InputBox.
 * Shows localized quick prompts, prompt templates, and plugin skills in groups.
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { usePromptsStore } from '../../store/prompts';
import { normalizePromptCategory, substituteVariables } from '../../types/prompt';
import { listInvocableSkills } from '../../lib/pluginApi';
import type { InvocablePluginSkill, PluginInvocation } from '../../types/plugin';

interface PromptPaletteProps {
  query: string;
  onSelectText: (resolvedText: string) => void;
  onSelectPluginSkill: (invocation: PluginInvocation, displayText: string) => void;
  onClose: () => void;
}

type SlashItem =
  | { kind: 'quick_prompt'; id: string; title: string; description: string; content: string }
  | { kind: 'prompt_template'; id: string; title: string; description?: string; category: string }
  | { kind: 'plugin_skill'; plugin_name: string; skill_name: string; description: string; allowed_tools: string[] };

interface SlashSection {
  key: 'quick_prompts' | 'prompt_templates' | 'plugin_skills';
  label: string;
  emptyLabel: string;
  items: SlashItem[];
}

export function PromptPalette({ query, onSelectText, onSelectPluginSkill, onClose }: PromptPaletteProps) {
  const { t, i18n } = useTranslation('simpleMode');
  const prompts = usePromptsStore((s) => s.prompts);
  const fetchPrompts = usePromptsStore((s) => s.fetchPrompts);
  const recordUse = usePromptsStore((s) => s.recordUse);

  const [selectedIndex, setSelectedIndex] = useState(0);
  const [variableMode, setVariableMode] = useState(false);
  const [selectedPromptId, setSelectedPromptId] = useState<string | null>(null);
  const [variableValues, setVariableValues] = useState<Record<string, string>>({});
  const [pluginSkills, setPluginSkills] = useState<InvocablePluginSkill[]>([]);
  const paletteRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void fetchPrompts();
  }, [fetchPrompts, i18n.language]);

  useEffect(() => {
    listInvocableSkills().then((res) => {
      if (res.success && res.data) {
        const sorted = [...res.data].sort((left, right) =>
          `${left.plugin_name}:${left.skill_name}`.localeCompare(`${right.plugin_name}:${right.skill_name}`),
        );
        setPluginSkills(sorted);
      }
    });
  }, []);

  const quickPrompts = useMemo<SlashItem[]>(
    () => [
      {
        kind: 'quick_prompt',
        id: 'quick-plan',
        title: t('promptPalette.quick.plan.title', { defaultValue: 'Plan the work' }),
        description: t('promptPalette.quick.plan.description', {
          defaultValue: 'Insert a planning prompt before implementation.',
        }),
        content: t('promptPalette.quick.plan.content', {
          defaultValue: 'Break this task into a short concrete plan before writing code.',
        }),
      },
      {
        kind: 'quick_prompt',
        id: 'quick-review',
        title: t('promptPalette.quick.review.title', { defaultValue: 'Review my changes' }),
        description: t('promptPalette.quick.review.description', {
          defaultValue: 'Insert a bug- and regression-focused review request.',
        }),
        content: t('promptPalette.quick.review.content', {
          defaultValue: 'Review my latest changes for bugs, regressions, and missing tests.',
        }),
      },
      {
        kind: 'quick_prompt',
        id: 'quick-summarize',
        title: t('promptPalette.quick.summarize.title', { defaultValue: 'Summarize this' }),
        description: t('promptPalette.quick.summarize.description', {
          defaultValue: 'Insert a concise summary request.',
        }),
        content: t('promptPalette.quick.summarize.content', {
          defaultValue: 'Summarize the important points and keep only the key decisions and risks.',
        }),
      },
      {
        kind: 'quick_prompt',
        id: 'quick-explain-error',
        title: t('promptPalette.quick.explainError.title', { defaultValue: 'Explain this error' }),
        description: t('promptPalette.quick.explainError.description', {
          defaultValue: 'Insert an error explanation and fix request.',
        }),
        content: t('promptPalette.quick.explainError.content', {
          defaultValue: 'Explain this error, identify the likely root cause, and suggest the smallest safe fix.',
        }),
      },
    ],
    [t],
  );

  const selectedPrompt = useMemo(() => prompts.find((p) => p.id === selectedPromptId), [prompts, selectedPromptId]);

  const matchesQuery = useCallback(
    (value: string, currentQuery: string) => value.toLowerCase().includes(currentQuery),
    [],
  );

  const sections = useMemo<SlashSection[]>(() => {
    const normalizedQuery = query.trim().toLowerCase();
    const promptTemplates: SlashItem[] = prompts.map((prompt) => ({
      kind: 'prompt_template',
      id: prompt.id,
      title: prompt.title,
      description: prompt.description || undefined,
      category: normalizePromptCategory(prompt.category),
    }));
    const pluginItems: SlashItem[] = pluginSkills.map((skill) => ({
      kind: 'plugin_skill',
      plugin_name: skill.plugin_name,
      skill_name: skill.skill_name,
      description: skill.description,
      allowed_tools: skill.allowed_tools,
    }));

    const filterItems = (items: SlashItem[]) => {
      if (!normalizedQuery) return items;
      return items.filter((item) => {
        if (item.kind === 'quick_prompt') {
          return (
            matchesQuery(item.title.toLowerCase(), normalizedQuery) ||
            matchesQuery(item.description.toLowerCase(), normalizedQuery) ||
            matchesQuery(item.content.toLowerCase(), normalizedQuery)
          );
        }
        if (item.kind === 'prompt_template') {
          return (
            matchesQuery(item.title.toLowerCase(), normalizedQuery) ||
            matchesQuery((item.description || '').toLowerCase(), normalizedQuery) ||
            matchesQuery(item.category.toLowerCase(), normalizedQuery)
          );
        }
        const label = `${item.plugin_name}:${item.skill_name}`.toLowerCase();
        return matchesQuery(label, normalizedQuery) || matchesQuery(item.description.toLowerCase(), normalizedQuery);
      });
    };

    return [
      {
        key: 'quick_prompts',
        label: t('promptPalette.groups.quickPrompts', { defaultValue: 'Quick Prompts' }),
        emptyLabel: t('promptPalette.empty.quickPrompts', { defaultValue: 'No quick prompts match this search.' }),
        items: filterItems(quickPrompts).slice(0, normalizedQuery ? 6 : quickPrompts.length),
      },
      {
        key: 'prompt_templates',
        label: t('promptPalette.groups.promptTemplates', { defaultValue: 'Prompt Templates' }),
        emptyLabel: t('promptPalette.empty.promptTemplates', {
          defaultValue: 'No prompt templates match this search.',
        }),
        items: filterItems(promptTemplates).slice(0, 6),
      },
      {
        key: 'plugin_skills',
        label: t('promptPalette.groups.pluginSkills', { defaultValue: 'Plugin Skills' }),
        emptyLabel: t('promptPalette.empty.pluginSkills', { defaultValue: 'No plugin skills match this search.' }),
        items: filterItems(pluginItems).slice(0, 6),
      },
    ];
  }, [matchesQuery, pluginSkills, prompts, query, quickPrompts, t]);

  const flatItems = useMemo(() => sections.flatMap((section) => section.items), [sections]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [flatItems.length, query]);

  const promptCategoryLabel = useCallback(
    (category: string) => {
      const normalizedCategory = normalizePromptCategory(category);
      return normalizedCategory
        ? t(`promptCategories.${normalizedCategory}`, {
            defaultValue: normalizedCategory,
          })
        : t('promptCategories.uncategorized', { defaultValue: 'Uncategorized' });
    },
    [t],
  );

  const handleSelectItem = useCallback(
    (item: SlashItem) => {
      if (item.kind === 'plugin_skill') {
        const display = `/plugin:${item.plugin_name}:${item.skill_name}`;
        onSelectPluginSkill(
          {
            plugin_name: item.plugin_name,
            skill_name: item.skill_name,
            args: {},
            source: 'slash',
          },
          display,
        );
        return;
      }

      if (item.kind === 'quick_prompt') {
        onSelectText(item.content);
        return;
      }

      const prompt = prompts.find((entry) => entry.id === item.id);
      if (!prompt) return;

      void recordUse(prompt.id);
      if (prompt.variables.length > 0) {
        setSelectedPromptId(prompt.id);
        setVariableMode(true);
        setVariableValues({});
        return;
      }
      onSelectText(prompt.content);
    },
    [onSelectPluginSkill, onSelectText, prompts, recordUse],
  );

  const handleVariableSubmit = useCallback(() => {
    if (!selectedPrompt) return;
    const resolved = substituteVariables(selectedPrompt.content, variableValues);
    onSelectText(resolved);
  }, [onSelectText, selectedPrompt, variableValues]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (variableMode) {
        if (event.key === 'Escape') {
          event.preventDefault();
          setVariableMode(false);
          setSelectedPromptId(null);
        } else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          handleVariableSubmit();
        }
        return;
      }

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setSelectedIndex((prev) => (prev < flatItems.length - 1 ? prev + 1 : prev));
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
        return;
      }
      if (event.key === 'Enter' && !event.metaKey && !event.ctrlKey) {
        event.preventDefault();
        const selected = flatItems[selectedIndex];
        if (selected) handleSelectItem(selected);
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
      }
    },
    [flatItems, handleSelectItem, handleVariableSubmit, onClose, selectedIndex, variableMode],
  );

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      handleKeyDown(event as unknown as React.KeyboardEvent);
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, [handleKeyDown]);

  const badgeTone = useCallback((item: SlashItem) => {
    if (item.kind === 'quick_prompt') return 'bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300';
    if (item.kind === 'plugin_skill') return 'bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300';
    if (item.category === 'coding') return 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300';
    if (item.category === 'writing')
      return 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300';
    if (item.category === 'analysis') return 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300';
    return 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300';
  }, []);

  if (!variableMode && flatItems.length === 0) return null;

  let runningIndex = -1;

  return (
    <div
      ref={paletteRef}
      className={clsx(
        'absolute z-50 left-4 right-16 max-h-80 overflow-auto',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'rounded-lg shadow-lg',
        'bottom-full mb-2',
      )}
    >
      {variableMode && selectedPrompt ? (
        <div className="p-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-gray-700 dark:text-gray-300">{selectedPrompt.title}</span>
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
              <label className="block text-2xs text-gray-500 dark:text-gray-400 mb-0.5">{`{{${varName}}}`}</label>
              <input
                type="text"
                value={variableValues[varName] || ''}
                onChange={(event) => setVariableValues((prev) => ({ ...prev, [varName]: event.target.value }))}
                placeholder={varName}
                className="w-full px-2 py-1 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                autoFocus={selectedPrompt.variables[0] === varName}
              />
            </div>
          ))}
          <button
            onClick={handleVariableSubmit}
            className="w-full px-3 py-1.5 text-xs font-medium rounded-md bg-primary-600 text-white hover:bg-primary-700 transition-colors"
          >
            {t('promptPalette.insertPrompt', { defaultValue: 'Insert Prompt' })}
          </button>
        </div>
      ) : (
        <>
          <div className="sticky top-0 px-3 py-1.5 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {t('promptPalette.title', { defaultValue: '/ Shortcuts' })}
              {query ? ` · ${flatItems.length}` : ''}
            </span>
          </div>
          <div className="py-1">
            {sections.map((section) => (
              <div key={section.key}>
                <div className="px-3 py-1 text-2xs font-semibold uppercase tracking-wide text-gray-400 dark:text-gray-500">
                  {section.label}
                </div>
                {section.items.length === 0 ? (
                  <div className="px-3 pb-2 text-2xs text-gray-400 dark:text-gray-500">{section.emptyLabel}</div>
                ) : (
                  section.items.map((item) => {
                    runningIndex += 1;
                    const currentIndex = runningIndex;
                    const title = item.kind === 'plugin_skill' ? `${item.plugin_name}:${item.skill_name}` : item.title;
                    const subtitle =
                      item.kind === 'quick_prompt'
                        ? item.description
                        : item.kind === 'prompt_template'
                          ? item.description
                          : item.description;
                    const badge =
                      item.kind === 'quick_prompt'
                        ? t('promptPalette.badges.quickPrompt', { defaultValue: 'Quick' })
                        : item.kind === 'prompt_template'
                          ? promptCategoryLabel(item.category)
                          : t('promptPalette.badges.pluginSkill', { defaultValue: 'Plugin' });

                    return (
                      <button
                        key={
                          item.kind === 'plugin_skill'
                            ? `plugin:${item.plugin_name}:${item.skill_name}`
                            : `${item.kind}:${item.id}`
                        }
                        onClick={() => handleSelectItem(item)}
                        className={clsx(
                          'w-full flex items-start gap-2 px-3 py-1.5 text-left transition-colors',
                          currentIndex === selectedIndex
                            ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-900 dark:text-primary-100'
                            : 'hover:bg-gray-100 dark:hover:bg-gray-700',
                        )}
                      >
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-1.5">
                            <span className="text-sm font-medium truncate">{title}</span>
                            <span className={clsx('text-2xs px-1 py-0.5 rounded shrink-0', badgeTone(item))}>
                              {badge}
                            </span>
                          </div>
                          {subtitle && (
                            <div className="text-xs text-gray-500 dark:text-gray-400 truncate">{subtitle}</div>
                          )}
                        </div>
                      </button>
                    );
                  })
                )}
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

export default PromptPalette;
