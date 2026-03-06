import { describe, expect, it } from 'vitest';

import { selectKernelChatRuntime } from './workflowKernelSelectors';
import type { WorkflowSession } from '../types/workflowKernel';

function createSession(): WorkflowSession {
  return {
    sessionId: 'root-1',
    sessionKind: 'simple_root',
    displayTitle: 'Chat session',
    workspacePath: '/tmp/project',
    status: 'active',
    activeMode: 'chat',
    modeSnapshots: {
      chat: {
        phase: 'streaming',
        draftInput: 'draft',
        turnCount: 2,
        lastUserMessage: 'hi',
        lastAssistantMessage: null,
      },
      plan: null,
      task: null,
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: {
      chat: 'claude:session-1',
    },
    backgroundState: 'foreground',
    contextLedger: {
      conversationTurnCount: 2,
      artifactRefCount: 0,
      contextSourceKinds: [],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeRuntimeMeta: {},
    lastError: null,
    createdAt: '2026-03-06T00:00:00Z',
    updatedAt: '2026-03-06T00:00:00Z',
    lastCheckpointId: null,
  };
}

describe('workflowKernelSelectors', () => {
  it('selectKernelChatRuntime exposes the linked chat session id', () => {
    const runtime = selectKernelChatRuntime(createSession());

    expect(runtime.linkedSessionId).toBe('claude:session-1');
    expect(runtime.phase).toBe('streaming');
    expect(runtime.pendingPrompt).toBe('draft');
    expect(runtime.isActive).toBe(true);
  });
});
