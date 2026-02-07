/**
 * StreamingOutput Component
 *
 * Terminal-style scrollable panel for real-time LLM responses and agent actions.
 * Features syntax highlighting for code blocks, auto-scroll behavior, and
 * ANSI-like coloring for different message types.
 *
 * Story 008: Real-time Execution Feedback
 */

import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { Highlight, themes } from 'prism-react-renderer';
import {
  ArrowDownIcon,
  Cross2Icon,
  CopyIcon,
  CheckIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, StreamLine, StreamLineType } from '../../store/execution';

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
  thinking: 'text-gray-500 italic',
  code: 'text-cyan-300',
};

const LINE_TYPE_PREFIX: Record<StreamLineType, string> = {
  text: '',
  info: 'INFO ',
  error: 'ERR  ',
  success: 'OK   ',
  warning: 'WARN ',
  tool: 'TOOL ',
  thinking: '     ',
  code: '',
};

// ============================================================================
// Code Block Detection
// ============================================================================

interface ParsedSegment {
  type: 'text' | 'code';
  content: string;
  language?: string;
}

function parseCodeBlocks(lines: StreamLine[]): ParsedSegment[] {
  const segments: ParsedSegment[] = [];
  let currentText: string[] = [];
  let inCodeBlock = false;
  let codeLines: string[] = [];
  let codeLang = '';

  for (const line of lines) {
    const trimmed = line.content.trim();

    if (!inCodeBlock && trimmed.startsWith('```')) {
      // Flush accumulated text
      if (currentText.length > 0) {
        segments.push({ type: 'text', content: currentText.join('\n') });
        currentText = [];
      }
      inCodeBlock = true;
      codeLang = trimmed.slice(3).trim() || 'text';
      codeLines = [];
    } else if (inCodeBlock && trimmed === '```') {
      // End code block
      segments.push({
        type: 'code',
        content: codeLines.join('\n'),
        language: codeLang,
      });
      inCodeBlock = false;
      codeLines = [];
      codeLang = '';
    } else if (inCodeBlock) {
      codeLines.push(line.content);
    } else {
      currentText.push(line.content);
    }
  }

  // Flush remaining
  if (inCodeBlock && codeLines.length > 0) {
    // Unclosed code block, treat as code anyway
    segments.push({
      type: 'code',
      content: codeLines.join('\n'),
      language: codeLang,
    });
  }
  if (currentText.length > 0) {
    segments.push({ type: 'text', content: currentText.join('\n') });
  }

  return segments;
}

// ============================================================================
// Code Block Component
// ============================================================================

function CodeBlock({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [code]);

  return (
    <div className="relative my-2 rounded-md overflow-hidden border border-gray-700">
      <div className="flex items-center justify-between px-3 py-1 bg-gray-800 border-b border-gray-700">
        <span className="text-2xs font-mono text-gray-500 uppercase">{language}</span>
        <button
          onClick={handleCopy}
          className={clsx(
            'p-1 rounded text-gray-500 hover:text-gray-300 transition-colors',
            copied && 'text-green-400'
          )}
          title="Copy code"
        >
          {copied ? <CheckIcon className="w-3 h-3" /> : <CopyIcon className="w-3 h-3" />}
        </button>
      </div>
      <Highlight theme={themes.nightOwl} code={code.trim()} language={language}>
        {({ className, style, tokens, getLineProps, getTokenProps }) => (
          <pre
            className={clsx(className, 'p-3 text-xs overflow-x-auto')}
            style={{ ...style, margin: 0, background: 'transparent' }}
          >
            {tokens.map((line, i) => {
              const lineProps = getLineProps({ line, key: i });
              return (
                <div key={i} {...lineProps}>
                  <span className="inline-block w-8 text-right mr-3 text-gray-600 select-none">
                    {i + 1}
                  </span>
                  {line.map((token, key) => (
                    <span key={key} {...getTokenProps({ token, key })} />
                  ))}
                </div>
              );
            })}
          </pre>
        )}
      </Highlight>
    </div>
  );
}

// ============================================================================
// StreamingOutput Component
// ============================================================================

export function StreamingOutput({
  maxHeight = '400px',
  className,
  showClear = true,
  compact = false,
}: StreamingOutputProps) {
  const { streamingOutput, clearStreamingOutput } = useExecutionStore();
  const containerRef = useRef<HTMLDivElement>(null);
  const [isAutoScroll, setIsAutoScroll] = useState(true);
  const [showScrollButton, setShowScrollButton] = useState(false);
  const lastLineCountRef = useRef(0);

  // Auto-scroll to bottom when new content arrives
  useEffect(() => {
    if (isAutoScroll && containerRef.current && streamingOutput.length > lastLineCountRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
    lastLineCountRef.current = streamingOutput.length;
  }, [streamingOutput.length, isAutoScroll]);

  // Track scroll position to toggle auto-scroll and show "scroll to bottom" button
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const isNearBottom = scrollHeight - scrollTop - clientHeight < 50;
    setIsAutoScroll(isNearBottom);
    setShowScrollButton(!isNearBottom && streamingOutput.length > 0);
  }, [streamingOutput.length]);

  const scrollToBottom = useCallback(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
      setIsAutoScroll(true);
      setShowScrollButton(false);
    }
  }, []);

  // Parse segments for code block detection
  const segments = useMemo(() => parseCodeBlocks(streamingOutput), [streamingOutput]);

  if (streamingOutput.length === 0) {
    return (
      <div
        className={clsx(
          'rounded-lg font-mono text-xs',
          'bg-gray-950 border border-gray-800',
          'flex items-center justify-center',
          compact ? 'p-4' : 'p-6',
          className
        )}
        style={{ minHeight: compact ? '120px' : '200px' }}
      >
        <span className="text-gray-600">Waiting for output...</span>
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
        ref={containerRef}
        onScroll={handleScroll}
        className={clsx(
          'rounded-b-lg overflow-y-auto overflow-x-hidden',
          'bg-gray-950 border border-gray-800',
          'font-mono',
          compact ? 'text-2xs p-2' : 'text-xs p-3'
        )}
        style={{ maxHeight }}
      >
        {segments.map((segment, idx) => {
          if (segment.type === 'code' && segment.language) {
            return (
              <CodeBlock
                key={`seg-${idx}`}
                code={segment.content}
                language={segment.language}
              />
            );
          }

          // Render text lines individually with type coloring
          const textLines = segment.content.split('\n');
          return textLines.map((text, lineIdx) => {
            // Find the corresponding StreamLine for type info
            const globalLineIdx = streamingOutput.findIndex(
              (sl) => sl.content === text
            );
            const streamLine = globalLineIdx >= 0 ? streamingOutput[globalLineIdx] : null;
            const lineType: StreamLineType = streamLine?.type || 'text';
            const prefix = LINE_TYPE_PREFIX[lineType];

            return (
              <div
                key={`seg-${idx}-line-${lineIdx}`}
                className={clsx(
                  'leading-5 whitespace-pre-wrap break-all',
                  LINE_TYPE_COLORS[lineType]
                )}
              >
                {prefix && (
                  <span className="text-gray-600 select-none">{prefix}</span>
                )}
                {text}
              </div>
            );
          });
        })}
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

export default StreamingOutput;
