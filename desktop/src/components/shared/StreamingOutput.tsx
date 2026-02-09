/**
 * StreamingOutput Component
 *
 * Terminal-style scrollable panel for real-time LLM responses and agent actions.
 * Features syntax highlighting for code blocks, auto-scroll behavior, and
 * ANSI-like coloring for different message types.
 *
 * Story 008: Real-time Execution Feedback
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  ArrowDownIcon,
  Cross2Icon,
  GearIcon,
  CheckCircledIcon,
  CrossCircledIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, StreamLineType } from '../../store/execution';
import { MarkdownRenderer } from '../ClaudeCodeMode/MarkdownRenderer';

// ============================================================================
// Types
// ============================================================================

interface StreamingOutputProps {
  /** Maximum height of the panel. Defaults to 400px */
  maxHeight?: string;
  /** Additional class names */
  className?: string;
  /** Whether to show the clear button */
  showClear?: boolean;
  /** Compact mode for SimpleMode (reduced padding, smaller text) */
  compact?: boolean;
}

// ============================================================================
// Color Mapping
// ============================================================================

const LINE_TYPE_COLORS: Record<StreamLineType, string> = {
  text: 'text-gray-200',
  info: 'text-blue-400',
  error: 'text-red-400',
  success: 'text-green-400',
  warning: 'text-yellow-400',
  tool: 'text-purple-400',
  tool_result: 'text-cyan-400',
  sub_agent: 'text-amber-400',
  thinking: 'text-gray-500 italic',
  code: 'text-cyan-300',
};

const LINE_TYPE_PREFIX: Record<StreamLineType, string> = {
  text: '',
  info: 'INFO ',
  error: 'ERR  ',
  success: 'OK   ',
  warning: 'WARN ',
  tool: '',
  tool_result: '',
  sub_agent: '',
  thinking: '     ',
  code: '',
};

// ============================================================================
// StreamingOutput Component
// ============================================================================

