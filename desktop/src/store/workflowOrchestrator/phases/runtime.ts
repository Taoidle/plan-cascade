export type SetWorkflowStateFn = (
  partial: Record<string, unknown> | ((state: unknown) => Record<string, unknown>),
) => void;

export type GetWorkflowStateFn = () => unknown;

export interface WorkflowPhaseRuntime {
  set: SetWorkflowStateFn;
  get: GetWorkflowStateFn;
  runToken: number;
  isRunActive: (get: GetWorkflowStateFn, runToken: number) => boolean;
  resolveTaskSessionId: (get: GetWorkflowStateFn, set: SetWorkflowStateFn) => string | null;
}
