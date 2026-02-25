/**
 * ClaudeCodeMode Component
 *
 * Main container for Claude Code mode that assembles chat view,
 * tool history sidebar, input components, and command palette.
 *
 * Story 011-5: Keyboard Shortcuts Implementation
 * Story 011-7: Command Palette with Fuzzy Search
 */

import { useEffect, useState, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ViewVerticalIcon,
  TrashIcon,
  DownloadIcon,
  ReloadIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  DotsHorizontalIcon,
  KeyboardIcon,
  ClockIcon,
  ChatBubbleIcon,
  BookmarkIcon,
} from '@radix-ui/react-icons';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useClaudeCodeStore } from '../../store/claudeCode';
import { useSettingsStore } from '../../store/settings';
import { ChatView } from './ChatView';
import { ChatInput } from './ChatInput';
import { ToolHistorySidebar } from './ToolHistorySidebar';
import { ExportDialog } from './ExportDialog';
import { CommandPaletteProvider, createDefaultCommands, useCommandPalette } from './CommandPalette';
import { ShortcutsHelpDialog, useChatShortcuts } from './KeyboardShortcuts';
import { SessionControlProvider } from './SessionControl';
import { ProjectSelector } from '../shared';

// ============================================================================
// ClaudeCodeMode Component (Inner)
// ============================================================================

