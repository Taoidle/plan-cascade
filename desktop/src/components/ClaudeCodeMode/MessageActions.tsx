/**
 * MessageActions Component
 *
 * Action buttons for chat messages including copy, regenerate,
 * edit & resend functionality. Supports keyboard navigation.
 *
 * Story 011-4: Message Actions (Copy, Regenerate, Edit & Resend)
 */

import { useState, useCallback, memo, useRef, useEffect } from 'react';
import { clsx } from 'clsx';
import { CopyIcon, ReloadIcon, Pencil1Icon, CheckIcon, Cross2Icon, Share1Icon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import { Message } from '../../store/claudeCode';

// ============================================================================
// Types
// ============================================================================

interface MessageActionsProps {
  message: Message;
  onCopy: () => Promise<void>;
  onRegenerate?: () => Promise<void>;
  onEdit?: (newContent: string) => Promise<void>;
  onFork?: () => void;
  isRegenerating?: boolean;
  className?: string;
}

interface EditModeProps {
  content: string;
  onSave: (newContent: string) => void;
  onCancel: () => void;
}

// ============================================================================
// MessageActions Component
// ============================================================================

export const MessageActions = memo(function MessageActions({
  message,
  onCopy,
  onRegenerate,
  onEdit,
  onFork,
  isRegenerating = false,
  className,
}: MessageActionsProps) {
  const { t } = useTranslation('claudeCode');
  const [copied, setCopied] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [actionFeedback, setActionFeedback] = useState<string | null>(null);

  const isUser = message.role === 'user';
  const isAssistant = message.role === 'assistant';

  const handleCopy = useCallback(async () => {
    try {
      await onCopy();
      setCopied(true);
      setActionFeedback('Copied!');
      setTimeout(() => {
        setCopied(false);
        setActionFeedback(null);
      }, 2000);
    } catch (err) {
      setActionFeedback('Failed to copy');
      setTimeout(() => setActionFeedback(null), 2000);
    }
  }, [onCopy]);

  const handleRegenerate = useCallback(async () => {
    if (onRegenerate) {
      try {
        await onRegenerate();
        setActionFeedback('Regenerating...');
      } catch (err) {
        setActionFeedback('Failed to regenerate');
        setTimeout(() => setActionFeedback(null), 2000);
      }
    }
  }, [onRegenerate]);

  const handleEdit = useCallback(() => {
    setIsEditing(true);
  }, []);

  const handleEditSave = useCallback(
    async (newContent: string) => {
      if (onEdit) {
        try {
          await onEdit(newContent);
          setIsEditing(false);
          setActionFeedback('Message updated');
          setTimeout(() => setActionFeedback(null), 2000);
        } catch (err) {
          setActionFeedback('Failed to update');
          setTimeout(() => setActionFeedback(null), 2000);
        }
      }
    },
    [onEdit],
  );

  const handleEditCancel = useCallback(() => {
    setIsEditing(false);
  }, []);

  if (isEditing && isUser) {
    return <EditMode content={message.content} onSave={handleEditSave} onCancel={handleEditCancel} />;
  }

  return (
    <div
      className={clsx(
        'flex items-center gap-1',
        'opacity-0 group-hover:opacity-100',
        'transition-opacity duration-150',
        className,
      )}
      role="toolbar"
      aria-label="Message actions"
    >
      {/* Copy button - always visible */}
      <ActionButton
        icon={copied ? CheckIcon : CopyIcon}
        label={copied ? t('messageActions.copied') : t('messageActions.copy')}
        onClick={handleCopy}
        isSuccess={copied}
      />

      {/* Regenerate button - for assistant messages */}
      {isAssistant && onRegenerate && (
        <ActionButton
          icon={ReloadIcon}
          label={t('messageActions.regenerate')}
          onClick={handleRegenerate}
          isLoading={isRegenerating}
          disabled={isRegenerating}
        />
      )}

      {/* Edit button - for user messages */}
      {isUser && onEdit && <ActionButton icon={Pencil1Icon} label={t('messageActions.edit')} onClick={handleEdit} />}

      {/* Fork/Branch button */}
      {onFork && <ActionButton icon={Share1Icon} label={t('messageActions.fork')} onClick={onFork} />}

      {/* Action feedback */}
      {actionFeedback && <span className="text-xs text-gray-500 dark:text-gray-400 ml-2">{actionFeedback}</span>}
    </div>
  );
});

// ============================================================================
// ActionButton Component
// ============================================================================

interface ActionButtonProps {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  onClick: () => void;
  isSuccess?: boolean;
  isLoading?: boolean;
  disabled?: boolean;
}

const ActionButton = memo(function ActionButton({
  icon: Icon,
  label,
  onClick,
  isSuccess = false,
  isLoading = false,
  disabled = false,
}: ActionButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={clsx(
        'p-1.5 rounded transition-colors',
        'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
        isSuccess
          ? 'bg-green-100 dark:bg-green-900/50 text-green-600 dark:text-green-400'
          : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400',
        !disabled && !isSuccess && 'hover:bg-gray-200 dark:hover:bg-gray-700',
        disabled && 'opacity-50 cursor-not-allowed',
      )}
      title={label}
      aria-label={label}
    >
      <Icon className={clsx('w-4 h-4', isLoading && 'animate-spin')} />
    </button>
  );
});

