/**
 * ExecutionReport Component Tests
 *
 * Story 004: Execution Report visualization components
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SummaryCard } from '../SummaryCard';
import { QualityRadarChart } from '../QualityRadarChart';
import { TimelineWaterfall } from '../TimelineWaterfall';
import { AgentPerformanceTable } from '../AgentPerformanceTable';
import { ExecutionReport } from '../ExecutionReport';
import { useExecutionReportStore } from '../../../store/executionReport';
import type { ReportSummary, RadarDimension, TimelineEntry, AgentPerformance } from '../../../store/executionReport';

// ============================================================================
// Test Fixtures
// ============================================================================

function makeSummary(overrides?: Partial<ReportSummary>): ReportSummary {
  return {
    totalStories: 6,
    storiesPassed: 5,
    storiesFailed: 1,
    totalTimeMs: 45000,
    totalTokens: null,
    estimatedCost: null,
    successRate: 83,
    ...overrides,
  };
}

function makeRadarDimensions(): RadarDimension[] {
  return [
    { dimension: 'correctness', averageScore: 8, maxScore: 10, storyScores: { s1: 9, s2: 7 } },
    { dimension: 'readability', averageScore: 7, maxScore: 10, storyScores: { s1: 8, s2: 6 } },
    { dimension: 'performance', averageScore: 6, maxScore: 10, storyScores: { s1: 7, s2: 5 } },
    { dimension: 'security', averageScore: 9, maxScore: 10, storyScores: { s1: 9, s2: 9 } },
    { dimension: 'maintainability', averageScore: 7, maxScore: 10, storyScores: { s1: 8, s2: 6 } },
  ];
}

function makeTimelineEntries(): TimelineEntry[] {
  return [
    {
      storyId: 's1',
      storyTitle: 'Story One',
      batchIndex: 0,
      agent: 'claude-sonnet',
      durationMs: 10000,
      startOffsetMs: 0,
      status: 'completed',
      gateResult: 'passed',
    },
    {
      storyId: 's2',
      storyTitle: 'Story Two',
      batchIndex: 0,
      agent: 'claude-haiku',
      durationMs: 8000,
      startOffsetMs: 0,
      status: 'completed',
      gateResult: 'passed',
    },
    {
      storyId: 's3',
      storyTitle: 'Story Three',
      batchIndex: 1,
      agent: 'claude-sonnet',
      durationMs: 12000,
      startOffsetMs: 10000,
      status: 'failed',
      gateResult: 'failed',
    },
  ];
}

function makeAgentPerformance(): AgentPerformance[] {
  return [
    {
      agentName: 'claude-sonnet',
      storiesAssigned: 4,
      storiesCompleted: 3,
      successRate: 75,
      averageDurationMs: 11000,
      averageQualityScore: 38.5,
    },
    {
      agentName: 'claude-haiku',
      storiesAssigned: 2,
      storiesCompleted: 2,
      successRate: 100,
      averageDurationMs: 8000,
      averageQualityScore: 35.0,
    },
  ];
}

// ============================================================================
// SummaryCard Tests
// ============================================================================

describe('SummaryCard', () => {
  it('renders success rate', () => {
    render(<SummaryCard summary={makeSummary()} />);
    expect(screen.getByTestId('summary-card')).toBeDefined();
    expect(screen.getByText('83% Success')).toBeDefined();
  });

  it('renders story counts', () => {
    render(<SummaryCard summary={makeSummary()} />);
    expect(screen.getByText('6')).toBeDefined(); // total
    expect(screen.getByText('5')).toBeDefined(); // passed
    expect(screen.getByText('1')).toBeDefined(); // failed
  });

  it('renders duration', () => {
    render(<SummaryCard summary={makeSummary()} />);
    expect(screen.getByText('45.0s')).toBeDefined();
  });

  it('renders tokens when available', () => {
    render(<SummaryCard summary={makeSummary({ totalTokens: 150000 })} />);
    expect(screen.getByText('150,000')).toBeDefined();
  });

  it('renders cost when available', () => {
    render(<SummaryCard summary={makeSummary({ estimatedCost: 0.0542 })} />);
    expect(screen.getByText('$0.0542')).toBeDefined();
  });

  it('does not render tokens/cost when null', () => {
    render(<SummaryCard summary={makeSummary()} />);
    expect(screen.queryByText('Tokens')).toBeNull();
    expect(screen.queryByText('Est. Cost')).toBeNull();
  });
});

// ============================================================================
// QualityRadarChart Tests
// ============================================================================

describe('QualityRadarChart', () => {
  it('renders radar chart', () => {
    render(<QualityRadarChart dimensions={makeRadarDimensions()} />);
    expect(screen.getByTestId('quality-radar-chart')).toBeDefined();
    expect(screen.getByText('Quality Radar')).toBeDefined();
  });

  it('renders dimension labels', () => {
    render(<QualityRadarChart dimensions={makeRadarDimensions()} />);
    expect(screen.getByText('correctness')).toBeDefined();
    expect(screen.getByText('readability')).toBeDefined();
    expect(screen.getByText('performance')).toBeDefined();
  });

  it('renders story legend', () => {
    render(<QualityRadarChart dimensions={makeRadarDimensions()} />);
    expect(screen.getByText('s1')).toBeDefined();
    expect(screen.getByText('s2')).toBeDefined();
    expect(screen.getByText('Average')).toBeDefined();
  });

  it('handles empty dimensions', () => {
    render(<QualityRadarChart dimensions={[]} />);
    expect(screen.getByText('No quality data available')).toBeDefined();
  });
});

// ============================================================================
// TimelineWaterfall Tests
// ============================================================================

describe('TimelineWaterfall', () => {
  it('renders timeline', () => {
    render(<TimelineWaterfall entries={makeTimelineEntries()} totalDurationMs={22000} />);
    expect(screen.getByTestId('timeline-waterfall')).toBeDefined();
    expect(screen.getByText('Execution Timeline')).toBeDefined();
  });

  it('renders batch labels', () => {
    render(<TimelineWaterfall entries={makeTimelineEntries()} totalDurationMs={22000} />);
    expect(screen.getByText('Batch 0')).toBeDefined();
    expect(screen.getByText('Batch 1')).toBeDefined();
  });

  it('renders story titles', () => {
    render(<TimelineWaterfall entries={makeTimelineEntries()} totalDurationMs={22000} />);
    expect(screen.getByText('Story One')).toBeDefined();
    expect(screen.getByText('Story Two')).toBeDefined();
    expect(screen.getByText('Story Three')).toBeDefined();
  });

  it('renders agent legend', () => {
    render(<TimelineWaterfall entries={makeTimelineEntries()} totalDurationMs={22000} />);
    expect(screen.getByText('claude-sonnet')).toBeDefined();
    expect(screen.getByText('claude-haiku')).toBeDefined();
  });

  it('handles empty entries', () => {
    render(<TimelineWaterfall entries={[]} totalDurationMs={0} />);
    expect(screen.getByText('No timeline data')).toBeDefined();
  });
});

// ============================================================================
// AgentPerformanceTable Tests
// ============================================================================

describe('AgentPerformanceTable', () => {
  it('renders agent table', () => {
    render(<AgentPerformanceTable agents={makeAgentPerformance()} />);
    expect(screen.getByTestId('agent-performance-table')).toBeDefined();
    expect(screen.getByText('Agent Performance')).toBeDefined();
  });

  it('renders agent names', () => {
    render(<AgentPerformanceTable agents={makeAgentPerformance()} />);
    expect(screen.getByText('claude-sonnet')).toBeDefined();
    expect(screen.getByText('claude-haiku')).toBeDefined();
  });

  it('renders success rates', () => {
    render(<AgentPerformanceTable agents={makeAgentPerformance()} />);
    expect(screen.getByText('75%')).toBeDefined();
    expect(screen.getByText('100%')).toBeDefined();
  });

  it('renders quality scores', () => {
    render(<AgentPerformanceTable agents={makeAgentPerformance()} />);
    expect(screen.getByText('38.5')).toBeDefined();
    expect(screen.getByText('35.0')).toBeDefined();
  });

  it('handles empty agents', () => {
    render(<AgentPerformanceTable agents={[]} />);
    expect(screen.getByText('No agent data')).toBeDefined();
  });
});

// ============================================================================
// ExecutionReport Integration Tests
// ============================================================================

describe('ExecutionReport', () => {
  beforeEach(() => {
    useExecutionReportStore.getState().reset();
  });

  it('returns null when no report', () => {
    const { container } = render(<ExecutionReport />);
    expect(container.querySelector('[data-testid="execution-report"]')).toBeNull();
  });

  it('renders full report when data is present', () => {
    // Generate report
    useExecutionReportStore.getState().generateReport({
      sessionId: 'test-session',
      prd: {
        title: 'Test',
        description: 'Test',
        stories: [
          { id: 's1', title: 'Story 1', description: '', priority: 'high', dependencies: [], acceptanceCriteria: [] },
          { id: 's2', title: 'Story 2', description: '', priority: 'medium', dependencies: [], acceptanceCriteria: [] },
        ],
        batches: [{ index: 0, storyIds: ['s1', 's2'] }],
      },
      executionReport: {
        sessionId: 'test-session',
        totalStories: 2,
        storiesCompleted: 2,
        storiesFailed: 0,
        totalDurationMs: 10000,
        agentAssignments: { s1: 'claude-sonnet', s2: 'claude-haiku' },
        success: true,
      },
      qualityGateResults: {
        s1: {
          storyId: 's1',
          overallStatus: 'passed',
          gates: [],
          codeReviewScores: [
            { dimension: 'correctness', score: 9, maxScore: 10, feedback: '' },
            { dimension: 'readability', score: 8, maxScore: 10, feedback: '' },
          ],
          totalScore: 17,
        },
      },
      storyStatuses: { s1: 'completed', s2: 'completed' },
    });

    render(<ExecutionReport />);
    expect(screen.getByTestId('execution-report')).toBeDefined();
    expect(screen.getByTestId('summary-card')).toBeDefined();
    expect(screen.getByText('Execution Report')).toBeDefined();
  });

  it('renders export buttons', () => {
    useExecutionReportStore.getState().generateReport({
      sessionId: 'test-session',
      prd: {
        title: 'Test',
        description: 'Test',
        stories: [
          { id: 's1', title: 'Story 1', description: '', priority: 'high', dependencies: [], acceptanceCriteria: [] },
        ],
        batches: [{ index: 0, storyIds: ['s1'] }],
      },
      executionReport: {
        sessionId: 'test-session',
        totalStories: 1,
        storiesCompleted: 1,
        storiesFailed: 0,
        totalDurationMs: 5000,
        agentAssignments: { s1: 'claude-sonnet' },
        success: true,
      },
      qualityGateResults: {},
      storyStatuses: { s1: 'completed' },
    });

    render(<ExecutionReport />);
    expect(screen.getByText('JSON')).toBeDefined();
    expect(screen.getByText('Markdown')).toBeDefined();
  });
});
