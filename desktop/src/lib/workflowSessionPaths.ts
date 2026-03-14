import type { WorkflowSession, WorkflowSessionCatalogItem } from '../types/workflowKernel';

type WorkflowSessionLike =
  | Pick<WorkflowSession, 'runtime' | 'workspacePath'>
  | Pick<WorkflowSessionCatalogItem, 'runtime' | 'workspacePath'>
  | null
  | undefined;

export function resolveSessionWorkspaceRootPath(
  session: WorkflowSessionLike,
  fallbackWorkspacePath: string | null = null,
): string | null {
  return session?.runtime?.rootPath ?? session?.workspacePath ?? fallbackWorkspacePath;
}

export function resolveSessionRuntimePath(
  session: WorkflowSessionLike,
  fallbackWorkspacePath: string | null = null,
): string | null {
  return session?.runtime?.runtimePath ?? session?.workspacePath ?? fallbackWorkspacePath;
}
