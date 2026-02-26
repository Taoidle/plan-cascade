/**
 * SkillsSourcePicker
 *
 * Popover content for selecting individual Skills grouped by source type.
 * Supports client-side search filtering by name, description, and tags.
 */

import { useState, useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useContextSourcesStore } from '../../store/contextSources';
import { useSettingsStore } from '../../store/settings';
import type { SkillSummary } from '../../types/skillMemory';

/** Order of source types for display */
const SOURCE_ORDER = ['detected', 'external', 'project_local', 'builtin', 'user', 'generated'] as const;

/** Get a grouping key for a skill. Detected skills form their own group. */
function getGroupKey(skill: SkillSummary): string {
  if (skill.detected) return 'detected';
  return skill.source.type;
}

export function SkillsSourcePicker() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const {
    selectedSkillIds,
    availableSkills,
    isLoadingSkills,
    skillPickerSearchQuery,
    toggleSkillItem,
    toggleSkillGroup,
    loadAvailableSkills,
    setSkillPickerSearchQuery,
  } = useContextSourcesStore();

  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set(['detected']));

  // Load skills on mount
  useEffect(() => {
    if (workspacePath && availableSkills.length === 0 && !isLoadingSkills) {
      loadAvailableSkills(workspacePath);
    }
  }, [workspacePath, availableSkills.length, isLoadingSkills, loadAvailableSkills]);

  const lowerQuery = skillPickerSearchQuery.toLowerCase().trim();

  // Group skills by source type, apply search filter
  const { groups, filteredSkills } = useMemo(() => {
    let skills = availableSkills.filter((s) => s.enabled);

    if (lowerQuery) {
      skills = skills.filter(
        (s) =>
          s.name.toLowerCase().includes(lowerQuery) ||
          s.description.toLowerCase().includes(lowerQuery) ||
          s.tags.some((tag) => tag.toLowerCase().includes(lowerQuery)),
      );
    }

    const grouped = new Map<string, SkillSummary[]>();
    for (const skill of skills) {
      const key = getGroupKey(skill);
      const arr = grouped.get(key) || [];
      arr.push(skill);
      grouped.set(key, arr);
    }

    // Sort groups by defined order
    const orderedGroups = SOURCE_ORDER.filter((key) => grouped.has(key)).map((key) => ({
      key,
      skills: grouped.get(key)!,
    }));

    return { groups: orderedGroups, filteredSkills: skills };
  }, [availableSkills, lowerQuery]);

  const getGroupCheckState = (
    _groupKey: string,
    groupSkills: SkillSummary[],
  ): 'checked' | 'unchecked' | 'indeterminate' => {
    const ids = groupSkills.map((s) => s.id);
    const selectedCount = ids.filter((id) => selectedSkillIds.includes(id)).length;
    if (selectedCount === 0) return 'unchecked';
    if (selectedCount === ids.length) return 'checked';
    return 'indeterminate';
  };

  const groupLabel = (key: string) => {
    const labelKey = `contextSources.skillsPicker.groups.${key}` as const;
    const defaults: Record<string, string> = {
      detected: 'Auto-Detected',
      external: 'External',
      project_local: 'Project',
      builtin: 'Built-in',
      user: 'User',
      generated: 'Generated',
    };
    return t(labelKey, { defaultValue: defaults[key] || key });
  };

  const toggleExpand = (groupKey: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupKey)) {
        next.delete(groupKey);
      } else {
        next.add(groupKey);
      }
      return next;
    });
  };

  const isSearchMode = !!lowerQuery;

  if (isLoadingSkills) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
      </div>
    );
  }

  if (availableSkills.length === 0) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.skillsPicker.noSkills', { defaultValue: 'No skills available' })}
      </div>
    );
  }

  return (
    <div className="py-1">
      <div className="px-3 py-1.5 text-xs font-semibold text-gray-600 dark:text-gray-300 border-b border-gray-100 dark:border-gray-700">
        {t('contextSources.skillsPicker.title', { defaultValue: 'Skills Sources' })}
      </div>

      {/* Search input */}
      <div className="px-2 py-1.5 border-b border-gray-100 dark:border-gray-700">
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-gray-400" />
          <input
            type="text"
            value={skillPickerSearchQuery}
            onChange={(e) => setSkillPickerSearchQuery(e.target.value)}
            placeholder={t('contextSources.skillsPicker.searchPlaceholder', {
              defaultValue: 'Search skills...',
            })}
            className={clsx(
              'w-full pl-6 pr-2 py-1 text-2xs rounded',
              'bg-gray-50 dark:bg-gray-750 border border-gray-200 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300 placeholder-gray-400',
              'focus:outline-none focus:ring-1 focus:ring-emerald-400',
            )}
          />
        </div>
      </div>

      <div className="max-h-64 overflow-y-auto">
        {isSearchMode && filteredSkills.length === 0 && (
          <div className="px-3 py-2 text-2xs text-gray-400">
            {t('contextSources.skillsPicker.noResults', { defaultValue: 'No matching skills' })}
          </div>
        )}

        {isSearchMode
          ? // Flat search results
            filteredSkills.map((skill) => (
              <div
                key={skill.id}
                className={clsx(
                  'flex items-center gap-1.5 px-2 py-1.5',
                  'hover:bg-gray-50 dark:hover:bg-gray-750',
                  'cursor-pointer select-none',
                )}
                onClick={() => toggleSkillItem(skill.id)}
              >
                <input
                  type="checkbox"
                  checked={selectedSkillIds.includes(skill.id)}
                  onChange={() => toggleSkillItem(skill.id)}
                  className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-emerald-600 focus:ring-emerald-500"
                />
                <span className="flex-1 text-2xs text-gray-700 dark:text-gray-300 truncate">{skill.name}</span>
                <span className="text-2xs text-gray-400 dark:text-gray-500 px-1 py-0 rounded bg-gray-100 dark:bg-gray-700">
                  {groupLabel(getGroupKey(skill))}
                </span>
              </div>
            ))
          : // Grouped tree view
            groups.map(({ key, skills: groupSkills }) => {
              const isExpanded = expandedGroups.has(key);
              const checkState = getGroupCheckState(key, groupSkills);

              return (
                <div key={key}>
                  {/* Group row */}
                  <div
                    className={clsx(
                      'flex items-center gap-1.5 px-2 py-1.5',
                      'hover:bg-gray-50 dark:hover:bg-gray-750',
                      'cursor-pointer select-none',
                    )}
                  >
                    <button
                      onClick={() => toggleExpand(key)}
                      className="p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                    >
                      {isExpanded ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
                    </button>

                    <input
                      type="checkbox"
                      checked={checkState === 'checked'}
                      ref={(el) => {
                        if (el) el.indeterminate = checkState === 'indeterminate';
                      }}
                      onChange={() => toggleSkillGroup(key === 'detected' ? '__detected__' : key)}
                      className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-emerald-600 focus:ring-emerald-500"
                    />

                    <span
                      className="flex-1 text-xs text-gray-700 dark:text-gray-300 cursor-pointer"
                      onClick={() => toggleExpand(key)}
                    >
                      {groupLabel(key)}
                    </span>

                    <span className="text-2xs text-gray-400 dark:text-gray-500">{groupSkills.length}</span>
                  </div>

                  {/* Skill items (expanded) */}
                  {isExpanded && (
                    <div className="ml-5 border-l border-gray-100 dark:border-gray-700">
                      {groupSkills.map((skill) => (
                        <div
                          key={skill.id}
                          className={clsx(
                            'flex items-center gap-1.5 px-2 py-1',
                            'hover:bg-gray-50 dark:hover:bg-gray-750',
                            'cursor-pointer select-none',
                          )}
                          onClick={() => toggleSkillItem(skill.id)}
                        >
                          <input
                            type="checkbox"
                            checked={selectedSkillIds.includes(skill.id)}
                            onChange={() => toggleSkillItem(skill.id)}
                            className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-emerald-600 focus:ring-emerald-500"
                          />
                          <span className="flex-1 text-2xs text-gray-600 dark:text-gray-400 truncate">
                            {skill.name}
                          </span>
                          {skill.detected && (
                            <span className="text-2xs text-emerald-500" title="Auto-detected">
                              &#10003;
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
      </div>
    </div>
  );
}