function ClaudeCodeModeInner() {
  const { t } = useTranslation('claudeCode');
  const {
    connectionStatus,
    initialize,
    cleanup,
    clearConversation,
    saveConversation,
    loadConversation,
    deleteConversation,
    conversations,
    messages,
    error,
    clearError,
  } = useClaudeCodeStore();

  const { open: openCommandPalette } = useCommandPalette();

  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const [showSidebar, setShowSidebar] = useState(true);
  const [showConversations, setShowConversations] = useState(false);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [showShortcutsDialog, setShowShortcutsDialog] = useState(false);

  // Initialize connection and start a session on mount.
  // Re-runs when workspacePath changes — cleans up old session, starts new one.
  useEffect(() => {
    const projectPath = workspacePath || '.';
    initialize(projectPath);
    return () => {
      cleanup();
    };
  }, [initialize, cleanup, workspacePath]);

  const handleClearChat = useCallback(() => {
    if (messages.length > 0 && confirm(t('chat.clearConfirm'))) {
      clearConversation();
    }
  }, [messages.length, clearConversation, t]);

  const handleExport = useCallback(() => {
    setShowExportDialog(true);
  }, []);

  const handleToggleSidebar = useCallback(() => {
    setShowSidebar((prev) => {
      if (!prev) setShowConversations(false); // mutually exclusive
      return !prev;
    });
  }, []);

  const handleToggleConversations = useCallback(() => {
    setShowConversations((prev) => {
      if (!prev) setShowSidebar(false); // mutually exclusive
      return !prev;
    });
  }, []);

  const handleSaveConversation = useCallback(() => {
    saveConversation();
  }, [saveConversation]);

  const handleLoadConversation = useCallback(
    (id: string) => {
      loadConversation(id);
      setShowConversations(false);
    },
    [loadConversation],
  );

  const handleDeleteConversation = useCallback(
    (id: string) => {
      deleteConversation(id);
    },
    [deleteConversation],
  );

  const handleShowShortcuts = useCallback(() => {
    setShowShortcutsDialog(true);
  }, []);

  // Chat shortcuts using the centralized hook
  useChatShortcuts(
    {
      onClearChat: handleClearChat,
      onExportConversation: handleExport,
      onToggleSidebar: handleToggleSidebar,
      onOpenCommandPalette: openCommandPalette,
    },
    { enabled: true },
  );

  const handleToolClick = useCallback((toolCallId: string) => {
    // Scroll to tool call in chat view
    const element = document.getElementById(`tool-call-${toolCallId}`);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'center' });
      element.classList.add('highlight-flash');
      setTimeout(() => element.classList.remove('highlight-flash'), 1000);
    }
  }, []);

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div
        className={clsx(
          'flex items-center justify-between px-4 py-2 shrink-0',
          'border-b border-gray-200 dark:border-gray-700',
          'bg-white dark:bg-gray-900',
        )}
      >
        <div className="flex items-center gap-3">
          <h2 className="font-semibold text-gray-900 dark:text-white">{t('title')}</h2>
          <ConnectionBadge status={connectionStatus} />
          <ProjectSelector compact />
        </div>

        <div className="flex items-center gap-2">
          {/* Command palette hint */}
          <button
            onClick={openCommandPalette}
            className={clsx(
              'hidden sm:flex items-center gap-2 px-2 py-1 rounded',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'text-xs transition-colors',
            )}
          >
            <span>{t('commandPalette.hint')}</span>
            <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">Ctrl+/</kbd>
          </button>

          {/* Keyboard shortcuts */}
          <button
            onClick={handleShowShortcuts}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              'bg-gray-100 dark:bg-gray-800 text-gray-500',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            title={t('shortcuts.title')}
          >
            <KeyboardIcon className="w-4 h-4" />
          </button>

          {/* Conversations history */}
          <button
            onClick={handleToggleConversations}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              showConversations
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            title="Conversations"
          >
            <ClockIcon className="w-4 h-4" />
          </button>

          {/* Toggle sidebar */}
          <button
            onClick={handleToggleSidebar}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              showSidebar
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            title={t('actions.toggleSidebar')}
          >
            <ViewVerticalIcon className="w-4 h-4" />
          </button>

          {/* More actions dropdown */}
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <button
                className={clsx(
                  'p-2 rounded-lg transition-colors',
                  'bg-gray-100 dark:bg-gray-800 text-gray-500',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                )}
              >
                <DotsHorizontalIcon className="w-4 h-4" />
              </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
              <DropdownMenu.Content
                className={clsx(
                  'min-w-[180px] rounded-lg p-1',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-200 dark:border-gray-700',
                  'shadow-lg',
                  'animate-in fade-in-0 zoom-in-95',
                )}
                sideOffset={5}
                align="end"
              >
                <DropdownMenu.Item
                  onClick={handleSaveConversation}
                  disabled={messages.length === 0}
                  className={clsx(
                    'flex items-center gap-2 px-3 py-2 rounded-md text-sm',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'cursor-pointer outline-none',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                  )}
                >
                  <BookmarkIcon className="w-4 h-4" />
                  Save Conversation
                </DropdownMenu.Item>

                <DropdownMenu.Item
                  onClick={handleExport}
                  className={clsx(
                    'flex items-center gap-2 px-3 py-2 rounded-md text-sm',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'cursor-pointer outline-none',
                  )}
                >
                  <DownloadIcon className="w-4 h-4" />
                  {t('actions.exportConversation')}
                  <span className="ml-auto text-xs text-gray-400">Ctrl+E</span>
                </DropdownMenu.Item>

                <DropdownMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

                <DropdownMenu.Item
                  onClick={handleClearChat}
                  disabled={messages.length === 0}
                  className={clsx(
                    'flex items-center gap-2 px-3 py-2 rounded-md text-sm',
                    'text-red-600 dark:text-red-400',
                    'hover:bg-red-50 dark:hover:bg-red-900/20',
                    'cursor-pointer outline-none',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                  )}
                >
                  <TrashIcon className="w-4 h-4" />
                  {t('actions.clearChat')}
                  <span className="ml-auto text-xs text-gray-400">Ctrl+L</span>
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          className={clsx(
            'flex items-center justify-between px-4 py-2',
            'bg-red-50 dark:bg-red-900/20',
            'border-b border-red-200 dark:border-red-800',
            'text-red-700 dark:text-red-400',
          )}
        >
          <span className="text-sm">{error}</span>
          <button onClick={clearError} className="p-1 rounded hover:bg-red-100 dark:hover:bg-red-900/30">
            <CrossCircledIcon className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden min-h-0">
        {/* Chat area */}
        <div className="flex-1 flex flex-col min-w-0 min-h-0">
          <ChatView />
          <ChatInput onOpenCommandPalette={openCommandPalette} />
        </div>

        {/* Sidebar (tool history) */}
        {showSidebar && (
          <div className="w-72 flex-shrink-0">
            <ToolHistorySidebar onToolClick={handleToolClick} onClose={() => setShowSidebar(false)} />
          </div>
        )}

        {/* Conversations panel */}
        {showConversations && (
          <div className="w-72 flex-shrink-0 border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
              <h3 className="text-sm font-semibold text-gray-900 dark:text-white">Conversations</h3>
              <button
                onClick={() => setShowConversations(false)}
                className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-500"
              >
                <CrossCircledIcon className="w-4 h-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              {conversations.length === 0 ? (
                <div className="p-4 text-center text-sm text-gray-500 dark:text-gray-400">
                  <ChatBubbleIcon className="w-8 h-8 mx-auto mb-2 text-gray-300 dark:text-gray-600" />
                  <p>No saved conversations</p>
                  <p className="text-xs mt-1 text-gray-400">Conversations are auto-saved when cleared</p>
                </div>
              ) : (
                <div className="p-2 space-y-1">
                  {conversations.map((conv) => (
                    <div
                      key={conv.id}
                      className={clsx(
                        'group p-2 rounded-lg cursor-pointer',
                        'hover:bg-gray-100 dark:hover:bg-gray-800',
                        'transition-colors',
                      )}
                      onClick={() => handleLoadConversation(conv.id)}
                    >
                      <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{conv.title}</p>
                      <div className="flex items-center justify-between mt-1">
                        <span className="text-xs text-gray-500 dark:text-gray-400">
                          {conv.messages.length} messages
                        </span>
                        <span className="text-xs text-gray-400 dark:text-gray-500">
                          {new Date(conv.updatedAt).toLocaleDateString(undefined, {
                            month: 'short',
                            day: 'numeric',
                          })}
                        </span>
                      </div>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeleteConversation(conv.id);
                        }}
                        className={clsx(
                          'mt-1 flex items-center gap-1 px-1.5 py-0.5 rounded text-xs',
                          'text-red-500 dark:text-red-400',
                          'hover:bg-red-50 dark:hover:bg-red-900/20',
                          'opacity-0 group-hover:opacity-100 transition-opacity',
                        )}
                      >
                        <TrashIcon className="w-3 h-3" />
                        Delete
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Export dialog */}
      <ExportDialog open={showExportDialog} onOpenChange={setShowExportDialog} />

      {/* Keyboard shortcuts help dialog */}
      <ShortcutsHelpDialog isOpen={showShortcutsDialog} onClose={() => setShowShortcutsDialog(false)} />

      {/* CSS for highlight flash animation */}
      <style>{`
        @keyframes highlight-flash {
          0%, 100% { background-color: transparent; }
          50% { background-color: rgba(var(--primary-500), 0.2); }
        }
        .highlight-flash {
          animation: highlight-flash 0.5s ease-in-out 2;
        }
      `}</style>
    </div>
  );
}

