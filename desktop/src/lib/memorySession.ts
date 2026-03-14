import type { MemoryEntry, MemoryScope } from '../types/skillMemory';

interface ResolveActiveMemorySessionIdOptions {
  foregroundOriginSessionId?: string | null;
  bindingSessionId?: string | null;
  taskId?: string | null;
  standaloneSessionId?: string | null;
}

function normalizePrefixedSessionId(value: string | null | undefined): string | null {
  const trimmed = value?.trim() ?? '';
  if (!trimmed) return null;
  if (trimmed.startsWith('claude:') || trimmed.startsWith('standalone:')) {
    return trimmed;
  }
  return null;
}

export function resolveActiveMemorySessionId(options: ResolveActiveMemorySessionIdOptions): string | null {
  const foregroundOriginSessionId = normalizePrefixedSessionId(options.foregroundOriginSessionId);
  if (foregroundOriginSessionId) return foregroundOriginSessionId;

  const bindingSessionId = normalizePrefixedSessionId(options.bindingSessionId);
  if (bindingSessionId) return bindingSessionId;

  const taskId = options.taskId?.trim() ?? '';
  if (taskId) return `claude:${taskId}`;

  const standaloneSessionId = options.standaloneSessionId?.trim() ?? '';
  if (standaloneSessionId) return `standalone:${standaloneSessionId}`;

  return null;
}

export function inferMemoryScope(entry: Pick<MemoryEntry, 'scope' | 'project_path'>): MemoryScope {
  if (entry.scope === 'project' || entry.scope === 'global' || entry.scope === 'session') {
    return entry.scope;
  }
  if (entry.project_path === '__global__') return 'global';
  if (entry.project_path.startsWith('__session__:')) return 'session';
  return 'project';
}
