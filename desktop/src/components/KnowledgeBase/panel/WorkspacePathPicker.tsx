import { useCallback } from 'react';
import { clsx } from 'clsx';

interface WorkspacePathPickerProps {
  value: string;
  onChange: (path: string) => void;
  placeholder: string;
  label: string;
  browseLabel: string;
  clearLabel: string;
}

export function WorkspacePathPicker({
  value,
  onChange,
  placeholder,
  label,
  browseLabel,
  clearLabel,
}: WorkspacePathPickerProps) {
  const handleBrowse = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: label,
        defaultPath: value || undefined,
      });
      if (selected && typeof selected === 'string') {
        onChange(selected);
      }
    } catch (err) {
      console.error('Failed to open directory picker:', err);
    }
  }, [value, onChange, label]);

  return (
    <div>
      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">{label}</label>
      <div className="flex gap-2">
        <input
          type="text"
          value={value}
          readOnly
          placeholder={placeholder}
          onClick={handleBrowse}
          className={clsx(
            'flex-1 min-w-0 px-3 py-2 rounded-lg cursor-pointer',
            'border border-gray-300 dark:border-gray-600',
            'bg-gray-50 dark:bg-gray-700',
            'text-gray-900 dark:text-white',
            'text-sm truncate',
            'hover:border-primary-400 dark:hover:border-primary-500',
            'transition-colors',
          )}
        />
        <button
          type="button"
          onClick={handleBrowse}
          className={clsx(
            'px-3 py-2 rounded-lg text-sm font-medium whitespace-nowrap',
            'border border-gray-300 dark:border-gray-600',
            'bg-white dark:bg-gray-700',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-50 dark:hover:bg-gray-600',
            'transition-colors',
          )}
        >
          {browseLabel}
        </button>
        {value && (
          <button
            type="button"
            onClick={() => onChange('')}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 p-2"
            title={clearLabel}
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
