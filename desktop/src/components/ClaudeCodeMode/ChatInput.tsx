/**
 * ChatInput Component
 *
 * Multi-line chat input with send button and keyboard shortcuts.
 */

import { useState, useRef, useEffect, KeyboardEvent } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  PaperPlaneIcon,
  StopIcon,
  KeyboardIcon,
} from '@radix-ui/react-icons';
import { useClaudeCodeStore } from '../../store/claudeCode';

// ============================================================================
// ChatInput Component
// ============================================================================

interface ChatInputProps {
  onSend?: (message: string) => void;
  disabled?: boolean;
}

export function ChatInput({ onSend, disabled = false }: ChatInputProps) {
  const { t } = useTranslation('claudeCode');
  const {
    sendMessage,
    isSending,
    isStreaming,
    cancelRequest,
    connectionStatus,
  } = useClaudeCodeStore();

  const [value, setValue] = useState('');
  const [showShortcuts, setShowShortcuts] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const isConnected = connectionStatus === 'connected';
  const isDisabled = disabled || !isConnected || isSending;
  const canSend = value.trim().length > 0 && !isDisabled;

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 200)}px`;
    }
  }, [value]);

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSend = async () => {
    if (!canSend) return;

    const message = value.trim();
    setValue('');

    if (onSend) {
      onSend(message);
    } else {
      await sendMessage(message);
    }

    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Ctrl/Cmd + Enter to send
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSend();
      return;
    }

    // Enter without modifier for newline (default behavior)
    // Shift + Enter also for newline (default behavior)
  };

  const handleCancel = () => {
    cancelRequest();
  };

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Keyboard shortcuts hint */}
      {showShortcuts && (
        <div className="px-4 py-2 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <div className="text-xs text-gray-500 dark:text-gray-400 space-y-1">
            <div className="flex items-center gap-2">
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl</kbd>
              <span>+</span>
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">Enter</kbd>
              <span>{t('shortcuts.send')}</span>
            </div>
            <div className="flex items-center gap-2">
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl</kbd>
              <span>+</span>
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">L</kbd>
              <span>{t('shortcuts.clear')}</span>
            </div>
            <div className="flex items-center gap-2">
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl</kbd>
              <span>+</span>
              <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">E</kbd>
              <span>{t('shortcuts.export')}</span>
            </div>
          </div>
        </div>
      )}

      {/* Connection status warning */}
      {!isConnected && (
        <div className="px-4 py-2 bg-yellow-50 dark:bg-yellow-900/20 text-yellow-700 dark:text-yellow-400 text-sm">
          {connectionStatus === 'connecting' && t('connection.connecting')}
          {connectionStatus === 'reconnecting' && t('connection.reconnecting')}
          {connectionStatus === 'disconnected' && t('connection.disconnected')}
        </div>
      )}

      {/* Input area */}
      <div className="flex items-end gap-2 p-4">
        <div className="flex-1 relative">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={isConnected ? t('input.placeholder') : t('input.placeholderDisconnected')}
            disabled={isDisabled}
            rows={1}
            className={clsx(
              'w-full px-4 py-3 rounded-lg resize-none',
              'bg-gray-50 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'focus:border-primary-500 focus:ring-1 focus:ring-primary-500',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'text-gray-900 dark:text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
            style={{ minHeight: '48px', maxHeight: '200px' }}
          />
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-2">
          {/* Keyboard shortcuts toggle */}
          <button
            onClick={() => setShowShortcuts(!showShortcuts)}
            className={clsx(
              'p-3 rounded-lg transition-colors',
              showShortcuts
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title={t('shortcuts.title')}
          >
            <KeyboardIcon className="w-5 h-5" />
          </button>

          {/* Send/Cancel button */}
          {isStreaming || isSending ? (
            <button
              onClick={handleCancel}
              className={clsx(
                'p-3 rounded-lg transition-colors',
                'bg-red-500 text-white hover:bg-red-600'
              )}
              title={t('actions.cancel')}
            >
              <StopIcon className="w-5 h-5" />
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!canSend}
              className={clsx(
                'p-3 rounded-lg transition-colors',
                canSend
                  ? 'bg-primary-600 text-white hover:bg-primary-700'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500 cursor-not-allowed'
              )}
              title={t('input.sendTitle')}
            >
              <PaperPlaneIcon className="w-5 h-5" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export default ChatInput;
