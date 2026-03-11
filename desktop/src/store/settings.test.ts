import { beforeEach, describe, expect, it, vi } from 'vitest';

const STORAGE_KEY = 'plan-cascade-settings';

function seedLegacySettingsState() {
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      state: {
        defaultMode: 'simple',
        knowledgeAutoEnsureDocsCollection: false,
        standaloneContextTurns: 8,
        enableThinking: false,
        showReasoningOutput: false,
        showSubAgentEvents: false,
        enableContextCompaction: false,
        phaseConfigs: {
          planning: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
          implementation: { defaultAgent: 'aider', fallbackChain: ['codex', 'claude-code'] },
          retry: { defaultAgent: 'aider', fallbackChain: ['claude-code'] },
          refactor: { defaultAgent: 'codex', fallbackChain: ['aider'] },
          review: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
        },
      },
      version: 1,
    }),
  );
}

describe('settings store migration', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();

    if (!window.matchMedia) {
      Object.defineProperty(window, 'matchMedia', {
        writable: true,
        value: vi.fn().mockImplementation((query: string) => ({
          matches: false,
          media: query,
          onchange: null,
          addListener: vi.fn(),
          removeListener: vi.fn(),
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          dispatchEvent: vi.fn(),
        })),
      });
    }
  });

  it('forces v2 defaults during migration from legacy persisted state', async () => {
    seedLegacySettingsState();

    const { useSettingsStore } = await import('./settings');
    const state = useSettingsStore.getState();

    expect(state.defaultMode).toBe('expert');
    expect(state.knowledgeAutoEnsureDocsCollection).toBe(true);
    expect(state.standaloneContextTurns).toBe(-1);
    expect(state.enableThinking).toBe(true);
    expect(state.showReasoningOutput).toBe(true);
    expect(state.showSubAgentEvents).toBe(true);
    expect(state.enableContextCompaction).toBe(true);

    expect(state.phaseConfigs.planning.defaultAgent).toBe('');
    expect(state.phaseConfigs.implementation.defaultAgent).toBe('');
    expect(state.phaseConfigs.retry.defaultAgent).toBe('');
    expect(state.phaseConfigs.refactor.defaultAgent).toBe('');
    expect(state.phaseConfigs.review.defaultAgent).toBe('');
    expect(state.phaseConfigs.plan_strategy.defaultAgent).toBe('');
    expect(state.phaseConfigs.plan_clarification.defaultAgent).toBe('');
    expect(state.phaseConfigs.plan_generation.defaultAgent).toBe('');
    expect(state.phaseConfigs.plan_execution.defaultAgent).toBe('');
    expect(state.phaseConfigs.plan_retry.defaultAgent).toBe('');

    expect(state.phaseConfigs.implementation.fallbackChain).toEqual(['codex', 'claude-code']);
    expect(state.memorySettings.autoExtractEnabled).toBe(true);
    expect(state.memorySettings.reviewMode).toBe('llm_review');
    expect(state.memorySettings.reviewAgentRef).toBe('');
    expect(state.developerModeEnabled).toBe(false);
    expect(state.developerPanels).toEqual({
      contextInspector: false,
      workflowReliability: false,
      executionLogs: false,
      streamingOutput: true,
    });
    expect(state.developerSettingsInitialized).toBe(false);

    const persisted = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}');
    expect(persisted.version).toBe(9);
  });

  it('does not keep forcing values after migration has completed', async () => {
    seedLegacySettingsState();

    const { useSettingsStore } = await import('./settings');
    useSettingsStore.setState({
      defaultMode: 'simple',
      knowledgeAutoEnsureDocsCollection: false,
      standaloneContextTurns: 8,
    });

    const persistApi = (useSettingsStore as unknown as { persist?: { rehydrate: () => Promise<void> } }).persist;
    await persistApi?.rehydrate();

    const state = useSettingsStore.getState();
    expect(state.defaultMode).toBe('simple');
    expect(state.knowledgeAutoEnsureDocsCollection).toBe(false);
    expect(state.standaloneContextTurns).toBe(8);
  });
});
