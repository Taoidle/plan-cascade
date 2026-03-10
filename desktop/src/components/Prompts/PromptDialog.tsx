/**
 * PromptDialog Component
 *
 * Full-screen dialog for managing prompt templates with a two-column layout.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, PlusIcon, MagnifyingGlassIcon, TrashIcon, CopyIcon } from '@radix-ui/react-icons';
import { usePromptsStore } from '../../store/prompts';
import {
  PROMPT_CATEGORIES,
  SYSTEM_PROMPT_CATEGORIES,
  extractVariables,
  normalizePromptCategory,
} from '../../types/prompt';
import type { PromptTemplate, PromptCreateRequest, PromptUpdateRequest } from '../../types/prompt';

// ============================================================================
// PromptDialog
// ============================================================================

export function PromptDialog() {
  const { t, i18n } = useTranslation('simpleMode');

  const prompts = usePromptsStore((s) => s.prompts);
  const loading = usePromptsStore((s) => s.loading);
  const dialogOpen = usePromptsStore((s) => s.dialogOpen);
  const selectedPrompt = usePromptsStore((s) => s.selectedPrompt);
  const fetchPrompts = usePromptsStore((s) => s.fetchPrompts);
  const createPrompt = usePromptsStore((s) => s.createPrompt);
  const updatePrompt = usePromptsStore((s) => s.updatePrompt);
  const deletePrompt = usePromptsStore((s) => s.deletePrompt);
  const closeDialog = usePromptsStore((s) => s.closeDialog);

  const [searchQuery, setSearchQuery] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('all');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Form state
  const [formTitle, setFormTitle] = useState('');
  const [formContent, setFormContent] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formCategory, setFormCategory] = useState('');
  const [formTags, setFormTags] = useState('');
  const selectedPromptObj = useMemo(() => prompts.find((p) => p.id === selectedId), [prompts, selectedId]);
  const isBuiltinPrompt = Boolean(selectedPromptObj?.is_builtin && !isNew);

  const categoryLabel = useCallback(
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

  const userCategories = useMemo(
    () =>
      Array.from(
        new Set(
          prompts
            .map((prompt) => normalizePromptCategory(prompt.category))
            .filter(
              (category) =>
                category && !SYSTEM_PROMPT_CATEGORIES.includes(category as (typeof SYSTEM_PROMPT_CATEGORIES)[number]),
            ),
        ),
      ).sort((left, right) => left.localeCompare(right)),
    [prompts],
  );

  const categoryFilters = useMemo(
    () => [
      ...PROMPT_CATEGORIES,
      ...userCategories.filter(
        (category) => !PROMPT_CATEGORIES.includes(category as (typeof PROMPT_CATEGORIES)[number]),
      ),
    ],
    [userCategories],
  );

  const categorySuggestions = useMemo(
    () => [''].concat([...SYSTEM_PROMPT_CATEGORIES], userCategories),
    [userCategories],
  );

  const filteredPrompts = useMemo(() => {
    let result = prompts;
    if (categoryFilter !== 'all') {
      if (categoryFilter === 'uncategorized') {
        result = result.filter((prompt) => !normalizePromptCategory(prompt.category));
      } else {
        result = result.filter((prompt) => normalizePromptCategory(prompt.category) === categoryFilter);
      }
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (p) => p.title.toLowerCase().includes(q) || (p.description && p.description.toLowerCase().includes(q)),
      );
    }
    return result;
  }, [prompts, categoryFilter, searchQuery]);

  const detectedVariables = useMemo(() => extractVariables(formContent), [formContent]);

  useEffect(() => {
    setConfirmDelete(false);
  }, [selectedId, isNew, formTitle, formContent, formDescription, formCategory, formTags]);

  // Load prompts when dialog opens
  useEffect(() => {
    if (dialogOpen) {
      fetchPrompts();
    }
  }, [dialogOpen, fetchPrompts, i18n.language]);

  // Select prompt if provided
  const populateForm = useCallback((prompt: PromptTemplate) => {
    setFormTitle(prompt.title);
    setFormContent(prompt.content);
    setFormDescription(prompt.description || '');
    setFormCategory(normalizePromptCategory(prompt.category));
    setFormTags(prompt.tags.join(', '));
  }, []);

  useEffect(() => {
    if (selectedPrompt && dialogOpen) {
      setSelectedId(selectedPrompt.id);
      populateForm(selectedPrompt);
      setIsNew(false);
    }
  }, [selectedPrompt, dialogOpen, populateForm]);

  const resetForm = useCallback(() => {
    setFormTitle('');
    setFormContent('');
    setFormDescription('');
    setFormCategory('');
    setFormTags('');
  }, []);

  const handleSelectPrompt = useCallback(
    (prompt: PromptTemplate) => {
      setSelectedId(prompt.id);
      populateForm(prompt);
      setIsNew(false);
      setConfirmDelete(false);
    },
    [populateForm],
  );

  const handleNewPrompt = useCallback(() => {
    setSelectedId(null);
    setIsNew(true);
    resetForm();
    setConfirmDelete(false);
  }, [resetForm]);

  const handleSave = useCallback(async () => {
    if (!formTitle.trim() || !formContent.trim()) return;

    const tags = formTags
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean);

    if (isNew) {
      const req: PromptCreateRequest = {
        title: formTitle.trim(),
        content: formContent,
        description: formDescription.trim() || null,
        category: formCategory.trim(),
        tags,
        is_pinned: false,
      };
      const result = await createPrompt(req);
      if (result) {
        setSelectedId(result.id);
        setIsNew(false);
        setConfirmDelete(false);
      }
    } else if (selectedId) {
      const req: PromptUpdateRequest = {
        title: formTitle.trim(),
        content: formContent,
        description: formDescription.trim() || null,
        category: formCategory.trim(),
        tags,
      };
      await updatePrompt(selectedId, req);
    }
  }, [isNew, selectedId, formTitle, formContent, formDescription, formCategory, formTags, createPrompt, updatePrompt]);

  const handleDelete = useCallback(async () => {
    if (!selectedId) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }

    const success = await deletePrompt(selectedId);
    if (success) {
      setSelectedId(null);
      resetForm();
      setIsNew(false);
      setConfirmDelete(false);
    }
  }, [confirmDelete, selectedId, deletePrompt, resetForm]);

  const handleDuplicate = useCallback(async () => {
    if (!selectedId) return;
    const source = prompts.find((p) => p.id === selectedId);
    if (!source) return;

    const req: PromptCreateRequest = {
      title: t('promptDialog.copyTitle', {
        defaultValue: '{{title}} (Copy)',
        title: source.title,
      }),
      content: source.content,
      description: source.description,
      category: normalizePromptCategory(source.category),
      tags: source.tags,
      is_pinned: false,
    };
    const result = await createPrompt(req);
    if (result) {
      setSelectedId(result.id);
      populateForm(result);
      setIsNew(false);
      setConfirmDelete(false);
    }
  }, [selectedId, prompts, createPrompt, populateForm, t]);
  return (
    <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[720px] h-[580px] z-50',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'flex flex-col overflow-hidden',
            'focus:outline-none',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('promptDialog.title', { defaultValue: 'Manage Prompts' })}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button className="p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800">
                <Cross2Icon className="w-4 h-4" />
              </button>
            </Dialog.Close>
          </div>

          {/* Body */}
          <div className="flex flex-1 min-h-0">
            {/* Left: Prompt list */}
            <div className="w-[200px] border-r border-gray-200 dark:border-gray-700 flex flex-col">
              {/* Category tabs */}
              <div className="flex flex-wrap gap-1 p-2 border-b border-gray-100 dark:border-gray-800">
                {categoryFilters.map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setCategoryFilter(cat)}
                    className={clsx(
                      'px-1.5 py-0.5 text-2xs rounded-md transition-colors',
                      categoryFilter === cat
                        ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                        : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    {cat === 'uncategorized'
                      ? t('promptCategories.uncategorized', { defaultValue: 'Uncategorized' })
                      : categoryLabel(cat)}
                  </button>
                ))}
              </div>

              <div className="p-2">
                <div className="relative mb-2">
                  <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={t('promptDialog.searchPlaceholder', { defaultValue: 'Search prompts...' })}
                    className="w-full pl-7 pr-2 py-1.5 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-primary-500"
                  />
                </div>
                <button
                  onClick={handleNewPrompt}
                  className={clsx(
                    'w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md text-xs',
                    'text-primary-600 dark:text-primary-400',
                    'hover:bg-primary-50 dark:hover:bg-primary-900/20',
                    'transition-colors',
                  )}
                >
                  <PlusIcon className="w-3.5 h-3.5" />
                  {t('promptDialog.newPrompt', { defaultValue: 'New Prompt' })}
                </button>
              </div>

              <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-0.5">
                {filteredPrompts.map((prompt) => (
                  <button
                    key={prompt.id}
                    onClick={() => handleSelectPrompt(prompt)}
                    className={clsx(
                      'w-full text-left px-2 py-1.5 rounded-md text-xs transition-colors',
                      selectedId === prompt.id
                        ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-900 dark:text-primary-100'
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    <div className="flex items-center gap-1">
                      {prompt.is_pinned && <span className="text-amber-500 text-2xs">&#128204;</span>}
                      <span className="truncate font-medium">{prompt.title}</span>
                      {prompt.is_builtin && (
                        <span className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400 shrink-0">
                          {t('promptDialog.builtinBadge', { defaultValue: 'Built-in' })}
                        </span>
                      )}
                    </div>
                  </button>
                ))}
              </div>
            </div>

            {/* Right: Edit form */}
            <div className="flex-1 overflow-y-auto p-4 space-y-3">
              {!selectedId && !isNew ? (
                <div className="h-full flex items-center justify-center text-xs text-gray-400">
                  {t('promptDialog.selectOrCreate', { defaultValue: 'Select or create a prompt' })}
                </div>
              ) : (
                <>
                  {/* Title */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('promptDialog.titleLabel', { defaultValue: 'Title' })}
                    </label>
                    <input
                      type="text"
                      value={formTitle}
                      onChange={(e) => setFormTitle(e.target.value)}
                      disabled={isBuiltinPrompt}
                      placeholder={t('promptDialog.titlePlaceholder', { defaultValue: 'Prompt title' })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                  </div>

                  {/* Category */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('promptDialog.category', { defaultValue: 'Category' })}
                    </label>
                    <input
                      list="prompt-category-options"
                      value={formCategory}
                      onChange={(e) => setFormCategory(e.target.value)}
                      disabled={isBuiltinPrompt}
                      placeholder={t('promptDialog.categoryPlaceholder', {
                        defaultValue: 'Leave empty for uncategorized, or enter a new category',
                      })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                    <datalist id="prompt-category-options">
                      {categorySuggestions.map((cat) => (
                        <option key={cat || '__uncategorized__'} value={cat}>
                          {categoryLabel(cat)}
                        </option>
                      ))}
                    </datalist>
                    <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">
                      {formCategory.trim()
                        ? t('promptDialog.categoryCustomHint', {
                            defaultValue: 'You can type a new category name directly.',
                          })
                        : t('promptDialog.categoryEmptyHint', {
                            defaultValue: 'Leave this empty to keep the prompt uncategorized.',
                          })}
                    </p>
                  </div>

                  {/* Description */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('promptDialog.description', { defaultValue: 'Description' })}
                    </label>
                    <input
                      type="text"
                      value={formDescription}
                      onChange={(e) => setFormDescription(e.target.value)}
                      disabled={isBuiltinPrompt}
                      placeholder={t('promptDialog.descriptionPlaceholder', {
                        defaultValue: 'Brief description (optional)',
                      })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                  </div>

                  {/* Content */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('promptDialog.content', { defaultValue: 'Content' })}
                    </label>
                    <textarea
                      value={formContent}
                      onChange={(e) => setFormContent(e.target.value)}
                      disabled={isBuiltinPrompt}
                      placeholder={t('promptDialog.contentPlaceholder', {
                        defaultValue: 'Write your prompt template...',
                      })}
                      rows={8}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500 resize-none min-h-[200px] font-mono"
                    />
                    <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">
                      {t('promptDialog.variableHint', { defaultValue: 'Use {{variable_name}} as placeholders' })}
                    </p>
                  </div>

                  {/* Variable preview */}
                  {detectedVariables.length > 0 && (
                    <div>
                      <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                        {t('promptDialog.variables', { defaultValue: 'Variables' })}
                      </label>
                      <div className="flex flex-wrap gap-1">
                        {detectedVariables.map((v) => (
                          <span
                            key={v}
                            className="px-1.5 py-0.5 text-2xs rounded-md bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 border border-amber-200 dark:border-amber-800"
                          >
                            {`{{${v}}}`}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Tags */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('promptDialog.tags', { defaultValue: 'Tags' })}
                    </label>
                    <input
                      type="text"
                      value={formTags}
                      onChange={(e) => setFormTags(e.target.value)}
                      disabled={isBuiltinPrompt}
                      placeholder={t('promptDialog.tagsPlaceholder', { defaultValue: 'Comma-separated tags' })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 pt-2 border-t border-gray-200 dark:border-gray-700">
                    <button
                      onClick={handleSave}
                      disabled={!formTitle.trim() || !formContent.trim() || loading || isBuiltinPrompt}
                      className={clsx(
                        'px-4 py-1.5 text-xs font-medium rounded-md transition-colors',
                        'bg-primary-600 text-white hover:bg-primary-700',
                        'disabled:opacity-50 disabled:cursor-not-allowed',
                      )}
                    >
                      {t('promptDialog.save', { defaultValue: 'Save' })}
                    </button>
                    {selectedId && !isNew && (
                      <>
                        {selectedPromptObj?.is_builtin ? (
                          <button
                            onClick={handleDuplicate}
                            className={clsx(
                              'px-3 py-1.5 text-xs font-medium rounded-md transition-colors',
                              'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
                            )}
                          >
                            <CopyIcon className="w-3.5 h-3.5 inline mr-1" />
                            {t('promptDialog.duplicate', { defaultValue: 'Duplicate as Custom' })}
                          </button>
                        ) : (
                          <button
                            onClick={handleDelete}
                            className={clsx(
                              'px-3 py-1.5 text-xs font-medium rounded-md transition-colors',
                              confirmDelete
                                ? 'bg-red-600 text-white hover:bg-red-700'
                                : 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20',
                            )}
                          >
                            <TrashIcon className="w-3.5 h-3.5 inline mr-1" />
                            {confirmDelete
                              ? t('promptDialog.confirmDelete', { defaultValue: 'Click again to confirm' })
                              : t('promptDialog.delete', { defaultValue: 'Delete' })}
                          </button>
                        )}
                      </>
                    )}
                  </div>
                  {isBuiltinPrompt && (
                    <p className="text-2xs text-gray-500 dark:text-gray-400">
                      {t('promptDialog.builtinReadonly', {
                        defaultValue: 'Built-in prompts are localized and read-only. Duplicate one to customize it.',
                      })}
                    </p>
                  )}
                </>
              )}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default PromptDialog;
