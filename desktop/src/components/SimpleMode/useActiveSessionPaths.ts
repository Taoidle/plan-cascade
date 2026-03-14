import { useMemo } from 'react';
import type { WorkflowSession } from '../../types/workflowKernel';
import { resolveSessionRuntimePath, resolveSessionWorkspaceRootPath } from '../../lib/workflowSessionPaths';

export interface ActiveSessionPaths {
  workspaceRootPath: string | null;
  runtimePath: string | null;
  runtimeKind: NonNullable<WorkflowSession['runtime']>['runtimeKind'];
  runtimeBranch: string | null;
  runtimeTargetBranch: string | null;
  managedWorktreeId: string | null;
}

export function useActiveSessionPaths(
  session: WorkflowSession | null,
  fallbackWorkspacePath: string | null,
): ActiveSessionPaths {
  return useMemo(
    () => ({
      workspaceRootPath: resolveSessionWorkspaceRootPath(session, fallbackWorkspacePath),
      runtimePath: resolveSessionRuntimePath(session, fallbackWorkspacePath),
      runtimeKind: session?.runtime?.runtimeKind ?? 'main',
      runtimeBranch: session?.runtime?.branch ?? null,
      runtimeTargetBranch: session?.runtime?.targetBranch ?? null,
      managedWorktreeId: session?.runtime?.managedWorktreeId ?? null,
    }),
    [fallbackWorkspacePath, session],
  );
}
