/**
 * Execution Report Store Tests
 *
 * Story 003: Execution Report Zustand store
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  useExecutionReportStore,
  aggregateRadarDimensions,
  buildTimeline,
  calculateAgentPerformance,
  formatReportAsMarkdown,
} from './executionReport';
import type {
  TaskPrd,
  ExecutionReport,
  StoryQualityGateResults,
} from './taskMode';

// ============================================================================
// Test Fixtures
// ============================================================================

function makePrd(): TaskPrd {
  return {
    title: 'Test Feature',
    description: 'Test feature description',
    stories: [
      {
        id: 'story-1',
        title: 'Story One',
        description: 'First story',
        priority: 'high',
        dependencies: [],
        acceptanceCriteria: ['AC 1'],
      },
      {
        id: 'story-2',
        title: 'Story Two',
        description: 'Second story',
        priority: 'medium',
        dependencies: ['story-1'],
        acceptanceCriteria: ['AC 2'],
      },
      {
        id: 'story-3',
        title: 'Story Three',
        description: 'Third story',
        priority: 'low',
        dependencies: [],
        acceptanceCriteria: ['AC 3'],
      },
    ],
    batches: [
      { index: 0, storyIds: ['story-1', 'story-3'] },
      { index: 1, storyIds: ['story-2'] },
    ],
  };
}

function makeExecutionReport(): ExecutionReport {
  return {
    sessionId: 'sess-001',
    totalStories: 3,
    storiesCompleted: 2,
    storiesFailed: 1,
    totalDurationMs: 30000,
    agentAssignments: {
      'story-1': 'claude-sonnet',
      'story-2': 'claude-sonnet',
      'story-3': 'claude-haiku',
    },
    success: false,
  };
}

function makeQualityGateResults(): Record<string, StoryQualityGateResults> {
  return {
    'story-1': {
      storyId: 'story-1',
      overallStatus: 'passed',
      gates: [],
      codeReviewScores: [
        { dimension: 'correctness', score: 9, maxScore: 10, feedback: 'Good' },
        { dimension: 'readability', score: 8, maxScore: 10, feedback: 'Clear' },
        { dimension: 'performance', score: 7, maxScore: 10, feedback: 'OK' },
        { dimension: 'security', score: 9, maxScore: 10, feedback: 'Secure' },
        { dimension: 'maintainability', score: 8, maxScore: 10, feedback: 'Clean' },
      ],
      totalScore: 41,
    },
    'story-2': {
      storyId: 'story-2',
      overallStatus: 'failed',
      gates: [],
      codeReviewScores: [
        { dimension: 'correctness', score: 5, maxScore: 10, feedback: 'Bugs found' },
        { dimension: 'readability', score: 7, maxScore: 10, feedback: 'OK' },
        { dimension: 'performance', score: 6, maxScore: 10, feedback: 'Slow' },
        { dimension: 'security', score: 8, maxScore: 10, feedback: 'OK' },
        { dimension: 'maintainability', score: 6, maxScore: 10, feedback: 'Complex' },
      ],
      totalScore: 32,
    },
    'story-3': {
      storyId: 'story-3',
      overallStatus: 'passed',
      gates: [],
      totalScore: 45,
    },
  };
}

function makeStoryStatuses(): Record<string, string> {
  return {
    'story-1': 'completed',
    'story-2': 'failed',
    'story-3': 'completed',
  };
}

// ============================================================================
// Helper Function Tests
// ============================================================================

describe('aggregateRadarDimensions', () => {
  it('should aggregate scores across stories', () => {
    const result = aggregateRadarDimensions(makeQualityGateResults());
    expect(result.length).toBe(5);

    const correctness = result.find((d) => d.dimension === 'correctness');
    expect(correctness).toBeDefined();
    expect(correctness!.averageScore).toBe(7); // (9+5)/2 = 7
    expect(correctness!.maxScore).toBe(10);
    expect(correctness!.storyScores['story-1']).toBe(9);
    expect(correctness!.storyScores['story-2']).toBe(5);
  });

  it('should handle empty results', () => {
    const result = aggregateRadarDimensions({});
    expect(result).toEqual([]);
  });

  it('should handle stories without code review scores', () => {
    const results: Record<string, StoryQualityGateResults> = {
      'story-1': {
        storyId: 'story-1',
        overallStatus: 'passed',
        gates: [],
        // No codeReviewScores
      },
    };
    const result = aggregateRadarDimensions(results);
    expect(result).toEqual([]);
  });
});

describe('buildTimeline', () => {
  it('should create timeline entries for all stories', () => {
    const prd = makePrd();
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();

    const timeline = buildTimeline(prd, report, statuses);
    expect(timeline.length).toBe(3);
  });

  it('should assign correct batch indices', () => {
    const prd = makePrd();
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();

    const timeline = buildTimeline(prd, report, statuses);
    const story1 = timeline.find((e) => e.storyId === 'story-1');
    const story2 = timeline.find((e) => e.storyId === 'story-2');

    expect(story1!.batchIndex).toBe(0);
    expect(story2!.batchIndex).toBe(1);
  });

  it('should include agent assignments', () => {
    const prd = makePrd();
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();

    const timeline = buildTimeline(prd, report, statuses);
    const story1 = timeline.find((e) => e.storyId === 'story-1');
    expect(story1!.agent).toBe('claude-sonnet');
  });

  it('should set gate results based on status', () => {
    const prd = makePrd();
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();

    const timeline = buildTimeline(prd, report, statuses);
    const completed = timeline.find((e) => e.storyId === 'story-1');
    const failed = timeline.find((e) => e.storyId === 'story-2');

    expect(completed!.gateResult).toBe('passed');
    expect(failed!.gateResult).toBe('failed');
  });
});

describe('calculateAgentPerformance', () => {
  it('should calculate per-agent metrics', () => {
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();
    const gateResults = makeQualityGateResults();

    const perf = calculateAgentPerformance(report, statuses, gateResults, 3);
    expect(perf.length).toBe(2);

    const sonnet = perf.find((p) => p.agentName === 'claude-sonnet');
    expect(sonnet!.storiesAssigned).toBe(2);
    expect(sonnet!.storiesCompleted).toBe(1); // story-1 completed, story-2 failed
    expect(sonnet!.successRate).toBe(50);
  });

  it('should calculate quality scores when available', () => {
    const report = makeExecutionReport();
    const statuses = makeStoryStatuses();
    const gateResults = makeQualityGateResults();

    const perf = calculateAgentPerformance(report, statuses, gateResults, 3);
    const sonnet = perf.find((p) => p.agentName === 'claude-sonnet');
    // story-1: 41, story-2: 32 -> avg 36.5
    expect(sonnet!.averageQualityScore).toBe(36.5);
  });

  it('should handle empty assignments', () => {
    const report: ExecutionReport = {
      sessionId: 'sess-001',
      totalStories: 0,
      storiesCompleted: 0,
      storiesFailed: 0,
      totalDurationMs: 0,
      agentAssignments: {},
      success: true,
    };

    const perf = calculateAgentPerformance(report, {}, {}, 0);
    expect(perf).toEqual([]);
  });
});

describe('formatReportAsMarkdown', () => {
  it('should produce valid markdown', () => {
    const prd = makePrd();
    const execReport = makeExecutionReport();
    const statuses = makeStoryStatuses();
    const gateResults = makeQualityGateResults();

    // Generate full report model
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd,
      executionReport: execReport,
      qualityGateResults: gateResults,
      storyStatuses: statuses,
    });

    const report = useExecutionReportStore.getState().report;
    expect(report).not.toBeNull();

    const md = formatReportAsMarkdown(report!);
    expect(md).toContain('# Execution Report');
    expect(md).toContain('## Summary');
    expect(md).toContain('Total Stories');
    expect(md).toContain('## Quality Scores');
    expect(md).toContain('## Agent Performance');
    expect(md).toContain('## Timeline');
  });
});

// ============================================================================
// Store Tests
// ============================================================================

describe('useExecutionReportStore', () => {
  beforeEach(() => {
    useExecutionReportStore.getState().reset();
  });

  it('should start with null report', () => {
    const state = useExecutionReportStore.getState();
    expect(state.report).toBeNull();
    expect(state.isGenerating).toBe(false);
    expect(state.error).toBeNull();
  });

  it('should generate report from task mode data', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const state = useExecutionReportStore.getState();
    expect(state.report).not.toBeNull();
    expect(state.report!.sessionId).toBe('sess-001');
    expect(state.report!.summary.totalStories).toBe(3);
    expect(state.report!.summary.storiesPassed).toBe(2);
    expect(state.report!.summary.storiesFailed).toBe(1);
    expect(state.report!.summary.successRate).toBe(67);
    expect(state.isGenerating).toBe(false);
  });

  it('should populate radar dimensions', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const state = useExecutionReportStore.getState();
    expect(state.report!.radarDimensions.length).toBe(5);
  });

  it('should populate timeline', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const state = useExecutionReportStore.getState();
    expect(state.report!.timeline.length).toBe(3);
  });

  it('should populate agent performance', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const state = useExecutionReportStore.getState();
    expect(state.report!.agentPerformance.length).toBe(2);
  });

  it('should export as JSON', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const json = useExecutionReportStore.getState().exportAsJson();
    expect(json).not.toBeNull();
    const parsed = JSON.parse(json!);
    expect(parsed.sessionId).toBe('sess-001');
    expect(parsed.summary).toBeDefined();
  });

  it('should export as Markdown', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    const md = useExecutionReportStore.getState().exportAsMarkdown();
    expect(md).not.toBeNull();
    expect(md).toContain('# Execution Report');
    expect(md).toContain('sess-001');
  });

  it('should return null for export when no report', () => {
    const state = useExecutionReportStore.getState();
    expect(state.exportAsJson()).toBeNull();
    expect(state.exportAsMarkdown()).toBeNull();
  });

  it('should reset to initial state', () => {
    const store = useExecutionReportStore.getState();
    store.generateReport({
      sessionId: 'sess-001',
      prd: makePrd(),
      executionReport: makeExecutionReport(),
      qualityGateResults: makeQualityGateResults(),
      storyStatuses: makeStoryStatuses(),
    });

    expect(useExecutionReportStore.getState().report).not.toBeNull();

    useExecutionReportStore.getState().reset();
    expect(useExecutionReportStore.getState().report).toBeNull();
  });
});
