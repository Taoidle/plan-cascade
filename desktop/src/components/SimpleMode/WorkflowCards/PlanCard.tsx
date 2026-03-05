/**
 * Plan Card
 *
 * Interactive plan review card for the reviewing_plan phase.
 * Supports full plan edits, validation gate, execute, and retry hooks.
 */

import { useState, useCallback, useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon } from '@radix-ui/react-icons';
import type { PlanBatchData, PlanCardData, PlanStepData } from '../../../types/planModeCard';
import type { PlanEditOperation } from '../../../types/workflowKernel';
import { usePlanOrchestratorStore } from '../../../store/planOrchestrator';
import { useWorkflowKernelStore } from '../../../store/workflowKernel';
import { useExecutionStore } from '../../../store/execution';
import { reportInteractiveActionFailure } from '../../../lib/workflowObservability';
import {
  applyPlanEditWithIntent,
  submitWorkflowActionIntentViaCoordinator,
} from '../../../store/simpleWorkflowCoordinator';
import {
  clampPlanMaxParallel,
  ensurePlanExecutionConfig,
  getPlanMaxParallel,
  recomputePlanBatches,
  validatePlanDraft,
  type PlanValidationIssue,
} from './planGraph';

interface StepEditDraft {
  title: string;
  description: string;
  priority: PlanStepData['priority'];
  dependenciesText: string;
}

interface AddStepDraft {
  id: string;
  title: string;
  description: string;
  priority: PlanStepData['priority'];
  dependenciesText: string;
}

type EditablePlanField = 'title' | 'description' | 'priority' | 'dependencies';

interface PlanEditSummaryEntry {
  id: string;
  operation: PlanEditOperation['type'];
  stepId?: string;
  details: string;
  before?: string;
  after?: string;
  batchRecomputed: boolean;
  status: 'pending' | 'success' | 'failed';
  errorMessage?: string;
}

interface FailedPlanEditOperation {
  summaryId: string;
  operation: PlanEditOperation;
  metadata?: Record<string, unknown>;
  beforePlan: PlanCardData;
  afterPlan: PlanCardData;
}

function clonePlan(plan: PlanCardData): PlanCardData {
  return JSON.parse(JSON.stringify(plan)) as PlanCardData;
}

function priorityColor(priority: string): string {
  switch (priority) {
    case 'high':
      return 'text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/40';
    case 'low':
      return 'text-gray-600 dark:text-gray-400 bg-gray-100 dark:bg-gray-800';
    default:
      return 'text-amber-600 dark:text-amber-400 bg-amber-100 dark:bg-amber-900/40';
  }
}

function parseDependencies(input: string): string[] {
  const values = input
    .split(',')
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
  return [...new Set(values)];
}

function buildDraft(step: PlanStepData): StepEditDraft {
  return {
    title: step.title,
    description: step.description,
    priority: step.priority,
    dependenciesText: step.dependencies.join(', '),
  };
}

function nextStepId(steps: PlanStepData[]): string {
  const used = new Set(steps.map((step) => step.id));
  let index = steps.length + 1;
  while (used.has(`step-${index}`)) {
    index += 1;
  }
  return `step-${index}`;
}

function buildAddDraft(steps: PlanStepData[]): AddStepDraft {
  return {
    id: nextStepId(steps),
    title: '',
    description: '',
    priority: 'medium',
    dependenciesText: '',
  };
}

function moveStep(steps: PlanStepData[], stepId: string, direction: 'up' | 'down'): PlanStepData[] {
  const index = steps.findIndex((step) => step.id === stepId);
  if (index < 0) return steps;
  const targetIndex = direction === 'up' ? index - 1 : index + 1;
  if (targetIndex < 0 || targetIndex >= steps.length) return steps;

  const next = [...steps];
  const [moved] = next.splice(index, 1);
  if (!moved) return steps;
  next.splice(targetIndex, 0, moved);
  return next;
}

function removeStepAndDetachDependencies(steps: PlanStepData[], stepId: string): PlanStepData[] {
  return steps
    .filter((step) => step.id !== stepId)
    .map((step) => ({
      ...step,
      dependencies: step.dependencies.filter((dep) => dep !== stepId),
    }));
}

function withRecomputedBatches(plan: PlanCardData, fallbackBatches?: PlanBatchData[]): PlanCardData {
  const normalized = ensurePlanExecutionConfig(plan);
  const maxParallel = getPlanMaxParallel(normalized);
  try {
    return {
      ...normalized,
      executionConfig: {
        maxParallel,
      },
      batches: recomputePlanBatches(normalized.steps, maxParallel),
    };
  } catch {
    return {
      ...normalized,
      executionConfig: {
        maxParallel,
      },
      batches: fallbackBatches ?? normalized.batches,
    };
  }
}

function summarizeValidationIssue(t: ReturnType<typeof useTranslation>['t'], issue: PlanValidationIssue): string {
  switch (issue.code) {
    case 'duplicate_step_id':
      return t('plan.validation.duplicateStepId', {
        stepId: issue.stepId,
        defaultValue: `Duplicate step id: ${issue.stepId}`,
      });
    case 'missing_dependency':
      return t('plan.validation.missingDependency', {
        stepId: issue.stepId,
        dependencyId: issue.dependencyId,
        defaultValue: `Step ${issue.stepId} depends on missing step ${issue.dependencyId}`,
      });
    case 'self_dependency':
      return t('plan.validation.selfDependency', {
        stepId: issue.stepId,
        defaultValue: `Step ${issue.stepId} cannot depend on itself`,
      });
    case 'cycle_dependency':
      return t('plan.validation.cycleDependency', {
        stepId: issue.stepId,
        defaultValue: `Dependency cycle detected near ${issue.stepId}`,
      });
    case 'parallel_out_of_range':
      return t('plan.validation.parallelOutOfRange', {
        min: 1,
        max: 8,
        defaultValue: 'Parallelism must be between 1 and 8',
      });
    default:
      return t('plan.validation.generic', { defaultValue: 'Plan validation failed' });
  }
}

