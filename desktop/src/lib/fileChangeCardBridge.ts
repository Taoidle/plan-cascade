/**
 * File Change Card Bridge
 *
 * Converts `file-change-recorded` Tauri events into structured chat cards
 * (FileChangeCard / TurnChangeSummaryCard) injected into the chat transcript.
 *
 * This is a standalone module (not a store) that bridges the backend file
 * change tracker with the frontend chat UI.
 */

import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useExecutionStore } from '../store/execution';
import { useFileChangesStore } from '../store/fileChanges';
import type { CommandResponse } from './tauri';
import type {
  CardPayload,
  FileChangeCardData,
  TurnChangeSummaryCardData,
} from '../types/workflowCard';

// ============================================================================
// Types
// ============================================================================

/** Payload from the `file-change-recorded` Tauri event. */
interface FileChangeEvent {
  session_id: string;
  turn_index: number;
  file_path: string;
  tool_name: string;
  change_id: string;
  before_hash: string | null;
  after_hash: string;
  description: string;
}

interface PendingChange {
  event: FileChangeEvent;
  diff: string | null;
  linesAdded: number;
  linesRemoved: number;
  diffPreview: string | null;
}

// ============================================================================
// Helpers
// ============================================================================

/** Parse a unified diff string and count added/removed lines. */
function parseDiffStats(diff: string): { linesAdded: number; linesRemoved: number } {
  let linesAdded = 0;
  let linesRemoved = 0;
  for (const line of diff.split('\n')) {
    if (line.startsWith('+')) linesAdded++;
    else if (line.startsWith('-')) linesRemoved++;
  }
  return { linesAdded, linesRemoved };
}

/** Extract the first N meaningful diff lines for preview. */
function extractDiffPreview(diff: string, maxLines: number = 8): string | null {
  const lines = diff.split('\n').filter((l) => l.startsWith('+') || l.startsWith('-') || l.startsWith(' '));
  if (lines.length === 0) return null;
  return lines.slice(0, maxLines).join('\n');
}

function makeCardPayload(cardType: 'file_change', data: FileChangeCardData): string;
function makeCardPayload(cardType: 'turn_change_summary', data: TurnChangeSummaryCardData): string;
function makeCardPayload(cardType: string, data: unknown): string {
  const payload: CardPayload = {
    cardType: cardType as CardPayload['cardType'],
    cardId: `${cardType}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    data: data as CardPayload['data'],
    interactive: false,
  };
  return JSON.stringify(payload);
}

// ============================================================================
// Bridge
// ============================================================================

export interface FileChangeCardBridge {
  startListening(): Promise<() => void>;
  onTurnEnd(turnIndex: number): void;
  reset(): void;
}

export function createFileChangeCardBridge(
  sessionId: string,
  projectRoot: string,
): FileChangeCardBridge {
  /** Accumulated changes per turn for summary card. */
  const turnChanges = new Map<number, PendingChange[]>();
  /** Debounce timer for batching rapid events. */
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  /** Queue of events waiting to be processed after debounce. */
  let pendingEvents: FileChangeEvent[] = [];

  async function processEvents(events: FileChangeEvent[]) {
    for (const event of events) {
      try {
        // Fetch the diff from backend
        let diff = '';
        try {
          const resp = await invoke<CommandResponse<string>>('get_file_change_diff', {
            sessionId: event.session_id,
            projectRoot,
            beforeHash: event.before_hash,
            afterHash: event.after_hash,
          });
          if (resp.success && resp.data) {
            diff = resp.data;
          }
        } catch {
          // Diff fetch failed — proceed without preview
        }

        const stats = parseDiffStats(diff);
        const diffPreview = extractDiffPreview(diff);

        // Pre-fill the diff cache so expanding in AI Changes tab is instant
        if (diff) {
          useFileChangesStore.getState().prefillDiffCache(event.change_id, diff);
        }

        const cardData: FileChangeCardData = {
          changeId: event.change_id,
          filePath: event.file_path,
          toolName: event.tool_name as 'Write' | 'Edit',
          changeType: event.before_hash === null ? 'new_file' : 'modified',
          beforeHash: event.before_hash,
          afterHash: event.after_hash,
          diffPreview,
          linesAdded: stats.linesAdded,
          linesRemoved: stats.linesRemoved,
          sessionId: event.session_id,
          turnIndex: event.turn_index,
          description: event.description,
        };

        // Inject the card into the chat
        useExecutionStore.getState().appendStreamLine(
          makeCardPayload('file_change', cardData),
          'card',
        );

        // Accumulate for turn summary
        const pending: PendingChange = {
          event,
          diff,
          linesAdded: stats.linesAdded,
          linesRemoved: stats.linesRemoved,
          diffPreview,
        };
        const turnList = turnChanges.get(event.turn_index) || [];
        turnList.push(pending);
        turnChanges.set(event.turn_index, turnList);
      } catch {
        // Silently skip failed cards
      }
    }
  }

  function flushPending() {
    if (pendingEvents.length === 0) return;
    const batch = pendingEvents;
    pendingEvents = [];
    // Process async — fire and forget
    processEvents(batch);
  }

  return {
    async startListening(): Promise<() => void> {
      const unlisten = await listen<FileChangeEvent>('file-change-recorded', (e) => {
        // Only process events for our session
        if (e.payload.session_id !== sessionId) return;

        pendingEvents.push(e.payload);

        // Debounce: wait 200ms for more events before processing
        if (debounceTimer !== null) {
          clearTimeout(debounceTimer);
        }
        debounceTimer = setTimeout(flushPending, 200);
      });

      return () => {
        unlisten();
        if (debounceTimer !== null) {
          clearTimeout(debounceTimer);
          debounceTimer = null;
        }
        // Flush any remaining events
        flushPending();
      };
    },

    onTurnEnd(turnIndex: number) {
      // Flush any pending debounced events first
      if (debounceTimer !== null) {
        clearTimeout(debounceTimer);
        debounceTimer = null;
      }
      flushPending();

      // Check if this turn has 2+ changes
      const changes = turnChanges.get(turnIndex);
      if (!changes || changes.length < 2) return;

      const summaryData: TurnChangeSummaryCardData = {
        turnIndex,
        sessionId,
        totalFiles: changes.length,
        files: changes.map((c) => ({
          filePath: c.event.file_path,
          toolName: c.event.tool_name as 'Write' | 'Edit',
          changeType: c.event.before_hash === null ? 'new_file' : 'modified',
          linesAdded: c.linesAdded,
          linesRemoved: c.linesRemoved,
        })),
        totalLinesAdded: changes.reduce((sum, c) => sum + c.linesAdded, 0),
        totalLinesRemoved: changes.reduce((sum, c) => sum + c.linesRemoved, 0),
      };

      useExecutionStore.getState().appendStreamLine(
        makeCardPayload('turn_change_summary', summaryData),
        'card',
      );
    },

    reset() {
      turnChanges.clear();
      pendingEvents = [];
      if (debounceTimer !== null) {
        clearTimeout(debounceTimer);
        debounceTimer = null;
      }
    },
  };
}
