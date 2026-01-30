/**
 * ChatView Component
 *
 * Displays conversation messages with user/assistant bubbles.
 * Supports markdown rendering and auto-scrolling.
 */

import { useEffect, useRef } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useClaudeCodeStore, Message } from '../../store/claudeCode';
import { ToolCallCard } from './ToolCallCard';

// ============================================================================
// ChatView Component
// ============================================================================

export function ChatView() {
  const { t } = useTranslation('claudeCode');
  const { messages, isStreaming, streamingContent } = useClaudeCodeStore();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

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
          <span role="img" aria-label="robot">&#129302;</span>
        </div>
        <h2 className="text-xl font-semibold text-gray-700 dark:text-gray-300 mb-2">
          {t('chat.empty.title')}
        </h2>
        <p className="text-gray-500 dark:text-gray-400 max-w-md">
          {t('chat.empty.description')}
        </p>
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
        <MessageBubble key={message.id} message={message} />
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
              'shadow-sm'
            )}
          >
            {streamingContent ? (
              <div className="prose prose-sm dark:prose-invert max-w-none">
                <MarkdownContent content={streamingContent} />
              </div>
            ) : (
              <TypingIndicator />
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
}

function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div
          className={clsx(
            'px-4 py-2 rounded-full text-sm',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-500 dark:text-gray-400'
          )}
        >
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className={clsx('flex gap-3', isUser && 'flex-row-reverse')}>
      {/* Avatar */}
      <div
        className={clsx(
          'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
          isUser ? 'bg-blue-600' : 'bg-primary-600'
        )}
      >
        <span className="text-white text-sm font-medium">
          {isUser ? 'U' : 'C'}
        </span>
      </div>

      {/* Message content */}
      <div className={clsx('flex flex-col gap-2', isUser ? 'items-end' : 'items-start')}>
        <div
          className={clsx(
            'max-w-[80%] rounded-lg p-4',
            isUser
              ? 'bg-blue-600 text-white'
              : 'bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
            'shadow-sm'
          )}
        >
          {isUser ? (
            <p className="whitespace-pre-wrap">{message.content}</p>
          ) : (
            <div className="prose prose-sm dark:prose-invert max-w-none">
              <MarkdownContent content={message.content} />
            </div>
          )}
        </div>

        {/* Tool calls */}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <div className="w-full max-w-[90%] space-y-2">
            {message.toolCalls.map((toolCall) => (
              <ToolCallCard key={toolCall.id} toolCall={toolCall} />
            ))}
          </div>
        )}

        {/* Timestamp */}
        <span className="text-xs text-gray-400 dark:text-gray-500">
          {formatTimestamp(message.timestamp)}
        </span>
      </div>
    </div>
  );
}

// ============================================================================
// MarkdownContent Component
// ============================================================================

interface MarkdownContentProps {
  content: string;
}

function MarkdownContent({ content }: MarkdownContentProps) {
  // Simple markdown-like rendering
  // For production, consider using react-markdown
  const lines = content.split('\n');
  const elements: JSX.Element[] = [];
  let codeBlock: string[] = [];
  let inCodeBlock = false;

  lines.forEach((line, i) => {
    // Code block start/end
    if (line.startsWith('```')) {
      if (inCodeBlock) {
        elements.push(
          <pre key={`code-${i}`} className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-sm">
            <code>{codeBlock.join('\n')}</code>
          </pre>
        );
        codeBlock = [];
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      return;
    }

    if (inCodeBlock) {
      codeBlock.push(line);
      return;
    }

    // Headers
    if (line.startsWith('### ')) {
      elements.push(
        <h3 key={i} className="text-lg font-semibold mt-4 mb-2">
          {line.slice(4)}
        </h3>
      );
      return;
    }
    if (line.startsWith('## ')) {
      elements.push(
        <h2 key={i} className="text-xl font-semibold mt-4 mb-2">
          {line.slice(3)}
        </h2>
      );
      return;
    }
    if (line.startsWith('# ')) {
      elements.push(
        <h1 key={i} className="text-2xl font-bold mt-4 mb-2">
          {line.slice(2)}
        </h1>
      );
      return;
    }

    // Bullet points
    if (line.startsWith('- ') || line.startsWith('* ')) {
      elements.push(
        <li key={i} className="ml-4">
          {renderInlineMarkdown(line.slice(2))}
        </li>
      );
      return;
    }

    // Numbered lists
    const numberedMatch = line.match(/^\d+\. /);
    if (numberedMatch) {
      elements.push(
        <li key={i} className="ml-4 list-decimal">
          {renderInlineMarkdown(line.slice(numberedMatch[0].length))}
        </li>
      );
      return;
    }

    // Empty lines
    if (line.trim() === '') {
      elements.push(<br key={i} />);
      return;
    }

    // Regular paragraph
    elements.push(
      <p key={i} className="my-1">
        {renderInlineMarkdown(line)}
      </p>
    );
  });

  // Handle unclosed code block
  if (inCodeBlock && codeBlock.length > 0) {
    elements.push(
      <pre key="code-final" className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-sm">
        <code>{codeBlock.join('\n')}</code>
      </pre>
    );
  }

  return <>{elements}</>;
}

function renderInlineMarkdown(text: string): JSX.Element {
  // Handle inline code
  const parts = text.split(/(`[^`]+`)/g);

  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('`') && part.endsWith('`')) {
          return (
            <code
              key={i}
              className="bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 rounded text-sm font-mono"
            >
              {part.slice(1, -1)}
            </code>
          );
        }

        // Handle bold
        const boldParts = part.split(/(\*\*[^*]+\*\*)/g);
        return boldParts.map((boldPart, j) => {
          if (boldPart.startsWith('**') && boldPart.endsWith('**')) {
            return <strong key={`${i}-${j}`}>{boldPart.slice(2, -2)}</strong>;
          }
          return <span key={`${i}-${j}`}>{boldPart}</span>;
        });
      })}
    </>
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
