import type {
  ChatControlCapabilities,
  PlanClarificationSnapshot,
  TaskConfigConfirmationState,
  TaskInterviewSnapshot,
  TaskStrategyRecommendationSnapshot,
  TaskWorkflowConfigSnapshot,
  WorkflowSession,
  WorkflowStatus,
} from '../types/workflowKernel';
import type { DebugRuntimeCapabilities } from '../types/debugMode';
import {
  isChatPhaseBusy,
  isDebugPhaseBusy,
  isKernelLifecyclePhaseTerminal,
  isPlanPhaseBusy,
  isTaskPhaseBusy,
  isWorkflowModeActive,
  normalizeChatPhase,
  normalizeDebugPhase,
  normalizePlanPhase,
  normalizeTaskPhase,
  type NormalizedChatPhase,
  type NormalizedDebugPhase,
  type NormalizedPlanPhase,
  type NormalizedTaskPhase,
} from './workflowPhaseModel';

export interface KernelRuntimeBase {
  phase: string;
  linkedSessionId: string | null;
  isActive: boolean;
  pendingPrompt: string | null;
}

export interface KernelTaskRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedTaskPhase;
  isBusy: boolean;
  pendingInterview: TaskInterviewSnapshot | null;
  interviewId: string | null;
  canCancel: boolean;
  blockReason: string | null;
  strategyRecommendation: TaskStrategyRecommendationSnapshot | null;
  configConfirmationState: TaskConfigConfirmationState;
  confirmedConfig: TaskWorkflowConfigSnapshot | null;
}

export interface KernelPlanRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedPlanPhase;
  isBusy: boolean;
  pendingClarification: PlanClarificationSnapshot | null;
}

export interface KernelChatRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedChatPhase;
  isBusy: boolean;
  canQueue: boolean;
  canCancel: boolean;
  canPause: boolean;
  canResume: boolean;
  blockReason: string | null;
  bindingSessionId: string | null;
  activeTurnId: string | null;
  backendKind: string | null;
}

export interface KernelDebugRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedDebugPhase;
  isBusy: boolean;
  pendingApproval: NonNullable<WorkflowSession['modeSnapshots']['debug']>['pendingApproval'];
  capabilityProfile: NonNullable<WorkflowSession['modeSnapshots']['debug']>['capabilityProfile'];
  toolBlockReason: string | null;
  runtimeCapabilities: DebugRuntimeCapabilities | null;
}

