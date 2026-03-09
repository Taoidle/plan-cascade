import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TabbedRightPanel } from './TabbedRightPanel';

const mockRefreshPolicy = vi.fn();
const mockRefreshObservability = vi.fn();

const mockSettingsState = {
  developerModeEnabled: false,
  developerPanels: {
    contextInspector: false,
    workflowReliability: false,
    executionLogs: false,
    streamingOutput: true,
  },
};

const mockContextOpsState = {
  policy: { context_inspector_ui: false },
  refreshPolicy: mockRefreshPolicy,
};

const mockObservabilityState = {
  snapshot: {
    metrics: {
      workflowLinkRehydrateTotal: 15,
      workflowLinkRehydrateSuccess: 15,
      workflowLinkRehydrateFailure: 0,
      interactiveActionFailTotal: 0,
      prdFeedbackApplyTotal: 0,
      prdFeedbackApplySuccess: 0,
      prdFeedbackApplyFailure: 0,
    },
    interactiveActionFailBreakdown: [],
    latestFailure: null,
  },
  refreshSnapshot: mockRefreshObservability,
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, string | number>) => {
      const translations: Record<string, string> = {
        'rightPanel.outputTab': 'Output',
        'rightPanel.gitTab': 'Git',
        'rightPanel.contextTab': 'Context',
        'rightPanel.workflowFailures.title': 'Workflow Reliability',
        'rightPanel.workflowFailures.none': 'No recent workflow failures.',
        'rightPanel.developerHidden.title': 'Developer content is hidden',
        'rightPanel.developerHidden.description': 'Enable panels in Settings > General > Developer Mode.',
        'analysisCoverage.title': 'Analysis Coverage',
        'analysisCoverage.observed': 'Observed',
        'analysisCoverage.readDepth': 'Read Depth',
        'analysisCoverage.testsRead': 'Tests Read',
        'executionLogs.title': 'Execution Logs',
      };

      const template = translations[key] ?? key;
      if (!params) return template;
      return Object.entries(params).reduce(
        (result, [name, value]) => result.replace(new RegExp(`{{${name}}}`, 'g'), String(value)),
        template,
      );
    },
  }),
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector: (state: typeof mockSettingsState) => unknown) => selector(mockSettingsState),
}));

vi.mock('../../store/contextOps', () => ({
  useContextOpsStore: (selector: (state: typeof mockContextOpsState) => unknown) => selector(mockContextOpsState),
}));

vi.mock('../../store/workflowObservability', () => ({
  useWorkflowObservabilityStore: (selector: (state: typeof mockObservabilityState) => unknown) =>
    selector(mockObservabilityState),
}));

vi.mock('../../store/execution', async () => {
  const actual = await vi.importActual<typeof import('../../store/execution')>('../../store/execution');
  return {
    ...actual,
  };
});

vi.mock('./GitPanel', () => ({
  GitPanel: () => <div>Git Panel</div>,
}));

vi.mock('./ContextOpsPanel', () => ({
  ContextOpsPanel: () => <div>Context Panel</div>,
}));

vi.mock('./WorkflowKernelProgressPanel', () => ({
  WorkflowKernelProgressPanel: () => <div>Workflow Event Timeline</div>,
}));

vi.mock('../shared', () => ({
  StreamingOutput: () => <div>Streaming Output</div>,
  ErrorState: () => null,
}));

describe('TabbedRightPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSettingsState.developerModeEnabled = false;
    mockSettingsState.developerPanels = {
      contextInspector: false,
      workflowReliability: false,
      executionLogs: false,
      streamingOutput: true,
    };
    mockContextOpsState.policy.context_inspector_ui = false;
  });

  it('keeps workflow progress visible while hiding developer panels when developer mode is off', () => {
    render(
      <TabbedRightPanel
        activeTab="output"
        onTabChange={vi.fn()}
        workflowMode="plan"
        workflowPhase="idle"
        logs={['line 1']}
        analysisCoverage={null}
        executionStatus="idle"
        modeTranscriptLines={[]}
        workspacePath="/repo"
        contextSessionId={null}
      />,
    );

    expect(screen.queryByText('Context')).not.toBeInTheDocument();
    expect(screen.getByText('Workflow Event Timeline')).toBeInTheDocument();
    expect(screen.queryByText('Workflow Reliability')).not.toBeInTheDocument();
    expect(screen.queryByText('Execution Logs')).not.toBeInTheDocument();
    expect(screen.queryByText('Streaming Output')).not.toBeInTheDocument();
  });

  it('shows analysis coverage even when developer mode is off', () => {
    render(
      <TabbedRightPanel
        activeTab="output"
        onTabChange={vi.fn()}
        workflowMode="plan"
        workflowPhase="idle"
        logs={[]}
        analysisCoverage={{
          status: 'completed',
          successfulPhases: 1,
          partialPhases: 0,
          failedPhases: 0,
          coverageRatio: 0.5,
          sampledReadRatio: 0.25,
          testCoverageRatio: 0.1,
          observedTestCoverageRatio: 0.1,
          observedPaths: 5,
          inventoryTotalFiles: 10,
          sampledReadFiles: 3,
          testFilesRead: 1,
          testFilesTotal: 4,
          coverageTargetRatio: 0.8,
          sampledReadTargetRatio: 0.6,
          testCoverageTargetRatio: 0.2,
          validationIssues: [],
          updatedAt: Date.now(),
        }}
        executionStatus="idle"
        modeTranscriptLines={[]}
        workspacePath="/repo"
        contextSessionId={null}
      />,
    );

    expect(screen.getByText('Analysis Coverage')).toBeInTheDocument();
  });

  it('requires developer mode and context capability to show the context tab', () => {
    mockSettingsState.developerModeEnabled = true;
    mockSettingsState.developerPanels.contextInspector = true;
    mockContextOpsState.policy.context_inspector_ui = true;

    render(
      <TabbedRightPanel
        activeTab="output"
        onTabChange={vi.fn()}
        workflowMode="chat"
        workflowPhase="idle"
        logs={[]}
        analysisCoverage={null}
        executionStatus="idle"
        modeTranscriptLines={[]}
        workspacePath="/repo"
        contextSessionId={null}
      />,
    );

    expect(screen.getByText('Context')).toBeInTheDocument();
  });

  it('shows enabled developer panels when developer mode is on', () => {
    mockSettingsState.developerModeEnabled = true;
    mockSettingsState.developerPanels = {
      contextInspector: true,
      workflowReliability: true,
      executionLogs: true,
      streamingOutput: true,
    };
    mockContextOpsState.policy.context_inspector_ui = true;

    render(
      <TabbedRightPanel
        activeTab="output"
        onTabChange={vi.fn()}
        workflowMode="chat"
        workflowPhase="streaming"
        logs={['line 1', 'line 2']}
        analysisCoverage={null}
        executionStatus="running"
        modeTranscriptLines={[]}
        workspacePath="/repo"
        contextSessionId={null}
      />,
    );

    expect(screen.getByText('Workflow Event Timeline')).toBeInTheDocument();
    expect(screen.getByText('Workflow Reliability')).toBeInTheDocument();
    expect(screen.getByText('Execution Logs')).toBeInTheDocument();
    expect(screen.getByText('Streaming Output')).toBeInTheDocument();
  });
});
