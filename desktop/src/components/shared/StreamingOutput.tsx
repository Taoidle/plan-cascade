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
import { invoke } from '@tauri-apps/api/core';
import {
  ArrowDownIcon,
  Cross2Icon,
  GearIcon,
  CheckCircledIcon,
  CrossCircledIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, StreamLine, StreamLineType } from '../../store/execution';
import { useModeStore } from '../../store/mode';
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

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

async function saveTextWithDialog(filename: string, content: string): Promise<boolean> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const selected = await save({
    title: 'Export Output',
    defaultPath: filename,
    canCreateDirectories: true,
  });
  if (!selected || Array.isArray(selected)) return false;
  const result = await invoke<CommandResponse<boolean>>('save_output_export', {
    path: selected,
    content,
  });
  if (!result.success) {
    throw new Error(result.error || 'Failed to save export');
  }
  return true;
}

function serializeRawOutput(lines: StreamLine[]): string {
  return lines
    .map((line) => {
      const prefix = LINE_TYPE_PREFIX[line.type];
      if (line.type === 'text') return line.content;
      return `${prefix}${line.content}`;
    })
    .join('\n');
}

function serializeConversationOutput(lines: StreamLine[]): string {
  const out: string[] = [];
  for (const line of lines) {
    const content = line.content.trim();
    if (!content) continue;
    switch (line.type) {
      case 'info':
        out.push(`User: ${content}`);
        break;
      case 'text':
        out.push(`Assistant: ${content}`);
        break;
      case 'error':
        out.push(`Error: ${content}`);
        break;
      case 'warning':
        out.push(`Warning: ${content}`);
        break;
      case 'success':
        out.push(`Status: ${content}`);
        break;
      default:
        break;
    }
  }
  return out.join('\n\n');
}

