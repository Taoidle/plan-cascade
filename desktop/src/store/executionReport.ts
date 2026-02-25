/**
 * Execution Report Store
 *
 * Zustand store for aggregating task execution data into a comprehensive
 * report model. Provides summary stats, quality radar scores, timeline
 * waterfall data, agent performance metrics, and export functionality.
 *
 * Story 003: Execution Report Zustand store
 */

import { create } from 'zustand';
import type {
  TaskPrd,
  ExecutionReport,
  StoryQualityGateResults,
  DimensionScore,
  BatchExecutionProgress,
} from './taskMode';

// ============================================================================
// Types
// ============================================================================

/** Summary card data for the execution report */
export interface ReportSummary {
  /** Total number of stories */
  totalStories: number;
  /** Stories that passed (completed successfully) */
  storiesPassed: number;
  /** Stories that failed */
  storiesFailed: number;
  /** Total execution time in milliseconds */
  totalTimeMs: number;
  /** Total tokens consumed (if available) */
  totalTokens: number | null;
  /** Estimated cost (if available) */
  estimatedCost: number | null;
  /** Overall success rate (0-100) */
  successRate: number;
}

/** Quality radar dimension aggregated across stories */
export interface RadarDimension {
  /** Dimension name (e.g., "correctness", "readability") */
  dimension: string;
  /** Average score across all stories */
  averageScore: number;
  /** Maximum possible score */
  maxScore: number;
  /** Per-story scores for this dimension */
  storyScores: Record<string, number>;
}

/** Timeline entry for waterfall visualization */
export interface TimelineEntry {
  /** Story ID */
  storyId: string;
  /** Story title */
  storyTitle: string;
  /** Batch index this story was in */
  batchIndex: number;
  /** Agent assigned to this story */
  agent: string;
  /** Duration in milliseconds */
  durationMs: number;
  /** Relative start time (ms from execution start) */
  startOffsetMs: number;
  /** Final status: 'completed' | 'failed' | 'cancelled' */
  status: string;
  /** Gate result: 'passed' | 'failed' | 'skipped' | null */
  gateResult: string | null;
}

/** Agent performance metrics */
export interface AgentPerformance {
  /** Agent name/type */
  agentName: string;
  /** Number of stories assigned to this agent */
  storiesAssigned: number;
  /** Number of stories completed successfully */
  storiesCompleted: number;
  /** Success rate (0-100) */
  successRate: number;
  /** Average duration per story in milliseconds */
  averageDurationMs: number;
  /** Average quality score (if available) */
  averageQualityScore: number | null;
}

/** Complete execution report model */
export interface ExecutionReportModel {
  /** Session ID */
  sessionId: string;
  /** Report generation timestamp (ISO 8601) */
  generatedAt: string;
  /** Summary statistics */
  summary: ReportSummary;
  /** Quality radar dimensions */
  radarDimensions: RadarDimension[];
  /** Timeline waterfall entries */
  timeline: TimelineEntry[];
  /** Agent performance breakdown */
  agentPerformance: AgentPerformance[];
}

// ============================================================================
// State Interface
// ============================================================================

export interface ExecutionReportState {
  /** Computed report model */
  report: ExecutionReportModel | null;

  /** Whether the report is being generated */
  isGenerating: boolean;

  /** Error message */
  error: string | null;

  // Actions

  /** Generate report from task mode data */
  generateReport: (params: {
    sessionId: string;
    prd: TaskPrd;
    executionReport: ExecutionReport;
    qualityGateResults: Record<string, StoryQualityGateResults>;
    storyStatuses: Record<string, string>;
  }) => void;

  /** Export report as JSON string */
  exportAsJson: () => string | null;

  /** Export report as Markdown string */
  exportAsMarkdown: () => string | null;

