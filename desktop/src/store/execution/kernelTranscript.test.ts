import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StreamLine } from './types';

const workflowKernelState = vi.hoisted(() => ({
  getCachedModeTranscript: vi.fn(),
  openSession: vi.fn(),
  patchModeTranscript: vi.fn(),
}));

vi.mock('../workflowKernel', () => ({
  useWorkflowKernelStore: {
    getState: () => workflowKernelState,
  },
}));

import { buildForkedChatSessionPayload, forkKernelChatSessionAtTurn } from './kernelTranscript';

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
