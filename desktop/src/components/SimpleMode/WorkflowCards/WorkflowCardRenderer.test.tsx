import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

const planOrchestratorHarness = vi.hoisted(() => ({
  retryClarification: vi.fn().mockResolvedValue(undefined),
  skipClarification: vi.fn().mockResolvedValue(undefined),
  cancelWorkflow: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key,
  }),
}));

vi.mock('../../../store/planOrchestrator', () => ({
  usePlanOrchestratorStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({
      phase: 'clarification_error',
      retryClarification: planOrchestratorHarness.retryClarification,
      skipClarification: planOrchestratorHarness.skipClarification,
      cancelWorkflow: planOrchestratorHarness.cancelWorkflow,
    }),
}));

import { WorkflowCardRenderer } from './WorkflowCardRenderer';

describe('WorkflowCardRenderer', () => {
  it('renders a visible warning card for unknown card types', () => {
    render(
      <WorkflowCardRenderer
        payload={
          {
            cardType: 'unknown_card_type',
            cardId: 'card-42',
            data: {},
            interactive: false,
          } as never
        }
      />,
    );

    expect(screen.getByText('Unknown workflow card')).toBeTruthy();
    expect(screen.getByText(/unknown_card_type/)).toBeTruthy();
    expect(screen.getByText(/card-42/)).toBeTruthy();
  });

  it('renders clarification resolution actions and dispatches retry/skip/cancel', async () => {
    render(
      <WorkflowCardRenderer
        payload={{
          cardType: 'plan_clarification_resolution',
          cardId: 'card-100',
          interactive: true,
          data: {
            title: 'Clarification needs attention',
            message: 'Clarification failed. Please choose next action.',
            reasonCode: 'clarification_submit_failed',
            canRetry: true,
            canSkip: true,
            canCancel: true,
          },
        }}
      />,
    );

    fireEvent.click(screen.getByText('Retry'));
    await waitFor(() => expect(planOrchestratorHarness.retryClarification).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByText('Skip clarification'));
    await waitFor(() => expect(planOrchestratorHarness.skipClarification).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByText('Cancel workflow'));
    await waitFor(() => expect(planOrchestratorHarness.cancelWorkflow).toHaveBeenCalledTimes(1));
  });
});
