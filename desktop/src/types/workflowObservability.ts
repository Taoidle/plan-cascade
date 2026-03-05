import type { WorkflowMode } from './workflowKernel';

export interface WorkflowObservabilityMetrics {
  workflowLinkRehydrateTotal: number;
  workflowLinkRehydrateSuccess: number;
  workflowLinkRehydrateFailure: number;
  interactiveActionFailTotal: number;
  prdFeedbackApplyTotal: number;
  prdFeedbackApplySuccess: number;
  prdFeedbackApplyFailure: number;
}

export interface InteractiveActionFailureMetric {
  card: string;
  action: string;
  errorCode: string;
  total: number;
}

export interface WorkflowFailureSummary {
  timestamp: string;
  action: string;
  card: string | null;
  mode: string | null;
  kernelSessionId: string | null;
  modeSessionId: string | null;
  phaseBefore: string | null;
  phaseAfter: string | null;
  errorCode: string | null;
  message: string | null;
}

export interface WorkflowObservabilitySnapshot {
  metrics: WorkflowObservabilityMetrics;
  interactiveActionFailBreakdown: InteractiveActionFailureMetric[];
  latestFailure: WorkflowFailureSummary | null;
}

export interface WorkflowInteractiveActionFailureRecordRequest {
  card: string;
  action: string;
  errorCode: string;
  message?: string | null;
  mode?: WorkflowMode | null;
  kernelSessionId?: string | null;
  modeSessionId?: string | null;
  phaseBefore?: string | null;
  phaseAfter?: string | null;
}
