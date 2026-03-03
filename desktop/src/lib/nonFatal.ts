export function createTraceId(prefix = 'trace'): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}`;
}

export function reportNonFatal(
  scope: string,
  error: unknown,
  metadata?: Record<string, unknown>,
): { message: string; traceId: string } {
  const message = error instanceof Error ? error.message : String(error);
  const traceId = createTraceId(scope.replace(/[^a-zA-Z0-9_-]/g, '').slice(0, 24) || 'nonfatal');
  console.warn(`[non-fatal][${scope}] ${message}`, {
    trace_id: traceId,
    ...(metadata || {}),
  });
  return { message, traceId };
}
