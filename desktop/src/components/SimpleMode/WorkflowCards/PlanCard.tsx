/**
 * Plan Card
 *
 * Interactive plan review card for the reviewing_plan phase.
 * Supports step edits, kernel-side edit audit events, execute, and retry hooks.
 */

import { useState, useCallback, useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon } from '@radix-ui/react-icons';
import type { PlanCardData, PlanStepData } from '../../../types/planModeCard';
import { usePlanOrchestratorStore } from '../../../store/planOrchestrator';
import { usePlanModeStore } from '../../../store/planMode';
import { useWorkflowKernelStore } from '../../../store/workflowKernel';
import {
  applyPlanEditViaCoordinator,
  executePlanViaCoordinator,
  retryPlanStepViaCoordinator,
  submitWorkflowActionIntentViaCoordinator,
} from '../../../store/simpleWorkflowCoordinator';

interface StepEditDraft {
  title: string;
  description: string;
  priority: PlanStepData['priority'];
  dependenciesText: string;
}

interface PlanEditSummaryEntry {
  id: string;
  stepId: string;
  changedFields: Array<'title' | 'description' | 'priority' | 'dependencies'>;
  before: Pick<PlanStepData, 'title' | 'description' | 'priority' | 'dependencies'>;
  after: Pick<PlanStepData, 'title' | 'description' | 'priority' | 'dependencies'>;
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

function StepRow({
  step,
  isExpanded,
  editable,
  onToggle,
  onEdit,
}: {
  step: PlanStepData;
  isExpanded: boolean;
  editable: boolean;
  onToggle: () => void;
  onEdit: () => void;
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
          {t(`plan.priority.${step.priority}`, step.priority)}
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
            <div className="pt-1">
              <button
                onClick={onEdit}
                className="px-2 py-1 rounded text-2xs font-medium bg-white dark:bg-gray-900 border border-teal-300 dark:border-teal-700 text-teal-700 dark:text-teal-300 hover:bg-teal-50 dark:hover:bg-teal-900/30 transition-colors"
              >
                {t('plan.editStep', 'Edit Step')}
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
  const [workingPlan, setWorkingPlan] = useState<PlanCardData>(data);
  const [editSummary, setEditSummary] = useState<PlanEditSummaryEntry[]>([]);
  const [editingStepId, setEditingStepId] = useState<string | null>(null);
  const [stepDraft, setStepDraft] = useState<StepEditDraft | null>(null);
  const [acted, setActed] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [retryingStepId, setRetryingStepId] = useState<string | null>(null);

  const approvePlan = usePlanOrchestratorStore((s) => s.approvePlan);
  const phase = usePlanOrchestratorStore((s) => s.phase);
  const stepStatuses = usePlanModeStore((s) => s.stepStatuses);

  const workflowSession = useWorkflowKernelStore((s) => s.session);

  useEffect(() => {
    setWorkingPlan(data);
    setEditSummary([]);
    setEditingStepId(null);
    setStepDraft(null);
    setActed(false);
  }, [data]);

  const isKernelPlanActive = workflowSession?.status === 'active' && workflowSession.activeMode === 'plan';
  const isActive = interactive && phase === 'reviewing_plan' && isKernelPlanActive && !acted;
  const failedSteps = useMemo(
    () => workingPlan.steps.filter((step) => stepStatuses[step.id] === 'failed'),
    [workingPlan.steps, stepStatuses],
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
    const changedFields: PlanEditSummaryEntry['changedFields'] = [];

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

    setWorkingPlan((prev) => ({
      ...prev,
      steps: prev.steps.map((step) => (step.id === editingStepId ? updatedStep : step)),
    }));

    setEditSummary((prev) => [
      {
        id: `${editingStepId}-${Date.now()}`,
        stepId: editingStepId,
        changedFields,
        before: {
          title: targetStep.title,
          description: targetStep.description,
          priority: targetStep.priority,
          dependencies: [...targetStep.dependencies],
        },
        after: {
          title: updatedStep.title,
          description: updatedStep.description,
          priority: updatedStep.priority,
          dependencies: [...updatedStep.dependencies],
        },
      },
      ...prev,
    ]);

    cancelEditStep();

    try {
      await submitWorkflowActionIntentViaCoordinator({
        mode: 'plan',
        type: 'plan_edit_instruction',
        source: 'plan_card',
        action: 'edit_step',
        content: `edit_step:${editingStepId}`,
        metadata: {
          stepId: editingStepId,
          changedFields,
        },
      });
      await applyPlanEditViaCoordinator({
        type: 'update_step',
        targetStepId: editingStepId,
        payload: {
          title: updatedStep.title,
          description: updatedStep.description,
          priority: updatedStep.priority,
          dependencies: updatedStep.dependencies,
        },
      });
    } catch {
      // Keep UI responsive even if kernel intent logging fails.
    }
  }, [cancelEditStep, editingStepId, isActive, stepDraft, workingPlan.steps]);

  const handleApprove = useCallback(async () => {
    if (!isActive || isSubmitting) return;
    setActed(true);
    setIsSubmitting(true);
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
        },
      });
      await executePlanViaCoordinator();
    } catch {
      // Keep orchestration available even if kernel logging fails.
    }

    try {
      await approvePlan(workingPlan);
    } finally {
      setIsSubmitting(false);
    }
  }, [approvePlan, editSummary.length, isActive, isSubmitting, workingPlan]);

  const handleRetryStep = useCallback(
    async (stepId: string) => {
      if (!stepId || retryingStepId) return;
      setRetryingStepId(stepId);
      try {
        await submitWorkflowActionIntentViaCoordinator({
          mode: 'plan',
          type: 'execution_control',
          source: 'plan_card',
          action: 'retry_step',
          content: `retry_step:${stepId}`,
          metadata: { stepId },
        });
        await retryPlanStepViaCoordinator(stepId);
      } finally {
        setRetryingStepId(null);
      }
    },
    [retryingStepId],
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
        </span>
      </div>

      {workingPlan.description && (
        <div className="px-3 py-1.5 text-xs text-gray-600 dark:text-gray-400 border-b border-teal-100 dark:border-teal-900/40">
          {workingPlan.description}
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
              return (
                <StepRow
                  key={stepId}
                  step={step}
                  isExpanded={expandedSteps.has(stepId)}
                  editable={isActive}
                  onToggle={() => toggleStep(stepId)}
                  onEdit={() => beginEditStep(stepId)}
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
              <option value="high">{t('plan.priority.high', 'high')}</option>
              <option value="medium">{t('plan.priority.medium', 'medium')}</option>
              <option value="low">{t('plan.priority.low', 'low')}</option>
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
            {t('plan.editSummary', 'Plan Edit Summary')}
          </div>
          {editSummary.slice(0, 3).map((entry) => (
            <div
              key={entry.id}
              className="rounded border border-teal-200 dark:border-teal-700 bg-white dark:bg-gray-900 px-2 py-1.5"
            >
              <div className="text-2xs font-medium text-gray-700 dark:text-gray-200">
                {entry.stepId} - {entry.changedFields.join(', ')}
              </div>
              <div className="mt-1 text-2xs text-gray-500 dark:text-gray-400">
                <span className="font-medium">{t('common:before', { defaultValue: 'Before' })}:</span>{' '}
                {entry.before.title}
                {' | '}
                {entry.before.priority}
              </div>
              <div className="text-2xs text-gray-500 dark:text-gray-400">
                <span className="font-medium">{t('common:after', { defaultValue: 'After' })}:</span> {entry.after.title}
                {' | '}
                {entry.after.priority}
              </div>
            </div>
          ))}
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
              disabled={isSubmitting}
              className={clsx(
                'px-3 py-1.5 rounded-md text-xs font-medium',
                'bg-teal-600 text-white hover:bg-teal-700 disabled:opacity-60 disabled:cursor-not-allowed',
                'transition-colors',
              )}
            >
              {t('plan.approveAndExecute', 'Approve & Execute')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
