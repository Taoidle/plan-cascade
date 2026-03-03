import type { GateResult, GateStatus } from '../store/taskMode';

export function deriveGateOverallStatus(gates: GateResult[]): GateStatus {
  if (gates.some((gate) => gate.status === 'failed')) {
    return 'failed';
  }
  if (gates.some((gate) => gate.status === 'running')) {
    return 'running';
  }
  if (gates.some((gate) => gate.status === 'pending')) {
    return 'pending';
  }
  if (gates.length > 0 && gates.every((gate) => gate.status === 'skipped')) {
    return 'skipped';
  }
  return 'passed';
}
