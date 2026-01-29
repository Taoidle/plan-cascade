/**
 * ClaudeCodeMode Component
 *
 * Main container for Claude Code mode that assembles chat view,
 * tool history sidebar, and input components.
 */

import { useEffect, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  SidebarIcon,
  TrashIcon,
  DownloadIcon,
  ReloadIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  DotsHorizontalIcon,
} from '@radix-ui/react-icons';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useClaudeCodeStore } from '../../store/claudeCode';
import { ChatView } from './ChatView';
import { ChatInput } from './ChatInput';
import { ToolHistorySidebar } from './ToolHistorySidebar';
import { ExportDialog } from './ExportDialog';

// ============================================================================
// ClaudeCodeMode Component
// ============================================================================

export function ClaudeCodeMode() {
  const {
    connectionStatus,
    initialize,
    cleanup,
    clearConversation,
    messages,
    error,
    clearError,
  } = useClaudeCodeStore();

  const [showSidebar, setShowSidebar] = useState(true);
  const [showExportDialog, setShowExportDialog] = useState(false);

  // Initialize WebSocket connection on mount
  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl/Cmd + L to clear chat
      if ((e.ctrlKey || e.metaKey) && e.key === 'l') {
        e.preventDefault();
        if (messages.length > 0 && confirm('Clear the conversation?')) {
          clearConversation();
        }
      }

      // Ctrl/Cmd + E to export
      if ((e.ctrlKey || e.metaKey) && e.key === 'e') {
        e.preventDefault();
        setShowExportDialog(true);
      }

      // Ctrl/Cmd + B to toggle sidebar
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault();
        setShowSidebar((prev) => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [messages.length, clearConversation]);

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
          'flex items-center justify-between px-4 py-2',
          'border-b border-gray-200 dark:border-gray-700',
          'bg-white dark:bg-gray-900'
        )}
      >
        <div className="flex items-center gap-3">
          <h2 className="font-semibold text-gray-900 dark:text-white">
            Claude Code
          </h2>
          <ConnectionBadge status={connectionStatus} />
        </div>

        <div className="flex items-center gap-2">
          {/* Toggle sidebar */}
          <button
            onClick={() => setShowSidebar(!showSidebar)}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              showSidebar
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title="Toggle tool history (Ctrl+B)"
          >
            <SidebarIcon className="w-4 h-4" />
          </button>

          {/* More actions dropdown */}
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <button
                className={clsx(
                  'p-2 rounded-lg transition-colors',
                  'bg-gray-100 dark:bg-gray-800 text-gray-500',
                  'hover:bg-gray-200 dark:hover:bg-gray-700'
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
                  'animate-in fade-in-0 zoom-in-95'
                )}
                sideOffset={5}
                align="end"
              >
                <DropdownMenu.Item
                  onClick={() => setShowExportDialog(true)}
                  className={clsx(
                    'flex items-center gap-2 px-3 py-2 rounded-md text-sm',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'cursor-pointer outline-none'
                  )}
                >
                  <DownloadIcon className="w-4 h-4" />
                  Export Conversation
                  <span className="ml-auto text-xs text-gray-400">Ctrl+E</span>
                </DropdownMenu.Item>

                <DropdownMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

                <DropdownMenu.Item
                  onClick={() => {
                    if (messages.length > 0 && confirm('Clear the conversation?')) {
                      clearConversation();
                    }
                  }}
                  disabled={messages.length === 0}
                  className={clsx(
                    'flex items-center gap-2 px-3 py-2 rounded-md text-sm',
                    'text-red-600 dark:text-red-400',
                    'hover:bg-red-50 dark:hover:bg-red-900/20',
                    'cursor-pointer outline-none',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  <TrashIcon className="w-4 h-4" />
                  Clear Chat
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
            'text-red-700 dark:text-red-400'
          )}
        >
          <span className="text-sm">{error}</span>
          <button
            onClick={clearError}
            className="p-1 rounded hover:bg-red-100 dark:hover:bg-red-900/30"
          >
            <CrossCircledIcon className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Chat area */}
        <div className="flex-1 flex flex-col min-w-0">
          <ChatView />
          <ChatInput />
        </div>

        {/* Sidebar */}
        {showSidebar && (
          <div className="w-72 flex-shrink-0">
            <ToolHistorySidebar
              onToolClick={handleToolClick}
              onClose={() => setShowSidebar(false)}
            />
          </div>
        )}
      </div>

      {/* Export dialog */}
      <ExportDialog open={showExportDialog} onOpenChange={setShowExportDialog} />

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
// ConnectionBadge Component
// ============================================================================

interface ConnectionBadgeProps {
  status: string;
}

function ConnectionBadge({ status }: ConnectionBadgeProps) {
  const config = {
    connected: {
      icon: CheckCircledIcon,
      text: 'Connected',
      bgColor: 'bg-green-100 dark:bg-green-900/50',
      textColor: 'text-green-700 dark:text-green-400',
      dotColor: 'bg-green-500',
    },
    connecting: {
      icon: ReloadIcon,
      text: 'Connecting',
      bgColor: 'bg-blue-100 dark:bg-blue-900/50',
      textColor: 'text-blue-700 dark:text-blue-400',
      dotColor: 'bg-blue-500 animate-pulse',
    },
    reconnecting: {
      icon: ReloadIcon,
      text: 'Reconnecting',
      bgColor: 'bg-yellow-100 dark:bg-yellow-900/50',
      textColor: 'text-yellow-700 dark:text-yellow-400',
      dotColor: 'bg-yellow-500 animate-pulse',
    },
    disconnected: {
      icon: CrossCircledIcon,
      text: 'Disconnected',
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
        config.textColor
      )}
    >
      <span className={clsx('w-1.5 h-1.5 rounded-full', config.dotColor)} />
      {config.text}
    </div>
  );
}

export default ClaudeCodeMode;
