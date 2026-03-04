import type { UserInputIntent, UserInputIntentType, WorkflowMode } from '../types/workflowKernel';
import { withWorkflowClientRequestMetadata } from './workflowClientRequest';

const ACTION_INTENT_PROTOCOL = 'workflow_kernel_action_intent_v1';
const ACTION_INTENT_VERSION = 1;

function normalizeCode(value: string, fallback: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return normalized || fallback;
}

interface BaseKernelIntentSpec {
  mode: WorkflowMode;
  type: UserInputIntentType;
  source: string;
  action: string;
  content?: string;
  phase?: string | null;
  reasonCode?: string | null;
  metadata?: Record<string, unknown>;
}

export interface SubmitKernelIntentSpec extends BaseKernelIntentSpec {
  transitionAndSubmitInput: (mode: WorkflowMode, intent: UserInputIntent) => Promise<unknown>;
}

export function createWorkflowKernelActionIntent(spec: BaseKernelIntentSpec): UserInputIntent {
  const source = normalizeCode(spec.source, 'unknown_source');
  const action = normalizeCode(spec.action, 'unknown_action');
  const reasonCode = spec.reasonCode ? normalizeCode(spec.reasonCode, 'unknown_reason') : null;
  const content = spec.content?.trim() || `[${action}]`;

  const metadata = withWorkflowClientRequestMetadata(
    {
      ...(spec.metadata ?? {}),
      protocol: ACTION_INTENT_PROTOCOL,
      version: ACTION_INTENT_VERSION,
      mode: spec.mode,
      source,
      action,
      ...(spec.phase ? { phase: spec.phase } : {}),
      ...(reasonCode ? { reasonCode } : {}),
    },
    spec.mode,
    action,
  );

  return {
    type: spec.type,
    content,
    metadata,
  };
}

export async function submitWorkflowKernelActionIntent(spec: SubmitKernelIntentSpec): Promise<unknown> {
  const intent = createWorkflowKernelActionIntent(spec);
  return spec.transitionAndSubmitInput(spec.mode, intent);
}