function StepRow({
  step,
  isExpanded,
  editable,
  canMoveUp,
  canMoveDown,
  canRemove,
  onToggle,
  onEdit,
  onMoveUp,
  onMoveDown,
  onRemove,
  onClearDependencies,
}: {
  step: PlanStepData;
  isExpanded: boolean;
  editable: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
  canRemove: boolean;
  onToggle: () => void;
  onEdit: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onRemove: () => void;
  onClearDependencies: () => void;
}) {
  const { t } = useTranslation('planMode');

  return (
    <div className="border-b border-teal-100 dark:border-teal-900/40 last:border-b-0">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-2 py-1.5 text-left hover:bg-teal-50 dark:hover:bg-teal-900/20 transition-colors"
      >
        {isExpanded ? (
          <ChevronDownIcon className="w-3 h-3 shrink-0 text-gray-400" />
        ) : (
          <ChevronRightIcon className="w-3 h-3 shrink-0 text-gray-400" />
        )}
        <span className="text-2xs font-mono text-gray-400 shrink-0">{step.id}</span>
        <span className="text-xs font-medium text-gray-800 dark:text-gray-200 flex-1 truncate">{step.title}</span>
        <span className={clsx('text-2xs px-1.5 py-0.5 rounded font-medium shrink-0', priorityColor(step.priority))}>
          {t(`plan.priority.${step.priority}`, { defaultValue: step.priority })}
        </span>
      </button>

      {isExpanded && (
        <div className="px-7 pb-2 space-y-1.5">
          <p className="text-xs text-gray-600 dark:text-gray-400">{step.description}</p>

          {step.completionCriteria.length > 0 && (
            <div>
              <span className="text-2xs font-medium text-gray-500 dark:text-gray-400">
                {t('plan.completionCriteria', 'Completion Criteria')}:
              </span>
              <ul className="mt-0.5 space-y-0.5">
                {step.completionCriteria.map((criterion, index) => (
                  <li key={index} className="text-2xs text-gray-600 dark:text-gray-400 flex items-start gap-1">
                    <span className="text-teal-500 mt-0.5">&#x2022;</span>
                    {criterion}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {step.dependencies.length > 0 && (
            <div className="flex flex-wrap gap-1">
              <span className="text-2xs text-gray-500">{t('plan.deps', 'deps')}:</span>
              {step.dependencies.map((dep) => (
                <span
                  key={dep}
                  className="text-2xs px-1.5 py-0.5 rounded bg-teal-100 dark:bg-teal-900/40 text-teal-600 dark:text-teal-400"
                >
                  {dep}
                </span>
              ))}
            </div>
          )}

          {step.expectedOutput && (
            <div className="text-2xs text-gray-500 dark:text-gray-400">
              <span className="font-medium">{t('plan.expectedOutput', 'Expected output')}:</span> {step.expectedOutput}
            </div>
          )}

          {editable && (
            <div className="pt-1 flex flex-wrap items-center gap-1.5">
              <button
                onClick={onEdit}
                className="px-2 py-1 rounded text-2xs font-medium bg-white dark:bg-gray-900 border border-teal-300 dark:border-teal-700 text-teal-700 dark:text-teal-300 hover:bg-teal-50 dark:hover:bg-teal-900/30 transition-colors"
              >
                {t('plan.editStep', 'Edit Step')}
              </button>
              <button
                onClick={onMoveUp}
                disabled={!canMoveUp}
                className="px-2 py-1 rounded text-2xs border border-gray-300 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                {t('plan.reorder.up', 'Move Up')}
              </button>
              <button
                onClick={onMoveDown}
                disabled={!canMoveDown}
                className="px-2 py-1 rounded text-2xs border border-gray-300 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                {t('plan.reorder.down', 'Move Down')}
              </button>
              <button
                onClick={onClearDependencies}
                disabled={step.dependencies.length === 0}
                className="px-2 py-1 rounded text-2xs border border-amber-300 dark:border-amber-700 text-amber-700 dark:text-amber-300 hover:bg-amber-50 dark:hover:bg-amber-900/30 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                {t('plan.clearDependencies', 'Clear Dependencies')}
              </button>
              <button
                onClick={onRemove}
                disabled={!canRemove}
                className="px-2 py-1 rounded text-2xs border border-rose-300 dark:border-rose-700 text-rose-700 dark:text-rose-300 hover:bg-rose-50 dark:hover:bg-rose-900/30 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                {t('plan.removeStep', 'Remove Step')}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function PlanCard({ data, interactive }: { data: PlanCardData; interactive: boolean }) {
  const { t } = useTranslation('planMode');
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());
  const [workingPlan, setWorkingPlan] = useState<PlanCardData>(() =>
    withRecomputedBatches(ensurePlanExecutionConfig(data)),
  );
  const [editSummary, setEditSummary] = useState<PlanEditSummaryEntry[]>([]);
  const [failedOps, setFailedOps] = useState<Record<string, FailedPlanEditOperation>>({});
  const [pendingOps, setPendingOps] = useState<Record<string, true>>({});
  const [editingStepId, setEditingStepId] = useState<string | null>(null);
  const [stepDraft, setStepDraft] = useState<StepEditDraft | null>(null);
  const [addStepDraft, setAddStepDraft] = useState<AddStepDraft>(() => buildAddDraft(data.steps));
  const [parallelDraft, setParallelDraft] = useState<number>(getPlanMaxParallel(data));
  const [workingPlanRevision, setWorkingPlanRevision] = useState<number>(0);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [retryingStepId, setRetryingStepId] = useState<string | null>(null);

  const approvePlan = usePlanOrchestratorStore((s) => s.approvePlan);
  const retryStep = usePlanOrchestratorStore((s) => s.retryStep);
  const stepStatuses = usePlanOrchestratorStore((s) => s.stepStatuses || {});
  const workflowSession = useWorkflowKernelStore((s) => s.session);
  const refreshWorkflowSessionState = useWorkflowKernelStore((s) => s.refreshSessionState);
  const appendExecutionCard = useExecutionStore((s) => s.appendCard);
  const kernelPlanRevision = workflowSession?.modeSnapshots.plan?.planRevision ?? 0;

  useEffect(() => {
    const normalized = withRecomputedBatches(ensurePlanExecutionConfig(data));
    setWorkingPlan(normalized);
    setWorkingPlanRevision(kernelPlanRevision);
    setExpandedSteps(new Set());
    setEditSummary([]);
    setFailedOps({});
    setPendingOps({});
    setEditingStepId(null);
    setStepDraft(null);
    setAddStepDraft(buildAddDraft(normalized.steps));
    setParallelDraft(getPlanMaxParallel(normalized));
    setSubmitError(null);
  }, [data, kernelPlanRevision]);

  const isKernelPlanActive = workflowSession?.status === 'active' && workflowSession.activeMode === 'plan';
  const kernelPlanPhase = workflowSession?.modeSnapshots.plan?.phase ?? 'idle';
  const isActive = interactive && kernelPlanPhase === 'reviewing_plan' && isKernelPlanActive;

  const failedSteps = useMemo(
    () => workingPlan.steps.filter((step) => stepStatuses[step.id] === 'failed'),
    [workingPlan.steps, stepStatuses],
  );

  const validationIssues = useMemo(() => validatePlanDraft(workingPlan), [workingPlan]);

  const appendEditSummary = useCallback((entry: Omit<PlanEditSummaryEntry, 'id' | 'status'>): string => {
    const id = `${entry.operation}-${Date.now()}-${Math.random()}`;
    setEditSummary((prev) => [{ id, status: 'pending', ...entry }, ...prev]);
    return id;
  }, []);

  const updateEditSummaryStatus = useCallback(
    (summaryId: string, status: PlanEditSummaryEntry['status'], error?: string) => {
      setEditSummary((prev) =>
        prev.map((entry) =>
          entry.id === summaryId
            ? {
                ...entry,
                status,
                errorMessage: status === 'failed' ? error || '' : undefined,
              }
            : entry,
        ),
      );
    },
    [],
  );

  const submitPlanEditOperation = useCallback(
    async (
      operation: PlanEditOperation,
      metadata?: Record<string, unknown>,
      options?: {
        summaryId?: string;
        beforePlan?: PlanCardData;
        afterPlan?: PlanCardData;
      },
    ): Promise<boolean> => {
      const summaryId = options?.summaryId ?? `${operation.type}-${Date.now()}`;
      const beforePlan = clonePlan(options?.beforePlan ?? workingPlan);
      const afterPlan = clonePlan(options?.afterPlan ?? workingPlan);
      const latestKernelRevision = useWorkflowKernelStore.getState().session?.modeSnapshots.plan?.planRevision ?? 0;
      if (latestKernelRevision > workingPlanRevision) {
        await refreshWorkflowSessionState();
        setWorkingPlan(withRecomputedBatches(ensurePlanExecutionConfig(data)));
        setWorkingPlanRevision(
          useWorkflowKernelStore.getState().session?.modeSnapshots.plan?.planRevision ?? latestKernelRevision,
        );
        const message = t('plan.editConflictDetected', {
          defaultValue: 'Plan changed remotely. Local draft was refreshed; please retry your edit.',
        });
        setSubmitError(message);
        updateEditSummaryStatus(summaryId, 'failed', message);
        setFailedOps((prev) => ({
          ...prev,
          [summaryId]: {
            summaryId,
            operation,
            metadata,
            beforePlan,
            afterPlan,
          },
        }));
        return false;
      }

      setPendingOps((prev) => ({ ...prev, [summaryId]: true }));
      const result = await applyPlanEditWithIntent({
        operation,
        source: 'plan_card',
        action: operation.type,
        content: `${operation.type}:${operation.targetStepId ?? ''}`,
        metadata: {
          stepId: operation.targetStepId ?? null,
          ...metadata,
        },
      });
      setPendingOps((prev) => {
        const next = { ...prev };
        delete next[summaryId];
        return next;
      });

      if (!result.ok) {
        const message = result.message || t('plan.editApplyFailed', { defaultValue: 'Failed to apply plan edit.' });
        setWorkingPlan(beforePlan);
        setSubmitError(message);
        updateEditSummaryStatus(summaryId, 'failed', message);
        setFailedOps((prev) => ({
          ...prev,
          [summaryId]: {
            summaryId,
            operation,
            metadata,
            beforePlan,
            afterPlan,
          },
        }));
        appendExecutionCard({
          cardType: 'workflow_error',
          cardId: `plan-edit-failed-${summaryId}`,
          data: {
            title: t('plan.editApplyFailed', { defaultValue: 'Failed to apply plan edit.' }),
            description: message,
            suggestedFix: t('plan.editRetryHint', { defaultValue: 'Retry the failed edit, then approve the plan.' }),
          },
          interactive: false,
        });
        await reportInteractiveActionFailure({
          card: 'plan_card',
          action: operation.type,
          errorCode: result.errorCode || 'plan_edit_failed',
          message,
          session: workflowSession,
        });
        return false;
      }

      setFailedOps((prev) => {
        const next = { ...prev };
        delete next[summaryId];
        return next;
      });
      setSubmitError(null);
      updateEditSummaryStatus(summaryId, 'success');
      const refreshed = await refreshWorkflowSessionState();
      const latestRevision = refreshed?.session.modeSnapshots.plan?.planRevision;
      if (typeof latestRevision === 'number') {
        setWorkingPlanRevision(latestRevision);
      }
      return true;
    },
    [
      appendExecutionCard,
      data,
      refreshWorkflowSessionState,
      t,
      updateEditSummaryStatus,
      workflowSession,
      workingPlan,
      workingPlanRevision,
    ],
  );

  const toggleStep = useCallback((stepId: string) => {
    setExpandedSteps((prev) => {
      const next = new Set(prev);
      if (next.has(stepId)) {
        next.delete(stepId);
      } else {
        next.add(stepId);
      }
      return next;
    });
  }, []);

  const beginEditStep = useCallback(
    (stepId: string) => {
      if (!isActive) return;
      const step = workingPlan.steps.find((item) => item.id === stepId);
      if (!step) return;
      setEditingStepId(stepId);
      setStepDraft(buildDraft(step));
    },
    [isActive, workingPlan.steps],
  );

  const cancelEditStep = useCallback(() => {
    setEditingStepId(null);
    setStepDraft(null);
  }, []);

  const saveEditStep = useCallback(async () => {
    if (!isActive || !editingStepId || !stepDraft) return;
    const targetStep = workingPlan.steps.find((item) => item.id === editingStepId);
    if (!targetStep) return;

    const nextDependencies = parseDependencies(stepDraft.dependenciesText);
    const normalizedTitle = stepDraft.title.trim();
    const normalizedDescription = stepDraft.description.trim();
    const changedFields: EditablePlanField[] = [];

    if (normalizedTitle !== targetStep.title) changedFields.push('title');
    if (normalizedDescription !== targetStep.description) changedFields.push('description');
    if (stepDraft.priority !== targetStep.priority) changedFields.push('priority');
    if (nextDependencies.join('|') !== targetStep.dependencies.join('|')) changedFields.push('dependencies');

    if (changedFields.length === 0) {
      cancelEditStep();
      return;
    }

    const updatedStep: PlanStepData = {
      ...targetStep,
      title: normalizedTitle || targetStep.title,
      description: normalizedDescription || targetStep.description,
      priority: stepDraft.priority,
      dependencies: nextDependencies,
    };

    const beforePlan = clonePlan(workingPlan);
    const nextPlan = withRecomputedBatches(
      {
        ...workingPlan,
        steps: workingPlan.steps.map((step) => (step.id === editingStepId ? updatedStep : step)),
      },
      workingPlan.batches,
    );
    const afterPlan = clonePlan(nextPlan);

    setWorkingPlan(nextPlan);
    const changedFieldLabels = changedFields.map((field) =>
      t(`plan.editSummary.field.${field}`, {
        defaultValue: field,
      }),
    );
    const summaryId = appendEditSummary({
      operation: 'update_step',
      stepId: editingStepId,
      details: changedFieldLabels.join(', '),
      before: `${targetStep.title} | ${targetStep.priority} | [${targetStep.dependencies.join(', ')}]`,
      after: `${updatedStep.title} | ${updatedStep.priority} | [${updatedStep.dependencies.join(', ')}]`,
      batchRecomputed: true,
    });
    cancelEditStep();

    const updatedOk = await submitPlanEditOperation(
      {
        type: 'update_step',
        targetStepId: editingStepId,
        payload: {
          title: updatedStep.title,
          description: updatedStep.description,
          priority: updatedStep.priority,
          dependencies: updatedStep.dependencies,
        },
      },
      {
        changedFields,
      },
      {
        summaryId,
        beforePlan,
        afterPlan,
      },
    );
    if (!updatedOk) return;

    const beforeDeps = new Set(targetStep.dependencies);
    const afterDeps = new Set(updatedStep.dependencies);

    for (const depId of afterDeps) {
      if (beforeDeps.has(depId)) continue;
      await submitPlanEditOperation(
        {
          type: 'set_dependency',
          targetStepId: editingStepId,
          payload: {
            dependencyStepId: depId,
          },
        },
        {
          dependencyStepId: depId,
        },
        {
          beforePlan: afterPlan,
          afterPlan,
        },
      );
    }

    for (const depId of beforeDeps) {
      if (afterDeps.has(depId)) continue;
      await submitPlanEditOperation(
        {
          type: 'clear_dependency',
          targetStepId: editingStepId,
          payload: {
            dependencyStepId: depId,
          },
        },
        {
          dependencyStepId: depId,
        },
        {
          beforePlan: afterPlan,
          afterPlan,
        },
      );
    }
  }, [appendEditSummary, cancelEditStep, editingStepId, isActive, stepDraft, submitPlanEditOperation, t, workingPlan]);

  const handleAddStep = useCallback(async () => {
    if (!isActive) return;

    const stepId = addStepDraft.id.trim();
    const title = addStepDraft.title.trim();
    if (!stepId || !title) return;

    const newStep: PlanStepData = {
      id: stepId,
      title,
      description: addStepDraft.description.trim(),
      priority: addStepDraft.priority,
      dependencies: parseDependencies(addStepDraft.dependenciesText),
      completionCriteria: [],
      expectedOutput: '',
    };

    const beforePlan = clonePlan(workingPlan);
    const nextPlan = withRecomputedBatches(
      {
        ...workingPlan,
        steps: [...workingPlan.steps, newStep],
      },
      workingPlan.batches,
    );
    const afterPlan = clonePlan(nextPlan);

    setWorkingPlan(nextPlan);
    setAddStepDraft(buildAddDraft(nextPlan.steps));
    const summaryId = appendEditSummary({
      operation: 'add_step',
      stepId: newStep.id,
      details: newStep.title,
      after: `${newStep.title} | ${newStep.priority}`,
      batchRecomputed: true,
    });

    await submitPlanEditOperation(
      {
        type: 'add_step',
        payload: {
          step: newStep,
        },
      },
      {
        stepId: newStep.id,
      },
      {
        summaryId,
        beforePlan,
        afterPlan,
      },
    );
  }, [addStepDraft, appendEditSummary, isActive, submitPlanEditOperation, workingPlan]);

  const handleRemoveStep = useCallback(
    async (stepId: string) => {
      if (!isActive) return;
      const targetStep = workingPlan.steps.find((step) => step.id === stepId);
      if (!targetStep) return;
      if (workingPlan.steps.length <= 1) return;

      const beforePlan = clonePlan(workingPlan);
      const nextPlan = withRecomputedBatches(
        {
          ...workingPlan,
          steps: removeStepAndDetachDependencies(workingPlan.steps, stepId),
        },
        workingPlan.batches,
      );
      const afterPlan = clonePlan(nextPlan);

      setWorkingPlan(nextPlan);
      setExpandedSteps((prev) => {
        const next = new Set(prev);
        next.delete(stepId);
        return next;
      });
      const summaryId = appendEditSummary({
        operation: 'remove_step',
        stepId,
        details: targetStep.title,
        before: `${targetStep.title} | ${targetStep.priority}`,
        batchRecomputed: true,
      });

      await submitPlanEditOperation(
        {
          type: 'remove_step',
          targetStepId: stepId,
        },
        {
          removedStepId: stepId,
        },
        {
          summaryId,
          beforePlan,
          afterPlan,
        },
      );
    },
    [appendEditSummary, isActive, submitPlanEditOperation, workingPlan],
  );

  const handleMoveStep = useCallback(
    async (stepId: string, direction: 'up' | 'down') => {
      if (!isActive) return;
      const moved = moveStep(workingPlan.steps, stepId, direction);
      if (moved === workingPlan.steps) return;

      const beforePlan = clonePlan(workingPlan);
      const nextPlan = withRecomputedBatches(
        {
          ...workingPlan,
          steps: moved,
        },
        workingPlan.batches,
      );
      const afterPlan = clonePlan(nextPlan);

      setWorkingPlan(nextPlan);
      const toIndex = moved.findIndex((step) => step.id === stepId);
      const summaryId = appendEditSummary({
        operation: 'reorder_step',
        stepId,
        details: t(`plan.reorder.${direction}`, {
          defaultValue: direction === 'up' ? 'Move Up' : 'Move Down',
        }),
        after: `index:${toIndex}`,
        batchRecomputed: true,
      });

      await submitPlanEditOperation(
        {
          type: 'reorder_step',
          targetStepId: stepId,
          payload: {
            direction,
            toIndex,
          },
        },
        {
          direction,
          toIndex,
        },
        {
          summaryId,
          beforePlan,
          afterPlan,
        },
      );
    },
    [appendEditSummary, isActive, submitPlanEditOperation, t, workingPlan],
  );

  const handleClearDependencies = useCallback(
    async (stepId: string) => {
      if (!isActive) return;
      const step = workingPlan.steps.find((item) => item.id === stepId);
      if (!step || step.dependencies.length === 0) return;

      const beforePlan = clonePlan(workingPlan);
      const nextPlan = withRecomputedBatches(
        {
          ...workingPlan,
          steps: workingPlan.steps.map((item) =>
            item.id === stepId
              ? {
                  ...item,
                  dependencies: [],
                }
              : item,
          ),
        },
        workingPlan.batches,
      );
      const afterPlan = clonePlan(nextPlan);

      setWorkingPlan(nextPlan);
      const summaryId = appendEditSummary({
        operation: 'clear_dependency',
        stepId,
        details: step.dependencies.join(', '),
        before: `[${step.dependencies.join(', ')}]`,
        after: '[]',
        batchRecomputed: true,
      });

      await submitPlanEditOperation(
        {
          type: 'clear_dependency',
          targetStepId: stepId,
          payload: {
            clearAll: true,
            dependencies: step.dependencies,
          },
        },
        {
          dependencies: step.dependencies,
        },
        {
          summaryId,
          beforePlan,
          afterPlan,
        },
      );
    },
    [appendEditSummary, isActive, submitPlanEditOperation, workingPlan],
  );

  const handleApplyParallelism = useCallback(async () => {
    if (!isActive) return;
    const nextMaxParallel = clampPlanMaxParallel(parallelDraft);
    const currentMaxParallel = getPlanMaxParallel(workingPlan);
    if (nextMaxParallel === currentMaxParallel) {
      setParallelDraft(nextMaxParallel);
      return;
    }

    const beforePlan = clonePlan(workingPlan);
    const nextPlan = withRecomputedBatches(
      {
        ...workingPlan,
        executionConfig: {
          maxParallel: nextMaxParallel,
        },
      },
      workingPlan.batches,
    );
    const afterPlan = clonePlan(nextPlan);

    setWorkingPlan(nextPlan);
    setParallelDraft(nextMaxParallel);
    const summaryId = appendEditSummary({
      operation: 'set_parallelism',
      details: `${currentMaxParallel} -> ${nextMaxParallel}`,
      before: String(currentMaxParallel),
      after: String(nextMaxParallel),
      batchRecomputed: true,
    });

    await submitPlanEditOperation(
      {
        type: 'set_parallelism',
        payload: {
          maxParallel: nextMaxParallel,
        },
      },
      {
        maxParallel: nextMaxParallel,
      },
      {
        summaryId,
        beforePlan,
        afterPlan,
      },
    );
  }, [appendEditSummary, isActive, parallelDraft, submitPlanEditOperation, workingPlan]);

  const handleApprove = useCallback(async () => {
    if (!isActive || isSubmitting) return;
    if (validationIssues.length > 0) return;

    setIsSubmitting(true);
    setSubmitError(null);

    try {
      await submitWorkflowActionIntentViaCoordinator({
        mode: 'plan',
        type: 'plan_approval',
        source: 'plan_card',
        action: 'approve_plan',
        content: 'approve_plan',
        metadata: {
          stepCount: workingPlan.steps.length,
          batchCount: workingPlan.batches.length,
          edited: editSummary.length > 0,
          maxParallel: getPlanMaxParallel(workingPlan),
        },
      });
    } catch {
      // Keep orchestration available even if kernel logging fails.
    }

    try {
      const result = await approvePlan(workingPlan);
      if (!result.ok) {
        const message = result.message || 'Failed to start plan execution';
        setSubmitError(message);
        await reportInteractiveActionFailure({
          card: 'plan_card',
          action: 'approve_plan',
          errorCode: result.errorCode || 'approve_plan_failed',
          message,
          session: workflowSession,
        });
      }
    } finally {
      setIsSubmitting(false);
    }
  }, [approvePlan, editSummary.length, isActive, isSubmitting, validationIssues.length, workingPlan, workflowSession]);

  const handleRetryStep = useCallback(
    async (stepId: string) => {
      if (!stepId || retryingStepId) return;
      setRetryingStepId(stepId);
      try {
        await retryStep(stepId);
      } finally {
        setRetryingStepId(null);
      }
    },
    [retryStep, retryingStepId],
  );

  const handleRetryFailedEdit = useCallback(
    async (summaryId: string) => {
      const failed = failedOps[summaryId];
      if (!failed || pendingOps[summaryId]) return;
      setWorkingPlan(clonePlan(failed.afterPlan));
      updateEditSummaryStatus(summaryId, 'pending');
      await submitPlanEditOperation(failed.operation, failed.metadata, {
        summaryId,
        beforePlan: failed.beforePlan,
        afterPlan: failed.afterPlan,
      });
    },
    [failedOps, pendingOps, submitPlanEditOperation, updateEditSummaryStatus],
  );

  return (
    <div className="rounded-lg border border-teal-200 dark:border-teal-800 bg-teal-50 dark:bg-teal-900/20">
      <div className="px-3 py-2 bg-teal-100/50 dark:bg-teal-900/30 border-b border-teal-200 dark:border-teal-800 flex items-center justify-between">
        <span className="text-xs font-semibold text-teal-700 dark:text-teal-300">
          {workingPlan.title || t('plan.title', 'Plan')}
        </span>
        <span className="text-2xs text-teal-600 dark:text-teal-400">
          {workingPlan.steps.length} {t('plan.steps', 'steps')} / {workingPlan.batches.length}{' '}
          {t('plan.batches', 'batches')}
          {' · '}
          {t('plan.parallelism.label', {
            count: getPlanMaxParallel(workingPlan),
            defaultValue: `parallel ${getPlanMaxParallel(workingPlan)}`,
          })}
        </span>
      </div>

      {workingPlan.description && (
        <div className="px-3 py-1.5 text-xs text-gray-600 dark:text-gray-400 border-b border-teal-100 dark:border-teal-900/40">
          {workingPlan.description}
        </div>
      )}

      {isActive && (
        <div className="px-3 py-2 border-b border-teal-200 dark:border-teal-800 bg-white/60 dark:bg-gray-900/20 space-y-2">
          <div className="flex flex-wrap items-end gap-2">
            <label className="text-2xs text-gray-600 dark:text-gray-300">
              {t('plan.parallelism.inputLabel', { defaultValue: 'Max parallel' })}
            </label>
            <input
              type="number"
              min={1}
              max={8}
              value={parallelDraft}
              onChange={(event) => setParallelDraft(Number.parseInt(event.target.value, 10) || 0)}
              className="w-20 rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            />
            <button
              onClick={() => {
                void handleApplyParallelism();
              }}
              className="px-2 py-1 rounded text-2xs font-medium border border-teal-300 dark:border-teal-700 text-teal-700 dark:text-teal-300 hover:bg-teal-50 dark:hover:bg-teal-900/30 transition-colors"
            >
              {t('plan.parallelism.apply', { defaultValue: 'Apply Parallelism' })}
            </button>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <input
              value={addStepDraft.id}
              onChange={(event) => setAddStepDraft((prev) => ({ ...prev, id: event.target.value }))}
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
              placeholder={t('plan.addStep.idPlaceholder', { defaultValue: 'step-id' })}
            />
            <input
              value={addStepDraft.title}
              onChange={(event) => setAddStepDraft((prev) => ({ ...prev, title: event.target.value }))}
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
              placeholder={t('plan.addStep.titlePlaceholder', { defaultValue: 'Step title' })}
            />
          </div>
          <textarea
            rows={2}
            value={addStepDraft.description}
            onChange={(event) => setAddStepDraft((prev) => ({ ...prev, description: event.target.value }))}
            className="w-full rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            placeholder={t('plan.addStep.descriptionPlaceholder', { defaultValue: 'Step description' })}
          />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <select
              value={addStepDraft.priority}
              onChange={(event) =>
                setAddStepDraft((prev) => ({ ...prev, priority: event.target.value as PlanStepData['priority'] }))
              }
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            >
              <option value="high">{t('plan.priority.high', { defaultValue: 'high' })}</option>
              <option value="medium">{t('plan.priority.medium', { defaultValue: 'medium' })}</option>
              <option value="low">{t('plan.priority.low', { defaultValue: 'low' })}</option>
            </select>
            <input
              value={addStepDraft.dependenciesText}
              onChange={(event) => setAddStepDraft((prev) => ({ ...prev, dependenciesText: event.target.value }))}
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
              placeholder={t('plan.dependenciesCsv', 'Dependencies (comma-separated IDs)')}
            />
          </div>
          <div className="flex justify-end">
            <button
              onClick={() => {
                void handleAddStep();
              }}
              className="px-2.5 py-1 rounded text-xs font-medium bg-teal-600 text-white hover:bg-teal-700 transition-colors"
            >
              {t('plan.addStep.action', { defaultValue: 'Add Step' })}
            </button>
          </div>
        </div>
      )}

      <div className="divide-y divide-teal-100 dark:divide-teal-900/40">
        {workingPlan.batches.map((batch) => (
          <div key={batch.index}>
            <div className="px-3 py-1 bg-teal-50/50 dark:bg-teal-900/10 text-2xs font-medium text-teal-600 dark:text-teal-400">
              {t('plan.batch', 'Batch')} {batch.index + 1}
              <span className="ml-1 text-gray-400">
                ({batch.stepIds.length} {batch.stepIds.length === 1 ? t('plan.step', 'step') : t('plan.steps', 'steps')}
                )
              </span>
            </div>
            {batch.stepIds.map((stepId) => {
              const step = workingPlan.steps.find((item) => item.id === stepId);
              if (!step) return null;
              const stepIndex = workingPlan.steps.findIndex((item) => item.id === stepId);
              return (
                <StepRow
                  key={stepId}
                  step={step}
                  isExpanded={expandedSteps.has(stepId)}
                  editable={isActive}
                  canMoveUp={stepIndex > 0}
                  canMoveDown={stepIndex >= 0 && stepIndex < workingPlan.steps.length - 1}
                  canRemove={workingPlan.steps.length > 1}
                  onToggle={() => toggleStep(stepId)}
                  onEdit={() => beginEditStep(stepId)}
                  onMoveUp={() => {
                    void handleMoveStep(stepId, 'up');
                  }}
                  onMoveDown={() => {
                    void handleMoveStep(stepId, 'down');
                  }}
                  onRemove={() => {
                    void handleRemoveStep(stepId);
                  }}
                  onClearDependencies={() => {
                    void handleClearDependencies(stepId);
                  }}
                />
              );
            })}
          </div>
        ))}
      </div>

      {editingStepId && stepDraft && (
        <div className="px-3 py-3 border-t border-teal-200 dark:border-teal-800 bg-white/80 dark:bg-gray-900/40 space-y-2">
          <div className="text-xs font-semibold text-teal-700 dark:text-teal-300">
            {t('plan.editStepFor', 'Edit Step {{id}}', { id: editingStepId })}
          </div>
          <input
            value={stepDraft.title}
            onChange={(event) => setStepDraft((prev) => (prev ? { ...prev, title: event.target.value } : prev))}
            className="w-full rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            placeholder={t('plan.stepTitle', 'Step title')}
          />
          <textarea
            value={stepDraft.description}
            onChange={(event) => setStepDraft((prev) => (prev ? { ...prev, description: event.target.value } : prev))}
            rows={3}
            className="w-full rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            placeholder={t('plan.stepDescription', 'Step description')}
          />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <select
              value={stepDraft.priority}
              onChange={(event) =>
                setStepDraft((prev) =>
                  prev
                    ? {
                        ...prev,
                        priority: event.target.value as PlanStepData['priority'],
                      }
                    : prev,
                )
              }
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
            >
              <option value="high">{t('plan.priority.high', { defaultValue: 'high' })}</option>
              <option value="medium">{t('plan.priority.medium', { defaultValue: 'medium' })}</option>
              <option value="low">{t('plan.priority.low', { defaultValue: 'low' })}</option>
            </select>
            <input
              value={stepDraft.dependenciesText}
              onChange={(event) =>
                setStepDraft((prev) => (prev ? { ...prev, dependenciesText: event.target.value } : prev))
              }
              className="rounded border border-teal-200 dark:border-teal-700 px-2 py-1 text-xs bg-white dark:bg-gray-900"
              placeholder={t('plan.dependenciesCsv', 'Dependencies (comma-separated IDs)')}
            />
          </div>
          <div className="flex items-center justify-end gap-2">
            <button
              onClick={cancelEditStep}
              className="px-2.5 py-1 rounded text-xs border border-gray-300 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
            >
              {t('common:cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              onClick={() => {
                void saveEditStep();
              }}
              className="px-2.5 py-1 rounded text-xs font-medium bg-teal-600 text-white hover:bg-teal-700 transition-colors"
            >
              {t('common:save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      )}

      {editSummary.length > 0 && (
        <div className="px-3 py-2 border-t border-teal-200 dark:border-teal-800 bg-teal-50/60 dark:bg-teal-900/15 space-y-2">
          <div className="text-2xs font-semibold uppercase tracking-wide text-teal-700 dark:text-teal-300">
            {t('plan.editSummary.title', 'Plan Edit Summary')}
          </div>
          {editSummary.slice(0, 5).map((entry) => (
            <div
              key={entry.id}
              className="rounded border border-teal-200 dark:border-teal-700 bg-white dark:bg-gray-900 px-2 py-1.5"
            >
              <div className="text-2xs font-medium text-gray-700 dark:text-gray-200">
                {t(`plan.editSummary.operation.${entry.operation}`, {
                  defaultValue: entry.operation,
                })}
                {entry.stepId ? ` · ${entry.stepId}` : ''}
                {entry.details ? ` · ${entry.details}` : ''}
              </div>
              <div className="mt-1 flex items-center gap-2 text-2xs">
                <span
                  className={clsx(
                    'px-1.5 py-0.5 rounded border',
                    entry.status === 'success'
                      ? 'border-emerald-200 dark:border-emerald-700 text-emerald-700 dark:text-emerald-300'
                      : entry.status === 'failed'
                        ? 'border-rose-200 dark:border-rose-700 text-rose-700 dark:text-rose-300'
                        : 'border-amber-200 dark:border-amber-700 text-amber-700 dark:text-amber-300',
                  )}
                >
                  {entry.status === 'success'
                    ? t('plan.editSummary.status.success', { defaultValue: 'Applied' })
                    : entry.status === 'failed'
                      ? t('plan.editSummary.status.failed', { defaultValue: 'Failed' })
                      : t('plan.editSummary.status.pending', { defaultValue: 'Pending' })}
                </span>
                {entry.status === 'failed' && failedOps[entry.id] && (
                  <button
                    onClick={() => {
                      void handleRetryFailedEdit(entry.id);
                    }}
                    disabled={!!pendingOps[entry.id]}
                    className="px-1.5 py-0.5 rounded border border-amber-300 dark:border-amber-700 text-amber-700 dark:text-amber-300 hover:bg-amber-50 dark:hover:bg-amber-900/30 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
                  >
                    {pendingOps[entry.id]
                      ? t('plan.editSummary.retrying', { defaultValue: 'Retrying...' })
                      : t('plan.editSummary.retry', { defaultValue: 'Retry' })}
                  </button>
                )}
              </div>
              {entry.before && (
                <div className="mt-1 text-2xs text-gray-500 dark:text-gray-400">
                  <span className="font-medium">{t('common:before', { defaultValue: 'Before' })}:</span> {entry.before}
                </div>
              )}
              {entry.after && (
                <div className="text-2xs text-gray-500 dark:text-gray-400">
                  <span className="font-medium">{t('common:after', { defaultValue: 'After' })}:</span> {entry.after}
                </div>
              )}
              {entry.batchRecomputed && (
                <div className="text-2xs text-teal-600 dark:text-teal-300">
                  {t('plan.editSummary.batchRecomputed', {
                    defaultValue: 'Execution batches recomputed.',
                  })}
                </div>
              )}
              {entry.status === 'failed' && entry.errorMessage && (
                <div className="text-2xs text-rose-600 dark:text-rose-300">{entry.errorMessage}</div>
              )}
            </div>
          ))}
        </div>
      )}

      {validationIssues.length > 0 && (
        <div className="px-3 py-2 border-t border-rose-200 dark:border-rose-800 bg-rose-50/70 dark:bg-rose-900/20 space-y-1">
          <div className="text-2xs font-semibold uppercase tracking-wide text-rose-700 dark:text-rose-300">
            {t('plan.validation.blockTitle', { defaultValue: 'Execution blocked by plan validation' })}
          </div>
          <ul className="space-y-0.5">
            {validationIssues.map((issue, index) => (
              <li
                key={`${issue.code}-${issue.stepId ?? 'global'}-${index}`}
                className="text-2xs text-rose-700 dark:text-rose-300"
              >
                {summarizeValidationIssue(t, issue)}
              </li>
            ))}
          </ul>
          <div className="text-2xs text-rose-600 dark:text-rose-300">
            {t('plan.validation.fixSuggestion', {
              defaultValue: 'Fix the issues above before execution.',
            })}
          </div>
        </div>
      )}

      {(isActive || failedSteps.length > 0) && (
        <div className="px-3 py-2 border-t border-teal-200 dark:border-teal-800 flex flex-wrap items-center justify-end gap-2">
          {failedSteps.map((step) => (
            <button
              key={step.id}
              onClick={() => {
                void handleRetryStep(step.id);
              }}
              disabled={!!retryingStepId}
              className="px-2.5 py-1 rounded-md text-xs font-medium border border-amber-300 dark:border-amber-700 text-amber-700 dark:text-amber-300 hover:bg-amber-50 dark:hover:bg-amber-900/30 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
            >
              {retryingStepId === step.id
                ? t('plan.retryingStep', 'Retrying {{id}}...', { id: step.id })
                : t('plan.retryStep', 'Retry {{id}}', { id: step.id })}
            </button>
          ))}

          {isActive && (
            <button
              onClick={() => {
                void handleApprove();
              }}
              disabled={isSubmitting || validationIssues.length > 0}
              className={clsx(
                'px-3 py-1.5 rounded-md text-xs font-medium',
                'bg-teal-600 text-white hover:bg-teal-700 disabled:opacity-60 disabled:cursor-not-allowed',
                'transition-colors',
              )}
            >
              {t('plan.approveAndExecute', 'Approve & Execute')}
            </button>
          )}
          {submitError && <div className="w-full text-2xs text-rose-600 dark:text-rose-300">{submitError}</div>}
        </div>
      )}
    </div>
  );
}