// ============================================================================
// ClaudeCodeMode Component (Outer with Providers)
// ============================================================================

export function ClaudeCodeMode() {
  const { t } = useTranslation('claudeCode');
  const { clearConversation, messages } = useClaudeCodeStore();

  // Create default commands
  const defaultCommands = useMemo(
    () =>
      createDefaultCommands(
        {
          onClearChat: () => {
            if (messages.length > 0 && confirm(t('chat.clearConfirm'))) {
              clearConversation();
            }
          },
          onNewConversation: () => {
            clearConversation();
          },
        },
        t,
      ),
    [clearConversation, messages.length, t],
  );

  return (
    <SessionControlProvider>
      <CommandPaletteProvider defaultCommands={defaultCommands}>
        <ClaudeCodeModeInner />
      </CommandPaletteProvider>
    </SessionControlProvider>
  );
}

// ============================================================================
// ConnectionBadge Component
// ============================================================================

interface ConnectionBadgeProps {
  status: string;
}

function ConnectionBadge({ status }: ConnectionBadgeProps) {
  const { t } = useTranslation('claudeCode');

  const config = {
    connected: {
      icon: CheckCircledIcon,
      text: t('connection.connected'),
      bgColor: 'bg-green-100 dark:bg-green-900/50',
      textColor: 'text-green-700 dark:text-green-400',
      dotColor: 'bg-green-500',
    },
    connecting: {
      icon: ReloadIcon,
      text: t('connection.connecting'),
      bgColor: 'bg-blue-100 dark:bg-blue-900/50',
      textColor: 'text-blue-700 dark:text-blue-400',
      dotColor: 'bg-blue-500 animate-pulse',
    },
    reconnecting: {
      icon: ReloadIcon,
      text: t('connection.reconnecting'),
      bgColor: 'bg-yellow-100 dark:bg-yellow-900/50',
      textColor: 'text-yellow-700 dark:text-yellow-400',
      dotColor: 'bg-yellow-500 animate-pulse',
    },
    disconnected: {
      icon: CrossCircledIcon,
      text: t('connection.disconnected'),
      bgColor: 'bg-red-100 dark:bg-red-900/50',
      textColor: 'text-red-700 dark:text-red-400',
      dotColor: 'bg-red-500',
    },
  }[status] || {
    icon: CrossCircledIcon,
    text: status,
    bgColor: 'bg-gray-100 dark:bg-gray-800',
    textColor: 'text-gray-700 dark:text-gray-400',
    dotColor: 'bg-gray-500',
  };

  return (
    <div
      className={clsx(
        'flex items-center gap-1.5 px-2 py-1 rounded-full text-xs font-medium',
        config.bgColor,
        config.textColor,
      )}
    >
      <span className={clsx('w-1.5 h-1.5 rounded-full', config.dotColor)} />
      {config.text}
    </div>
  );
}

export default ClaudeCodeMode;
