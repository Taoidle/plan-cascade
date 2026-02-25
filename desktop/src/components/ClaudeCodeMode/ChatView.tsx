/**
 * ChatView Component
 *
 * Displays conversation messages with user/assistant bubbles.
 * Supports enhanced markdown rendering, message actions, and auto-scrolling.
 *
 * Story 011-1: Enhanced Markdown Rendering with Syntax Highlighting
 * Story 011-4: Message Actions (Copy, Regenerate, Edit & Resend)
 */

import { useEffect, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useClaudeCodeStore, Message } from '../../store/claudeCode';
import { ToolCallCard } from './ToolCallCard';
import { MarkdownRenderer } from './MarkdownRenderer';
import { MessageActions, useMessageActions } from './MessageActions';
import { SessionStateIndicator } from './SessionControl';
import { FileChip } from './FileAttachment';

// ============================================================================
// ChatView Component
// ============================================================================

export function ChatView() {
  const { t } = useTranslation('claudeCode');
  const { messages, isStreaming, streamingContent, sessionState, sendMessage, updateMessages } = useClaudeCodeStore();

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDarkMode, setIsDarkMode] = useState(true);

  // Detect dark mode
  useEffect(() => {
    const checkDarkMode = () => {
      setIsDarkMode(document.documentElement.classList.contains('dark'));
    };
    checkDarkMode();

    const observer = new MutationObserver(checkDarkMode);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });

    return () => observer.disconnect();
  }, []);

  // Message actions hook
  const { copyMessage, regenerateMessage, editMessage, forkConversation, regeneratingMessageId } = useMessageActions(
    messages,
    sendMessage,
    updateMessages,
  );

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, streamingContent]);

  if (messages.length === 0 && !isStreaming) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-center p-8">
        <div className="text-6xl mb-4">
          <span role="img" aria-label="robot">
            &#129302;
          </span>
        </div>
        <h2 className="text-xl font-semibold text-gray-700 dark:text-gray-300 mb-2">{t('chat.empty.title')}</h2>
        <p className="text-gray-500 dark:text-gray-400 max-w-md">{t('chat.empty.description')}</p>
        <div className="mt-6 text-sm text-gray-400 dark:text-gray-500">
          <p>{t('chat.empty.tryAsking')}</p>
          <ul className="mt-2 space-y-1">
            <li>"{t('chat.empty.examples.structure')}"</li>
            <li>"{t('chat.empty.examples.readme')}"</li>
            <li>"{t('chat.empty.examples.component')}"</li>
          </ul>
        </div>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="flex-1 overflow-auto p-4 space-y-4">
      {messages.map((message) => (
        <MessageBubble
          key={message.id}
          message={message}
          isDarkMode={isDarkMode}
          onCopy={() => copyMessage(message)}
          onRegenerate={message.role === 'assistant' ? () => regenerateMessage(message.id) : undefined}
          onEdit={message.role === 'user' ? (newContent) => editMessage(message.id, newContent) : undefined}
          onFork={() => forkConversation(message.id)}
          isRegenerating={regeneratingMessageId === message.id}
        />
      ))}

      {/* Streaming message indicator */}
      {isStreaming && (
        <div className="flex gap-3">
          <div className="flex-shrink-0 w-8 h-8 rounded-full bg-primary-600 flex items-center justify-center">
            <span className="text-white text-sm font-medium">C</span>
          </div>
          <div
            className={clsx(
              'flex-1 max-w-[80%] rounded-lg p-4',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'shadow-sm',
            )}
          >
            {streamingContent ? (
              <div className="prose prose-sm dark:prose-invert max-w-none">
                <MarkdownRenderer content={streamingContent} isDarkMode={isDarkMode} />
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <TypingIndicator />
                <SessionStateIndicator state={sessionState} size="sm" />
              </div>
            )}
          </div>
        </div>
      )}

      <div ref={messagesEndRef} />
    </div>
  );
}

// ============================================================================
// MessageBubble Component
// ============================================================================

interface MessageBubbleProps {
  message: Message;
  isDarkMode: boolean;
  onCopy: () => Promise<void>;
  onRegenerate?: () => Promise<void>;
  onEdit?: (newContent: string) => Promise<void>;
  onFork?: () => void;
  isRegenerating?: boolean;
}

function MessageBubble({
  message,
  isDarkMode,
  onCopy,
  onRegenerate,
  onEdit,
  onFork,
  isRegenerating,
}: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div
          className={clsx(
            'px-4 py-2 rounded-full text-sm',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-500 dark:text-gray-400',
          )}
        >
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className={clsx('flex gap-3 group', isUser && 'flex-row-reverse')} id={`message-${message.id}`}>
      {/* Avatar */}
      <div
        className={clsx(
          'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
          isUser ? 'bg-blue-600' : 'bg-primary-600',
        )}
      >
        <span className="text-white text-sm font-medium">{isUser ? 'U' : 'C'}</span>
      </div>

      {/* Message content */}
      <div className={clsx('flex flex-col gap-2', isUser ? 'items-end' : 'items-start')}>
        <div
          className={clsx(
            'max-w-[80%] rounded-lg p-4',
            isUser ? 'bg-blue-600 text-white' : 'bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
            'shadow-sm',
          )}
        >
          {/* File attachments */}
          {message.attachments && message.attachments.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mb-2 pb-2 border-b border-gray-200 dark:border-gray-700">
              {message.attachments.map((file) => (
                <FileChip key={file.id} file={file} />
              ))}
            </div>
          )}

          {/* File references */}
          {message.references && message.references.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mb-2 pb-2 border-b border-gray-200 dark:border-gray-700">
              {message.references.map((ref) => (
                <FileChip key={ref.id} file={ref} isReference />
              ))}
            </div>
          )}

          {isUser ? (
            <p className="whitespace-pre-wrap">{message.content}</p>
          ) : (
            <div className="prose prose-sm dark:prose-invert max-w-none">
              <MarkdownRenderer content={message.content} isDarkMode={isDarkMode} />
            </div>
          )}
        </div>

        {/* Message actions */}
        <MessageActions
          message={message}
          onCopy={onCopy}
          onRegenerate={onRegenerate}
          onEdit={onEdit}
          onFork={onFork}
          isRegenerating={isRegenerating}
        />

        {/* Tool calls */}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <div className="w-full max-w-[90%] space-y-2">
            {message.toolCalls.map((toolCall) => (
              <div key={toolCall.id} id={`tool-call-${toolCall.id}`}>
                <ToolCallCard toolCall={toolCall} />
              </div>
            ))}
          </div>
        )}

        {/* Timestamp */}
        <span className="text-xs text-gray-400 dark:text-gray-500">{formatTimestamp(message.timestamp)}</span>
      </div>
    </div>
  );
}

// ============================================================================
// TypingIndicator Component
// ============================================================================

function TypingIndicator() {
  return (
    <div className="flex items-center gap-1">
      <span className="w-2 h-2 bg-gray-400 rounded-full animate-bounce [animation-delay:-0.3s]" />
      <span className="w-2 h-2 bg-gray-400 rounded-full animate-bounce [animation-delay:-0.15s]" />
      <span className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" />
    </div>
  );
}

// ============================================================================
// Helper Functions
// ============================================================================

function formatTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;

  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours}h ago`;

  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export default ChatView;
