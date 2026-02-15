/**
 * SimpleMode Component Tests
 *
 * Tests the one-click execution flow, strategy recommendation display,
 * streaming progress rendering, and idle state.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SimpleMode } from '../SimpleMode/index';
import { createMockStrategyAnalysis } from './test-utils';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

// Mock react-i18next
vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

// Track store state for controlled testing
const mockAnalyzeStrategy = vi.fn();
const mockStart = vi.fn();
const mockSendFollowUp = vi.fn();
const mockReset = vi.fn();
const mockClearStrategyAnalysis = vi.fn();
const mockInitialize = vi.fn();
const mockCleanup = vi.fn();

let mockStoreState = {
  status: 'idle' as string,
  connectionStatus: 'connected' as string,
  isSubmitting: false,
  apiError: null as string | null,
  start: mockStart,
  sendFollowUp: mockSendFollowUp,
  reset: mockReset,
  result: null as { success: boolean; message: string } | null,
  initialize: mockInitialize,
  cleanup: mockCleanup,
  analyzeStrategy: mockAnalyzeStrategy,
  strategyAnalysis: null as ReturnType<typeof createMockStrategyAnalysis> | null,
  isAnalyzingStrategy: false,
  clearStrategyAnalysis: mockClearStrategyAnalysis,
  isChatSession: false,
  streamingOutput: [] as Array<Record<string, unknown>>,
  standaloneTurns: [] as Array<Record<string, unknown>>,
  history: [] as Array<Record<string, unknown>>,
  clearHistory: vi.fn(),
  deleteHistory: vi.fn(),
  renameHistory: vi.fn(),
  restoreFromHistory: vi.fn(),
  sessionUsageTotals: null,
  latestUsage: null,
  analysisCoverage: null,
  logs: [] as string[],
  attachments: [] as Array<Record<string, unknown>>,
  addAttachment: vi.fn(),
  removeAttachment: vi.fn(),
  backgroundSessions: {} as Record<string, unknown>,
  switchToSession: vi.fn(),
  removeBackgroundSession: vi.fn(),
};

vi.mock('../../store/execution', () => ({
  useExecutionStore: () => mockStoreState,
}));

// Mock child components to isolate SimpleMode logic
vi.mock('../SimpleMode/InputBox', () => ({
  InputBox: ({ value, onChange, onSubmit, disabled, placeholder, isLoading }: {
    value: string; onChange: (v: string) => void; onSubmit: () => void;
    disabled: boolean; placeholder: string; isLoading: boolean;
  }) => (
    <div data-testid="input-box">
      <input
        data-testid="task-input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
      />
      <button data-testid="submit-btn" onClick={onSubmit} disabled={disabled}>
        {isLoading ? 'Loading...' : 'Execute'}
      </button>
    </div>
  ),
}));

vi.mock('../SimpleMode/ProgressView', () => ({
  ProgressView: () => <div data-testid="progress-view">Progress View</div>,
}));

vi.mock('../SimpleMode/ResultView', () => ({
  ResultView: ({ result }: { result: unknown }) => (
    <div data-testid="result-view">{JSON.stringify(result)}</div>
  ),
}));

vi.mock('../SimpleMode/HistoryPanel', () => ({
  HistoryPanel: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="history-panel">
      <button onClick={onClose}>Close History</button>
    </div>
  ),
}));

vi.mock('../SimpleMode/ConnectionStatus', () => ({
  ConnectionStatus: ({ status }: { status: string }) => (
    <div data-testid="connection-status">{status}</div>
  ),
}));

// Mock shared streaming/progress components imported by SimpleMode
vi.mock('../shared', () => ({
  StreamingOutput: () => <div data-testid="streaming-output">Streaming</div>,
  GlobalProgressBar: () => <div data-testid="global-progress">Progress</div>,
  ErrorState: () => <div data-testid="error-state">Errors</div>,
  ProjectSelector: () => <div data-testid="project-selector">Project Selector</div>,
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('SimpleMode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState = {
      status: 'idle',
      connectionStatus: 'connected',
      isSubmitting: false,
      apiError: null,
      start: mockStart,
      sendFollowUp: mockSendFollowUp,
      reset: mockReset,
      result: null,
      initialize: mockInitialize,
      cleanup: mockCleanup,
      analyzeStrategy: mockAnalyzeStrategy,
      strategyAnalysis: null,
      isAnalyzingStrategy: false,
      clearStrategyAnalysis: mockClearStrategyAnalysis,
      isChatSession: false,
      streamingOutput: [],
      standaloneTurns: [],
      history: [],
      clearHistory: vi.fn(),
      deleteHistory: vi.fn(),
      renameHistory: vi.fn(),
      restoreFromHistory: vi.fn(),
      sessionUsageTotals: null,
      latestUsage: null,
      analysisCoverage: null,
      logs: [],
      attachments: [],
      addAttachment: vi.fn(),
      removeAttachment: vi.fn(),
      backgroundSessions: {},
      switchToSession: vi.fn(),
      removeBackgroundSession: vi.fn(),
    };
  });

  it('renders idle state with input box and empty state message', () => {
    render(<SimpleMode />);

    expect(screen.getByTestId('input-box')).toBeInTheDocument();
    expect(screen.getByTestId('connection-status')).toHaveTextContent('connected');
    // The rocket emoji element with role="img"
    expect(screen.getByRole('img', { name: 'rocket' })).toBeInTheDocument();
  });

  it('calls initialize on mount and cleanup on unmount', () => {
    const { unmount } = render(<SimpleMode />);

    expect(mockInitialize).toHaveBeenCalledTimes(1);

    unmount();
    expect(mockCleanup).toHaveBeenCalledTimes(1);
  });

  it('executes one-click flow: type description -> click execute -> analyzeStrategy -> start', async () => {
    const mockAnalysis = createMockStrategyAnalysis();
    mockAnalyzeStrategy.mockResolvedValue(mockAnalysis);

    render(<SimpleMode />);

    // Type a task description
    const input = screen.getByTestId('task-input');
    fireEvent.change(input, { target: { value: 'Build a login page' } });

    // Click execute
    const submitBtn = screen.getByTestId('submit-btn');
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(mockAnalyzeStrategy).toHaveBeenCalledWith('Build a login page');
    });

    // After analysis succeeds, start should be called
    await waitFor(() => {
      expect(mockStart).toHaveBeenCalledWith('Build a login page', 'simple');
    });
  });

  it('does not start execution when analyzeStrategy returns null', async () => {
    mockAnalyzeStrategy.mockResolvedValue(null);

    render(<SimpleMode />);

    const input = screen.getByTestId('task-input');
    fireEvent.change(input, { target: { value: 'Some task' } });

    fireEvent.click(screen.getByTestId('submit-btn'));

    await waitFor(() => {
      expect(mockAnalyzeStrategy).toHaveBeenCalled();
    });

    expect(mockStart).not.toHaveBeenCalled();
  });

  it('displays strategy analysis banner when strategy is being analyzed', () => {
    mockStoreState.isAnalyzingStrategy = true;

    render(<SimpleMode />);

    expect(screen.getByText('Analyzing task complexity...')).toBeInTheDocument();
  });

  it('displays strategy recommendation with confidence when analysis completes', () => {
    const analysis = createMockStrategyAnalysis({
      strategy: 'hybrid_auto',
      confidence: 0.85,
      reasoning: 'Medium complexity task benefits from parallel execution.',
    });
    mockStoreState.strategyAnalysis = analysis;

    render(<SimpleMode />);

    expect(screen.getByText(/hybrid auto/i)).toBeInTheDocument();
    expect(screen.getByText('(85% confidence)')).toBeInTheDocument();
    expect(screen.getByText('Medium complexity task benefits from parallel execution.')).toBeInTheDocument();
  });

  it('shows ProgressView when status is running', () => {
    mockStoreState.status = 'running';

    render(<SimpleMode />);

    expect(screen.getByTestId('progress-view')).toBeInTheDocument();
  });

  it('shows ResultView when status is completed', () => {
    mockStoreState.status = 'completed';
    mockStoreState.result = { success: true, message: 'Done' };

    render(<SimpleMode />);

    expect(screen.getByTestId('result-view')).toBeInTheDocument();
  });

  it('displays API error message when present', () => {
    mockStoreState.apiError = 'Connection refused';

    render(<SimpleMode />);

    expect(screen.getByText('Connection refused')).toBeInTheDocument();
  });

  it('toggles history panel visibility', () => {
    render(<SimpleMode />);

    // History should not be visible initially
    expect(screen.queryByTestId('history-panel')).not.toBeInTheDocument();

    // Click history button
    fireEvent.click(screen.getByText('history.button'));

    // History panel should appear
    expect(screen.getByTestId('history-panel')).toBeInTheDocument();
  });

  it('disables input during submission', () => {
    mockStoreState.isSubmitting = true;

    render(<SimpleMode />);

    expect(screen.getByTestId('task-input')).toBeDisabled();
    expect(screen.getByTestId('submit-btn')).toBeDisabled();
  });
});
