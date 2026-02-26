/**
 * Plan Card
 *
 * Interactive plan review card for the reviewing_plan phase.
 * Displays steps grouped by batch with collapsible details.
 * Supports approve & execute action.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon } from '@radix-ui/react-icons';
import type { PlanCardData, PlanStepData } from '../../../types/planModeCard';
import { usePlanOrchestratorStore } from '../../../store/planOrchestrator';

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

function StepRow({ step, isExpanded, onToggle }: { step: PlanStepData; isExpanded: boolean; onToggle: () => void }) {
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
                {step.completionCriteria.map((c, i) => (
                  <li key={i} className="text-2xs text-gray-600 dark:text-gray-400 flex items-start gap-1">
                    <span className="text-teal-500 mt-0.5">&#x2022;</span>
                    {c}
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
        </div>
      )}
    </div>
  );
}

export function PlanCard({ data, interactive }: { data: PlanCardData; interactive: boolean }) {
  const { t } = useTranslation('planMode');
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());
  const approvePlan = usePlanOrchestratorStore((s) => s.approvePlan);
  const phase = usePlanOrchestratorStore((s) => s.phase);
  const isActive = interactive && phase === 'reviewing_plan';

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

  const handleApprove = () => {
    approvePlan(data);
  };

  return (
    <div className="rounded-lg border border-teal-200 dark:border-teal-800 bg-teal-50 dark:bg-teal-900/20">
      {/* Header */}
      <div className="px-3 py-2 bg-teal-100/50 dark:bg-teal-900/30 border-b border-teal-200 dark:border-teal-800 flex items-center justify-between">
        <span className="text-xs font-semibold text-teal-700 dark:text-teal-300">
          {data.title || t('plan.title', 'Plan')}
        </span>
        <span className="text-2xs text-teal-600 dark:text-teal-400">
          {data.steps.length} {t('plan.steps', 'steps')} / {data.batches.length} {t('plan.batches', 'batches')}
        </span>
      </div>

      {/* Description */}
      {data.description && (
        <div className="px-3 py-1.5 text-xs text-gray-600 dark:text-gray-400 border-b border-teal-100 dark:border-teal-900/40">
          {data.description}
        </div>
      )}

      {/* Steps grouped by batch */}
      <div className="divide-y divide-teal-100 dark:divide-teal-900/40">
        {data.batches.map((batch) => (
          <div key={batch.index}>
            <div className="px-3 py-1 bg-teal-50/50 dark:bg-teal-900/10 text-2xs font-medium text-teal-600 dark:text-teal-400">
              {t('plan.batch', 'Batch')} {batch.index + 1}
              <span className="ml-1 text-gray-400">
                ({batch.stepIds.length} {batch.stepIds.length === 1 ? t('plan.step', 'step') : t('plan.steps', 'steps')}
                )
              </span>
            </div>
            {batch.stepIds.map((stepId) => {
              const step = data.steps.find((s) => s.id === stepId);
              if (!step) return null;
              return (
                <StepRow
                  key={stepId}
                  step={step}
                  isExpanded={expandedSteps.has(stepId)}
                  onToggle={() => toggleStep(stepId)}
                />
              );
            })}
          </div>
        ))}
      </div>

      {/* Action buttons */}
      {isActive && (
        <div className="px-3 py-2 border-t border-teal-200 dark:border-teal-800 flex items-center justify-end gap-2">
          <button
            onClick={handleApprove}
            className={clsx(
              'px-3 py-1.5 rounded-md text-xs font-medium',
              'bg-teal-600 text-white hover:bg-teal-700',
              'transition-colors',
            )}
          >
            {t('plan.approveAndExecute', 'Approve & Execute')}
          </button>
        </div>
      )}
    </div>
  );
}
