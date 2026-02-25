/**
 * MessageActions Component for SimpleMode
 *
 * Hover action buttons on message bubbles for edit, regenerate, rollback, and copy.
 * Appears on hover using CSS opacity transition within a group container.
 *
 * Feature 002: Message Actions (Edit, Regenerate, Rollback)
 */

import { useState, useCallback, useRef, useEffect, memo } from 'react';
import { clsx } from 'clsx';
import {
  CopyIcon,
  ReloadIcon,
  Pencil1Icon,
  CheckIcon,
  Cross2Icon,
  ResetIcon,
  ScissorsIcon,
} from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import type { StreamLine } from '../../store/execution';

// ============================================================================
// Types
// ============================================================================

export interface MessageActionsProps {
  line: StreamLine;
  isUserMessage: boolean;
  isLastTurn: boolean;
  isClaudeCodeBackend: boolean;
  disabled: boolean;
  onEdit: (lineId: number, newContent: string) => void;
  onRegenerate: (lineId: number) => void;
  onRollback: (lineId: number) => void;
  onCopy: (content: string) => void;
  onFork?: (lineId: number) => void;
  onEditStart?: (lineId: number) => void;
  onEditCancel?: () => void;
}

// ============================================================================
// MessageActions (Toolbar overlay)
// ============================================================================

export const MessageActions = memo(function MessageActions({
  line,
  isUserMessage,
  isLastTurn: _isLastTurn,
  isClaudeCodeBackend,
  disabled,
  onRegenerate,
  onRollback,
  onCopy,
  onFork,
  onEditStart,
}: MessageActionsProps) {
  const { t } = useTranslation('simpleMode');
  const [copied, setCopied] = useState(false);

  const isAssistantMessage = line.type === 'text';

  const handleCopy = useCallback(() => {
    onCopy(line.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [line.content, onCopy]);

  const handleRegenerate = useCallback(() => {
    if (!disabled) {
      onRegenerate(line.id);
    }
  }, [disabled, line.id, onRegenerate]);

  const handleRollback = useCallback(() => {
    if (!disabled) {
      onRollback(line.id);
    }
  }, [disabled, line.id, onRollback]);

  const handleFork = useCallback(() => {
    if (!disabled && onFork) {
      onFork(line.id);
    }
  }, [disabled, line.id, onFork]);

  const handleEditStart = useCallback(() => {
    if (!disabled && onEditStart) {
      onEditStart(line.id);
    }
  }, [disabled, line.id, onEditStart]);

  const showContextWarning = isClaudeCodeBackend;

  return (
    <div
      className={clsx(
        'absolute top-1 flex items-center gap-0.5 z-10',
        'opacity-0 group-hover:opacity-100',
        'transition-opacity duration-150',
        isUserMessage ? 'right-1' : 'left-1',
      )}
      role="toolbar"
      aria-label="Message actions"
    >
      {/* User message actions */}
      {isUserMessage && (
        <>
          <ActionButton
            icon={Pencil1Icon}
            label={t('messageActions.edit')}
            onClick={handleEditStart}
            disabled={disabled}
            warningTooltip={showContextWarning ? t('messageActions.contextWarning') : undefined}
          />
          <ActionButton
            icon={ResetIcon}
            label={t('messageActions.rollback')}
            onClick={handleRollback}
            disabled={disabled}
          />
        </>
      )}

      {/* Assistant message actions */}
      {isAssistantMessage && (
        <>
          <CopyButton
            copied={copied}
            label={copied ? t('messageActions.copied') : t('messageActions.copy')}
            onClick={handleCopy}
          />
          <ActionButton
            icon={ReloadIcon}
            label={t('messageActions.regenerate')}
            onClick={handleRegenerate}
            disabled={disabled}
            warningTooltip={showContextWarning ? t('messageActions.contextWarning') : undefined}
          />
          {onFork && (
            <ActionButton
              icon={ScissorsIcon}
              label={t('messageActions.fork')}
              onClick={handleFork}
              disabled={disabled}
            />
          )}
        </>
      )}
    </div>
  );
});

// ============================================================================
// EditMode Component (replaces message bubble when editing)
// ============================================================================

export interface EditModeProps {
  content: string;
  onSave: (newContent: string) => void;
  onCancel: () => void;
  isClaudeCodeBackend: boolean;
}

export const EditMode = memo(function EditMode({ content, onSave, onCancel, isClaudeCodeBackend }: EditModeProps) {
  const { t } = useTranslation('simpleMode');
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
        if (editedContent.trim()) {
          onSave(editedContent);
        }
      }
    },
    [editedContent, onSave, onCancel],
  );

  const hasChanges = editedContent.trim() !== content.trim();

  return (
    <div className="w-full max-w-[82%] ml-auto space-y-2">
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
        placeholder={t('messageActions.editPlaceholder')}
      />

      {isClaudeCodeBackend && (
        <div className="px-2 py-1 rounded text-2xs bg-amber-50 dark:bg-amber-900/20 text-amber-600 dark:text-amber-400 border border-amber-200 dark:border-amber-800">
          {t('messageActions.contextWarning')}
        </div>
      )}

      <div className="flex items-center justify-end gap-2">
        <span className="text-xs text-gray-500 dark:text-gray-400 mr-auto">Cmd/Ctrl+Enter to save, Esc to cancel</span>

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
          <Cross2Icon className="w-3.5 h-3.5" />
          {t('messageActions.cancel')}
        </button>

        <button
          onClick={() => onSave(editedContent)}
          disabled={!hasChanges || !editedContent.trim()}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded',
            'text-sm font-medium',
            hasChanges && editedContent.trim()
              ? 'bg-primary-600 text-white hover:bg-primary-700'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-400 cursor-not-allowed',
            'transition-colors',
          )}
        >
          <CheckIcon className="w-3.5 h-3.5" />
          {t('messageActions.save')}
        </button>
      </div>
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
  disabled?: boolean;
  warningTooltip?: string;
}

