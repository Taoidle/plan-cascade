import * as Dialog from '@radix-ui/react-dialog';
import { useTranslation } from 'react-i18next';

interface WorkflowModeSwitchDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
  reason?: string | null;
}

export function WorkflowModeSwitchDialog({ open, onOpenChange, onConfirm, reason }: WorkflowModeSwitchDialogProps) {
  const { t } = useTranslation('simpleMode');

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
          <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {t('workflow.modeSwitchConfirmTitle', { defaultValue: 'Switch workflow mode?' })}
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
            {reason ||
              t('workflow.modeSwitchConfirm', {
                defaultValue:
                  'An execution is still running. Switching modes now may change your active workflow context. Continue?',
              })}
          </Dialog.Description>
          <div className="mt-5 flex items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-800"
            >
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="rounded-md bg-primary-600 px-3 py-1.5 text-sm text-white transition-colors hover:bg-primary-700"
            >
              {t('common.confirm', { defaultValue: 'Confirm' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default WorkflowModeSwitchDialog;
