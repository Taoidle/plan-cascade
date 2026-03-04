import type { PlanBatchData, PlanCardData, PlanStepData } from '../../../types/planModeCard';

export const PLAN_MAX_PARALLEL_MIN = 1;
export const PLAN_MAX_PARALLEL_MAX = 8;
export const PLAN_DEFAULT_MAX_PARALLEL = 4;

export type PlanValidationIssueCode =
  | 'duplicate_step_id'
  | 'missing_dependency'
  | 'self_dependency'
  | 'cycle_dependency'
  | 'parallel_out_of_range';

export interface PlanValidationIssue {
  code: PlanValidationIssueCode;
  stepId?: string;
  dependencyId?: string;
}

export function clampPlanMaxParallel(value: number): number {
  if (!Number.isFinite(value)) return PLAN_DEFAULT_MAX_PARALLEL;
  return Math.min(PLAN_MAX_PARALLEL_MAX, Math.max(PLAN_MAX_PARALLEL_MIN, Math.trunc(value)));
}

export function getPlanMaxParallel(plan: Pick<PlanCardData, 'executionConfig'>): number {
  return clampPlanMaxParallel(plan.executionConfig?.maxParallel ?? PLAN_DEFAULT_MAX_PARALLEL);
}

export function ensurePlanExecutionConfig(plan: PlanCardData): PlanCardData {
  return {
    ...plan,
    executionConfig: {
      maxParallel: getPlanMaxParallel(plan),
    },
  };
}

function buildStepLookup(steps: PlanStepData[]): Map<string, PlanStepData> {
  return new Map(steps.map((step) => [step.id, step]));
}

export function recomputePlanBatches(steps: PlanStepData[], maxParallel: number): PlanBatchData[] {
  const normalizedMaxParallel = clampPlanMaxParallel(maxParallel);
  const stepById = buildStepLookup(steps);
  const remaining = new Set(steps.map((step) => step.id));
  const inDegree = new Map<string, number>();
  const dependents = new Map<string, string[]>();

  for (const step of steps) {
    const deps = step.dependencies.filter((dep) => stepById.has(dep) && dep !== step.id);
    inDegree.set(step.id, deps.length);
    for (const dep of deps) {
      dependents.set(dep, [...(dependents.get(dep) ?? []), step.id]);
    }
  }

  const batches: PlanBatchData[] = [];
  while (remaining.size > 0) {
    const ready = [...remaining].filter((stepId) => (inDegree.get(stepId) ?? 0) === 0);
    if (ready.length === 0) {
      throw new Error('cycle_dependency');
    }

    ready.sort((a, b) => {
      const aIndex = steps.findIndex((step) => step.id === a);
      const bIndex = steps.findIndex((step) => step.id === b);
      return aIndex - bIndex;
    });

    for (let index = 0; index < ready.length; index += normalizedMaxParallel) {
      const chunk = ready.slice(index, index + normalizedMaxParallel);
      batches.push({
        index: batches.length,
        stepIds: chunk,
      });
    }

    for (const stepId of ready) {
      remaining.delete(stepId);
      const children = dependents.get(stepId) ?? [];
      for (const childId of children) {
        inDegree.set(childId, Math.max(0, (inDegree.get(childId) ?? 0) - 1));
      }
    }
  }

  return batches;
}

function findCycleStepId(steps: PlanStepData[]): string | null {
  const stepById = buildStepLookup(steps);
  const visited = new Set<string>();
  const stack = new Set<string>();

  const dfs = (stepId: string): string | null => {
    if (stack.has(stepId)) return stepId;
    if (visited.has(stepId)) return null;
    visited.add(stepId);
    stack.add(stepId);

    const step = stepById.get(stepId);
    if (step) {
      for (const dep of step.dependencies) {
        if (!stepById.has(dep)) continue;
        const cycleAt = dfs(dep);
        if (cycleAt) return cycleAt;
      }
    }

    stack.delete(stepId);
    return null;
  };

  for (const step of steps) {
    const cycleAt = dfs(step.id);
    if (cycleAt) return cycleAt;
  }
  return null;
}

export function validatePlanDraft(plan: PlanCardData): PlanValidationIssue[] {
  const issues: PlanValidationIssue[] = [];
  const maxParallel = plan.executionConfig?.maxParallel ?? PLAN_DEFAULT_MAX_PARALLEL;
  if (!Number.isFinite(maxParallel) || maxParallel < PLAN_MAX_PARALLEL_MIN || maxParallel > PLAN_MAX_PARALLEL_MAX) {
    issues.push({ code: 'parallel_out_of_range' });
  }

  const idCount = new Map<string, number>();
  for (const step of plan.steps) {
    idCount.set(step.id, (idCount.get(step.id) ?? 0) + 1);
  }
  for (const [stepId, count] of idCount.entries()) {
    if (count > 1) {
      issues.push({ code: 'duplicate_step_id', stepId });
    }
  }

  const existingIds = new Set(plan.steps.map((step) => step.id));
  for (const step of plan.steps) {
    for (const dep of step.dependencies) {
      if (dep === step.id) {
        issues.push({ code: 'self_dependency', stepId: step.id, dependencyId: dep });
        continue;
      }
      if (!existingIds.has(dep)) {
        issues.push({ code: 'missing_dependency', stepId: step.id, dependencyId: dep });
      }
    }
  }

  const cycleAt = findCycleStepId(plan.steps);
  if (cycleAt) {
    issues.push({ code: 'cycle_dependency', stepId: cycleAt });
  }

  return issues;
}
