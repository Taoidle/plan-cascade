import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '../ClaudeCodeMode/MarkdownRenderer';
import { Collapsible } from './Collapsible';
import { MessageActions, EditMode } from './MessageActions';
import { useExecutionStore, type ExecutionStatus, type StreamLine } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import {
  buildDisplayBlocks,
  ToolCallLine,
  SubAgentLine,
  AnalysisLine,
  ToolResultLine,
  SubAgentGroupPanel,
  EventGroupLine,
} from '../shared/StreamingOutput';
import { WorkflowCardRenderer } from './WorkflowCards/WorkflowCardRenderer';
import type { CardPayload } from '../../types/workflowCard';

interface RichTurn {
  turnIndex: number;
  userLine: StreamLine;
  assistantLines: StreamLine[];
}

export function ChatTranscript({
  lines,
  status,
  scrollRef,
}: {
  lines: StreamLine[];
  status: ExecutionStatus;
  scrollRef?: React.RefObject<HTMLDivElement | null>;
}) {
  const { t } = useTranslation('simpleMode');
  const containerRef = useRef<HTMLDivElement>(null);

  // Sync containerRef to the external scrollRef so the parent can access it
  const setRef = useCallback(
    (node: HTMLDivElement | null) => {
      (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
      if (scrollRef) {
        (scrollRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
      }
    },
    [scrollRef],
  );
  const [editingLineId, setEditingLineId] = useState<number | null>(null);

  // Derive rich conversation turns from ALL lines (not just text)
  const richTurns = useMemo((): RichTurn[] => {
    const result: RichTurn[] = [];
    let turnIndex = 0;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].type !== 'info') continue;

      let endIndex = lines.length - 1;
      for (let j = i + 1; j < lines.length; j++) {
        if (lines[j].type === 'info') {
          endIndex = j - 1;
          break;
        }
      }

      const assistantLines: StreamLine[] = [];
      for (let j = i + 1; j <= endIndex; j++) {
        assistantLines.push(lines[j]);
      }

      result.push({ turnIndex: turnIndex++, userLine: lines[i], assistantLines });
    }
    // Fallback: if no info lines but content exists, synthesize a turn so the panel isn't empty
    if (result.length === 0 && lines.length > 0 && lines.some((l) => l.type !== 'info')) {
      const syntheticUserLine: StreamLine = {
        id: -1,
        content: '(continued)',
        type: 'info',
        timestamp: lines[0].timestamp,
      };
      result.push({
        turnIndex: 0,
        userLine: syntheticUserLine,
        assistantLines: lines.filter((l) => l.type !== 'info'),
      });
    }
    return result;
  }, [lines]);

  const backend = useSettingsStore((s) => s.backend);
  const isClaudeCodeBackendValue = backend === 'claude-code';
  const isActionsDisabled = status === 'running' || status === 'paused';
  const lastTurnIndex = richTurns.length > 0 ? richTurns.length - 1 : -1;

  // Clear editing state when lines change
  useEffect(() => {
    if (editingLineId !== null) {
      const lineStillExists = lines.some((l) => l.id === editingLineId);
      if (!lineStillExists) setEditingLineId(null);
    }
  }, [lines, editingLineId]);

  // Action handlers
  const handleEdit = useCallback((lineId: number, newContent: string) => {
    setEditingLineId(null);
    useExecutionStore.getState().editAndResend(lineId, newContent);
  }, []);

  const handleEditStart = useCallback((lineId: number) => {
    setEditingLineId(lineId);
  }, []);

  const handleEditCancel = useCallback(() => {
    setEditingLineId(null);
  }, []);

  const handleCopy = useCallback((content: string) => {
    navigator.clipboard.writeText(content).catch(() => {});
  }, []);

  // Sticky-bottom auto-scroll: only scroll if user is near the bottom
  const isNearBottom = useRef(true);
  const [showScrollBtn, setShowScrollBtn] = useState(false);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const nearBottom = scrollHeight - scrollTop - clientHeight < 50;
    isNearBottom.current = nearBottom;
    setShowScrollBtn(!nearBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    containerRef.current?.scrollTo({ top: containerRef.current.scrollHeight, behavior: 'smooth' });
  }, []);

  useEffect(() => {
    if (!containerRef.current || !isNearBottom.current) return;
    containerRef.current.scrollTop = containerRef.current.scrollHeight;
  }, [lines]);

  const hasContent = lines.length > 0 && lines.some((l) => l.type !== 'info');
  if (richTurns.length === 0 && !hasContent) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
        {status === 'running'
          ? t('emptyChat.thinking', { defaultValue: 'Thinking...' })
          : t('emptyChat.startConversation', { defaultValue: 'Start a conversation on the right input box.' })}
      </div>
    );
  }

  return (
    <div className="relative h-full">
      <div ref={setRef} onScroll={handleScroll} className="h-full overflow-y-auto px-4 py-4 space-y-4">
        {richTurns.map((turn) => {
          const isLastTurn = turn.turnIndex === lastTurnIndex;

          return (
            <Fragment key={turn.userLine.id}>
              {/* User message bubble */}
              {editingLineId === turn.userLine.id ? (
                <div className="flex justify-end">
                  <EditMode
                    content={turn.userLine.content}
                    onSave={(newContent) => handleEdit(turn.userLine.id, newContent)}
                    onCancel={handleEditCancel}
                    isClaudeCodeBackend={isClaudeCodeBackendValue}
                  />
                </div>
              ) : (
                <div className="group relative flex justify-end">
                  <div className="max-w-[82%] px-4 py-2 rounded-2xl rounded-br-sm bg-primary-600 text-white text-sm whitespace-pre-wrap">
                    {turn.userLine.content}
                  </div>
                  <MessageActions
                    line={turn.userLine}
                    isUserMessage={true}
                    isLastTurn={isLastTurn}
                    isClaudeCodeBackend={isClaudeCodeBackendValue}
                    disabled={isActionsDisabled}
                    onEdit={handleEdit}
                    onRegenerate={() => useExecutionStore.getState().regenerateResponse(turn.userLine.id)}
                    onRollback={() => useExecutionStore.getState().rollbackToTurn(turn.userLine.id)}
                    onCopy={handleCopy}
                    onEditStart={handleEditStart}
                    onEditCancel={handleEditCancel}
                  />
                </div>
              )}

              {/* Assistant response section */}
              {turn.assistantLines.length > 0 ? (
                <ChatAssistantSection
                  lines={turn.assistantLines}
                  isLastTurn={isLastTurn}
                  userLineId={turn.userLine.id}
                  disabled={isActionsDisabled}
                  isClaudeCodeBackend={isClaudeCodeBackendValue}
                  onEdit={handleEdit}
                  onCopy={handleCopy}
                  onFork={() => useExecutionStore.getState().forkSessionAtTurn(turn.userLine.id)}
                />
              ) : status === 'running' && isLastTurn ? (
                <div className="flex justify-start">
                  <div className="px-4 py-2 rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 text-sm italic flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-pulse" />
                    {t('emptyChat.thinking', { defaultValue: 'Thinking...' })}
                  </div>
                </div>
              ) : null}
            </Fragment>
          );
        })}
      </div>

      {/* Scroll to bottom button */}
      {showScrollBtn && (
        <button
          onClick={scrollToBottom}
          className={clsx(
            'absolute bottom-4 left-1/2 -translate-x-1/2 z-10',
            'flex items-center justify-center',
            'w-8 h-8 rounded-full',
            'bg-white dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'shadow-md',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-50 dark:hover:bg-gray-700',
            'transition-colors',
            'animate-in fade-in-0 zoom-in-75 duration-150',
          )}
          title={t('chat.scrollToBottom', { defaultValue: 'Scroll to bottom' })}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M4 6l4 4 4-4" />
          </svg>
        </button>
      )}
    </div>
  );
}

