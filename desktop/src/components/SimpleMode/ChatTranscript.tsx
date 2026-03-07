import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
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
import { buildTurnViewModelsFromNormalized, type TurnViewModel } from './chatTranscriptModel';
import { normalizeTurnBoundaries } from '../../lib/conversationUtils';

const VIRTUALIZATION_THRESHOLD_TURNS = 50;
const VIRTUAL_OVERSCAN = 6;
const ESTIMATED_TURN_HEIGHT = 180;

const EMPTY_LINES: StreamLine[] = [];

interface ChatTranscriptProps {
  lines: StreamLine[];
  status: ExecutionStatus;
  scrollRef?: React.RefObject<HTMLDivElement | null>;
  forceFullRender?: boolean;
  showPendingPlaceholder?: boolean;
}

interface TurnRowProps {
  turn: TurnViewModel;
  userLine: StreamLine;
  assistantLines: StreamLine[];
  status: ExecutionStatus;
  showPendingPlaceholder: boolean;
  isLastTurn: boolean;
  editingLineId: number | null;
  isActionsDisabled: boolean;
  isClaudeCodeBackend: boolean;
  onEdit: (lineId: number, newContent: string) => void;
  onEditStart: (lineId: number) => void;
  onEditCancel: () => void;
  onCopy: (content: string) => void;
}

