import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { PhaseAgentSection } from './PhaseAgentSection';

const mockUpdatePhaseConfig = vi.fn();
const mockInvoke = vi.fn();

const mockSettingsState = {
  agents: [
    { name: 'claude-code', enabled: true, command: 'claude', isDefault: true },
    { name: 'codex', enabled: true, command: 'codex', isDefault: false },
  ],
  phaseConfigs: {
    plan_strategy: { defaultAgent: '', fallbackChain: [] },
    plan_exploration: { defaultAgent: '', fallbackChain: [] },
    plan_interview: { defaultAgent: '', fallbackChain: [] },
    plan_requirements: { defaultAgent: '', fallbackChain: [] },
    plan_architecture: { defaultAgent: '', fallbackChain: [] },
    plan_prd: { defaultAgent: '', fallbackChain: [] },
    planning: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
    implementation: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
    retry: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
    refactor: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
    review: { defaultAgent: 'claude-code', fallbackChain: ['codex'] },
  },
  updatePhaseConfig: mockUpdatePhaseConfig,
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'phases.title': 'Phase Agents',
        'phases.description': 'Configure phase agents',
        'phases.cliAgents': 'CLI Agents',
        'phases.globalDefault': '(Global Default)',
        'phases.columns.phase': 'Phase',
        'phases.columns.defaultAgent': 'Default Agent',
        'phases.columns.fallbackChain': 'Fallback Chain',
        'phases.groups.planning': 'Planning',
        'phases.groups.planningDesc': 'Planning phases',
        'phases.groups.execution': 'Execution',
        'phases.groups.executionDesc': 'Execution phases',
        'phases.planStrategy.name': 'Plan Strategy',
        'phases.planStrategy.description': 'Plan strategy',
        'phases.planExploration.name': 'Plan Exploration',
        'phases.planExploration.description': 'Plan exploration',
        'phases.planInterview.name': 'Plan Interview',
        'phases.planInterview.description': 'Plan interview',
        'phases.planRequirements.name': 'Plan Requirements',
        'phases.planRequirements.description': 'Plan requirements',
        'phases.planArchitecture.name': 'Plan Architecture',
        'phases.planArchitecture.description': 'Plan architecture',
        'phases.planPrd.name': 'Plan PRD',
        'phases.planPrd.description': 'Plan PRD',
        'phases.planning.name': 'Execution Planning',
        'phases.planning.description': 'Execution planning phase',
        'phases.implementation.name': 'Implementation',
        'phases.implementation.description': 'Implementation phase',
        'phases.retry.name': 'Retry',
        'phases.retry.description': 'Retry phase',
        'phases.refactor.name': 'Refactor',
        'phases.refactor.description': 'Refactor phase',
        'phases.review.name': 'Review',
        'phases.review.description': 'Review phase',
        'phases.fallback.noFallbacks': 'No fallbacks',
        'phases.fallback.title': 'Fallback Chain Configuration',
        'phases.fallback.help': 'Fallback help',
        'phases.fallback.addFallback': '+ Add fallback agent...',
        'phases.fallback.moveUp': 'Move up',
        'phases.fallback.moveDown': 'Move down',
        'phases.fallback.remove': 'Remove',
        'phases.info.title': 'How phase assignment works',
        'phases.info.description': 'Description',
      };
      return translations[key] || key;
    },
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: () => mockSettingsState,
}));

describe('PhaseAgentSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue({ success: true, data: ['anthropic'], error: null });
    mockSettingsState.phaseConfigs.planning.defaultAgent = 'claude-code';
  });

  it('includes global default option in execution phase default-agent select', async () => {
    render(<PhaseAgentSection />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    const selects = screen.getAllByRole('combobox');
    const executionPlanningSelect = selects[6] as HTMLSelectElement;
    const hasGlobalDefaultOption = Array.from(executionPlanningSelect.options).some(
      (option) => option.value === '' && option.textContent === '(Global Default)',
    );

    expect(hasGlobalDefaultOption).toBe(true);
  });

  it('allows selecting global default for execution phases', async () => {
    render(<PhaseAgentSection />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    const selects = screen.getAllByRole('combobox');
    const executionPlanningSelect = selects[6] as HTMLSelectElement;

    fireEvent.change(executionPlanningSelect, { target: { value: '' } });

    expect(mockUpdatePhaseConfig).toHaveBeenCalledWith('planning', { defaultAgent: '' });
  });
});