/** Assistant response section: renders rich content (text, tools, sub-agents, thinking) within a chat bubble */
function ChatAssistantSection({
  lines,
  isLastTurn,
  userLineId,
  disabled,
  isClaudeCodeBackend,
  onEdit,
  onCopy,
  onFork,
}: {
  lines: StreamLine[];
  isLastTurn: boolean;
  userLineId: number;
  disabled: boolean;
  isClaudeCodeBackend: boolean;
  onEdit: (lineId: number, newContent: string) => void;
  onCopy: (content: string) => void;
  onFork: (userLineId: number) => void;
}) {
  const showReasoning = useSettingsStore((s) => s.showReasoningOutput);

  // Separate thinking from other lines
  const thinkingLines = useMemo(() => lines.filter((l) => l.type === 'thinking'), [lines]);
  const contentLines = useMemo(() => lines.filter((l) => l.type !== 'thinking'), [lines]);

  // Build display blocks for content (always grouped, like compact mode)
  const blocks = useMemo(() => buildDisplayBlocks(contentLines, true), [contentLines]);

  // Collect text content for copy
  const textContent = useMemo(
    () =>
      lines
        .filter((l) => l.type === 'text')
        .map((l) => l.content)
        .join(''),
    [lines],
  );

  // Find last text line for MessageActions
  const lastTextLine = useMemo(() => [...lines].reverse().find((l) => l.type === 'text'), [lines]);

  // Check if there's rich content (tools, sub-agents, etc.)
  const hasRichContent = contentLines.some(
    (l) =>
      l.type === 'tool' || l.type === 'tool_result' || l.type === 'sub_agent' || l.type === 'analysis' || l.subAgentId,
  );

  return (
    <div className="group relative flex justify-start">
      <div
        className={clsx(
          'max-w-[88%] rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100',
          hasRichContent ? 'px-3 py-2 space-y-2' : 'px-4 py-2',
        )}
      >
        {/* Thinking section (collapsed by default) */}
        {showReasoning && thinkingLines.length > 0 && <ChatThinkingSection lines={thinkingLines} />}

        {/* Content blocks */}
        {blocks.map((block, idx) => {
          if (block.kind === 'sub_agent_group') {
            return (
              <SubAgentGroupPanel
                key={`sa-${block.subAgentId}-${block.lines[0]?.id}`}
                subAgentId={block.subAgentId}
                lines={block.lines}
                depth={block.depth}
                compact
              />
            );
          }
          if (block.kind === 'group') {
            return (
              <EventGroupLine
                key={block.groupId}
                groupId={block.groupId}
                kind={block.groupKind}
                lines={block.lines}
                compact
              />
            );
          }
          // Single line block
          const line = block.line;
          if (line.type === 'card') {
            let payload: CardPayload | null = line.cardPayload ?? null;
            try {
              if (!payload) {
                payload = JSON.parse(line.content) as CardPayload;
              }
            } catch (error) {
              console.warn('Failed to parse workflow card payload', error, { lineId: line.id });
            }
            if (!payload) {
              return (
                <div
                  key={line.id}
                  className="my-1 text-xs px-3 py-2 rounded border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300"
                >
                  Invalid workflow card payload
                </div>
              );
            }
            return (
              <div key={line.id} className="my-1">
                <WorkflowCardRenderer payload={payload} />
              </div>
            );
          }
          if (line.type === 'text') {
            return (
              <div key={line.id}>
                <MarkdownRenderer content={line.content} className="text-sm" />
              </div>
            );
          }
          if (line.type === 'tool') {
            return <ToolCallLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'tool_result') {
            return <ToolResultLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'sub_agent') {
            return <SubAgentLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'analysis') {
            return <AnalysisLine key={line.id} content={line.content} compact />;
          }
          // error, warning, success
          if (line.type === 'error' || line.type === 'warning' || line.type === 'success') {
            const toneClass =
              line.type === 'error'
                ? 'border-red-300 bg-red-50 text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300'
                : line.type === 'warning'
                  ? 'border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-900/20 dark:text-amber-300'
                  : 'border-green-300 bg-green-50 text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-300';
            return (
              <div key={line.id} className={clsx('text-xs px-3 py-2 rounded border', toneClass)}>
                {line.content}
              </div>
            );
          }
          return <div key={`block-${idx}`} />;
        })}
      </div>

      {/* MessageActions on the assistant section */}
      {lastTextLine && (
        <MessageActions
          line={lastTextLine}
          isUserMessage={false}
          isLastTurn={isLastTurn}
          isClaudeCodeBackend={isClaudeCodeBackend}
          disabled={disabled}
          onEdit={onEdit}
          onRegenerate={() => useExecutionStore.getState().regenerateResponse(userLineId)}
          onRollback={() => useExecutionStore.getState().rollbackToTurn(userLineId)}
          onCopy={() => onCopy(textContent)}
          onFork={() => onFork(userLineId)}
        />
      )}
    </div>
  );
}

/** Collapsible thinking section for chat bubbles — collapsed by default */
function ChatThinkingSection({ lines }: { lines: StreamLine[] }) {
  const [expanded, setExpanded] = useState(false);
  const content = lines.map((l) => l.content).join('');

  if (!content.trim()) return null;

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-600 overflow-hidden">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-200/50 dark:hover:bg-gray-700/50 transition-colors"
      >
        <svg
          className={clsx('w-3 h-3 transition-transform', expanded && 'rotate-90')}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
        <span className="italic">Thinking</span>
        <span className="text-2xs text-gray-400 dark:text-gray-500">({content.length} chars)</span>
      </button>
      <Collapsible open={expanded}>
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-600 text-xs text-gray-500 dark:text-gray-400 italic font-mono whitespace-pre-wrap max-h-64 overflow-y-auto">
          {content}
        </div>
      </Collapsible>
    </div>
  );
}
