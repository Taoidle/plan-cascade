import type { ExecutionState, ExecutionStatus, SessionSnapshot, StreamLine, StreamLineType } from './types';

/**
 * Find a background session by its taskId (which corresponds to the
 * session_id emitted by the Rust backend).
 */
export function findBackgroundSessionByTaskId(
  state: ExecutionState,
  sessionId: string,
): { key: string; snapshot: SessionSnapshot } | undefined {
  for (const [key, snapshot] of Object.entries(state.backgroundSessions)) {
    if (snapshot.taskId === sessionId || snapshot.standaloneSessionId === sessionId) {
      return { key, snapshot };
    }
  }
  return undefined;
}

/**
 * Apply a lightweight update to a background session identified by its taskId.
 */
export function updateBackgroundSessionByTaskId(
  state: ExecutionState,
  sessionId: string,
  updater: (snapshot: SessionSnapshot) => Partial<SessionSnapshot>,
): Partial<ExecutionState> {
  const found = findBackgroundSessionByTaskId(state, sessionId);
  if (!found) return {};
  const { key, snapshot } = found;
  const updates = updater(snapshot);
  return {
    backgroundSessions: {
      ...state.backgroundSessions,
      [key]: { ...snapshot, ...updates },
    },
  };
}

/**
 * Append a StreamLine to a background session's streamingOutput.
 */
export function appendToBackgroundSession(
  state: ExecutionState,
  sessionId: string,
  content: string,
  type: StreamLineType,
): Partial<ExecutionState> {
  return updateBackgroundSessionByTaskId(state, sessionId, (snapshot) => {
    const lines = snapshot.streamingOutput;
    const last = lines.length > 0 ? lines[lines.length - 1] : null;

    if ((type === 'text' || type === 'thinking') && last && last.type === type) {
      const updated = { ...last, content: last.content + content };
      const newLines = lines.slice();
      newLines[newLines.length - 1] = updated;
      return {
        streamingOutput: newLines,
        updatedAt: Date.now(),
      };
    }

    const nextId = snapshot.streamLineCounter + 1;
    const newLine: StreamLine = {
      id: nextId,
      content,
      type,
      timestamp: Date.now(),
    };
    return {
      streamingOutput: [...lines, newLine],
      streamLineCounter: nextId,
      updatedAt: Date.now(),
    };
  });
}

/**
 * Check whether an incoming session_id belongs to the foreground session.
 */
export function isForegroundSession(state: ExecutionState, sessionId: string | undefined): boolean {
  if (!sessionId) return true;
  const fg = state.taskId || state.standaloneSessionId;
  if (!fg) return false;
  return fg === sessionId;
}

export type { ExecutionStatus };