export function StreamingOutput({
  maxHeight = '400px',
  className,
  showClear = true,
  compact = false,
}: StreamingOutputProps) {
  const { streamingOutput, clearStreamingOutput, status } = useExecutionStore();
  const bottomRef = useRef<HTMLDivElement>(null);
  const [isAutoScroll, setIsAutoScroll] = useState(true);
  const [showScrollButton, setShowScrollButton] = useState(false);

  // Track content changes for auto-scroll (length alone won't change when
  // text deltas are concatenated into the last entry)
  const lastLine = streamingOutput.length > 0 ? streamingOutput[streamingOutput.length - 1] : null;
  const scrollTrigger = lastLine ? `${lastLine.id}:${lastLine.content.length}` : '';

  // Use IntersectionObserver on the bottom anchor to detect when user scrolls away.
  // This works regardless of which ancestor container actually scrolls.
  useEffect(() => {
    const el = bottomRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        const visible = entry.isIntersecting;
        setIsAutoScroll(visible);
        setShowScrollButton(!visible && streamingOutput.length > 0);
      },
      { threshold: 0.1 }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [streamingOutput.length]);

  // Auto-scroll to bottom when new content arrives
  useEffect(() => {
    if (isAutoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
    }
  }, [scrollTrigger, isAutoScroll]);

  const scrollToBottom = useCallback(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
      setIsAutoScroll(true);
      setShowScrollButton(false);
    }
  }, []);


  if (streamingOutput.length === 0) {
    return (
      <div
        className={clsx(
          'rounded-lg font-mono text-xs',
          'bg-gray-950 border border-gray-800',
          'flex items-center justify-center gap-3',
          compact ? 'p-4' : 'p-6',
          className
        )}
        style={{ minHeight: compact ? '120px' : '200px' }}
      >
        {status === 'running' ? (
          <>
            <div className="w-4 h-4 border-2 border-primary-500 border-t-transparent rounded-full animate-spin" />
            <span className="text-gray-400">Processing...</span>
          </>
        ) : (
          <span className="text-gray-600">Waiting for output...</span>
        )}
      </div>
    );
  }

  return (
    <div className={clsx('relative', className)}>
      {/* Header bar */}
      <div
        className={clsx(
          'flex items-center justify-between rounded-t-lg border border-b-0 border-gray-800',
          'bg-gray-900 px-3 py-1.5'
        )}
      >
        <div className="flex items-center gap-2">
          <div className="flex gap-1">
            <div className="w-2.5 h-2.5 rounded-full bg-red-500/70" />
            <div className="w-2.5 h-2.5 rounded-full bg-yellow-500/70" />
            <div className="w-2.5 h-2.5 rounded-full bg-green-500/70" />
          </div>
          <span className="text-2xs text-gray-500 font-mono ml-2">
            output ({streamingOutput.length} lines)
          </span>
          {status === 'running' && (
            <span className="flex items-center gap-1 text-2xs text-primary-400 font-mono">
              <span className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-pulse" />
              running
            </span>
          )}
          {status === 'completed' && streamingOutput.length > 0 && (
            <span className="flex items-center gap-1 text-2xs text-green-400 font-mono">
              <CheckCircledIcon className="w-3 h-3" />
              done
            </span>
          )}
          {status === 'failed' && streamingOutput.length > 0 && (
            <span className="flex items-center gap-1 text-2xs text-red-400 font-mono">
              <CrossCircledIcon className="w-3 h-3" />
              failed
            </span>
          )}
        </div>
        {showClear && (
          <button
            onClick={clearStreamingOutput}
            className="p-1 rounded text-gray-500 hover:text-gray-300 hover:bg-gray-800 transition-colors"
            title="Clear output"
          >
            <Cross2Icon className="w-3 h-3" />
          </button>
        )}
      </div>

      {/* Output panel */}
      <div
        className={clsx(
          'rounded-b-lg overflow-y-auto overflow-x-hidden',
          'bg-gray-950 border border-gray-800',
          compact ? 'text-2xs p-2' : 'text-xs p-3'
        )}
        style={{ maxHeight }}
      >
        {streamingOutput.map((line) => {
          // Text content: render as markdown (accumulated by appendStreamLine)
          if (line.type === 'text') {
            return (
              <div key={line.id} className="text-gray-200">
                <MarkdownRenderer content={line.content} className={compact ? 'text-xs' : 'text-sm'} />
              </div>
            );
          }

          // User message in chat (info type): render as a styled user bubble
          if (line.type === 'info') {
            return (
              <div key={line.id} className="my-4 flex justify-end">
                <div
                  className={clsx(
                    'max-w-[80%] px-4 py-2 rounded-2xl rounded-br-sm',
                    'bg-primary-600 text-white',
                    compact ? 'text-xs' : 'text-sm'
                  )}
                >
                  {line.content}
                </div>
              </div>
            );
          }

          if (line.type === 'thinking') {
            return (
              <div key={line.id} className="text-gray-500 italic font-mono text-xs whitespace-pre-wrap">
                {line.content}
              </div>
            );
          }

          // Tool call: render as a compact card with tool name and arguments
          if (line.type === 'tool') {
            return <ToolCallLine key={line.id} content={line.content} compact={compact} />;
          }

          if (line.type === 'sub_agent') {
            return <SubAgentLine key={line.id} content={line.content} compact={compact} />;
          }

          // Tool result: render as collapsible result
          if (line.type === 'tool_result') {
            return <ToolResultLine key={line.id} content={line.content} compact={compact} />;
          }

          // Success lines: render as completion banner
          if (line.type === 'success') {
            return (
              <div
                key={line.id}
                className="mt-3 mb-1 flex items-center gap-2 py-2 px-3 rounded border border-green-700/50 bg-green-950/30 text-green-400 text-xs font-mono animate-[fadeIn_0.3s_ease-out]"
              >
                <CheckCircledIcon className="w-3.5 h-3.5 flex-shrink-0" />
                <span>{line.content}</span>
              </div>
            );
          }

          // Error lines that look like completion failures
          if (line.type === 'error' && line.content.startsWith('Execution finished')) {
            return (
              <div
                key={line.id}
                className="mt-3 mb-1 flex items-center gap-2 py-2 px-3 rounded border border-red-700/50 bg-red-950/30 text-red-400 text-xs font-mono animate-[fadeIn_0.3s_ease-out]"
              >
                <CrossCircledIcon className="w-3.5 h-3.5 flex-shrink-0" />
                <span>{line.content}</span>
              </div>
            );
          }

          // Non-text lines: render terminal-style with prefix and color
          const prefix = LINE_TYPE_PREFIX[line.type];
          return (
            <div
              key={line.id}
              className={clsx(
                'leading-5 whitespace-pre-wrap break-all font-mono',
                LINE_TYPE_COLORS[line.type]
              )}
            >
              {prefix && (
                <span className="text-gray-600 select-none">{prefix}</span>
              )}
              {line.content}
            </div>
          );
        })}
        {/* Working indicator — visible while AI is processing */}
        {status === 'running' && <WorkingIndicator />}

        {/* Bottom anchor for auto-scroll */}
        <div ref={bottomRef} />
      </div>

      {/* Scroll to bottom button */}
      {showScrollButton && (
        <button
          onClick={scrollToBottom}
          className={clsx(
            'absolute bottom-4 right-4',
            'flex items-center gap-1 px-2 py-1 rounded-md',
            'bg-gray-800/90 text-gray-300 text-xs',
            'border border-gray-700',
            'hover:bg-gray-700 transition-colors',
            'shadow-lg backdrop-blur-sm'
          )}
        >
          <ArrowDownIcon className="w-3 h-3" />
          Scroll to bottom
        </button>
      )}
    </div>
  );
}

