/**
 * InputBox Component
 *
 * Text input with submit button for task descriptions.
 * Supports multiline input and keyboard shortcuts.
 */

import { clsx } from 'clsx';
import { KeyboardEvent, useRef } from 'react';
import { PaperPlaneIcon } from '@radix-ui/react-icons';

interface InputBoxProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  placeholder?: string;
}

export function InputBox({
  value,
  onChange,
  onSubmit,
  disabled = false,
  placeholder = 'Describe your task...',
}: InputBoxProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Submit on Cmd/Ctrl + Enter
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      onSubmit();
    }
  };

  // Auto-resize textarea
  const handleInput = () => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
    }
  };

  return (
    <div
      className={clsx(
        'relative flex items-end gap-2 p-4 rounded-xl',
        'bg-white dark:bg-gray-800',
        'border-2 border-gray-200 dark:border-gray-700',
        'focus-within:border-primary-500 dark:focus-within:border-primary-500',
        'shadow-sm transition-all',
        disabled && 'opacity-60 cursor-not-allowed'
      )}
    >
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          handleInput();
        }}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        placeholder={placeholder}
        rows={1}
        className={clsx(
          'flex-1 resize-none bg-transparent',
          'text-gray-900 dark:text-white',
          'placeholder-gray-400 dark:placeholder-gray-500',
          'focus:outline-none',
          'text-base leading-relaxed',
          disabled && 'cursor-not-allowed'
        )}
      />

      <button
        onClick={onSubmit}
        disabled={disabled || !value.trim()}
        className={clsx(
          'flex items-center justify-center',
          'w-10 h-10 rounded-lg',
          'bg-primary-600 text-white',
          'hover:bg-primary-700',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'transition-colors'
        )}
        title="Submit (Cmd+Enter)"
      >
        <PaperPlaneIcon className="w-5 h-5" />
      </button>
    </div>
  );
}

export default InputBox;
