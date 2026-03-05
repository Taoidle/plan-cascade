import { describe, expect, it } from 'vitest';
import {
  isPlanPhaseBusy,
  isPlanPhaseTerminal,
  isTaskPhaseBusy,
  isTaskPhaseTerminal,
  markUnknownPhaseForReporting,
  normalizePlanPhase,
  normalizeTaskPhase,
  resolveModeSwitchBlockReasonFromKernel,
} from './workflowPhaseModel';

describe('workflowPhaseModel', () => {
  it('normalizes known task/plan phases', () => {
    expect(normalizeTaskPhase('generating_prd')).toBe('generating_prd');
    expect(normalizePlanPhase('reviewing_plan')).toBe('reviewing_plan');
  });

  it('treats unknown phase as unknown and conservatively busy', () => {
    expect(normalizeTaskPhase('mystery')).toBe('unknown');
    expect(normalizePlanPhase('mystery')).toBe('unknown');
    expect(isTaskPhaseBusy('mystery')).toBe(true);
    expect(isPlanPhaseBusy('mystery')).toBe(true);
  });

  it('distinguishes terminal phases for task and plan', () => {
    expect(isTaskPhaseTerminal('completed')).toBe(true);
    expect(isTaskPhaseTerminal('executing')).toBe(false);
    expect(isPlanPhaseTerminal('failed')).toBe(true);
    expect(isPlanPhaseTerminal('planning')).toBe(false);
  });

  it('keeps mode switch blocking behavior aligned with kernel state', () => {
    expect(
      resolveModeSwitchBlockReasonFromKernel({
        isRunning: false,
        workflowMode: 'task',
        workflowPhase: 'analyzing',
        planPhase: 'idle',
        isTaskWorkflowActive: false,
        isPlanWorkflowActive: false,
        hasStructuredInterviewQuestion: false,
        hasPlanClarifyQuestion: false,
      }),
    ).toBe('task_workflow_active');
  });

  it('reports unknown phases only once per mode/phase tuple', () => {
    expect(markUnknownPhaseForReporting('task', 'mystery_task_phase')).toBe(true);
    expect(markUnknownPhaseForReporting('task', 'mystery_task_phase')).toBe(false);
    expect(markUnknownPhaseForReporting('plan', 'mystery_plan_phase')).toBe(true);
    expect(markUnknownPhaseForReporting('plan', 'mystery_plan_phase')).toBe(false);
  });
});
