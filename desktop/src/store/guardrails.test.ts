/**
 * Guardrails Store Tests
 *
 * Unit tests for the Zustand guardrails store.
 * These test the store logic in isolation without Tauri IPC calls.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock the API module before importing the store
vi.mock('../lib/guardrailsApi', () => ({
  listGuardrails: vi.fn(),
  toggleGuardrailEnabled: vi.fn(),
  addCustomRule: vi.fn(),
  removeCustomRule: vi.fn(),
  getTriggerLog: vi.fn(),
  clearTriggerLog: vi.fn(),
}));

import { useGuardrailsStore } from './guardrails';
import * as api from '../lib/guardrailsApi';

describe('GuardrailsStore', () => {
  beforeEach(() => {
    // Reset store state before each test
    useGuardrailsStore.setState({
      guardrails: [],
      triggerLog: [],
      isLoading: false,
      isTogglingGuardrail: false,
      isAddingRule: false,
      isLoadingLog: false,
      error: null,
    });
    vi.clearAllMocks();
  });

  describe('fetchGuardrails', () => {
    it('should load guardrails on success', async () => {
      const mockGuardrails = [
        { name: 'SensitiveData', guardrail_type: 'builtin', enabled: true, description: 'Detects API keys' },
        { name: 'CodeSecurity', guardrail_type: 'builtin', enabled: true, description: 'Detects eval()' },
      ];

      vi.mocked(api.listGuardrails).mockResolvedValue({
        success: true,
        data: mockGuardrails,
        error: null,
      });

      await useGuardrailsStore.getState().fetchGuardrails();

      const state = useGuardrailsStore.getState();
      expect(state.guardrails).toEqual(mockGuardrails);
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('should set error on failure', async () => {
      vi.mocked(api.listGuardrails).mockResolvedValue({
        success: false,
        data: null,
        error: 'Not initialized',
      });

      await useGuardrailsStore.getState().fetchGuardrails();

      const state = useGuardrailsStore.getState();
      expect(state.guardrails).toEqual([]);
      expect(state.error).toBe('Not initialized');
    });
  });

  describe('toggleGuardrail', () => {
    it('should optimistically update guardrail state', async () => {
      useGuardrailsStore.setState({
        guardrails: [{ name: 'SensitiveData', guardrail_type: 'builtin', enabled: true, description: '' }],
      });

      vi.mocked(api.toggleGuardrailEnabled).mockResolvedValue({
        success: true,
        data: true,
        error: null,
      });

      await useGuardrailsStore.getState().toggleGuardrail('SensitiveData', false);

      const state = useGuardrailsStore.getState();
      expect(state.guardrails[0].enabled).toBe(false);
    });
  });

  describe('addCustomRule', () => {
    it('should add a rule on success', async () => {
      const newRule = {
        name: 'NoTODO',
        guardrail_type: 'custom',
        enabled: true,
        description: 'User-defined guardrail rule',
      };

      vi.mocked(api.addCustomRule).mockResolvedValue({
        success: true,
        data: newRule,
        error: null,
      });

      const result = await useGuardrailsStore.getState().addCustomRule('NoTODO', 'TODO', 'warn');

      expect(result).toBe(true);
      const state = useGuardrailsStore.getState();
      expect(state.guardrails).toHaveLength(1);
      expect(state.guardrails[0].name).toBe('NoTODO');
    });

    it('should return false on failure', async () => {
      vi.mocked(api.addCustomRule).mockResolvedValue({
        success: false,
        data: null,
        error: 'Invalid regex',
      });

      const result = await useGuardrailsStore.getState().addCustomRule('Bad', '[invalid', 'warn');

      expect(result).toBe(false);
      const state = useGuardrailsStore.getState();
      expect(state.error).toBe('Invalid regex');
    });
  });

  describe('removeCustomRule', () => {
    it('should remove a rule on success', async () => {
      useGuardrailsStore.setState({
        guardrails: [{ name: 'MyRule', guardrail_type: 'custom', enabled: true, description: '' }],
      });

      vi.mocked(api.removeCustomRule).mockResolvedValue({
        success: true,
        data: true,
        error: null,
      });

      const result = await useGuardrailsStore.getState().removeCustomRule('MyRule');

      expect(result).toBe(true);
      const state = useGuardrailsStore.getState();
      expect(state.guardrails).toHaveLength(0);
    });
  });

  describe('fetchTriggerLog', () => {
    it('should load trigger log entries', async () => {
      const mockLog = [
        {
          id: 1,
          guardrail_name: 'SensitiveData',
          direction: 'input',
          result_type: 'redact',
          content_snippet: 'sk-abc...',
          timestamp: '2026-02-17T12:00:00Z',
        },
      ];

      vi.mocked(api.getTriggerLog).mockResolvedValue({
        success: true,
        data: mockLog,
        error: null,
      });

      await useGuardrailsStore.getState().fetchTriggerLog(50, 0);

      const state = useGuardrailsStore.getState();
      expect(state.triggerLog).toEqual(mockLog);
      expect(state.isLoadingLog).toBe(false);
    });
  });

  describe('clearTriggerLog', () => {
    it('should clear the trigger log', async () => {
      useGuardrailsStore.setState({
        triggerLog: [
          {
            id: 1,
            guardrail_name: 'Test',
            direction: 'input',
            result_type: 'warn',
            content_snippet: '',
            timestamp: '',
          },
        ],
      });

      vi.mocked(api.clearTriggerLog).mockResolvedValue({
        success: true,
        data: true,
        error: null,
      });

      await useGuardrailsStore.getState().clearTriggerLog();

      const state = useGuardrailsStore.getState();
      expect(state.triggerLog).toHaveLength(0);
    });
  });

  describe('clearError', () => {
    it('should clear the error state', () => {
      useGuardrailsStore.setState({ error: 'Some error' });
      useGuardrailsStore.getState().clearError();
      expect(useGuardrailsStore.getState().error).toBeNull();
    });
  });
});