  /** Reset store */
  reset: () => void;
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Aggregate quality gate dimension scores into radar chart data
 */
export function aggregateRadarDimensions(
  qualityGateResults: Record<string, StoryQualityGateResults>,
): RadarDimension[] {
  const dimensionMap = new Map<
    string,
    { totalScore: number; maxScore: number; count: number; storyScores: Record<string, number> }
  >();

  for (const [storyId, result] of Object.entries(qualityGateResults)) {
    if (!result.codeReviewScores) continue;
    for (const dim of result.codeReviewScores) {
      const existing = dimensionMap.get(dim.dimension) ?? {
        totalScore: 0,
        maxScore: dim.maxScore,
        count: 0,
        storyScores: {},
      };
      existing.totalScore += dim.score;
      existing.count += 1;
      existing.storyScores[storyId] = dim.score;
      dimensionMap.set(dim.dimension, existing);
    }
  }

  return Array.from(dimensionMap.entries()).map(([dimension, data]) => ({
    dimension,
    averageScore: data.count > 0 ? Math.round((data.totalScore / data.count) * 10) / 10 : 0,
    maxScore: data.maxScore,
    storyScores: data.storyScores,
  }));
}

/**
 * Build timeline entries from execution data
 */
export function buildTimeline(
  prd: TaskPrd,
  executionReport: ExecutionReport,
  storyStatuses: Record<string, string>,
): TimelineEntry[] {
  const entries: TimelineEntry[] = [];
  let offsetMs = 0;

  for (const batch of prd.batches) {
    const batchStoryCount = batch.storyIds.length;
    const avgDurationPerStory =
      batchStoryCount > 0 ? Math.round(executionReport.totalDurationMs / prd.stories.length) : 0;

    for (const storyId of batch.storyIds) {
      const story = prd.stories.find((s) => s.id === storyId);
      const agent = executionReport.agentAssignments[storyId] ?? 'unknown';
      const status = storyStatuses[storyId] ?? 'pending';

      entries.push({
        storyId,
        storyTitle: story?.title ?? storyId,
        batchIndex: batch.index,
        agent,
        durationMs: avgDurationPerStory,
        startOffsetMs: offsetMs,
        status,
        gateResult: status === 'completed' ? 'passed' : status === 'failed' ? 'failed' : null,
      });
    }

    offsetMs += avgDurationPerStory;
  }

  return entries;
}

/**
 * Calculate agent performance metrics
 */
export function calculateAgentPerformance(
  executionReport: ExecutionReport,
  storyStatuses: Record<string, string>,
  qualityGateResults: Record<string, StoryQualityGateResults>,
  totalStories: number,
): AgentPerformance[] {
  const agentMap = new Map<
    string,
    {
      assigned: number;
      completed: number;
      totalDuration: number;
      qualityScores: number[];
    }
  >();

  const avgDuration = totalStories > 0 ? Math.round(executionReport.totalDurationMs / totalStories) : 0;

  for (const [storyId, agent] of Object.entries(executionReport.agentAssignments)) {
    const existing = agentMap.get(agent) ?? {
      assigned: 0,
      completed: 0,
      totalDuration: 0,
      qualityScores: [],
    };

    existing.assigned += 1;
    existing.totalDuration += avgDuration;

    const status = storyStatuses[storyId];
    if (status === 'completed') {
      existing.completed += 1;
    }

    const gateResult = qualityGateResults[storyId];
    if (gateResult?.totalScore !== undefined) {
      existing.qualityScores.push(gateResult.totalScore);
    }

    agentMap.set(agent, existing);
  }

  return Array.from(agentMap.entries()).map(([agentName, data]) => ({
    agentName,
    storiesAssigned: data.assigned,
    storiesCompleted: data.completed,
    successRate: data.assigned > 0 ? Math.round((data.completed / data.assigned) * 100) : 0,
    averageDurationMs: data.assigned > 0 ? Math.round(data.totalDuration / data.assigned) : 0,
    averageQualityScore:
      data.qualityScores.length > 0
        ? Math.round((data.qualityScores.reduce((a, b) => a + b, 0) / data.qualityScores.length) * 10) / 10
        : null,
  }));
}

/**
 * Format report as Markdown
 */
export function formatReportAsMarkdown(report: ExecutionReportModel): string {
  const lines: string[] = [];

  lines.push(`# Execution Report`);
  lines.push('');
  lines.push(`**Session:** ${report.sessionId}`);
  lines.push(`**Generated:** ${report.generatedAt}`);
  lines.push('');

  // Summary
  lines.push(`## Summary`);
  lines.push('');
  lines.push(`| Metric | Value |`);
  lines.push(`| --- | --- |`);
  lines.push(`| Total Stories | ${report.summary.totalStories} |`);
  lines.push(`| Passed | ${report.summary.storiesPassed} |`);
  lines.push(`| Failed | ${report.summary.storiesFailed} |`);
  lines.push(`| Success Rate | ${report.summary.successRate}% |`);
  lines.push(`| Total Time | ${(report.summary.totalTimeMs / 1000).toFixed(1)}s |`);
  if (report.summary.totalTokens !== null) {
    lines.push(`| Total Tokens | ${report.summary.totalTokens} |`);
  }
  if (report.summary.estimatedCost !== null) {
    lines.push(`| Estimated Cost | $${report.summary.estimatedCost.toFixed(4)} |`);
  }
  lines.push('');

  // Quality Dimensions
  if (report.radarDimensions.length > 0) {
    lines.push(`## Quality Scores`);
    lines.push('');
    lines.push(`| Dimension | Average | Max |`);
    lines.push(`| --- | --- | --- |`);
    for (const dim of report.radarDimensions) {
      lines.push(`| ${dim.dimension} | ${dim.averageScore} | ${dim.maxScore} |`);
    }
    lines.push('');
  }

  // Agent Performance
  if (report.agentPerformance.length > 0) {
    lines.push(`## Agent Performance`);
    lines.push('');
    lines.push(`| Agent | Assigned | Completed | Success Rate | Avg Duration |`);
    lines.push(`| --- | --- | --- | --- | --- |`);
    for (const agent of report.agentPerformance) {
      lines.push(
        `| ${agent.agentName} | ${agent.storiesAssigned} | ${agent.storiesCompleted} | ${agent.successRate}% | ${(agent.averageDurationMs / 1000).toFixed(1)}s |`,
      );
    }
    lines.push('');
  }

  // Timeline
  if (report.timeline.length > 0) {
    lines.push(`## Timeline`);
    lines.push('');
    lines.push(`| Story | Batch | Agent | Duration | Status |`);
    lines.push(`| --- | --- | --- | --- | --- |`);
    for (const entry of report.timeline) {
      lines.push(
        `| ${entry.storyTitle} | ${entry.batchIndex} | ${entry.agent} | ${(entry.durationMs / 1000).toFixed(1)}s | ${entry.status} |`,
      );
    }
    lines.push('');
  }

  return lines.join('\n');
}

// ============================================================================
// Store
// ============================================================================

const DEFAULT_STATE = {
  report: null,
  isGenerating: false,
  error: null,
};

export const useExecutionReportStore = create<ExecutionReportState>()((set, get) => ({
  ...DEFAULT_STATE,

  generateReport: (params) => {
    set({ isGenerating: true, error: null });

    try {
      const { sessionId, prd, executionReport, qualityGateResults, storyStatuses } = params;

      const totalStories = prd.stories.length;
      const storiesPassed = executionReport.storiesCompleted;
      const storiesFailed = executionReport.storiesFailed;

      const summary: ReportSummary = {
        totalStories,
        storiesPassed,
        storiesFailed,
        totalTimeMs: executionReport.totalDurationMs,
        totalTokens: null,
        estimatedCost: null,
        successRate: totalStories > 0 ? Math.round((storiesPassed / totalStories) * 100) : 0,
      };

      const radarDimensions = aggregateRadarDimensions(qualityGateResults);
      const timeline = buildTimeline(prd, executionReport, storyStatuses);
      const agentPerformance = calculateAgentPerformance(
        executionReport,
        storyStatuses,
        qualityGateResults,
        totalStories,
      );

      const report: ExecutionReportModel = {
        sessionId,
        generatedAt: new Date().toISOString(),
        summary,
        radarDimensions,
        timeline,
        agentPerformance,
      };

      set({ report, isGenerating: false });
    } catch (e) {
      set({ isGenerating: false, error: String(e) });
    }
  },

  exportAsJson: () => {
    const { report } = get();
    if (!report) return null;
    return JSON.stringify(report, null, 2);
  },

  exportAsMarkdown: () => {
    const { report } = get();
    if (!report) return null;
    return formatReportAsMarkdown(report);
  },

  reset: () => {
    set({ ...DEFAULT_STATE });
  },
}));

export default useExecutionReportStore;
