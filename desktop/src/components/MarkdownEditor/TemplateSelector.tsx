/**
 * TemplateSelector Component
 *
 * Provides a dropdown to select and insert CLAUDE.md templates.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import { FileTextIcon, Cross2Icon, CheckIcon } from '@radix-ui/react-icons';
import { claudeMdTemplates, type ClaudeMdTemplate } from '../../data/claudeMdTemplates';

interface TemplateSelectorProps {
  onSelect: (content: string) => void;
  disabled?: boolean;
}

export function TemplateSelector({ onSelect, disabled }: TemplateSelectorProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [selectedTemplate, setSelectedTemplate] = useState<ClaudeMdTemplate | null>(null);

  const handleSelect = () => {
    if (selectedTemplate) {
      onSelect(selectedTemplate.content);
      setIsOpen(false);
      setSelectedTemplate(null);
    }
  };

  const getTemplateDisplayName = (template: ClaudeMdTemplate) => {
    // Use type assertion to allow dynamic keys
    const key = `markdownEditor.templates.${template.id}`;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const translated = (t as any)(key);
    // Fall back to template name if translation not found
    return String(translated) === key ? template.name : String(translated);
  };

  return (
    <Dialog.Root open={isOpen} onOpenChange={setIsOpen}>
      <Dialog.Trigger asChild>
        <button
          disabled={disabled}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
            'text-sm font-medium',
            'bg-gray-100 dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
        >
          <FileTextIcon className="w-4 h-4" />
          {t('markdownEditor.templates.title')}
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-2xl max-h-[85vh]',
            'bg-white dark:bg-gray-900',
            'rounded-lg shadow-xl',
            'flex flex-col',
            'z-50',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {t('markdownEditor.templates.selectTemplate')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors">
                <Cross2Icon className="w-5 h-5 text-gray-500" />
              </button>
            </Dialog.Close>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-hidden flex">
            {/* Template List */}
            <div className="w-1/3 border-r border-gray-200 dark:border-gray-700 overflow-y-auto">
              <div className="p-2 space-y-1">
                {claudeMdTemplates.map((template) => (
                  <button
                    key={template.id}
                    onClick={() => setSelectedTemplate(template)}
                    className={clsx(
                      'w-full text-left px-3 py-2 rounded-md',
                      'transition-colors',
                      selectedTemplate?.id === template.id
                        ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300'
                        : 'hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300',
                    )}
                  >
                    <div className="font-medium text-sm">{getTemplateDisplayName(template)}</div>
                    <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 line-clamp-2">
                      {template.description}
                    </div>
                  </button>
                ))}
              </div>
            </div>

            {/* Preview */}
            <div className="flex-1 overflow-y-auto p-4">
              {selectedTemplate ? (
                <div>
                  <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-2">
                    {t('markdownEditor.preview.title')}
                  </h3>
                  <pre className="text-xs text-gray-600 dark:text-gray-400 whitespace-pre-wrap font-mono bg-gray-50 dark:bg-gray-800 p-3 rounded-lg overflow-x-auto">
                    {selectedTemplate.content}
                  </pre>
                </div>
              ) : (
                <div className="h-full flex items-center justify-center text-gray-400">
                  <p className="text-sm">{t('markdownEditor.templates.selectTemplate')}</p>
                </div>
              )}
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-200 dark:border-gray-700">
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'px-4 py-2 rounded-md',
                  'text-sm font-medium',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'transition-colors',
                )}
              >
                {t('buttons.cancel')}
              </button>
            </Dialog.Close>
            <button
              onClick={handleSelect}
              disabled={!selectedTemplate}
              className={clsx(
                'flex items-center gap-1.5 px-4 py-2 rounded-md',
                'text-sm font-medium',
                'bg-primary-600 dark:bg-primary-500',
                'text-white',
                'hover:bg-primary-700 dark:hover:bg-primary-600',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors',
              )}
            >
              <CheckIcon className="w-4 h-4" />
              {String(t('markdownEditor.templates.insert'))}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default TemplateSelector;