function collectAssistantReplies(lines: StreamLine[]): Array<{ id: number; content: string }> {
  return lines
    .filter((line) => line.type === 'text' && line.content.trim().length > 0)
    .map((line) => ({ id: line.id, content: line.content }));
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
  analysis: 'text-sky-300',
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
  analysis: '',
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
  const isFillHeight = maxHeight === 'none';
  const { streamingOutput, clearStreamingOutput, status } = useExecutionStore();
  const mode = useModeStore((state) => state.mode);
  const isSimpleMode = mode === 'simple';
  const displayBlocks = buildDisplayBlocks(streamingOutput, isSimpleMode);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [isAutoScroll, setIsAutoScroll] = useState(true);
  const [showScrollButton, setShowScrollButton] = useState(false);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [exportNotice, setExportNotice] = useState<{ type: 'ok' | 'error'; text: string } | null>(
    null
  );
  const exportMenuRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (!showExportMenu) return;
    const onMouseDown = (event: MouseEvent) => {
      if (!exportMenuRef.current) return;
      if (!exportMenuRef.current.contains(event.target as Node)) {
        setShowExportMenu(false);
      }
    };
    window.addEventListener('mousedown', onMouseDown);
    return () => window.removeEventListener('mousedown', onMouseDown);
  }, [showExportMenu]);

  const scrollToBottom = useCallback(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
      setIsAutoScroll(true);
      setShowScrollButton(false);
    }
  }, []);

  const notifyExport = useCallback((type: 'ok' | 'error', text: string) => {
    setExportNotice({ type, text });
    window.setTimeout(() => {
      setExportNotice((current) => (current?.text === text ? null : current));
    }, 3000);
  }, []);
  const assistantReplies = collectAssistantReplies(streamingOutput);

  const exportAll = useCallback(async () => {
    try {
      const content = serializeConversationOutput(streamingOutput);
      if (!content.trim()) {
        notifyExport('error', 'No conversation content available to export.');
        return;
      }
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      const saved = await saveTextWithDialog(`conversation-${stamp}.txt`, content);
      if (!saved) {
        notifyExport('error', 'Export canceled.');
        return;
      }
      notifyExport('ok', 'Conversation transcript exported.');
    } catch (error) {
      console.error('Failed to export conversation transcript:', error);
      notifyExport('error', `Export failed: ${error instanceof Error ? error.message : 'unknown error'}`);
    }
  }, [notifyExport, streamingOutput]);

  const exportRaw = useCallback(async () => {
    try {
      const content = serializeRawOutput(streamingOutput);
      if (!content.trim()) {
        notifyExport('error', 'No output available to export.');
        return;
      }
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      const saved = await saveTextWithDialog(`output-raw-${stamp}.txt`, content);
      if (!saved) {
        notifyExport('error', 'Export canceled.');
        return;
      }
      notifyExport('ok', 'Raw output exported.');
    } catch (error) {
      console.error('Failed to export raw output:', error);
      notifyExport('error', `Export failed: ${error instanceof Error ? error.message : 'unknown error'}`);
    }
  }, [notifyExport, streamingOutput]);

  const exportLatestReply = useCallback(async () => {
    try {
      const latest = assistantReplies[assistantReplies.length - 1];
      if (!latest) {
        notifyExport('error', 'No AI reply available to export.');
        return;
      }
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      const saved = await saveTextWithDialog(`ai-reply-latest-${stamp}.md`, latest.content);
      if (!saved) {
        notifyExport('error', 'Export canceled.');
        return;
      }
      notifyExport('ok', 'Latest AI reply exported.');
    } catch (error) {
      console.error('Failed to export latest reply:', error);
      notifyExport('error', `Export failed: ${error instanceof Error ? error.message : 'unknown error'}`);
    }
  }, [assistantReplies, notifyExport]);

  const exportReplyByNumber = useCallback(async () => {
    try {
      if (assistantReplies.length === 0) {
        notifyExport('error', 'No AI replies available to export.');
        return;
      }
      const raw = window.prompt(
        `Export which AI reply? Enter 1-${assistantReplies.length}`,
        String(assistantReplies.length)
      );
      if (!raw) return;
      const selected = Number(raw);
      if (!Number.isFinite(selected)) {
        notifyExport('error', 'Invalid reply number.');
        return;
      }
      const index = Math.floor(selected) - 1;
      if (index < 0 || index >= assistantReplies.length) {
        notifyExport('error', `Reply number must be in range 1-${assistantReplies.length}.`);
        return;
      }
      const reply = assistantReplies[index];
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      const saved = await saveTextWithDialog(`ai-reply-${index + 1}-${stamp}.md`, reply.content);
      if (!saved) {
        notifyExport('error', 'Export canceled.');
        return;
      }
      notifyExport('ok', `AI reply #${index + 1} exported.`);
    } catch (error) {
      console.error('Failed to export selected reply:', error);
      notifyExport('error', `Export failed: ${error instanceof Error ? error.message : 'unknown error'}`);
    }
  }, [assistantReplies, notifyExport]);


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
    <div className={clsx('relative', isFillHeight && 'h-full min-h-0 flex flex-col', className)}>
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
          <div className="flex items-center gap-2">
            {exportNotice && (
              <span
                className={clsx(
                  'text-2xs font-mono',
                  exportNotice.type === 'ok' ? 'text-green-400' : 'text-red-400'
                )}
              >
                {exportNotice.text}
              </span>
            )}
            <div ref={exportMenuRef} className="relative">
              <button
                onClick={() => setShowExportMenu((v) => !v)}
                disabled={streamingOutput.length === 0}
                className="px-2 py-1 rounded text-gray-400 hover:text-gray-200 hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed text-2xs font-mono transition-colors"
                title="Export output"
              >
                export
              </button>
              {showExportMenu && (
                <div className="absolute right-0 mt-1 w-44 rounded border border-gray-700 bg-gray-900 shadow-lg z-20 p-1">
                  <button
                    onClick={() => {
                      setShowExportMenu(false);
                      void exportAll();
                    }}
                    disabled={streamingOutput.length === 0}
                    className="w-full text-left px-2 py-1 rounded text-2xs font-mono text-gray-300 hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    conversation transcript
                  </button>
                  <button
                    onClick={() => {
                      setShowExportMenu(false);
                      void exportLatestReply();
                    }}
                    disabled={assistantReplies.length === 0}
                    className="w-full text-left px-2 py-1 rounded text-2xs font-mono text-gray-300 hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    latest AI reply
                  </button>
                  <button
                    onClick={() => {
                      setShowExportMenu(false);
                      void exportReplyByNumber();
                    }}
                    disabled={assistantReplies.length === 0}
                    className="w-full text-left px-2 py-1 rounded text-2xs font-mono text-gray-300 hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    choose reply number
                  </button>
                  <button
                    onClick={() => {
                      setShowExportMenu(false);
                      void exportRaw();
                    }}
                    disabled={streamingOutput.length === 0}
                    className="w-full text-left px-2 py-1 rounded text-2xs font-mono text-gray-300 hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    raw full output
                  </button>
                </div>
              )}
            </div>
            <button
              onClick={clearStreamingOutput}
              className="p-1 rounded text-gray-500 hover:text-gray-300 hover:bg-gray-800 transition-colors"
              title="Clear output"
            >
              <Cross2Icon className="w-3 h-3" />
            </button>
          </div>
        )}
      </div>

      {/* Output panel */}
      <div
        className={clsx(
          'rounded-b-lg overflow-y-auto overflow-x-hidden',
          'bg-gray-950 border border-gray-800',
          compact ? 'text-2xs p-2' : 'text-xs p-3',
          isFillHeight && 'flex-1 min-h-0'
        )}
        style={isFillHeight ? undefined : { maxHeight }}
      >
        {displayBlocks.map((block) => {
          if (block.kind === 'group') {
            return (
              <EventGroupLine
                key={block.groupId}
                groupId={block.groupId}
                kind={block.groupKind}
                lines={block.lines}
                compact={compact}
              />
            );
          }

          const line = block.line;
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

          if (line.type === 'analysis') {
            return <AnalysisLine key={line.id} content={line.content} compact={compact} />;
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

type DisplayBlock =
  | { kind: 'line'; line: StreamLine }
  | {
      kind: 'group';
      groupId: string;
      groupKind: 'tool_activity' | 'analysis_activity';
      lines: StreamLine[];
    };

function classifyGroupKind(line: StreamLine): 'tool_activity' | 'analysis_activity' | null {
  if (line.type === 'tool' || line.type === 'tool_result') {
    return 'tool_activity';
  }
  if (line.type === 'analysis') {
    if (
      line.content.startsWith('[analysis:evidence:') ||
      line.content.startsWith('[analysis:phase_progress:')
    ) {
      return 'analysis_activity';
    }
  }
  return null;
}

function buildDisplayBlocks(lines: StreamLine[], compactMode: boolean): DisplayBlock[] {
  if (!compactMode || lines.length === 0) {
    return lines.map((line) => ({ kind: 'line', line }));
  }

  const blocks: DisplayBlock[] = [];
  let i = 0;
  while (i < lines.length) {
    const kind = classifyGroupKind(lines[i]);
    if (!kind) {
      blocks.push({ kind: 'line', line: lines[i] });
      i += 1;
      continue;
    }

    const group: StreamLine[] = [lines[i]];
    let j = i + 1;
    while (j < lines.length) {
      const nextKind = classifyGroupKind(lines[j]);
      if (nextKind !== kind) break;
      group.push(lines[j]);
      j += 1;
    }

    if (group.length >= 4) {
      const first = group[0];
      const last = group[group.length - 1];
      blocks.push({
        kind: 'group',
        groupId: `group-${first.id}-${last.id}`,
        groupKind: kind,
        lines: group,
      });
    } else {
      for (const line of group) {
        blocks.push({ kind: 'line', line });
      }
    }
    i = j;
  }

  return blocks;
}

function EventGroupLine({
  groupId,
  kind,
  lines,
  compact,
}: {
  groupId: string;
  kind: 'tool_activity' | 'analysis_activity';
  lines: StreamLine[];
  compact: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  const toolCalls = lines.filter((line) => line.type === 'tool').length;
  const toolResults = lines.filter((line) => line.type === 'tool_result').length;
  const analysisEvents = lines.filter((line) => line.type === 'analysis').length;

  const title =
    kind === 'tool_activity'
      ? `Tool activity (${lines.length} events)`
      : `Analysis activity (${lines.length} events)`;
  const summary =
    kind === 'tool_activity'
      ? `${toolCalls} calls, ${toolResults} results`
      : `${analysisEvents} evidence/progress updates`;

  return (
    <div
      className={clsx(
        'my-1 rounded border',
        kind === 'tool_activity'
          ? 'border-purple-800/40 bg-purple-950/20'
          : 'border-sky-800/40 bg-sky-950/20',
        compact ? 'px-2 py-1' : 'px-3 py-1.5'
      )}
    >
      <div className="flex items-center gap-2">
        <GearIcon
          className={clsx(
            'w-3 h-3 flex-shrink-0',
            kind === 'tool_activity' ? 'text-purple-300' : 'text-sky-300'
          )}
        />
        <span
          className={clsx(
            'font-mono text-xs font-semibold',
            kind === 'tool_activity' ? 'text-purple-300' : 'text-sky-300'
          )}
        >
          {title}
        </span>
        <span
          className={clsx(
            'font-mono text-xs truncate',
            kind === 'tool_activity' ? 'text-purple-400/80' : 'text-sky-400/80'
          )}
        >
          {summary}
        </span>
        <button
          onClick={() => setExpanded((value) => !value)}
          className="ml-auto text-2xs text-gray-400 hover:text-gray-200"
        >
          {expanded ? 'collapse' : 'expand'}
        </button>
      </div>

      {expanded && (
        <div className="mt-2 space-y-1 border-t border-gray-700/40 pt-2">
          {lines.map((line) => (
            <div
              key={`${groupId}-${line.id}`}
              className={clsx(
                'font-mono text-xs whitespace-pre-wrap break-all',
                LINE_TYPE_COLORS[line.type]
              )}
            >
              {LINE_TYPE_PREFIX[line.type]}
              {line.content}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

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

function parseAnalysisContent(content: string): {
  kind: 'phase_start' | 'phase_progress' | 'evidence' | 'phase_end' | 'validation' | 'generic';
  phase: string;
  status: string;
  details: string;
} {
  const match = content.match(/^\[analysis:([^:\]]+)(?::([^:\]]+))?(?::([^:\]]+))?\]\s*(.*)$/s);
  if (!match) {
    return { kind: 'generic', phase: '', status: '', details: content };
  }

  const rawKind = match[1] || '';
  const part2 = match[2] || '';
  const part3 = match[3] || '';
  const details = match[4] || '';

  if (rawKind === 'phase_start') {
    return { kind: 'phase_start', phase: part2, status: '', details };
  }
  if (rawKind === 'phase_progress') {
    return { kind: 'phase_progress', phase: part2, status: '', details };
  }
  if (rawKind === 'evidence') {
    return { kind: 'evidence', phase: part2, status: part3, details };
  }
  if (rawKind === 'phase_end') {
    return { kind: 'phase_end', phase: part2, status: '', details };
  }
  if (rawKind === 'validation') {
    return { kind: 'validation', phase: '', status: part2, details };
  }
  return { kind: 'generic', phase: part2, status: part3, details };
}

function AnalysisLine({ content, compact }: { content: string; compact: boolean }) {
  const parsed = parseAnalysisContent(content);
  const isError = parsed.status === 'error' || /failed|warning|issue/i.test(parsed.details);
  const isDone = parsed.kind === 'phase_end' && !isError;

  const borderClass = isError
    ? 'border-red-800/40 bg-red-950/20'
    : isDone
      ? 'border-green-800/40 bg-green-950/20'
      : 'border-sky-800/40 bg-sky-950/20';
  const textClass = isError
    ? 'text-red-300'
    : isDone
      ? 'text-green-300'
      : 'text-sky-300';
  const detailClass = isError
    ? 'text-red-400/80'
    : isDone
      ? 'text-green-400/80'
      : 'text-sky-400/80';

  const label = parsed.kind === 'evidence'
    ? 'Evidence'
    : parsed.kind === 'validation'
      ? 'Validation'
      : 'Analysis';
  const phaseLabel = parsed.phase ? ` ${parsed.phase}` : '';

  return (
    <div
      className={clsx(
        'my-1 rounded border',
        borderClass,
        compact ? 'px-2 py-1' : 'px-3 py-1.5',
      )}
    >
      <div className="flex items-center gap-2">
        {isDone ? (
          <CheckCircledIcon className="w-3 h-3 text-green-400 flex-shrink-0" />
        ) : isError ? (
          <CrossCircledIcon className="w-3 h-3 text-red-400 flex-shrink-0" />
        ) : (
          <GearIcon className="w-3 h-3 text-sky-400" />
        )}
        <span className={clsx('font-mono text-xs font-semibold uppercase tracking-wide', textClass)}>
          {label}{phaseLabel}
        </span>
        <span className={clsx('font-mono text-xs truncate', detailClass)}>
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