// ============================================================================
// Tool Call Display Components
// ============================================================================

/** Parse tool name and arguments from formatted content like "[tool:Read] /path/to/file" */
function parseToolContent(content: string): { toolName: string; args: string } {
  const match = content.match(/^\[tool:([^\]]+)\]\s*(.*)/s);
  if (match) {
    return { toolName: match[1], args: match[2] || '' };
  }

  const subAgentMatch = content.match(/^\[sub_agent:(start|end)\]\s*(.*)/s);
  if (subAgentMatch) {
    const action = subAgentMatch[1];
    const details = subAgentMatch[2] || '';
    return { toolName: 'SubAgent', args: `${action} ${details}`.trim() };
  }

  // Fallback: try to extract from "[tool] Name started" format
  const legacyMatch = content.match(/^\[tool\]\s*([^\s]+)\s+started/);
  if (legacyMatch) {
    return { toolName: legacyMatch[1], args: '' };
  }
  return { toolName: 'Tool', args: content };
}

function ToolCallLine({ content, compact }: { content: string; compact: boolean }) {
  const { toolName, args } = parseToolContent(content);

  return (
    <div
      className={clsx(
        'my-1 rounded border border-purple-800/40 bg-purple-950/30',
        compact ? 'px-2 py-1' : 'px-3 py-1.5',
      )}
    >
      <div className="flex items-center gap-2">
        <GearIcon className="w-3 h-3 text-purple-400 animate-spin" />
        <span className="font-mono text-purple-300 font-semibold text-xs">
          {toolName}
        </span>
        {args && (
          <span className="font-mono text-purple-400/70 text-xs truncate">
            {args}
          </span>
        )}
      </div>
    </div>
  );
}

function parseSubAgentContent(content: string): {
  phase: 'start' | 'end' | 'update';
  details: string;
  success: boolean | null;
} {
  const startMatch = content.match(/^\[sub_agent:start\]\s*(.*)$/s);
  if (startMatch) {
    return {
      phase: 'start',
      details: startMatch[1] || 'Sub-agent started',
      success: null,
    };
  }

  const endMatch = content.match(/^\[sub_agent:end\]\s*(.*)$/s);
  if (endMatch) {
    const details = endMatch[1] || 'Sub-agent finished';
    if (/failed|error/i.test(details)) {
      return { phase: 'end', details, success: false };
    }
    if (/completed|success/i.test(details)) {
      return { phase: 'end', details, success: true };
    }
    return { phase: 'end', details, success: null };
  }

  return { phase: 'update', details: content, success: null };
}

