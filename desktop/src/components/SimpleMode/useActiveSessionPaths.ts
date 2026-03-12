import { useMemo } from 'react';
import type { WorkflowSession } from '../../types/workflowKernel';

export interface ActiveSessionPaths {
  workspaceRootPath: string | null;
  runtimePath: string | null;
  runtimeKind: NonNullable<WorkflowSession['runtime']>['runtimeKind'];
  runtimeBranch: string | null;
  runtimeTargetBranch: string | null;
  managedWorktreeId: string | null;
}

export function resolveSessionRuntimePath(
  session: WorkflowSession | null,
  fallbackWorkspacePath: string | null,
): string | null {
  return session?.runtime?.runtimePath ?? session?.workspacePath ?? fallbackWorkspacePath;
}

export function useActiveSessionPaths(
  session: WorkflowSession | null,
  fallbackWorkspacePath: string | null,
): ActiveSessionPaths {
  return useMemo(
    () => ({
      workspaceRootPath: session?.runtime?.rootPath ?? session?.workspacePath ?? fallbackWorkspacePath,
      runtimePath: resolveSessionRuntimePath(session, fallbackWorkspacePath),
      runtimeKind: session?.runtime?.runtimeKind ?? 'main',
      runtimeBranch: session?.runtime?.branch ?? null,
      runtimeTargetBranch: session?.runtime?.targetBranch ?? null,
      managedWorktreeId: session?.runtime?.managedWorktreeId ?? null,
    }),
    [fallbackWorkspacePath, session],
  );
}
