import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { assembleTurnContext, prepareTurnContextV2 } from './contextApi';

const mockInvoke = vi.mocked(invoke);

describe('contextApi', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('prepareTurnContextV2 delegates to assemble_turn_context and maps envelope data', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: {
        request_meta: {
          turn_id: 'turn-1',
          mode: 'chat',
          query: 'q',
        },
        assembled_prompt: 'assembled',
        trace_id: 'trace-1',
        budget: {
          input_token_budget: 1000,
          reserved_output_tokens: 200,
          hard_limit: 1200,
          used_input_tokens: 300,
          over_budget: false,
        },
        sources: [],
        blocks: [],
        compaction: {
          triggered: false,
          trigger_reason: 'within_budget',
          strategy: 'none',
          before_tokens: 300,
          after_tokens: 300,
          compaction_tokens: 0,
          net_saving: 0,
          quality_score: 1,
        },
        injected_source_kinds: ['history'],
        fallback_used: false,
        fallback_reason: null,
      },
      error: null,
    });

    const response = await prepareTurnContextV2({
      project_path: '/tmp/project',
      query: 'q',
    });

    expect(mockInvoke).toHaveBeenCalledWith('assemble_turn_context', {
      request: {
        project_path: '/tmp/project',
        query: 'q',
      },
    });
    expect(response.success).toBe(true);
    expect(response.data?.assembled_prompt).toBe('assembled');
    expect(response.data?.trace_id).toBe('trace-1');
  });

  it('prepareTurnContextV2 returns error when assembly fails', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: false,
      data: null,
      error: 'assembly failed',
    });

    const response = await prepareTurnContextV2({
      project_path: '/tmp/project',
      query: 'q',
    });

    expect(response.success).toBe(false);
    expect(response.error).toBe('assembly failed');
  });

  it('assembleTurnContext calls assemble_turn_context command', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: false,
      data: null,
      error: 'noop',
    });

    await assembleTurnContext({
      project_path: '/tmp/project',
      query: 'hello',
    });

    expect(mockInvoke).toHaveBeenCalledWith('assemble_turn_context', {
      request: {
        project_path: '/tmp/project',
        query: 'hello',
      },
    });
  });
});
