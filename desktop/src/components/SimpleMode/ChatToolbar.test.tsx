import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ChatToolbar } from './ChatToolbar';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue || _key,
  }),
}));

vi.mock('../shared/ContextSourceBar', () => ({
  ContextSourceBar: () => <div data-testid="context-source-bar" />,
}));

function renderToolbar(debugWorkflowActive: boolean) {
  render(
    <ChatToolbar
      workflowMode="debug"
      onWorkflowModeChange={() => undefined}
      onFilePick={() => undefined}
      isFilePickDisabled={false}
      executionStatus="idle"
      onPause={() => undefined}
      onResume={() => undefined}
      onCancel={() => undefined}
      debugWorkflowActive={debugWorkflowActive}
      onCancelWorkflow={() => undefined}
      onExportImage={() => undefined}
      isExportDisabled={false}
      isCapturing={false}
      rightPanelOpen={false}
      rightPanelTab="output"
      onToggleOutput={() => undefined}
      detailLineCount={0}
    />,
  );
}

describe('ChatToolbar', () => {
  it('does not show cancel workflow for an idle debug toolbar state', () => {
    renderToolbar(false);

    expect(screen.queryByText('Cancel workflow')).not.toBeInTheDocument();
  });

  it('shows cancel workflow only when debug workflow is marked running', () => {
    renderToolbar(true);

    expect(screen.getByText('workflow.cancelWorkflow')).toBeInTheDocument();
  });
});
