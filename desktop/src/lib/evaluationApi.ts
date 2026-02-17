/**
 * Evaluation API
 *
 * TypeScript wrapper for the evaluation Tauri commands.
 * Provides typed access to evaluator and evaluation run operations.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  Evaluator,
  EvaluatorInfo,
  EvaluationRun,
  EvaluationRunInfo,
  EvaluationReport,
  CommandResponse,
} from '../types/evaluation';

/**
 * List all evaluators (summary info).
 */
export async function listEvaluators(): Promise<EvaluatorInfo[]> {
  const response = await invoke<CommandResponse<EvaluatorInfo[]>>('list_evaluators');
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to list evaluators');
}

/**
 * Create a new evaluator.
 */
export async function createEvaluator(evaluator: Evaluator): Promise<Evaluator> {
  const response = await invoke<CommandResponse<Evaluator>>('create_evaluator', { evaluator });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to create evaluator');
}

/**
 * Delete an evaluator by ID.
 */
export async function deleteEvaluator(id: string): Promise<boolean> {
  const response = await invoke<CommandResponse<boolean>>('delete_evaluator', { id });
  if (response.success) {
    return response.data ?? false;
  }
  throw new Error(response.error ?? 'Failed to delete evaluator');
}

/**
 * Create a new evaluation run.
 */
export async function createEvaluationRun(run: EvaluationRun): Promise<EvaluationRun> {
  const response = await invoke<CommandResponse<EvaluationRun>>('create_evaluation_run', { run });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to create evaluation run');
}

/**
 * List all evaluation runs (summary info).
 */
export async function listEvaluationRuns(): Promise<EvaluationRunInfo[]> {
  const response = await invoke<CommandResponse<EvaluationRunInfo[]>>('list_evaluation_runs');
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to list evaluation runs');
}

/**
 * Get evaluation reports for a specific run.
 */
export async function getEvaluationReports(runId: string): Promise<EvaluationReport[]> {
  const response = await invoke<CommandResponse<EvaluationReport[]>>('get_evaluation_reports', { runId });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to get evaluation reports');
}

/**
 * Delete an evaluation run and its reports.
 */
export async function deleteEvaluationRun(runId: string): Promise<boolean> {
  const response = await invoke<CommandResponse<boolean>>('delete_evaluation_run', { runId });
  if (response.success) {
    return response.data ?? false;
  }
  throw new Error(response.error ?? 'Failed to delete evaluation run');
}
