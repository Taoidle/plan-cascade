import { describe, expect, it } from 'vitest';
import { inferMemoryScope, resolveActiveMemorySessionId } from './memorySession';

describe('memorySession helpers', () => {
  it('prefers prefixed workflow and history session ids', () => {
    expect(
      resolveActiveMemorySessionId({
        foregroundOriginSessionId: 'standalone:history-1',
        bindingSessionId: 'claude:runtime-1',
        taskId: 'task-1',
        standaloneSessionId: 'standalone-1',
      }),
    ).toBe('standalone:history-1');

    expect(
      resolveActiveMemorySessionId({
        foregroundOriginSessionId: null,
        bindingSessionId: 'claude:runtime-1',
        taskId: 'task-1',
        standaloneSessionId: 'standalone-1',
      }),
    ).toBe('claude:runtime-1');
  });

  it('builds scoped ids from raw task and standalone session ids', () => {
    expect(
      resolveActiveMemorySessionId({
        taskId: 'task-1',
        standaloneSessionId: null,
      }),
    ).toBe('claude:task-1');

    expect(
      resolveActiveMemorySessionId({
        taskId: null,
        standaloneSessionId: 'simple-1',
      }),
    ).toBe('standalone:simple-1');
  });

  it('infers memory scope from explicit scope or sentinel project path', () => {
    expect(inferMemoryScope({ scope: 'session', project_path: '/tmp/project' })).toBe('session');
    expect(inferMemoryScope({ scope: undefined, project_path: '__global__' })).toBe('global');
    expect(inferMemoryScope({ scope: undefined, project_path: '__session__:abc-1' })).toBe('session');
    expect(inferMemoryScope({ scope: undefined, project_path: '/tmp/project' })).toBe('project');
  });
});
