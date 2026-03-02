import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';

interface DeleteConfirmDialogProps {
  isOpen: boolean;
  collectionName: string;
  onClose: () => void;
  onConfirm: () => void;
  isLoading: boolean;
}

export function DeleteConfirmDialog({
  isOpen,
  collectionName,
  onClose,
  onConfirm,
  isLoading,
}: DeleteConfirmDialogProps) {
  const { t } = useTranslation('knowledge');

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        className={clsx(
          'w-full max-w-sm rounded-xl p-6',
          'bg-white dark:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
          'shadow-xl',
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-2">{t('deleteCollection')}</h3>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-6">{t('deleteConfirm', { name: collectionName })}</p>
        <div className="flex justify-end gap-3">
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
            onClick={onConfirm}
            disabled={isLoading}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-red-600 hover:bg-red-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors',
            )}
          >
            {isLoading ? t('deleting') : t('delete')}
          </button>
        </div>
      </div>
    </div>
  );
}
