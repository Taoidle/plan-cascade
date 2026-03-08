import type { TFunction } from 'i18next';
import { useWorkflowModeSwitchGuard } from './useWorkflowModeSwitchGuard';
import type { HandoffContextBundle, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';

type ToastLevel = 'info' | 'success' | 'error';

export function useSimpleModeSwitch(params: {
  workflowMode: WorkflowMode;
  isRunning: boolean;
  workflowPhase: string;
  planPhase: string;
  isTaskWorkflowActive: boolean;
  isPlanWorkflowActive: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasPlanClarifyQuestion: boolean;
  setWorkflowMode: (mode: WorkflowMode) => void;
  transitionWorkflowKernelMode: (
    targetMode: WorkflowMode,
    handoff: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  appendWorkflowKernelContextItems?: (
    targetMode: WorkflowMode,
    handoff: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  showToast: (message: string, level?: ToastLevel) => void;
  t: TFunction<'simpleMode'>;
}) {
  return useWorkflowModeSwitchGuard(params);
}
