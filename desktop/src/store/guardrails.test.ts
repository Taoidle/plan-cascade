import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../lib/guardrailsApi', () => ({
  listGuardrails: vi.fn(),
  setGuardrailMode: vi.fn(),
  toggleGuardrailEnabled: vi.fn(),
  createCustomGuardrail: vi.fn(),
  updateGuardrail: vi.fn(),
  deleteGuardrail: vi.fn(),
  listGuardrailEvents: vi.fn(),
  clearGuardrailEvents: vi.fn(),
}));

import { useGuardrailsStore } from './guardrails';
import * as api from '../lib/guardrailsApi';
import type { GuardrailInfo } from '../lib/guardrailsApi';

const builtinRule: GuardrailInfo = {
  id: 'builtin-sensitive-data',
  name: 'SensitiveData',
  guardrail_type: 'builtin' as const,
  enabled: true,
  description: 'Detects API keys',
  scope: ['input', 'tool_call', 'tool_result', 'assistant_output', 'artifact'],
  action: 'mixed',
  editable: false,
};

const customRule: GuardrailInfo = {
  id: 'rule-1',
  name: 'NoTODO',
  guardrail_type: 'custom' as const,
  enabled: true,
  description: 'User-defined guardrail rule',
  scope: ['input'],
  action: 'warn',
  editable: true,
  pattern: 'TODO',
};

describe('GuardrailsStore', () => {
  beforeEach(() => {
    useGuardrailsStore.setState({
      guardrails: [],
      runtime: null,
      events: [],
      triggerLog: [],
      isLoading: false,
      isMutating: false,
      isLoadingEvents: false,
      isLoadingLog: false,
      error: null,
    });
    vi.clearAllMocks();
  });

  it('fetches guardrails and runtime state', async () => {
    vi.mocked(api.listGuardrails).mockResolvedValue({
      success: true,
      data: {
        guardrails: [builtinRule],
        runtime: {
          mode: 'strict',
          strict_mode: true,
          native_runtime_managed: true,
          claude_code_managed: false,
        },
      },
      error: null,
    });

    await useGuardrailsStore.getState().fetchGuardrails();

    expect(useGuardrailsStore.getState().guardrails).toEqual([builtinRule]);
    expect(useGuardrailsStore.getState().runtime?.native_runtime_managed).toBe(true);
  });

  it('updates guardrail mode', async () => {
    vi.mocked(api.setGuardrailMode).mockResolvedValue({
      success: true,
      data: {
        mode: 'monitor_only',
        strict_mode: false,
        native_runtime_managed: true,
        claude_code_managed: false,
      },
      error: null,
    });

    const result = await useGuardrailsStore.getState().setMode('monitor_only');

    expect(result).toBe(true);
    expect(useGuardrailsStore.getState().runtime?.mode).toBe('monitor_only');
  });

  it('toggles a guardrail by id', async () => {
    useGuardrailsStore.setState({ guardrails: [builtinRule] });
    vi.mocked(api.toggleGuardrailEnabled).mockResolvedValue({
      success: true,
      data: { ...builtinRule, enabled: false },
      error: null,
    });

    await useGuardrailsStore.getState().toggleGuardrail(builtinRule.id, false);

    expect(useGuardrailsStore.getState().guardrails[0].enabled).toBe(false);
  });

  it('creates a custom rule through compatibility action', async () => {
    vi.mocked(api.createCustomGuardrail).mockResolvedValue({
      success: true,
      data: customRule,
      error: null,
    });

    const result = await useGuardrailsStore.getState().addCustomRule('NoTODO', 'TODO', 'warn');

    expect(result).toBe(true);
    expect(useGuardrailsStore.getState().guardrails).toEqual([customRule]);
  });

  it('removes a custom rule through compatibility action', async () => {
    useGuardrailsStore.setState({ guardrails: [customRule] });
    vi.mocked(api.deleteGuardrail).mockResolvedValue({
      success: true,
      data: true,
      error: null,
    });

    const result = await useGuardrailsStore.getState().removeCustomRule(customRule.id);

    expect(result).toBe(true);
    expect(useGuardrailsStore.getState().guardrails).toHaveLength(0);
  });

  it('loads audit events through compatibility action', async () => {
    const events = [
      {
        id: 1,
        rule_id: customRule.id,
        rule_name: customRule.name,
        surface: 'input',
        decision: 'warn',
        content_hash: 'abc',
        safe_preview: '[REDACTED]',
        timestamp: '2026-03-13T00:00:00Z',
      },
    ];
    vi.mocked(api.listGuardrailEvents).mockResolvedValue({
      success: true,
      data: events,
      error: null,
    });

    await useGuardrailsStore.getState().fetchTriggerLog(50, 0);

    expect(useGuardrailsStore.getState().triggerLog).toEqual(events);
  });
});
