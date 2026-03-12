import { useWorkflowKernelStore } from '../store/workflowKernel';
import type { WorkflowMode } from '../types/workflowKernel';
import type { QualityCustomGate, QualityGateOutcome } from '../types/workflowQuality';

interface RunCustomQualityOptions {
  mode: WorkflowMode;
  projectPath: string | null | undefined;
  scopeId?: string | null;
  metadata?: Record<string, unknown> | null;
  customGates: QualityCustomGate[];
}

export async function runCustomQualityGatesForMode(options: RunCustomQualityOptions): Promise<QualityGateOutcome[]> {
  const projectPath = options.projectPath?.trim();
  if (!projectPath) {
    return [];
  }
  const gates = options.customGates.filter((gate) => gate.modes.includes(options.mode));
  if (gates.length === 0) {
    return [];
  }
  return useWorkflowKernelStore
    .getState()
    .runCustomQualityGates(options.mode, projectPath, gates, options.scopeId, options.metadata ?? null);
}
