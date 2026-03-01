/**
 * Context Ops Store
 *
 * Frontend control plane for Context v2:
 * - Inspector data (latest envelope)
 * - Trace viewer
 * - Context artifacts
 * - Rollout / A-B / chaos operations dashboard
 */

import { create } from 'zustand';
import {
  applyContextArtifact,
  deleteContextArtifact,
  getContextOpsDashboard,
  getContextPolicy,
  getContextRollout,
  getContextTrace,
  listContextArtifacts,
  listContextChaosRuns,
  runContextChaosProbe,
  saveContextArtifact,
  setContextPolicy,
  setContextRollout,
  type ContextArtifactMeta,
  type ContextChaosProbeReport,
  type ContextChaosRunMeta,
  type ContextEnvelope,
  type ContextOpsDashboard,
  type ContextPolicy,
  type ContextRolloutConfig,
  type ContextTrace,
} from '../lib/contextApi';

const DEFAULT_POLICY: ContextPolicy = {
  context_v2_pipeline: true,
  memory_v2_ranker: true,
  context_inspector_ui: false,
  pinned_sources: [],
  excluded_sources: [],
  soft_threshold_ratio: 0.85,
  hard_threshold_ratio: 0.95,
};

const DEFAULT_ROLLOUT: ContextRolloutConfig = {
  enabled: true,
  rollout_percentage: 100,
  ab_mode: 'off',
  experiment_key: null,
  chaos_enabled: false,
  chaos_probability: 0,
};

export interface ContextOpsState {
  latestEnvelope: ContextEnvelope | null;
  selectedTraceId: string | null;
  traces: Record<string, ContextTrace>;
  policy: ContextPolicy;
  rollout: ContextRolloutConfig;
  artifacts: ContextArtifactMeta[];
  dashboard: ContextOpsDashboard | null;
  chaosRuns: ContextChaosRunMeta[];
  lastChaosReport: ContextChaosProbeReport | null;
  isBusy: boolean;
  error: string | null;

  setLatestEnvelope: (envelope: ContextEnvelope | null) => void;
  selectTrace: (traceId: string | null) => void;
  refreshPolicy: () => Promise<void>;
  savePolicy: (patch: Partial<ContextPolicy>) => Promise<boolean>;
  refreshRollout: () => Promise<void>;
  saveRollout: (patch: Partial<ContextRolloutConfig>) => Promise<boolean>;
  loadTrace: (traceId: string) => Promise<void>;
  loadArtifacts: (projectPath: string, sessionId?: string | null) => Promise<void>;
  saveCurrentEnvelopeAsArtifact: (name: string, projectPath: string, sessionId?: string | null) => Promise<boolean>;
  applyArtifact: (artifactId: string, sessionId?: string | null) => Promise<boolean>;
  deleteArtifact: (artifactId: string, projectPath: string, sessionId?: string | null) => Promise<boolean>;
  loadDashboard: (projectPath: string, windowHours?: number) => Promise<void>;
  runChaosProbe: (
    projectPath: string,
    sessionId?: string | null,
    iterations?: number,
    failureProbability?: number,
  ) => Promise<boolean>;
  loadChaosRuns: (projectPath: string, limit?: number) => Promise<void>;
  clearError: () => void;
}