// ============================================================================
// EditMode Component
// ============================================================================

const EditMode = memo(function EditMode({ content, onSave, onCancel }: EditModeProps) {
  const { t } = useTranslation('claudeCode');
  const [editedContent, setEditedContent] = useState(content);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Focus and select all on mount
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.focus();
      textareaRef.current.select();
    }
  }, []);

  // Handle keyboard shortcuts
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
      } else if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        onSave(editedContent);
      }
    },
    [editedContent, onSave, onCancel],
  );

  const hasChanges = editedContent !== content;

  return (
    <div className="w-full space-y-2">
      <textarea
        ref={textareaRef}
        value={editedContent}
        onChange={(e) => setEditedContent(e.target.value)}
        onKeyDown={handleKeyDown}
        className={clsx(
          'w-full px-3 py-2 rounded-lg resize-none',
          'bg-white dark:bg-gray-800',
          'border border-primary-300 dark:border-primary-700',
          'focus:border-primary-500 focus:ring-1 focus:ring-primary-500',
          'text-gray-900 dark:text-white',
          'text-sm',
        )}
        rows={Math.min(10, editedContent.split('\n').length + 1)}
      />

      <div className="flex items-center justify-end gap-2">
        <span className="text-xs text-gray-500 dark:text-gray-400 mr-auto">{t('messageActions.editHint')}</span>

        <button
          onClick={onCancel}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded',
            'text-sm font-medium',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'transition-colors',
          )}
        >
          <Cross2Icon className="w-4 h-4" />
          {t('messageActions.cancel')}
        </button>

        <button
          onClick={() => onSave(editedContent)}
          disabled={!hasChanges}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded',
            'text-sm font-medium',
            hasChanges
              ? 'bg-primary-600 text-white hover:bg-primary-700'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-400 cursor-not-allowed',
            'transition-colors',
          )}
        >
          <CheckIcon className="w-4 h-4" />
          {t('messageActions.saveAndResend')}
        </button>
      </div>
    </div>
  );
});

// ============================================================================
// useMessageActions Hook
// ============================================================================

export interface UseMessageActionsResult {
  copyMessage: (message: Message) => Promise<void>;
  regenerateMessage: (messageId: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  forkConversation: (messageId: string) => void;
  isRegenerating: boolean;
  regeneratingMessageId: string | null;
}

export function useMessageActions(
  messages: Message[],
  onSendMessage: (content: string) => Promise<void>,
  updateMessages: (messages: Message[]) => void,
): UseMessageActionsResult {
  const [isRegenerating, setIsRegenerating] = useState(false);
  const [regeneratingMessageId, setRegeneratingMessageId] = useState<string | null>(null);

  const copyMessage = useCallback(async (message: Message) => {
    await navigator.clipboard.writeText(message.content);
  }, []);

  const regenerateMessage = useCallback(
    async (messageId: string) => {
      // Find the message and all previous messages
      const messageIndex = messages.findIndex((m) => m.id === messageId);
      if (messageIndex === -1) return;

      // Find the last user message before this assistant message
      let lastUserMessage: Message | null = null;
      for (let i = messageIndex - 1; i >= 0; i--) {
        if (messages[i].role === 'user') {
          lastUserMessage = messages[i];
          break;
        }
      }

      if (!lastUserMessage) return;

      setIsRegenerating(true);
      setRegeneratingMessageId(messageId);

      try {
        // Remove the current assistant message and resend
        const newMessages = messages.slice(0, messageIndex);
        updateMessages(newMessages);
        await onSendMessage(lastUserMessage.content);
      } finally {
        setIsRegenerating(false);
        setRegeneratingMessageId(null);
      }
    },
    [messages, onSendMessage, updateMessages],
  );

  const editMessage = useCallback(
    async (messageId: string, newContent: string) => {
      const messageIndex = messages.findIndex((m) => m.id === messageId);
      if (messageIndex === -1) return;

      // Keep messages up to and including the edited message
      // but replace the content
      const newMessages = messages
        .slice(0, messageIndex + 1)
        .map((m) => (m.id === messageId ? { ...m, content: newContent } : m));

      // Remove all messages after the edited one
      updateMessages(newMessages);

      // Resend to get new response
      await onSendMessage(newContent);
    },
    [messages, onSendMessage, updateMessages],
  );

  const forkConversation = useCallback(
    (messageId: string) => {
      const messageIndex = messages.findIndex((m) => m.id === messageId);
      if (messageIndex === -1) return;

      // Create a new conversation branch with messages up to this point
      // This would typically save to a new conversation
      const branchMessages = messages.slice(0, messageIndex + 1);

      // TODO: Implement conversation branching in store
      console.log('Fork conversation at message:', messageId, branchMessages);
    },
    [messages],
  );

  return {
    copyMessage,
    regenerateMessage,
    editMessage,
    forkConversation,
    isRegenerating,
    regeneratingMessageId,
  };
}

export default MessageActions;
