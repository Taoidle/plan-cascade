/**
 * ExpertMode Component Tests
 *
 * Tests the SpecInterviewPanel conversation flow, StrategySelector
 * recommendation display, and execution control interactions.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SpecInterviewPanel } from '../ExpertMode/SpecInterviewPanel';
import { StrategySelector } from '../ExpertMode/StrategySelector';
import {
  createMockInterviewSession,
  createMockInterviewQuestion,
  createMockInterviewHistoryEntry,
  createMockStrategyAnalysis,
} from './test-utils';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

// Mock spec interview store
const mockStartInterview = vi.fn();
const mockSubmitAnswer = vi.fn();
const mockCompileSpec = vi.fn();
const mockInterviewReset = vi.fn();
const mockClearError = vi.fn();

let mockSpecInterviewState = {
  session: null as ReturnType<typeof createMockInterviewSession> | null,
  compiledSpec: null as {
    spec_json: Record<string, unknown>;
    spec_md: string;
    prd_json: Record<string, unknown>;
  } | null,
  loading: { starting: false, submitting: false, fetching: false, compiling: false },
  error: null as string | null,
  startInterview: mockStartInterview,
  submitAnswer: mockSubmitAnswer,
  compileSpec: mockCompileSpec,
  reset: mockInterviewReset,
  clearError: mockClearError,
};

vi.mock('../../store/specInterview', () => ({
  useSpecInterviewStore: () => mockSpecInterviewState,
  getPhaseLabel: (phase: string) => {
    const labels: Record<string, string> = {
      overview: 'Overview',
      scope: 'Scope',
      requirements: 'Requirements',
      interfaces: 'Interfaces',
      stories: 'Stories',
      review: 'Review',
      complete: 'Complete',
    };
    return labels[phase] || phase;
  },
  getPhaseOrder: () => ['overview', 'scope', 'requirements', 'interfaces', 'stories', 'review', 'complete'],
}));

// Mock PRD store for StrategySelector
const mockSetStrategy = vi.fn();
let mockPRDState = {
  prd: {
    stories: [],
    strategy: 'hybrid_auto' as string,
  },
  setStrategy: mockSetStrategy,
};

vi.mock('../../store/prd', () => ({
  usePRDStore: () => mockPRDState,
}));

// Mock execution store for StrategySelector
const mockAnalyzeStrategy = vi.fn();
let mockExecutionState = {
  strategyAnalysis: null as ReturnType<typeof createMockStrategyAnalysis> | null,
  isAnalyzingStrategy: false,
  analyzeStrategy: mockAnalyzeStrategy,
};

vi.mock('../../store/execution', () => ({
  useExecutionStore: () => mockExecutionState,
}));

// Mock Radix UI Tooltip to avoid portal issues in tests
vi.mock('@radix-ui/react-tooltip', () => ({
  Provider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Root: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Trigger: ({ children, asChild }: { children: React.ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <span>{children}</span>,
  Portal: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Content: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Arrow: () => null,
}));

// --------------------------------------------------------------------------
// SpecInterviewPanel Tests
// --------------------------------------------------------------------------

describe('SpecInterviewPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSpecInterviewState = {
      session: null,
      compiledSpec: null,
      loading: { starting: false, submitting: false, fetching: false, compiling: false },
      error: null,
      startInterview: mockStartInterview,
      submitAnswer: mockSubmitAnswer,
      compileSpec: mockCompileSpec,
      reset: mockInterviewReset,
      clearError: mockClearError,
    };
  });

  it('displays flow level options (quick, standard, full)', () => {
    render(<SpecInterviewPanel />);

    expect(screen.getByText('Quick')).toBeInTheDocument();
    expect(screen.getByText('Standard')).toBeInTheDocument();
    expect(screen.getByText('Full')).toBeInTheDocument();
  });

  it('hides Skip button for required questions', () => {
    mockSpecInterviewState.session = createMockInterviewSession({
      current_question: createMockInterviewQuestion({ required: true }),
    });

    render(<SpecInterviewPanel />);

    expect(screen.queryByText('Skip')).not.toBeInTheDocument();
  });

  it('renders conversation history entries', () => {
    const history = [
      createMockInterviewHistoryEntry({
        turn_number: 1,
        question: 'What do you want to build?',
        answer: 'A task manager',
      }),
      createMockInterviewHistoryEntry({
        turn_number: 2,
        question: 'Who is the audience?',
        answer: 'Developers',
      }),
    ];

    mockSpecInterviewState.session = createMockInterviewSession({
      history,
      current_question: createMockInterviewQuestion(),
    });

    render(<SpecInterviewPanel />);

    expect(screen.getByText('What do you want to build?')).toBeInTheDocument();
    expect(screen.getByText('A task manager')).toBeInTheDocument();
    expect(screen.getByText('Who is the audience?')).toBeInTheDocument();
    expect(screen.getByText('Developers')).toBeInTheDocument();
  });
});

// --------------------------------------------------------------------------
// StrategySelector Tests
// --------------------------------------------------------------------------

describe('StrategySelector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mockPRDState = {
      prd: { stories: [], strategy: 'hybrid_auto' },
      setStrategy: mockSetStrategy,
    };
    mockExecutionState = {
      strategyAnalysis: null,
      isAnalyzingStrategy: false,
      analyzeStrategy: mockAnalyzeStrategy,
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders all three strategy options', () => {
    render(<StrategySelector />);

    expect(screen.getByText('Direct')).toBeInTheDocument();
    expect(screen.getByText('Hybrid Auto')).toBeInTheDocument();
    expect(screen.getByText('Mega Plan')).toBeInTheDocument();
  });

  it('shows strategy descriptions', () => {
    render(<StrategySelector />);

    expect(screen.getByText(/Execute task directly/)).toBeInTheDocument();
    expect(screen.getByText(/Automatic PRD generation/)).toBeInTheDocument();
    expect(screen.getByText(/Full project planning/)).toBeInTheDocument();
  });

  it('selects a strategy when clicking on an option', () => {
    render(<StrategySelector />);

    // Click the "Direct" option label
    const directLabel = screen.getByText('Direct').closest('label');
    if (directLabel) fireEvent.click(directLabel);

    expect(mockSetStrategy).toHaveBeenCalledWith('direct');
  });

  it('displays analyzing banner when strategy analysis is in progress', () => {
    mockExecutionState.isAnalyzingStrategy = true;

    render(<StrategySelector />);

    expect(screen.getByText('Analyzing task complexity...')).toBeInTheDocument();
  });

  it('displays AI recommendation with confidence score', () => {
    mockExecutionState.strategyAnalysis = createMockStrategyAnalysis({
      strategy: 'hybrid_auto',
      confidence: 0.92,
      reasoning: 'Task has multiple interdependent features.',
    });

    render(<StrategySelector />);

    expect(screen.getByText(/AI Recommendation: Hybrid Auto/)).toBeInTheDocument();
    expect(screen.getByText(/High confidence/)).toBeInTheDocument();
    expect(screen.getAllByText(/92%/).length).toBeGreaterThan(0);
    expect(screen.getByText('Task has multiple interdependent features.')).toBeInTheDocument();
  });

  it('shows AI Pick badge on the recommended strategy option', () => {
    mockExecutionState.strategyAnalysis = createMockStrategyAnalysis({
      strategy: 'hybrid_auto',
      confidence: 0.85,
    });

    render(<StrategySelector />);

    expect(screen.getByText(/AI Pick/)).toBeInTheDocument();
  });

  it('shows dimension scores in the analysis banner', () => {
    mockExecutionState.strategyAnalysis = createMockStrategyAnalysis({
      dimension_scores: { scope: 0.6, complexity: 0.7, risk: 0.3, parallelization: 0.5 },
    });

    render(<StrategySelector />);

    expect(screen.getByText('Scope')).toBeInTheDocument();
    expect(screen.getByText('Complexity')).toBeInTheDocument();
    expect(screen.getByText('Risk')).toBeInTheDocument();
    expect(screen.getByText('Parallel')).toBeInTheDocument();
  });

  it('triggers auto-analysis when task description is provided', async () => {
    render(<StrategySelector taskDescription="Build a complex dashboard with charts" />);

    // Advance past the debounce delay
    vi.advanceTimersByTime(600);

    expect(mockAnalyzeStrategy).toHaveBeenCalledWith('Build a complex dashboard with charts');
  });

  it('does not trigger analysis for short descriptions', () => {
    render(<StrategySelector taskDescription="short" />);

    vi.advanceTimersByTime(600);

    expect(mockAnalyzeStrategy).not.toHaveBeenCalled();
  });
});

// --------------------------------------------------------------------------
// DesignDocPanel Tests
// --------------------------------------------------------------------------

const mockGenerateDesignDoc = vi.fn();
const mockImportDesignDoc = vi.fn();
const mockLoadDesignDoc = vi.fn();
const mockDesignDocReset = vi.fn();
const mockDesignDocClearError = vi.fn();

let mockDesignDocState = {
  designDoc: null as Record<string, unknown> | null,
  generationInfo: null as Record<string, unknown> | null,
  importWarnings: null as Array<{ message: string; field: string | null; severity: string }> | null,
  loading: { generating: false, importing: false, loading: false },
  error: null as string | null,
  generateDesignDoc: mockGenerateDesignDoc,
  importDesignDoc: mockImportDesignDoc,
  loadDesignDoc: mockLoadDesignDoc,
  reset: mockDesignDocReset,
  clearError: mockDesignDocClearError,
};

vi.mock('../../store/designDoc', () => ({
  useDesignDocStore: () => mockDesignDocState,
}));

// Lazy import to apply mock
const { DesignDocPanel } = await import('../ExpertMode/DesignDocPanel');

describe('DesignDocPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDesignDocState = {
      designDoc: null,
      generationInfo: null,
      importWarnings: null,
      loading: { generating: false, importing: false, loading: false },
      error: null,
      generateDesignDoc: mockGenerateDesignDoc,
      importDesignDoc: mockImportDesignDoc,
      loadDesignDoc: mockLoadDesignDoc,
      reset: mockDesignDocReset,
      clearError: mockDesignDocClearError,
    };
  });

  it('renders action panel when no design doc is loaded', () => {
    render(<DesignDocPanel />);

    expect(screen.getAllByText(/Generate/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Import/i).length).toBeGreaterThan(0);
  });

  it('shows loading state during generation', () => {
    mockDesignDocState.loading.generating = true;

    render(<DesignDocPanel />);

    expect(screen.getByText(/Generating/i)).toBeInTheDocument();
  });
});