const ActionButton = memo(function ActionButton({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  warningTooltip,
}: ActionButtonProps) {
  return (
    <div className="relative group/action">
      <button
        onClick={(e) => {
          e.stopPropagation();
          onClick();
        }}
        disabled={disabled}
        className={clsx(
          'p-1 rounded transition-colors',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
          'bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm',
          'text-gray-500 dark:text-gray-400',
          !disabled && 'hover:bg-gray-200 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200',
          disabled && 'opacity-50 cursor-not-allowed',
        )}
        title={warningTooltip ? `${label} - ${warningTooltip}` : label}
        aria-label={label}
      >
        <Icon className="w-3.5 h-3.5" />
      </button>
      {warningTooltip && (
        <div
          className={clsx(
            'absolute bottom-full left-1/2 -translate-x-1/2 mb-1',
            'hidden group-hover/action:block',
            'px-2 py-1 rounded text-2xs whitespace-nowrap',
            'bg-amber-100 dark:bg-amber-900/80 text-amber-700 dark:text-amber-300',
            'border border-amber-200 dark:border-amber-800',
            'pointer-events-none',
          )}
        >
          {warningTooltip}
        </div>
      )}
    </div>
  );
});

// ============================================================================
// CopyButton Component
// ============================================================================

interface CopyButtonProps {
  copied: boolean;
  label: string;
  onClick: () => void;
}

const CopyButton = memo(function CopyButton({ copied, label, onClick }: CopyButtonProps) {
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className={clsx(
        'p-1 rounded transition-colors',
        'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
        'bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm',
        copied
          ? 'text-green-600 dark:text-green-400'
          : 'text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200',
      )}
      title={label}
      aria-label={label}
    >
      {copied ? <CheckIcon className="w-3.5 h-3.5" /> : <CopyIcon className="w-3.5 h-3.5" />}
    </button>
  );
});

export default MessageActions;
