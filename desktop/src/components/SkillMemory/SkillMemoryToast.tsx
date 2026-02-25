/**
 * SkillMemoryToast Component
 *
 * Toast notification for skill/memory events.
 * Auto-dismisses after 3 seconds.
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { CheckCircledIcon, CrossCircledIcon, InfoCircledIcon, Cross2Icon } from '@radix-ui/react-icons';
import { useSkillMemoryStore } from '../../store/skillMemory';

const TOAST_DURATION = 3000;

const iconMap = {
  success: <CheckCircledIcon className="w-4 h-4 text-green-500" />,
  error: <CrossCircledIcon className="w-4 h-4 text-red-500" />,
  info: <InfoCircledIcon className="w-4 h-4 text-blue-500" />,
};

const bgMap = {
  success: 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800',
  error: 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800',
  info: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800',
};

export function SkillMemoryToast() {
  const toastMessage = useSkillMemoryStore((s) => s.toastMessage);
  const toastType = useSkillMemoryStore((s) => s.toastType);
  const clearToast = useSkillMemoryStore((s) => s.clearToast);

  useEffect(() => {
    if (toastMessage) {
      const timer = setTimeout(clearToast, TOAST_DURATION);
      return () => clearTimeout(timer);
    }
  }, [toastMessage, clearToast]);

  if (!toastMessage) return null;

  return (
    <div
      data-testid="skill-memory-toast"
      className={clsx(
        'fixed bottom-4 right-4 z-[60]',
        'flex items-center gap-2 px-4 py-2.5 rounded-lg shadow-lg border',
        'animate-[slideUp_0.3s_ease-out]',
        bgMap[toastType],
      )}
    >
      {iconMap[toastType]}
      <span className="text-xs text-gray-700 dark:text-gray-300">{toastMessage}</span>
      <button
        onClick={clearToast}
        className="p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 ml-1"
      >
        <Cross2Icon className="w-3 h-3" />
      </button>
    </div>
  );
}

export default SkillMemoryToast;
