/**
 * Test Utilities
 *
 * Shared helpers for component tests including mock factories,
 * custom render wrapper, and store reset utilities.
 *
 * Story 009: React Component Test Coverage
 */

import { render, RenderOptions } from '@testing-library/react';
import { ReactElement } from 'react';
import type { Project, Session, SessionDetails } from '../../types/project';
import type { StrategyAnalysis, DimensionScores } from '../../store/execution';
import type { InterviewSession, InterviewQuestion, InterviewHistoryEntry } from '../../store/specInterview';
import type { DashboardSummary, UsageStats, ModelUsage, ProjectUsage, TimeSeriesPoint } from '../../store/analytics';

// ============================================================================
// Mock Data Factories
// ============================================================================

export function createMockProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'project-1',
    name: 'My Test Project',
    path: '/home/user/projects/test',
    last_activity: '2025-03-15T10:30:00Z',
    session_count: 5,
    message_count: 42,
    ...overrides,
  };
}

export function createMockSession(overrides: Partial<Session> = {}): Session {
  return {
    id: 'session-1',
    project_id: 'project-1',
    file_path: '/home/user/.claude/sessions/session-1.json',
    created_at: '2025-03-15T10:00:00Z',
    last_activity: '2025-03-15T10:30:00Z',
    message_count: 12,
    first_message_preview: 'Implement the login page',
    ...overrides,
  };
}

export function createMockSessionDetails(overrides: Partial<SessionDetails> = {}): SessionDetails {
  return {
    session: createMockSession(),
    messages: [
      { message_type: 'human', content_preview: 'Implement the login page', timestamp: '2025-03-15T10:00:00Z' },
      { message_type: 'assistant', content_preview: 'I will create the login...', timestamp: '2025-03-15T10:01:00Z' },
    ],
    checkpoint_count: 2,
    ...overrides,
  };
}

export function createMockDimensionScores(overrides: Partial<DimensionScores> = {}): DimensionScores {
  return {
    scope: 0.6,
    complexity: 0.7,
    risk: 0.3,
    parallelization: 0.5,
    ...overrides,
  };
}

export function createMockStrategyAnalysis(overrides: Partial<StrategyAnalysis> = {}): StrategyAnalysis {
  return {
    strategy: 'hybrid_auto',
    confidence: 0.85,
    reasoning: 'Medium complexity task with multiple components that benefit from parallel execution.',
    estimated_stories: 5,
    estimated_features: 1,
    estimated_duration_hours: 4,
    complexity_indicators: ['multiple files', 'API integration'],
    recommendations: ['Use hybrid auto for optimal parallelization'],
    dimension_scores: createMockDimensionScores(),
    ...overrides,
  };
}

export function createMockInterviewQuestion(overrides: Partial<InterviewQuestion> = {}): InterviewQuestion {
  return {
    id: 'q-1',
    question: 'What is the primary purpose of this project?',
    phase: 'overview',
    hint: 'Describe the main goal',
    required: true,
    input_type: 'text',
    field_name: 'purpose',
    options: [],
    allow_custom: false,
    ...overrides,
  };
}

export function createMockInterviewHistoryEntry(overrides: Partial<InterviewHistoryEntry> = {}): InterviewHistoryEntry {
  return {
    turn_number: 1,
    phase: 'overview',
    question: 'What is the primary purpose of this project?',
    answer: 'Build a task management application',
    timestamp: '2025-03-15T10:05:00Z',
    ...overrides,
  };
}

export function createMockInterviewSession(overrides: Partial<InterviewSession> = {}): InterviewSession {
  return {
    id: 'interview-1',
    status: 'in_progress',
    phase: 'overview',
    flow_level: 'standard',
    description: 'Build a task management application',
    question_cursor: 2,
    max_questions: 18,
    current_question: createMockInterviewQuestion(),
    progress: 15,
    history: [createMockInterviewHistoryEntry()],
    ...overrides,
  };
}

export function createMockUsageStats(overrides: Partial<UsageStats> = {}): UsageStats {
  return {
    total_input_tokens: 150000,
    total_output_tokens: 50000,
    total_cost_microdollars: 2500000, // $2.50
    request_count: 42,
    avg_tokens_per_request: 4762,
    avg_cost_per_request: 59524,
    ...overrides,
  };
}

export function createMockModelUsage(overrides: Partial<ModelUsage> = {}): ModelUsage {
  return {
    model_name: 'claude-sonnet-4-20250514',
    provider: 'anthropic',
    stats: createMockUsageStats(),
    ...overrides,
  };
}

export function createMockProjectUsage(overrides: Partial<ProjectUsage> = {}): ProjectUsage {
  return {
    project_id: 'proj-1',
    project_name: 'My Project',
    stats: createMockUsageStats(),
    ...overrides,
  };
}

export function createMockTimeSeriesPoint(overrides: Partial<TimeSeriesPoint> = {}): TimeSeriesPoint {
  return {
    timestamp: 1710500000,
    timestamp_formatted: '2025-03-15',
    stats: createMockUsageStats(),
    ...overrides,
  };
}

export function createMockDashboardSummary(overrides: Partial<DashboardSummary> = {}): DashboardSummary {
  return {
    current_period: createMockUsageStats(),
    previous_period: createMockUsageStats({ total_cost_microdollars: 2000000 }),
    cost_change_percent: 25.0,
    tokens_change_percent: 10.0,
    requests_change_percent: 15.0,
    by_model: [createMockModelUsage()],
    by_project: [createMockProjectUsage()],
    time_series: [createMockTimeSeriesPoint()],
    ...overrides,
  };
}

// ============================================================================
// Custom Render
// ============================================================================

/**
 * Custom render wrapper. Currently just wraps the default render
 * but can be extended with providers as needed.
 */
function customRender(ui: ReactElement, options?: Omit<RenderOptions, 'wrapper'>) {
  return render(ui, { ...options });
}

export { customRender as render };

// ============================================================================
// Store Reset Utilities
// ============================================================================

/**
 * Reset a zustand store to its initial state.
 * Call this in beforeEach to ensure clean state between tests.
 */
export function resetZustandStore(store: { setState: (state: unknown) => void; getInitialState?: () => unknown }) {
  if (store.getInitialState) {
    store.setState(store.getInitialState());
  }
}
