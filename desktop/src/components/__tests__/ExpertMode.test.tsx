/**
 * ExpertMode Component Tests
 *
 * Tests the SpecInterviewPanel conversation flow, StrategySelector
 * recommendation display, and execution control interactions.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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
  compiledSpec: null as { spec_json: Record<string, unknown>; spec_md: string; prd_json: Record<string, unknown> } | null,
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

  it('renders the start interview form when no session exists', () => {
    render(<SpecInterviewPanel />);

    expect(screen.getByText('Spec Interview')).toBeInTheDocument();
    expect(screen.getByText(/Answer a series of questions/)).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Describe what you want to build...')).toBeInTheDocument();
    expect(screen.getByText('Start Interview')).toBeInTheDocument();
  });

  it('displays flow level options (quick, standard, full)', () => {
    render(<SpecInterviewPanel />);

    expect(screen.getByText('Quick')).toBeInTheDocument();
    expect(screen.getByText('Standard')).toBeInTheDocument();
    expect(screen.getByText('Full')).toBeInTheDocument();
  });

  it('submits start interview form with correct config', async () => {
    render(<SpecInterviewPanel />);

    // Fill in description
    const textarea = screen.getByPlaceholderText('Describe what you want to build...');
    fireEvent.change(textarea, { target: { value: 'Build a task manager' } });

    // Click "Full" flow level
    fireEvent.click(screen.getByText('Full'));

    // Submit
    fireEvent.click(screen.getByText('Start Interview'));

    await waitFor(() => {
      expect(mockStartInterview).toHaveBeenCalledWith(
        expect.objectContaining({
          description: 'Build a task manager',
          flow_level: 'full',
          max_questions: 18,
          first_principles: false,
          project_path: null,
        })
      );
    });
  });

  it('disables submit button when description is empty', () => {
    render(<SpecInterviewPanel />);

    const submitButton = screen.getByText('Start Interview');
    expect(submitButton).toBeDisabled();
  });

  it('shows "Starting Interview..." while loading', () => {
    mockSpecInterviewState.loading = { ...mockSpecInterviewState.loading, starting: true };

    render(<SpecInterviewPanel />);

    expect(screen.getByText('Starting Interview...')).toBeInTheDocument();
  });

  it('displays active interview with progress bar and question', () => {
    const question = createMockInterviewQuestion({
      question: 'What is the target audience?',
      phase: 'scope',
    });
    mockSpecInterviewState.session = createMockInterviewSession({
      phase: 'scope',
      progress: 30,
      current_question: question,
    });

    render(<SpecInterviewPanel />);

    // Phase labels should be visible in progress bar
    expect(screen.getAllByText('Overview').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Scope').length).toBeGreaterThan(0);
    expect(screen.getByText(/30\s*% complete/)).toBeInTheDocument();

    // Current question should be displayed
    expect(screen.getByText('What is the target audience?')).toBeInTheDocument();
  });

  it('submits an answer to the current question', async () => {
    mockSpecInterviewState.session = createMockInterviewSession({
      current_question: createMockInterviewQuestion({
        question: 'What is the purpose?',
        input_type: 'text',
        required: false,
      }),
    });

    render(<SpecInterviewPanel />);

    const input = screen.getByPlaceholderText(/Describe the main goal|Type your answer/);
    fireEvent.change(input, { target: { value: 'Build a fast app' } });

    fireEvent.click(screen.getByText('Submit'));

    await waitFor(() => {
      expect(mockSubmitAnswer).toHaveBeenCalledWith('Build a fast app');
    });
  });

  it('shows Skip button for non-required questions', () => {
    mockSpecInterviewState.session = createMockInterviewSession({
      current_question: createMockInterviewQuestion({ required: false }),
    });

    render(<SpecInterviewPanel />);

    expect(screen.getByText('Skip')).toBeInTheDocument();
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

  it('shows compile action when session is finalized', () => {
    mockSpecInterviewState.session = createMockInterviewSession({
      status: 'finalized',
      current_question: null,
    });

    render(<SpecInterviewPanel />);

    expect(screen.getByText('Interview Complete')).toBeInTheDocument();
    expect(screen.getByText('Compile Specification')).toBeInTheDocument();
  });

  it('shows compiled spec results after compilation', () => {
    mockSpecInterviewState.session = createMockInterviewSession({ status: 'finalized' });
    mockSpecInterviewState.compiledSpec = {
      spec_json: { title: 'Test Spec' },
      spec_md: '# Test Spec\n\nThis is a test.',
      prd_json: { stories: [] },
    };

    render(<SpecInterviewPanel />);

    expect(screen.getByText('Compiled Specification')).toBeInTheDocument();
    expect(screen.getByText('spec.md')).toBeInTheDocument();
    expect(screen.getByText('spec.json')).toBeInTheDocument();
    expect(screen.getByText('prd.json')).toBeInTheDocument();
  });

  it('displays error banner and dismiss button', () => {
    mockSpecInterviewState.session = createMockInterviewSession();
    mockSpecInterviewState.error = 'Connection timeout';

    render(<SpecInterviewPanel />);

    expect(screen.getByText('Connection timeout')).toBeInTheDocument();
    expect(screen.getByText('Dismiss')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Dismiss'));
    expect(mockClearError).toHaveBeenCalled();
  });

  it('calls reset when Cancel Interview is clicked', () => {
    mockSpecInterviewState.session = createMockInterviewSession();

    render(<SpecInterviewPanel />);

    fireEvent.click(screen.getByText('Cancel Interview'));
    expect(mockInterviewReset).toHaveBeenCalled();
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

  it('displays error message with dismiss option', () => {
    mockDesignDocState.error = 'Failed to generate design document';

    render(<DesignDocPanel />);

    expect(screen.getByText('Failed to generate design document')).toBeInTheDocument();
    expect(screen.getByText('Dismiss')).toBeInTheDocument();
  });

  it('renders document viewer with collapsible sections when doc is loaded', () => {
    mockDesignDocState.designDoc = {
      metadata: { created_at: null, version: '1.0.0', source: 'ai-generated', level: 'feature', mega_plan_reference: null },
      overview: { title: 'Test Design', summary: 'Test summary', goals: ['goal1'], non_goals: ['non-goal1'] },
      architecture: {
        system_overview: 'Overview text',
        components: [{ name: 'ComponentA', description: 'Desc', responsibilities: ['r1'], dependencies: [], features: [] }],
        data_flow: 'Data flow description',
        patterns: [{ name: 'Pattern1', description: 'P desc', rationale: 'reason', applies_to: [] }],
        infrastructure: { existing_services: [], new_services: [] },
      },
      interfaces: { api_standards: { style: 'REST', error_handling: 'standard', async_pattern: 'async/await' }, shared_data_models: [] },
      decisions: [{ id: 'ADR-F001', title: 'Use React', context: 'ctx', decision: 'dec', rationale: 'rat', alternatives_considered: [], status: 'accepted', applies_to: [] }],
      feature_mappings: {},
    };

    render(<DesignDocPanel />);

    expect(screen.getByText('Test Design')).toBeInTheDocument();
    expect(screen.getByText('ComponentA')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Design Patterns/i }));
    expect(screen.getByText('Pattern1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Architecture Decisions/i }));
    expect(screen.getByText('ADR-F001')).toBeInTheDocument();
  });

  it('shows import warnings banner when warnings exist', () => {
    mockDesignDocState.designDoc = {
      metadata: { created_at: null, version: '1.0.0', source: 'imported', level: 'feature', mega_plan_reference: null },
      overview: { title: 'Imported Doc', summary: 'summary', goals: [], non_goals: [] },
      architecture: { system_overview: '', components: [], data_flow: '', patterns: [], infrastructure: { existing_services: [], new_services: [] } },
      interfaces: { api_standards: { style: '', error_handling: '', async_pattern: '' }, shared_data_models: [] },
      decisions: [],
      feature_mappings: {},
    };
    mockDesignDocState.importWarnings = [
      { message: 'Missing components section', field: 'architecture.components', severity: 'medium' },
    ];

    render(<DesignDocPanel />);

    expect(screen.getByText(/warning/i)).toBeInTheDocument();
  });

  it('calls reset when reset button is clicked', () => {
    mockDesignDocState.designDoc = {
      metadata: { created_at: null, version: '1.0.0', source: 'ai-generated', level: 'feature', mega_plan_reference: null },
      overview: { title: 'Doc', summary: '', goals: [], non_goals: [] },
      architecture: { system_overview: '', components: [], data_flow: '', patterns: [], infrastructure: { existing_services: [], new_services: [] } },
      interfaces: { api_standards: { style: '', error_handling: '', async_pattern: '' }, shared_data_models: [] },
      decisions: [],
      feature_mappings: {},
    };

    render(<DesignDocPanel />);

    const resetBtn = screen.getByRole('button', { name: /new document/i });
    if (resetBtn) {
      fireEvent.click(resetBtn);
      expect(mockDesignDocReset).toHaveBeenCalled();
    }
  });
});
