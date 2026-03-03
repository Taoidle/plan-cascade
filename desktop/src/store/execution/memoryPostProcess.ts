import { invoke } from '@tauri-apps/api/core';
import { reportNonFatal } from '../../lib/nonFatal';
import { useSettingsStore } from '../settings';
import { buildHistorySessionId } from './sessionLifecycle';
import type { ExecutionState } from './types';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export function clearSessionScopedMemory(sessionId: string | null | undefined): void {
  const normalized = sessionId?.trim();
  if (!normalized) return;
  void Promise.resolve(
    invoke<CommandResponse<number>>('clear_session_memories', {
      sessionId: normalized,
    }),
  ).catch((error: unknown) => {
    reportNonFatal('execution.clearSessionScopedMemory', error, { sessionId: normalized });
  });
}

export async function triggerMemoryExtraction(state: ExecutionState): Promise<void> {
  try {
    const projectPath = useSettingsStore.getState().workspacePath;
    if (!projectPath) return;

    const taskDescription = state.taskDescription;
    if (!taskDescription) return;

    const MAX_LINE_CHARS = 300;
    const TOTAL_CHAR_BUDGET = 20000;
    const meaningfulTypes: Set<string> = new Set(['text', 'info', 'success', 'error']);
    const lines: string[] = [];
    let totalChars = 0;

    for (let i = state.streamingOutput.length - 1; i >= 0; i--) {
      const line = state.streamingOutput[i];
      if (!meaningfulTypes.has(line.type) || line.content.trim().length === 0) continue;

      const trimmed = line.content.trim();
      const truncated = trimmed.length > MAX_LINE_CHARS ? trimmed.slice(0, MAX_LINE_CHARS) + '...' : trimmed;
      if (totalChars + truncated.length + 1 > TOTAL_CHAR_BUDGET) break;
      lines.unshift(truncated);
      totalChars += truncated.length + 1;
    }

    const conversationSummary = lines.join('\n');
    if (conversationSummary.length < 50) return;

    const sessionId = buildHistorySessionId(state.taskId, state.standaloneSessionId) || undefined;
    const result = await invoke<{ success: boolean; data?: { extracted_count: number } }>('extract_session_memories', {
      projectPath,
      taskDescription,
      conversationSummary,
      sessionId: sessionId || null,
    });

    if (result?.success && result.data && result.data.extracted_count > 0) {
      console.log(`[memory] Extracted ${result.data.extracted_count} memories from session`);
    }
  } catch (error) {
    reportNonFatal('execution.triggerMemoryExtraction', error);
  }
}