function normalizePendingPrompt(prompt: string | null | undefined): string | null {
  const normalized = prompt?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

export function isKernelPhaseTerminal(phase: string | null | undefined): boolean {
  return isKernelLifecyclePhaseTerminal(phase);
}

export function isKernelRuntimeBusy(runtime: Pick<KernelRuntimeBase, 'phase' | 'isActive'>): boolean {
  return runtime.isActive && !isKernelPhaseTerminal(runtime.phase);
}

export function selectKernelChatRuntime(session: WorkflowSession | null): KernelChatRuntime {
  const phase = session?.modeSnapshots.chat?.phase ?? 'ready';
  const normalizedPhase = normalizeChatPhase(phase);
  const meta = session?.modeRuntimeMeta?.chat;
  const capabilities: ChatControlCapabilities | null = meta?.controlCapabilities ?? null;
  const isActive = isWorkflowModeActive({
    mode: 'chat',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  const isBusy = isActive && isChatPhaseBusy(phase);
  const blockReason = meta?.blockReason ?? null;
  return {
    phase,
    normalizedPhase,
    linkedSessionId: meta?.bindingSessionId ?? session?.linkedModeSessions?.chat ?? null,
    bindingSessionId: meta?.bindingSessionId ?? session?.linkedModeSessions?.chat ?? null,
    isActive,
    isBusy,
    pendingPrompt: normalizePendingPrompt(session?.modeSnapshots.chat?.pendingInput),
    canQueue: isBusy && blockReason !== 'tool_permission',
    canCancel: isBusy && !!capabilities?.canCancel,
    canPause: isBusy && !!capabilities?.canPause,
    canResume: isBusy && !!capabilities?.canResume,
    blockReason,
    activeTurnId: session?.modeSnapshots.chat?.activeTurnId ?? null,
    backendKind: meta?.backendKind ?? null,
  };
}

export const selectKernelChatRuntimeViewModel = selectKernelChatRuntime;

export function selectKernelTaskRuntime(session: WorkflowSession | null): KernelTaskRuntime {
  const pendingInterview = session?.modeSnapshots.task?.pendingInterview ?? null;
  const phase = session?.modeSnapshots.task?.phase ?? 'idle';
  const normalizedPhase = normalizeTaskPhase(phase);
  const meta = session?.modeRuntimeMeta?.task;
  const capabilities: ChatControlCapabilities | null = meta?.controlCapabilities ?? null;
  const isActive = isWorkflowModeActive({
    mode: 'task',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  return {
    phase,
    normalizedPhase,
    linkedSessionId: session?.linkedModeSessions?.task ?? null,
    isActive,
    isBusy: isActive && isTaskPhaseBusy(phase),
    pendingPrompt: normalizePendingPrompt(pendingInterview?.question),
    pendingInterview,
    interviewId: pendingInterview?.interviewId ?? null,
    canCancel: isActive && !isKernelPhaseTerminal(phase) && !!capabilities?.canCancel,
    blockReason: meta?.blockReason ?? null,
    strategyRecommendation: session?.modeSnapshots.task?.strategyRecommendation ?? null,
    configConfirmationState: session?.modeSnapshots.task?.configConfirmationState ?? 'pending',
    confirmedConfig: session?.modeSnapshots.task?.confirmedConfig ?? null,
  };
}

export function selectKernelPlanRuntime(session: WorkflowSession | null): KernelPlanRuntime {
  const pendingClarification = session?.modeSnapshots.plan?.pendingClarification ?? null;
  const phase = session?.modeSnapshots.plan?.phase ?? 'idle';
  const normalizedPhase = normalizePlanPhase(phase);
  const isActive = isWorkflowModeActive({
    mode: 'plan',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  return {
    phase,
    normalizedPhase,
    linkedSessionId: session?.linkedModeSessions?.plan ?? null,
    isActive,
    isBusy: isActive && isPlanPhaseBusy(phase),
    pendingPrompt: normalizePendingPrompt(pendingClarification?.question),
    pendingClarification,
  };
}

export function selectKernelDebugRuntime(session: WorkflowSession | null): KernelDebugRuntime {
  const phase = session?.modeSnapshots.debug?.phase ?? 'intaking';
  const normalizedPhase = normalizeDebugPhase(phase);
  const meta = session?.modeRuntimeMeta?.debug;
  const pendingApproval = session?.modeSnapshots.debug?.pendingApproval ?? meta?.pendingApproval ?? null;
  const isActive = isWorkflowModeActive({
    mode: 'debug',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  return {
    phase,
    normalizedPhase,
    linkedSessionId: session?.linkedModeSessions?.debug ?? null,
    isActive,
    isBusy: isActive && isDebugPhaseBusy(phase),
    pendingPrompt: normalizePendingPrompt(session?.modeSnapshots.debug?.pendingPrompt),
    pendingApproval,
    capabilityProfile: session?.modeSnapshots.debug?.capabilityProfile ?? meta?.debugCapabilityProfile ?? 'dev_full',
    toolBlockReason: session?.modeSnapshots.debug?.toolBlockReason ?? meta?.blockReason ?? null,
    runtimeCapabilities: meta?.debugRuntimeCapabilities ?? null,
  };
}

export function isKernelModeActive(
  session: WorkflowSession | null,
  mode: 'chat' | 'plan' | 'task' | 'debug',
  expectedStatus: WorkflowStatus = 'active',
): boolean {
  return isWorkflowModeActive({
    mode,
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === expectedStatus,
    phase:
      mode === 'task'
        ? (session?.modeSnapshots.task?.phase ?? 'idle')
        : mode === 'plan'
          ? (session?.modeSnapshots.plan?.phase ?? 'idle')
          : mode === 'debug'
            ? (session?.modeSnapshots.debug?.phase ?? 'intaking')
            : (session?.modeSnapshots.chat?.phase ?? 'ready'),
  });
}

export interface KernelRuntimeStatus {
  isTaskActive: boolean;
  isPlanActive: boolean;
  isDebugActive: boolean;
  isTaskBusy: boolean;
  isPlanBusy: boolean;
  isDebugBusy: boolean;
}

export function selectKernelRuntimeStatus(session: WorkflowSession | null): KernelRuntimeStatus {
  const taskRuntime = selectKernelTaskRuntime(session);
  const planRuntime = selectKernelPlanRuntime(session);
  const debugRuntime = selectKernelDebugRuntime(session);
  return {
    isTaskActive: taskRuntime.isActive,
    isPlanActive: planRuntime.isActive,
    isDebugActive: debugRuntime.isActive,
    isTaskBusy: taskRuntime.isBusy,
    isPlanBusy: planRuntime.isBusy,
    isDebugBusy: debugRuntime.isBusy,
  };
}