export function ChatTranscript({
  lines,
  status,
  scrollRef,
  forceFullRender = false,
  showPendingPlaceholder = status === 'running',
}: ChatTranscriptProps) {
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
  const [showScrollBtn, setShowScrollBtn] = useState(false);
  const isNearBottom = useRef(true);

  const normalizedLines = useMemo(() => normalizeTurnBoundaries(lines), [lines]);
  const turns = useMemo(() => buildTurnViewModelsFromNormalized(normalizedLines), [normalizedLines]);

  const syntheticUserLine = useMemo((): StreamLine | null => {
    if (turns.length !== 1 || turns[0].userLineIndex >= 0 || normalizedLines.length === 0) return null;
    return {
      id: -1,
      content: t('chat.userLabel', { defaultValue: 'User' }),
      type: 'info',
      timestamp: normalizedLines[0].timestamp,
      turnId: 1,
      turnBoundary: 'user',
    };
  }, [turns, normalizedLines, t]);

  const backend = useSettingsStore((s) => s.backend);
  const isClaudeCodeBackend = backend === 'claude-code';
  const isActionsDisabled = status === 'running' || status === 'paused';
  const lastTurnIndex = turns.length > 0 ? turns.length - 1 : -1;

  const isVirtualized = !forceFullRender && turns.length >= VIRTUALIZATION_THRESHOLD_TURNS;

  const virtualizer = useVirtualizer({
    count: turns.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => ESTIMATED_TURN_HEIGHT,
    overscan: VIRTUAL_OVERSCAN,
    enabled: isVirtualized,
  });

  // Clear editing state when lines change
  useEffect(() => {
    if (editingLineId !== null) {
      const lineStillExists = lines.some((line) => line.id === editingLineId);
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

  useEffect(() => {
    if (isVirtualized) {
      virtualizer.measure();
    }
  }, [isVirtualized, virtualizer, lines]);

  const hasContent = normalizedLines.length > 0;
  if (turns.length === 0 && !hasContent) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
        {showPendingPlaceholder
          ? t('emptyChat.thinking', { defaultValue: 'Thinking...' })
          : t('emptyChat.startConversation', { defaultValue: 'Start a conversation on the right input box.' })}
      </div>
    );
  }

  const resolveUserLine = (turn: TurnViewModel): StreamLine | null => {
    if (turn.userLineIndex >= 0) {
      return normalizedLines[turn.userLineIndex] ?? null;
    }
    return syntheticUserLine;
  };

  const resolveAssistantLines = (turn: TurnViewModel): StreamLine[] => {
    if (turn.assistantEndIndex < turn.assistantStartIndex || turn.assistantStartIndex < 0) {
      return EMPTY_LINES;
    }
    return normalizedLines.slice(turn.assistantStartIndex, turn.assistantEndIndex + 1);
  };

  return (
    <div className="relative h-full">
      <div
        ref={setRef}
        onScroll={handleScroll}
        className="h-full overflow-y-auto px-4 py-4"
        data-testid="chat-transcript-scroll"
        data-render-mode={isVirtualized ? 'virtual' : 'full'}
      >
        {isVirtualized ? (
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const turn = turns[virtualRow.index];
              const userLine = resolveUserLine(turn);
              if (!userLine) return null;
              return (
                <div
                  key={`turn-${turn.userLineId}-${turn.turnIndex}`}
                  ref={virtualizer.measureElement}
                  data-index={virtualRow.index}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <div className="pb-4">
                    <TurnRow
                      turn={turn}
                      userLine={userLine}
                      assistantLines={resolveAssistantLines(turn)}
                      status={status}
                      showPendingPlaceholder={showPendingPlaceholder}
                      isLastTurn={turn.turnIndex === lastTurnIndex}
                      editingLineId={editingLineId}
                      isActionsDisabled={isActionsDisabled}
                      isClaudeCodeBackend={isClaudeCodeBackend}
                      onEdit={handleEdit}
                      onEditStart={handleEditStart}
                      onEditCancel={handleEditCancel}
                      onCopy={handleCopy}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="space-y-4">
            {turns.map((turn) => {
              const userLine = resolveUserLine(turn);
              if (!userLine) return null;
              return (
                <TurnRow
                  key={`turn-${turn.userLineId}-${turn.turnIndex}`}
                  turn={turn}
                  userLine={userLine}
                  assistantLines={resolveAssistantLines(turn)}
                  status={status}
                  showPendingPlaceholder={showPendingPlaceholder}
                  isLastTurn={turn.turnIndex === lastTurnIndex}
                  editingLineId={editingLineId}
                  isActionsDisabled={isActionsDisabled}
                  isClaudeCodeBackend={isClaudeCodeBackend}
                  onEdit={handleEdit}
                  onEditStart={handleEditStart}
                  onEditCancel={handleEditCancel}
                  onCopy={handleCopy}
                />
              );
            })}
          </div>
        )}
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

const TurnRow = memo(function TurnRow({
  turn,
  userLine,
  assistantLines,
  status,
  showPendingPlaceholder,
  isLastTurn,
  editingLineId,
  isActionsDisabled,
  isClaudeCodeBackend,
  onEdit,
  onEditStart,
  onEditCancel,
  onCopy,
}: TurnRowProps) {
  const { t } = useTranslation('simpleMode');

  return (
    <>
      {editingLineId === userLine.id ? (
        <div className="flex justify-end">
          <EditMode
            content={userLine.content}
            onSave={(newContent) => onEdit(userLine.id, newContent)}
            onCancel={onEditCancel}
            isClaudeCodeBackend={isClaudeCodeBackend}
          />
        </div>
      ) : (
        <div className="group relative flex justify-end">
          <div className="max-w-[82%] px-4 py-2 rounded-2xl rounded-br-sm bg-primary-600 text-white text-sm whitespace-pre-wrap">
            {userLine.content}
          </div>
          <MessageActions
            line={userLine}
            isUserMessage={true}
            isLastTurn={isLastTurn}
            isClaudeCodeBackend={isClaudeCodeBackend}
            disabled={isActionsDisabled}
            onEdit={onEdit}
            onRegenerate={() => useExecutionStore.getState().regenerateResponse(userLine.id)}
            onRollback={() => useExecutionStore.getState().rollbackToTurn(userLine.id)}
            onCopy={onCopy}
            onEditStart={onEditStart}
            onEditCancel={onEditCancel}
          />
        </div>
      )}

      {assistantLines.length > 0 ? (
        <ChatAssistantSection
          lines={assistantLines}
          isLastTurn={isLastTurn}
          userLineId={userLine.id}
          disabled={isActionsDisabled}
          isClaudeCodeBackend={isClaudeCodeBackend}
          onEdit={onEdit}
          onCopy={onCopy}
          onFork={() => useExecutionStore.getState().forkSessionAtTurn(userLine.id)}
        />
      ) : showPendingPlaceholder && status === 'running' && turn.turnIndex >= 0 && isLastTurn ? (
        <div className="flex justify-start">
          <div className="px-4 py-2 rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 text-sm italic flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-pulse" />
            {t('emptyChat.thinking', { defaultValue: 'Thinking...' })}
          </div>
        </div>
      ) : null}
    </>
  );
});

/** Assistant response section: renders rich content (text, tools, sub-agents, thinking) within a chat bubble */
const ChatAssistantSection = memo(function ChatAssistantSection({
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

  const lineStats = useMemo(() => {
    const thinkingLines: StreamLine[] = [];
    const contentLines: StreamLine[] = [];
    const textParts: string[] = [];
    let lastTextLine: StreamLine | null = null;
    let hasRichContent = false;

    for (const line of lines) {
      if (line.type === 'thinking') {
        thinkingLines.push(line);
        continue;
      }

      contentLines.push(line);
      if (line.type === 'text') {
        textParts.push(line.content);
        lastTextLine = line;
      }

      if (
        line.type === 'tool' ||
        line.type === 'tool_result' ||
        line.type === 'sub_agent' ||
        line.type === 'analysis' ||
        Boolean(line.subAgentId)
      ) {
        hasRichContent = true;
      }
    }

    return {
      thinkingLines,
      contentLines,
      textContent: textParts.join(''),
      lastTextLine,
      hasRichContent,
    };
  }, [lines]);

  const blocks = useMemo(() => buildDisplayBlocks(lineStats.contentLines, true), [lineStats.contentLines]);

  return (
    <div className="group relative flex justify-start">
      <div
        className={clsx(
          'max-w-[88%] rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100',
          lineStats.hasRichContent ? 'px-3 py-2 space-y-2' : 'px-4 py-2',
        )}
      >
        {showReasoning && lineStats.thinkingLines.length > 0 && <ChatThinkingSection lines={lineStats.thinkingLines} />}

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

          const line = block.line;
          if (line.type === 'card') {
            const payload = (line as { cardPayload?: unknown }).cardPayload as CardPayload | undefined;
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

      {lineStats.lastTextLine && (
        <MessageActions
          line={lineStats.lastTextLine}
          isUserMessage={false}
          isLastTurn={isLastTurn}
          isClaudeCodeBackend={isClaudeCodeBackend}
          disabled={disabled}
          onEdit={onEdit}
          onRegenerate={() => useExecutionStore.getState().regenerateResponse(userLineId)}
          onRollback={() => useExecutionStore.getState().rollbackToTurn(userLineId)}
          onCopy={() => onCopy(lineStats.textContent)}
          onFork={() => onFork(userLineId)}
        />
      )}
    </div>
  );
});

/** Collapsible thinking section for chat bubbles — collapsed by default */
function ChatThinkingSection({ lines }: { lines: StreamLine[] }) {
  const [expanded, setExpanded] = useState(false);
  const content = lines.map((line) => line.content).join('');

  if (!content.trim()) return null;

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-600 overflow-hidden">
      <button
        onClick={() => setExpanded((value) => !value)}
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
