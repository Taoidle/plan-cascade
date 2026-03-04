import type { WorkflowMode } from '../types/workflowKernel';

function sanitizeSegment(value: string | null | undefined, fallback: string): string {
  const normalized = (value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return normalized || fallback;
}

export function createWorkflowClientRequestId(mode?: WorkflowMode | null, action?: string | null): string {
  const modePart = sanitizeSegment(mode ?? 'unknown', 'unknown');
  const actionPart = sanitizeSegment(action ?? 'request', 'request');
  const now = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 10);
  return `wf_${modePart}_${actionPart}_${now}_${random}`;
}

export function withWorkflowClientRequestMetadata<T extends Record<string, unknown>>(
  metadata: T | null | undefined,
  mode?: WorkflowMode | null,
  action?: string | null,
): T & { clientRequestId: string } {
  const base = (metadata ?? {}) as Record<string, unknown>;
  const existing = typeof base.clientRequestId === 'string' ? base.clientRequestId.trim() : '';
  const clientRequestId = existing || createWorkflowClientRequestId(mode, action);
  return {
    ...(base as T),
    clientRequestId,
  };
}
