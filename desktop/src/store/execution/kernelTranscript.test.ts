import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StreamLine } from './types';

const workflowKernelState = vi.hoisted(() => ({
  getCachedModeTranscript: vi.fn(),
  openSession: vi.fn(),
  patchModeTranscript: vi.fn(),
  session: null as Record<string, unknown> | null,
}));

vi.mock('../workflowKernel', () => ({
  useWorkflowKernelStore: {
    getState: () => workflowKernelState,
  },
}));

import {
  buildForkedChatSessionPayload,
  buildOptimisticKernelChatTranscript,
  forkKernelChatSessionAtTurn,
} from './kernelTranscript';

function buildTranscript(): StreamLine[] {
  return [
    {
      id: 1,
      content: 'first user',
      type: 'info',
      timestamp: 1,
      turnId: 1,
      turnBoundary: 'user',
    },
    {
      id: 2,
      content: 'first assistant',
      type: 'text',
      timestamp: 2,
      turnId: 1,
    },
    {
      id: 3,
      content: 'second user',
      type: 'info',
      timestamp: 3,
      turnId: 2,
      turnBoundary: 'user',
    },
    {
      id: 4,
      content: 'second assistant',
      type: 'text',
      timestamp: 4,
      turnId: 2,
    },
  ];
}

describe('kernelTranscript fork helpers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    workflowKernelState.session = {
      sessionId: 'root-session-1',
      modeSnapshots: {
        chat: {
          turnCount: 2,
          lastUserMessage: 'second user',
        },
      },
    };
    workflowKernelState.getCachedModeTranscript.mockReturnValue({
      lines: buildTranscript(),
    });
    workflowKernelState.openSession.mockResolvedValue({
      sessionId: 'fork-session-1',
    });
    workflowKernelState.patchModeTranscript.mockResolvedValue({
      sessionId: 'fork-session-1',
      mode: 'chat',
      revision: 1,
      lines: buildTranscript().slice(0, 2),
    });
  });

  it('builds a truncated transcript payload for the selected user turn', () => {
    const payload = buildForkedChatSessionPayload(buildTranscript(), 1);

    expect(payload).toEqual({
      conversationContext: [
        {
          user: 'first user',
          assistant: 'first assistant',
        },
      ],
      truncatedLines: buildTranscript().slice(0, 2),
    });
  });

  it('optimistically appends the pending user turn when transcript lags session state', () => {
    const laggingTranscript = buildTranscript().slice(0, 2);

    const lines = buildOptimisticKernelChatTranscript('root-session-1', laggingTranscript, 'second user');

    expect(lines).toEqual([
      ...laggingTranscript,
      expect.objectContaining({
        type: 'info',
        content: 'second user',
        turnBoundary: 'user',
        turnId: 2,
      }),
    ]);
  });

  it('does not append an optimistic user turn once transcript turn count is already current', () => {
    workflowKernelState.session = {
      sessionId: 'root-session-1',
      modeSnapshots: {
        chat: {
          turnCount: 2,
          lastUserMessage: 'second user',
        },
      },
    };

    const lines = buildOptimisticKernelChatTranscript('root-session-1', buildTranscript(), 'second user');

    expect(lines).toEqual(buildTranscript());
  });

  it('opens and seeds a new workflow session when forking a chat turn', async () => {
    const result = await forkKernelChatSessionAtTurn({
      rootSessionId: 'root-session-1',
      userLineId: 1,
      displayTitle: 'Forked billing session',
      workspacePath: '/repo/app',
      artifactRefs: ['artifact.md'],
      contextSources: ['chat_history'],
    });

    expect(workflowKernelState.openSession).toHaveBeenCalledWith(
      'chat',
      expect.objectContaining({
        conversationContext: [
          {
            user: 'first user',
            assistant: 'first assistant',
          },
        ],
        artifactRefs: ['artifact.md'],
        contextSources: ['simple_mode', 'chat_fork', 'chat_history'],
        metadata: expect.objectContaining({
          entry: 'fork_chat_turn',
          sourceSessionId: 'root-session-1',
          sourceUserLineId: 1,
          displayTitle: 'Forked billing session',
          workspacePath: '/repo/app',
        }),
      }),
    );
    expect(workflowKernelState.patchModeTranscript).toHaveBeenCalledWith('fork-session-1', 'chat', {
      replaceFromLineId: 0,
      appendedLines: buildTranscript().slice(0, 2),
    });
    expect(result).toEqual({
      sessionId: 'fork-session-1',
    });
  });
});