export const useContextOpsStore = create<ContextOpsState>()((set, get) => ({
  latestEnvelope: null,
  selectedTraceId: null,
  traces: {},
  policy: DEFAULT_POLICY,
  rollout: DEFAULT_ROLLOUT,
  artifacts: [],
  dashboard: null,
  chaosRuns: [],
  lastChaosReport: null,
  isBusy: false,
  error: null,

  setLatestEnvelope: (envelope) => {
    set((state) => ({
      latestEnvelope: envelope,
      selectedTraceId: envelope?.trace_id ?? state.selectedTraceId,
    }));
  },

  selectTrace: (traceId) => set({ selectedTraceId: traceId }),

  refreshPolicy: async () => {
    const response = await getContextPolicy();
    if (response.success && response.data) {
      set({ policy: response.data, error: null });
      return;
    }
    set({ error: response.error ?? 'Failed to load context policy' });
  },

  savePolicy: async (patch) => {
    const nextPolicy: ContextPolicy = {
      ...get().policy,
      ...patch,
      pinned_sources: patch.pinned_sources ?? get().policy.pinned_sources,
      excluded_sources: patch.excluded_sources ?? get().policy.excluded_sources,
    };
    const response = await setContextPolicy(nextPolicy);
    if (!response.success) {
      set({ error: response.error ?? 'Failed to save context policy' });
      return false;
    }
    set({ policy: nextPolicy, error: null });
    return true;
  },

  refreshRollout: async () => {
    const response = await getContextRollout();
    if (response.success && response.data) {
      set({ rollout: response.data, error: null });
      return;
    }
    set({ error: response.error ?? 'Failed to load rollout config' });
  },

  saveRollout: async (patch) => {
    const nextRollout: ContextRolloutConfig = {
      ...get().rollout,
      ...patch,
    };
    const response = await setContextRollout(nextRollout);
    if (!response.success) {
      set({ error: response.error ?? 'Failed to save rollout config' });
      return false;
    }
    set({ rollout: nextRollout, error: null });
    return true;
  },

  loadTrace: async (traceId: string) => {
    const id = traceId.trim();
    if (!id) return;
    const response = await getContextTrace(id);
    if (response.success && response.data) {
      set((state) => ({
        traces: { ...state.traces, [id]: response.data! },
        selectedTraceId: id,
        error: null,
      }));
      return;
    }
    set({ error: response.error ?? 'Failed to load trace' });
  },

  loadArtifacts: async (projectPath: string, sessionId?: string | null) => {
    if (!projectPath.trim()) {
      set({ artifacts: [] });
      return;
    }
    const response = await listContextArtifacts({
      project_path: projectPath,
      session_id: sessionId ?? null,
      limit: 100,
    });
    if (response.success && response.data) {
      set({ artifacts: response.data, error: null });
      return;
    }
    set({ error: response.error ?? 'Failed to load context artifacts' });
  },

  saveCurrentEnvelopeAsArtifact: async (name, projectPath, sessionId) => {
    const envelope = get().latestEnvelope;
    if (!envelope) {
      set({ error: 'No context envelope available to save' });
      return false;
    }
    const trimmedName = name.trim();
    if (!trimmedName) {
      set({ error: 'Artifact name is required' });
      return false;
    }
    const response = await saveContextArtifact({
      name: trimmedName,
      project_path: projectPath,
      session_id: sessionId ?? null,
      envelope,
    });
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to save context artifact' });
      return false;
    }
    set((state) => ({
      artifacts: [response.data!, ...state.artifacts.filter((a) => a.id !== response.data!.id)],
      error: null,
    }));
    return true;
  },

  applyArtifact: async (artifactId, sessionId) => {
    const response = await applyContextArtifact(artifactId, sessionId ?? null);
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to apply context artifact' });
      return false;
    }
    set({
      latestEnvelope: response.data.envelope,
      selectedTraceId: response.data.envelope.trace_id,
      error: null,
    });
    return true;
  },

  deleteArtifact: async (artifactId, projectPath, sessionId) => {
    const response = await deleteContextArtifact(artifactId);
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to delete context artifact' });
      return false;
    }
    await get().loadArtifacts(projectPath, sessionId ?? null);
    return true;
  },

  loadDashboard: async (projectPath, windowHours) => {
    const response = await getContextOpsDashboard(projectPath, windowHours);
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to load context ops dashboard' });
      return;
    }
    set({
      dashboard: response.data,
      policy: response.data.policy,
      rollout: response.data.rollout,
      error: null,
    });
  },

  runChaosProbe: async (projectPath, sessionId, iterations, failureProbability) => {
    set({ isBusy: true });
    const response = await runContextChaosProbe({
      project_path: projectPath,
      session_id: sessionId ?? null,
      iterations,
      failure_probability: failureProbability,
    });
    set({ isBusy: false });
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to run chaos probe' });
      return false;
    }
    set({ lastChaosReport: response.data, error: null });
    await get().loadChaosRuns(projectPath, 20);
    await get().loadDashboard(projectPath, get().dashboard?.window_hours ?? 24);
    return true;
  },

  loadChaosRuns: async (projectPath, limit) => {
    const response = await listContextChaosRuns(projectPath, limit ?? 20);
    if (!response.success || !response.data) {
      set({ error: response.error ?? 'Failed to load chaos run history' });
      return;
    }
    set({ chaosRuns: response.data, error: null });
  },

  clearError: () => set({ error: null }),
}));
