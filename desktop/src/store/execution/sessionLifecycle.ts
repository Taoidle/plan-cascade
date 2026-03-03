export function createStandaloneSessionId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `simple-${crypto.randomUUID()}`;
  }
  return `simple-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
}

export function createStandaloneExecutionId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `standalone-${crypto.randomUUID()}`;
  }
  return `standalone-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
}

export function buildHistorySessionId(taskId: string | null, standaloneSessionId: string | null): string | null {
  if (taskId && taskId.trim().length > 0) {
    return `claude:${taskId.trim()}`;
  }
  if (standaloneSessionId && standaloneSessionId.trim().length > 0) {
    return `standalone:${standaloneSessionId.trim()}`;
  }
  return null;
}