function SubAgentLine({ content, compact }: { content: string; compact: boolean }) {
  const parsed = parseSubAgentContent(content);
  const isStart = parsed.phase === 'start';
  const isEndSuccess = parsed.phase === 'end' && parsed.success === true;
  const isEndFail = parsed.phase === 'end' && parsed.success === false;

  return (
    <div
      className={clsx(
        'my-1 rounded border',
        isEndSuccess
          ? 'border-green-700/40 bg-green-950/20'
          : isEndFail
            ? 'border-red-700/40 bg-red-950/20'
            : 'border-amber-700/40 bg-amber-950/20',
        compact ? 'px-2 py-1' : 'px-3 py-1.5',
      )}
    >
      <div className="flex items-center gap-2">
        {isEndSuccess ? (
          <CheckCircledIcon className="w-3 h-3 text-green-400 flex-shrink-0" />
        ) : isEndFail ? (
          <CrossCircledIcon className="w-3 h-3 text-red-400 flex-shrink-0" />
        ) : (
          <GearIcon className={clsx('w-3 h-3 text-amber-400', isStart && 'animate-spin')} />
        )}
        <span
          className={clsx(
            'font-mono text-xs font-semibold uppercase tracking-wide',
            isEndSuccess ? 'text-green-300' : isEndFail ? 'text-red-300' : 'text-amber-300'
          )}
        >
          Sub-agent
        </span>
        <span
          className={clsx(
            'font-mono text-xs truncate',
            isEndSuccess ? 'text-green-400/80' : isEndFail ? 'text-red-400/80' : 'text-amber-400/80'
          )}
        >
          {parsed.details}
        </span>
      </div>
    </div>
  );
}

/** Parse tool result content from "[tool_result:id] content" or "[tool_error:id] content" */
function parseToolResultContent(content: string): { toolId: string; result: string; isError: boolean } {
  const errorMatch = content.match(/^\[tool_error:([^\]]*)\]\s*(.*)/s);
  if (errorMatch) {
    return { toolId: errorMatch[1], result: errorMatch[2], isError: true };
  }
  const resultMatch = content.match(/^\[tool_result:([^\]]*)\]\s*(.*)/s);
  if (resultMatch) {
    return { toolId: resultMatch[1], result: resultMatch[2], isError: false };
  }
  return { toolId: '', result: content, isError: false };
}

function ToolResultLine({ content, compact }: { content: string; compact: boolean }) {
  const { result, isError } = parseToolResultContent(content);
  const [expanded, setExpanded] = useState(false);
  const isLong = result.length > 200;
  const displayText = isLong && !expanded ? result.substring(0, 200) + '...' : result;

  return (
    <div
      className={clsx(
        'my-1 rounded border',
        isError
          ? 'border-red-800/40 bg-red-950/20'
          : 'border-cyan-800/30 bg-cyan-950/20',
        compact ? 'px-2 py-1' : 'px-3 py-1.5',
      )}
    >
      <div className="flex items-center gap-2 mb-0.5">
        {isError ? (
          <CrossCircledIcon className="w-3 h-3 text-red-400 flex-shrink-0" />
        ) : (
          <CheckCircledIcon className="w-3 h-3 text-cyan-400 flex-shrink-0" />
        )}
        <span className={clsx('font-mono text-xs font-semibold', isError ? 'text-red-300' : 'text-cyan-300')}>
          {isError ? 'Error' : 'Result'}
        </span>
        {isLong && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-2xs text-gray-500 hover:text-gray-300 ml-auto"
          >
            {expanded ? 'collapse' : 'expand'}
          </button>
        )}
      </div>
      <pre
        className={clsx(
          'font-mono text-xs whitespace-pre-wrap break-all',
          isError ? 'text-red-400/80' : 'text-cyan-400/70',
        )}
      >
        {displayText}
      </pre>
    </div>
  );
}

// ============================================================================
// Working Indicator (pulsing dots while AI is processing)
// ============================================================================

function WorkingIndicator() {
  return (
    <div className="flex items-center gap-2 py-3 px-1">
      <div className="flex items-center gap-1">
        <span
          className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-bounce"
          style={{ animationDelay: '0ms', animationDuration: '1.2s' }}
        />
        <span
          className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-bounce"
          style={{ animationDelay: '200ms', animationDuration: '1.2s' }}
        />
        <span
          className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-bounce"
          style={{ animationDelay: '400ms', animationDuration: '1.2s' }}
        />
      </div>
      <span className="text-xs text-gray-500 font-mono">working...</span>
    </div>
  );
}

export default StreamingOutput;
