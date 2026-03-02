import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { WorkspacePathPicker } from './WorkspacePathPicker';

interface CreateCollectionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (name: string, description: string, workspacePath?: string) => void;
  isLoading: boolean;
}

export function CreateCollectionDialog({ isOpen, onClose, onSubmit, isLoading }: CreateCollectionDialogProps) {
  const { t } = useTranslation('knowledge');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [workspacePath, setWorkspacePath] = useState('');

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        className={clsx(
          'w-full max-w-md rounded-xl p-6',
          'bg-white dark:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
          'shadow-xl',
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">{t('createCollection')}</h3>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('collectionName')}
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('collectionNamePlaceholder')}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-700',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                'text-sm',
              )}
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('description')}
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('descriptionPlaceholder')}
              rows={3}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-700',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                'text-sm resize-none',
              )}
            />
          </div>
          <WorkspacePathPicker
            value={workspacePath}
            onChange={setWorkspacePath}
            placeholder={t('workspacePathPlaceholder')}
            label={t('workspacePath')}
            browseLabel={t('browse')}
            clearLabel={t('clearPath')}
          />
        </div>
        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={onClose}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors',
            )}
          >
            {t('cancel', { ns: 'common' })}
          </button>
          <button
            onClick={() => {
              if (name.trim()) {
                onSubmit(name.trim(), description.trim(), workspacePath.trim() || undefined);
              }
            }}
            disabled={!name.trim() || isLoading}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 hover:bg-primary-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors',
            )}
          >
            {isLoading ? t('creating') : t('create')}
          </button>
        </div>
      </div>
    </div>
  );
}
