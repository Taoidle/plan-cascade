import { useWorkflowKernelSessionBridge } from './useWorkflowKernelSessionBridge';

export function useSimpleKernelSession(params: Parameters<typeof useWorkflowKernelSessionBridge>[0]) {
  return useWorkflowKernelSessionBridge(params);
}
