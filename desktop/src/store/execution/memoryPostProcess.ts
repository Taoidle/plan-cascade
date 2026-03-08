import { invoke } from '@tauri-apps/api/core';
import { reportNonFatal } from '../../lib/nonFatal';

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
