/**
 * ChatInput Component
 *
 * Multi-line chat input with file attachments, @ references,
 * keyboard shortcuts, and session control.
 *
 * Story 011-3: File Attachment and @ File References
 * Story 011-5: Keyboard Shortcuts Implementation
 * Story 011-6: Session Control (Interrupt, Pause, Resume)
 */

import { useState, useRef, useEffect, useCallback, KeyboardEvent } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  PaperPlaneIcon,
  StopIcon,
  KeyboardIcon,
  PlusIcon,
} from '@radix-ui/react-icons';
import { useClaudeCodeStore, FileAttachment as FileAttachmentData, FileReference } from '../../store/claudeCode';
import { useSettingsStore } from '../../store/settings';
import {
  FileAttachmentDropZone,
  FileReferenceAutocomplete,
  useFileReferences,
} from './FileAttachment';
import { InlineSessionControl } from './SessionControl';
import { useChatShortcuts, KeyboardShortcutHint } from './KeyboardShortcuts';

// ============================================================================
// ChatInput Component
// ============================================================================

interface ChatInputProps {
  onSend?: (message: string, attachments?: FileAttachmentData[], references?: FileReference[]) => void;
  disabled?: boolean;
  onOpenCommandPalette?: () => void;
}

export function ChatInput({
  onSend,
  disabled = false,
  onOpenCommandPalette,
}: ChatInputProps) {
  const { t } = useTranslation('claudeCode');
  const {
    sendMessage,
    isSending,
    isStreaming,
    cancelRequest,
    connectionStatus,
    sessionState,
    pauseStreaming,
    resumeStreaming,
    workspaceFiles,
    messages,
  } = useClaudeCodeStore();

  const { maxFileAttachmentSize } = useSettingsStore();

  const [value, setValue] = useState('');
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [attachments, setAttachments] = useState<FileAttachmentData[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // File references hook
  const {
    references,
    isAutocompleteOpen,
    autocompleteQuery,
    autocompletePosition,
    handleInputChange,
    handleSelectFile,
    closeAutocomplete,
  } = useFileReferences(workspaceFiles);

  const isConnected = connectionStatus === 'connected';
  const isDisabled = disabled || !isConnected || isSending;
  const canSend = value.trim().length > 0 && !isDisabled;
  const isActive = sessionState === 'generating' || sessionState === 'paused';

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

  const handleSend = useCallback(async () => {
    if (!canSend) return;

    const message = value.trim();
    setValue('');
    setAttachments([]);

    if (onSend) {
      onSend(message, attachments, references);
    } else {
      await sendMessage(message);
    }

    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [canSend, value, attachments, references, onSend, sendMessage]);

  const handleCancel = useCallback(() => {
    cancelRequest();
  }, [cancelRequest]);

  const handlePause = useCallback(() => {
    pauseStreaming();
  }, [pauseStreaming]);

  const handleResume = useCallback(() => {
    resumeStreaming();
  }, [resumeStreaming]);

  const handleClearInput = useCallback(() => {
    if (isAutocompleteOpen) {
      closeAutocomplete();
    } else {
      setValue('');
      setAttachments([]);
    }
  }, [isAutocompleteOpen, closeAutocomplete]);

  const handleEditLastMessage = useCallback(() => {
    // Find last user message
    const lastUserMessage = [...messages].reverse().find((m) => m.role === 'user');
    if (lastUserMessage) {
      setValue(lastUserMessage.content);
      textareaRef.current?.focus();
    }
  }, [messages]);

  const focusInput = useCallback(() => {
    textareaRef.current?.focus();
  }, []);

  // Keyboard shortcuts
  useChatShortcuts(
    {
      onSendMessage: handleSend,
      onCancelGeneration: handleCancel,
      onClearInput: handleClearInput,
      onEditLastMessage: handleEditLastMessage,
      onOpenCommandPalette,
      onFocusInput: focusInput,
    },
    {
      enabled: true,
      inputEmpty: value.trim().length === 0,
      isStreaming,
    }
  );

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Ctrl/Cmd + Enter to send
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSend();
      return;
    }

    // Handle @ mention detection
    if (e.key === '@') {
      const cursorPosition = e.currentTarget.selectionStart || 0;
      handleInputChange(value + '@', cursorPosition + 1);
    }

    // Up arrow when input is empty to edit last message
    if (e.key === 'ArrowUp' && value.trim().length === 0) {
      e.preventDefault();
      handleEditLastMessage();
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value;
    setValue(newValue);

    // Check for @ mention
    const cursorPosition = e.target.selectionStart || 0;
    handleInputChange(newValue, cursorPosition);
  };

  const handleFileAttach = useCallback((files: FileAttachmentData[]) => {
    setAttachments((prev) => [...prev, ...files]);
  }, []);

  const handleFileRemove = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const handleFileSelect = useCallback(
    (file: FileReference) => {
      const replacement = handleSelectFile(file);
      // Replace the @query with the selected file reference
      const cursorPosition = textareaRef.current?.selectionStart || value.length;
      const beforeCursor = value.slice(0, cursorPosition);
      const afterCursor = value.slice(cursorPosition);

      // Find the @ symbol position
      const atIndex = beforeCursor.lastIndexOf('@');
      if (atIndex >= 0) {
        const newValue = beforeCursor.slice(0, atIndex) + replacement + afterCursor;
        setValue(newValue);
      }
    },
    [handleSelectFile, value]
  );

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shrink-0">
      {/* File attachment drop zone */}
      <FileAttachmentDropZone
        attachments={attachments}
        onAttach={handleFileAttach}
        onRemove={handleFileRemove}
        maxSize={maxFileAttachmentSize}
        disabled={isDisabled}
      />

      {/* File reference autocomplete */}
      <FileReferenceAutocomplete
        isOpen={isAutocompleteOpen}
        searchQuery={autocompleteQuery}
        files={workspaceFiles}
        onSelect={handleFileSelect}
        onClose={closeAutocomplete}
        position={autocompletePosition}
      />

      {/* Keyboard shortcuts hint */}
      {showShortcuts && (
        <div className="px-4 py-2 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <div className="text-xs text-gray-500 dark:text-gray-400 space-y-1">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-2">
                <KeyboardShortcutHint shortcut="mod+enter" />
                <span>{t('shortcuts.send')}</span>
              </span>
              <span className="flex items-center gap-2">
                <KeyboardShortcutHint shortcut="mod+/" />
                <span>{t('shortcuts.commandPalette')}</span>
              </span>
            </div>
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-2">
                <KeyboardShortcutHint shortcut="escape" />
                <span>{t('shortcuts.cancel')}</span>
              </span>
              <span className="flex items-center gap-2">
                <KeyboardShortcutHint shortcut="up" />
                <span>{t('shortcuts.editLast')}</span>
              </span>
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
        {/* Attach file button */}
        <button
          onClick={() => {
            // Trigger file input
            const input = document.createElement('input');
            input.type = 'file';
            input.multiple = true;
            input.onchange = async (e) => {
              const files = (e.target as HTMLInputElement).files;
              if (files) {
                // Process files - simplified version
                const newAttachments: FileAttachmentData[] = [];
                for (const file of Array.from(files)) {
                  newAttachments.push({
                    id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
                    name: file.name,
                    path: file.name,
                    size: file.size,
                    type: 'text',
                  });
                }
                handleFileAttach(newAttachments);
              }
            };
            input.click();
          }}
          disabled={isDisabled}
          className={clsx(
            'p-3 rounded-lg transition-colors',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
          title={t('input.attachFile')}
        >
          <PlusIcon className="w-5 h-5" />
        </button>

        <div className="flex-1 relative">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
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

          {/* Session control or Send button */}
          {isActive ? (
            <InlineSessionControl
              state={sessionState}
              onStop={handleCancel}
              onPause={handlePause}
              onResume={handleResume}
            />
          ) : isStreaming || isSending ? (
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
