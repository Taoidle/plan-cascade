/**
 * Settings Store
 *
 * Manages application settings with persistence.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import i18n from '../i18n';
import type { DebugEnvironment } from '../types/debugMode';
import {
  DEFAULT_QUALITY_SETTINGS,
  sanitizeQualityGateIds,
  sanitizeQualityCustomGates,
  type QualitySettings,
} from '../types/workflowQuality';

export type Backend = 'claude-code' | 'claude-api' | 'openai' | 'deepseek' | 'glm' | 'qwen' | 'minimax' | 'ollama';
export type Theme = 'system' | 'light' | 'dark';
export type Language = 'en' | 'zh' | 'ja';
export type SessionPathSort = 'recent' | 'name';
export type StandaloneContextTurns = 2 | 4 | 6 | 8 | 10 | 20 | 50 | 100 | 200 | 500 | -1;
export type GlmEndpoint = 'standard' | 'coding' | 'international' | 'international-coding';
export type MinimaxEndpoint = 'international' | 'china';
export type QwenEndpoint = 'china' | 'singapore' | 'us';
export type MemoryReviewMode = 'llm_review' | 'auto_approve' | 'manual_only';
export type UpdateChannel = 'stable' | 'beta' | 'alpha';
export type DebugBrowserProfile = 'desktop' | 'mobile';
const SETTINGS_PERSIST_VERSION = 10;
const PLAN_MODE_PHASE_IDS = [
  'plan_strategy',
  'plan_clarification',
  'plan_generation',
  'plan_execution',
  'plan_retry',
] as const;
const TASK_PLANNING_PHASE_IDS = [
  'plan_exploration',
  'plan_interview',
  'plan_requirements',
  'plan_architecture',
  'plan_prd',
] as const;
const EXECUTION_PHASE_IDS = ['planning', 'implementation', 'retry', 'refactor', 'review'] as const;

function normalizeProviderKey(provider: string): string {
  return provider.trim().toLowerCase();
}

interface Agent {
  name: string;
  enabled: boolean;
  command: string;
  isDefault: boolean;
}

export interface PhaseAgentConfig {
  defaultAgent: string;
  fallbackChain: string[];
}

export interface MemorySettings {
  autoExtractEnabled: boolean;
  reviewMode: MemoryReviewMode;
  reviewAgentRef: string;
  injectActiveOnly: true;
  extractSuccessfulSessionsOnly: true;
}

export interface DeveloperPanels {
  contextInspector: boolean;
  workflowReliability: boolean;
  executionLogs: boolean;
  streamingOutput: boolean;
}

export interface UpdatePreferences {
  updateChannel: UpdateChannel;
  autoCheckForUpdates: boolean;
  ignoredUpdateVersionByChannel: Record<UpdateChannel, string | null>;
  lastUpdateCheckAt: string | null;
}

interface SettingsState {
  // Backend settings
  backend: Backend;
  provider: string;
  model: string;
  modelByProvider: Record<string, string>;
  apiKey: string;

  // Agent settings
  agents: Agent[];
  agentSelection: 'smart' | 'prefer_default' | 'manual';
  defaultAgent: string;

  // Quality
  quality: QualitySettings;

  // Execution settings
  maxParallelStories: number;
  maxTotalTokens: number;
  timeoutSeconds: number;
  maxConcurrentSubagents: number;

  // UI settings
  defaultMode: 'simple' | 'expert';
  theme: Theme;
  language: Language;
  standaloneContextTurns: StandaloneContextTurns;

  // Chat UI settings
  showLineNumbers: boolean;
  maxFileAttachmentSize: number; // in bytes
  enableMarkdownMath: boolean;
  enableCodeBlockCopy: boolean;

  // Onboarding settings
  onboardingCompleted: boolean;
  tourCompleted: boolean;
  workspacePath: string;
  knowledgeAutoEnsureDocsCollection: boolean;
  kbQueryRunsV2: boolean;
  kbPickerServerSearch: boolean;
  kbIngestJobScopedProgress: boolean;

  // Sidebar settings
  pinnedDirectories: string[];
  sidebarCollapsed: boolean;
  autoPanelHoverEnabled: boolean;
  closeToBackgroundEnabled: boolean;
  worktreeAutoCleanupOnSessionDelete: boolean;
  sessionPathSort: SessionPathSort;
  showArchivedSessions: boolean;

  // Developer mode
  developerModeEnabled: boolean;
  developerPanels: DeveloperPanels;
  developerSettingsInitialized: boolean;

  // Update preferences
  updatePreferences: UpdatePreferences;

  // Context compaction
  enableContextCompaction: boolean;
  showReasoningOutput: boolean;
  enableThinking: boolean;
  showSubAgentEvents: boolean;

  // GLM endpoint
  glmEndpoint: GlmEndpoint;

  // MiniMax endpoint
  minimaxEndpoint: MinimaxEndpoint;

  // Qwen endpoint
  qwenEndpoint: QwenEndpoint;

  // Search provider settings
  searchProvider: 'tavily' | 'brave' | 'duckduckgo';

  // Debug mode settings
  debugDefaultEnvironment: DebugEnvironment;
  debugBrowserProfile: DebugBrowserProfile;
  debugViewportPresets: string[];
  debugProductionDiagnosticsAllowlist: string[];

  // Phase agent configs
  phaseConfigs: Record<string, PhaseAgentConfig>;

  // Memory pipeline settings
  memorySettings: MemorySettings;

  // Actions
  setBackend: (backend: Backend) => void;
  setProvider: (provider: string) => void;
  setModel: (model: string) => void;
  setModelByProvider: (provider: string, model: string) => void;
  setApiKey: (apiKey: string) => void;
  setTheme: (theme: Theme) => void;
  setLanguage: (language: Language) => void;
  setDefaultMode: (mode: 'simple' | 'expert') => void;
  setStandaloneContextTurns: (turns: StandaloneContextTurns) => void;
  updateAgent: (name: string, updates: Partial<Agent>) => void;
  updateQualitySettings: (updates: Partial<QualitySettings>) => void;
  resetToDefaults: () => void;
  setEnableContextCompaction: (enable: boolean) => void;
  setShowReasoningOutput: (show: boolean) => void;
  setEnableThinking: (enable: boolean) => void;
  setShowSubAgentEvents: (show: boolean) => void;
  setMaxTotalTokens: (tokens: number) => void;
  setGlmEndpoint: (endpoint: GlmEndpoint) => void;
  setMinimaxEndpoint: (endpoint: MinimaxEndpoint) => void;
  setQwenEndpoint: (endpoint: QwenEndpoint) => void;
  setMaxConcurrentSubagents: (value: number) => void;
  setSearchProvider: (provider: 'tavily' | 'brave' | 'duckduckgo') => void;
  setDebugDefaultEnvironment: (environment: DebugEnvironment) => void;
  setDebugBrowserProfile: (profile: DebugBrowserProfile) => void;
  setDebugViewportPresets: (presets: string[]) => void;
  setDebugProductionDiagnosticsAllowlist: (allowlist: string[]) => void;

  // Phase agent actions
  setPhaseConfigs: (configs: Record<string, PhaseAgentConfig>) => void;
  updatePhaseConfig: (phaseId: string, config: Partial<PhaseAgentConfig>) => void;
  setMemorySettings: (settings: MemorySettings) => void;
  updateMemorySettings: (patch: Partial<MemorySettings>) => void;

  // Chat UI actions
  setShowLineNumbers: (show: boolean) => void;
  setMaxFileAttachmentSize: (size: number) => void;
  setEnableMarkdownMath: (enable: boolean) => void;
  setEnableCodeBlockCopy: (enable: boolean) => void;

  // Onboarding actions
  setOnboardingCompleted: (completed: boolean) => void;
  setTourCompleted: (completed: boolean) => void;
  setWorkspacePath: (path: string) => void;
  setKnowledgeAutoEnsureDocsCollection: (enabled: boolean) => void;
  setKbQueryRunsV2: (enabled: boolean) => void;
  setKbPickerServerSearch: (enabled: boolean) => void;
  setKbIngestJobScopedProgress: (enabled: boolean) => void;

  // Sidebar actions
  addPinnedDirectory: (path: string) => void;
  removePinnedDirectory: (path: string) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setAutoPanelHoverEnabled: (enabled: boolean) => void;
  setCloseToBackgroundEnabled: (enabled: boolean) => void;
  setWorktreeAutoCleanupOnSessionDelete: (enabled: boolean) => void;
  setSessionPathSort: (sort: SessionPathSort) => void;
  setShowArchivedSessions: (show: boolean) => void;
  setDeveloperModeEnabled: (enabled: boolean) => void;
  setDeveloperPanels: (patch: Partial<DeveloperPanels>) => void;
  setDeveloperSettingsInitialized: (initialized: boolean) => void;
  setUpdateChannel: (channel: UpdateChannel) => void;
  setAutoCheckForUpdates: (enabled: boolean) => void;
  setIgnoredUpdateVersion: (channel: UpdateChannel, version: string | null) => void;
  setLastUpdateCheckAt: (timestamp: string | null) => void;
}

const defaultSettings = {
  // Backend
  backend: 'claude-code' as Backend,
  provider: 'anthropic',
  model: '',
  modelByProvider: { anthropic: '' } as Record<string, string>,
  apiKey: '',

  // Agents
  agents: [
    { name: 'claude-code', enabled: true, command: 'claude', isDefault: true },
    { name: 'aider', enabled: false, command: 'aider', isDefault: false },
    { name: 'codex', enabled: false, command: 'codex', isDefault: false },
  ] as Agent[],
  agentSelection: 'prefer_default' as const,
  defaultAgent: 'claude-code',

  // Quality
  quality: DEFAULT_QUALITY_SETTINGS,

  // Execution
  maxParallelStories: 3,
  maxTotalTokens: 1_000_000,
  timeoutSeconds: 300,
  maxConcurrentSubagents: 0,

  // UI
  defaultMode: 'expert' as const,
  theme: 'system' as Theme,
  language: 'en' as Language,
  standaloneContextTurns: -1 as StandaloneContextTurns,

  // Chat UI
  showLineNumbers: true,
  maxFileAttachmentSize: 10 * 1024 * 1024, // 10MB
  enableMarkdownMath: true,
  enableCodeBlockCopy: true,

  // Onboarding
  onboardingCompleted: false,
  tourCompleted: false,
  workspacePath: '',
  knowledgeAutoEnsureDocsCollection: true,
  kbQueryRunsV2: true,
  kbPickerServerSearch: true,
  kbIngestJobScopedProgress: true,

  // Sidebar
  pinnedDirectories: [] as string[],
  sidebarCollapsed: false,
  autoPanelHoverEnabled: false,
  closeToBackgroundEnabled: true,
  worktreeAutoCleanupOnSessionDelete: false,
  sessionPathSort: 'recent' as SessionPathSort,
  showArchivedSessions: false,

  // Developer mode
  developerModeEnabled: false,
  developerPanels: {
    contextInspector: false,
    workflowReliability: false,
    executionLogs: false,
    streamingOutput: true,
  } as DeveloperPanels,
  developerSettingsInitialized: false,

  // Updates
  updatePreferences: {
    updateChannel: 'stable' as UpdateChannel,
    autoCheckForUpdates: true,
    ignoredUpdateVersionByChannel: {
      stable: null,
      beta: null,
      alpha: null,
    },
    lastUpdateCheckAt: null,
  } as UpdatePreferences,

  // Context compaction
  enableContextCompaction: true,
  showReasoningOutput: true,
  enableThinking: true,
  showSubAgentEvents: true,

  // GLM endpoint
  glmEndpoint: 'standard' as GlmEndpoint,

  // MiniMax endpoint
  minimaxEndpoint: 'international' as MinimaxEndpoint,

  // Qwen endpoint
  qwenEndpoint: 'china' as QwenEndpoint,

  // Search provider
  searchProvider: 'duckduckgo' as const,

  // Debug mode
  debugDefaultEnvironment: 'dev' as DebugEnvironment,
  debugBrowserProfile: 'desktop' as DebugBrowserProfile,
  debugViewportPresets: ['desktop:1440x900', 'tablet:1024x768', 'mobile:390x844'],
  debugProductionDiagnosticsAllowlist: [] as string[],

  // Phase agent configs
  phaseConfigs: {
    // Plan mode phases (LLM-only for now, CLI schema reserved)
    plan_strategy: { defaultAgent: '', fallbackChain: [] },
    plan_clarification: { defaultAgent: '', fallbackChain: [] },
    plan_generation: { defaultAgent: '', fallbackChain: [] },
    plan_execution: { defaultAgent: '', fallbackChain: [] },
    plan_retry: { defaultAgent: '', fallbackChain: [] },
    // Task workflow planning phases
    plan_exploration: { defaultAgent: '', fallbackChain: [] },
    plan_interview: { defaultAgent: '', fallbackChain: [] },
    plan_requirements: { defaultAgent: '', fallbackChain: [] },
    plan_architecture: { defaultAgent: '', fallbackChain: [] },
    plan_prd: { defaultAgent: '', fallbackChain: [] },
    // Task execution phases (CLI agents + LLM)
    planning: { defaultAgent: '', fallbackChain: ['codex'] },
    implementation: { defaultAgent: '', fallbackChain: ['codex', 'aider'] },
    retry: { defaultAgent: '', fallbackChain: ['aider'] },
    refactor: { defaultAgent: '', fallbackChain: ['claude-code'] },
    review: { defaultAgent: '', fallbackChain: ['codex'] },
  } as Record<string, PhaseAgentConfig>,

  // Memory pipeline
  memorySettings: {
    autoExtractEnabled: true,
    reviewMode: 'llm_review' as MemoryReviewMode,
    reviewAgentRef: '',
    injectActiveOnly: true as const,
    extractSuccessfulSessionsOnly: true as const,
  } as MemorySettings,
};

function applyV2ForcedDefaults(state: Partial<SettingsState>): Partial<SettingsState> {
  const nextState: Partial<SettingsState> = { ...state };
  nextState.defaultMode = 'expert';
  nextState.knowledgeAutoEnsureDocsCollection = true;
  nextState.standaloneContextTurns = -1;
  nextState.enableThinking = true;
  nextState.showReasoningOutput = true;
  nextState.showSubAgentEvents = true;
  nextState.enableContextCompaction = true;

  const currentPhaseConfigs = (nextState.phaseConfigs ?? {}) as Record<string, PhaseAgentConfig>;
  const phaseConfigs: Record<string, PhaseAgentConfig> = { ...currentPhaseConfigs };

  for (const phaseId of EXECUTION_PHASE_IDS) {
    const current = currentPhaseConfigs[phaseId] ?? defaultSettings.phaseConfigs[phaseId];
    phaseConfigs[phaseId] = {
      defaultAgent: '',
      fallbackChain: [...(current?.fallbackChain ?? defaultSettings.phaseConfigs[phaseId].fallbackChain)],
    };
  }
  nextState.phaseConfigs = phaseConfigs;
  return nextState;
}

function resetPlanModePhaseConfigs(phaseConfigs: Record<string, PhaseAgentConfig>): Record<string, PhaseAgentConfig> {
  const next = { ...phaseConfigs };
  for (const phaseId of PLAN_MODE_PHASE_IDS) {
    next[phaseId] = {
      defaultAgent: defaultSettings.phaseConfigs[phaseId].defaultAgent,
      fallbackChain: [...defaultSettings.phaseConfigs[phaseId].fallbackChain],
    };
  }
  return next;
}

function ensurePhaseConfigs(
  phaseConfigs: Partial<Record<string, PhaseAgentConfig>> | undefined,
): Record<string, PhaseAgentConfig> {
  const current = (phaseConfigs ?? {}) as Record<string, PhaseAgentConfig>;
  const next = { ...current } as Record<string, PhaseAgentConfig>;
  for (const phaseId of [...PLAN_MODE_PHASE_IDS, ...TASK_PLANNING_PHASE_IDS, ...EXECUTION_PHASE_IDS]) {
    if (!(phaseId in next)) {
      next[phaseId] = {
        defaultAgent: defaultSettings.phaseConfigs[phaseId].defaultAgent,
        fallbackChain: [...defaultSettings.phaseConfigs[phaseId].fallbackChain],
      };
      continue;
    }
    next[phaseId] = {
      defaultAgent: typeof next[phaseId]?.defaultAgent === 'string' ? next[phaseId].defaultAgent : '',
      fallbackChain: Array.isArray(next[phaseId]?.fallbackChain) ? [...next[phaseId].fallbackChain] : [],
    };
  }
  return next;
}

function applyV3ForcedDefaults(state: Partial<SettingsState>): Partial<SettingsState> {
  const nextState = applyV2ForcedDefaults(state);
  delete (nextState as Record<string, unknown>).simpleKernelSot;
  delete (nextState as Record<string, unknown>).typedCardPipeline;
  return nextState;
}

function ensureMemorySettings(settings: Partial<SettingsState>): MemorySettings {
  const current = (settings.memorySettings ?? {}) as Partial<MemorySettings>;
  return {
    autoExtractEnabled:
      typeof current.autoExtractEnabled === 'boolean'
        ? current.autoExtractEnabled
        : defaultSettings.memorySettings.autoExtractEnabled,
    reviewMode:
      current.reviewMode === 'auto_approve' ||
      current.reviewMode === 'manual_only' ||
      current.reviewMode === 'llm_review'
        ? current.reviewMode
        : defaultSettings.memorySettings.reviewMode,
    reviewAgentRef:
      typeof current.reviewAgentRef === 'string'
        ? current.reviewAgentRef
        : defaultSettings.memorySettings.reviewAgentRef,
    injectActiveOnly: true,
    extractSuccessfulSessionsOnly: true,
  };
}

function ensureDeveloperPanels(settings: Partial<SettingsState>): DeveloperPanels {
  const current = (settings.developerPanels ?? {}) as Partial<DeveloperPanels>;
  return {
    contextInspector:
      typeof current.contextInspector === 'boolean'
        ? current.contextInspector
        : defaultSettings.developerPanels.contextInspector,
    workflowReliability:
      typeof current.workflowReliability === 'boolean'
        ? current.workflowReliability
        : defaultSettings.developerPanels.workflowReliability,
    executionLogs:
      typeof current.executionLogs === 'boolean'
        ? current.executionLogs
        : defaultSettings.developerPanels.executionLogs,
    streamingOutput:
      typeof current.streamingOutput === 'boolean'
        ? current.streamingOutput
        : defaultSettings.developerPanels.streamingOutput,
  };
}

function ensureUpdatePreferences(settings: Partial<SettingsState>): UpdatePreferences {
  const current = (settings.updatePreferences ?? {}) as Partial<UpdatePreferences>;
  const ignored = (current.ignoredUpdateVersionByChannel ?? {}) as Partial<Record<UpdateChannel, string | null>>;
  return {
    updateChannel:
      current.updateChannel === 'stable' || current.updateChannel === 'beta' || current.updateChannel === 'alpha'
        ? current.updateChannel
        : defaultSettings.updatePreferences.updateChannel,
    autoCheckForUpdates:
      typeof current.autoCheckForUpdates === 'boolean'
        ? current.autoCheckForUpdates
        : defaultSettings.updatePreferences.autoCheckForUpdates,
    ignoredUpdateVersionByChannel: {
      stable: typeof ignored.stable === 'string' || ignored.stable === null ? (ignored.stable ?? null) : null,
      beta: typeof ignored.beta === 'string' || ignored.beta === null ? (ignored.beta ?? null) : null,
      alpha: typeof ignored.alpha === 'string' || ignored.alpha === null ? (ignored.alpha ?? null) : null,
    },
    lastUpdateCheckAt:
      typeof current.lastUpdateCheckAt === 'string' || current.lastUpdateCheckAt === null
        ? (current.lastUpdateCheckAt ?? null)
        : null,
  };
}

function ensureDebugViewportPresets(value: Partial<SettingsState>['debugViewportPresets']): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string' && entry.trim().length > 0)
    : [...defaultSettings.debugViewportPresets];
}

function ensureDebugProductionDiagnosticsAllowlist(
  value: Partial<SettingsState>['debugProductionDiagnosticsAllowlist'],
): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string' && entry.trim().length > 0)
    : [...defaultSettings.debugProductionDiagnosticsAllowlist];
}

function ensureQualitySettings(state: Partial<SettingsState> & Record<string, unknown>): QualitySettings {
  const next: QualitySettings = {
    ...DEFAULT_QUALITY_SETTINGS,
    defaultBehaviorByMode: { ...DEFAULT_QUALITY_SETTINGS.defaultBehaviorByMode },
    retryPolicyByMode: {
      chat: { ...DEFAULT_QUALITY_SETTINGS.retryPolicyByMode.chat },
      plan: { ...DEFAULT_QUALITY_SETTINGS.retryPolicyByMode.plan },
      task: { ...DEFAULT_QUALITY_SETTINGS.retryPolicyByMode.task },
      debug: { ...DEFAULT_QUALITY_SETTINGS.retryPolicyByMode.debug },
    },
    profileOverridesByMode: {
      chat: { ...DEFAULT_QUALITY_SETTINGS.profileOverridesByMode.chat },
      plan: { ...DEFAULT_QUALITY_SETTINGS.profileOverridesByMode.plan },
      task: { ...DEFAULT_QUALITY_SETTINGS.profileOverridesByMode.task },
      debug: { ...DEFAULT_QUALITY_SETTINGS.profileOverridesByMode.debug },
    },
    customGates: [...DEFAULT_QUALITY_SETTINGS.customGates],
    pluginPolicy: { ...DEFAULT_QUALITY_SETTINGS.pluginPolicy },
  };

  const quality = state.quality;
  if (quality && typeof quality === 'object') {
    const candidate = quality as Partial<QualitySettings>;
    const candidateProfiles =
      (candidate.profileOverridesByMode as Partial<QualitySettings['profileOverridesByMode']> | undefined) ?? {};
    return {
      ...next,
      ...candidate,
      defaultBehaviorByMode: {
        ...next.defaultBehaviorByMode,
        ...(candidate.defaultBehaviorByMode ?? {}),
      },
      retryPolicyByMode: {
        ...next.retryPolicyByMode,
        ...(candidate.retryPolicyByMode ?? {}),
      },
      profileOverridesByMode: {
        chat: {
          ...next.profileOverridesByMode.chat,
          ...(candidateProfiles.chat ?? {}),
          defaultGateIds: sanitizeQualityGateIds('chat', candidateProfiles.chat?.defaultGateIds),
        },
        plan: {
          ...next.profileOverridesByMode.plan,
          ...(candidateProfiles.plan ?? {}),
          defaultGateIds: sanitizeQualityGateIds('plan', candidateProfiles.plan?.defaultGateIds),
        },
        task: {
          ...next.profileOverridesByMode.task,
          ...(candidateProfiles.task ?? {}),
          defaultGateIds: sanitizeQualityGateIds('task', candidateProfiles.task?.defaultGateIds),
        },
        debug: {
          ...next.profileOverridesByMode.debug,
          ...(candidateProfiles.debug ?? {}),
          defaultGateIds: sanitizeQualityGateIds('debug', candidateProfiles.debug?.defaultGateIds),
        },
      },
      customGates:
        candidate.customGates !== undefined ? sanitizeQualityCustomGates(candidate.customGates) : next.customGates,
      pluginPolicy: {
        ...next.pluginPolicy,
        ...(candidate.pluginPolicy ?? {}),
      },
    };
  }

  const legacy = state.qualityGates;
  if (legacy && typeof legacy === 'object') {
    const old = legacy as Record<string, unknown>;
    const taskGateIds = ['dor'];
    if (old.typecheck !== false) taskGateIds.push('typecheck');
    if (old.test !== false) taskGateIds.push('test');
    if (old.lint !== false) taskGateIds.push('lint');
    taskGateIds.push('dod');
    const customScript = typeof old.customScript === 'string' ? old.customScript.trim() : '';
    const maxRetries = typeof old.maxRetries === 'number' && Number.isFinite(old.maxRetries) ? old.maxRetries : 2;
    return {
      ...next,
      enabled: taskGateIds.length > 0 || customScript.length > 0,
      retryPolicyByMode: {
        ...next.retryPolicyByMode,
        task: {
          enabled: maxRetries > 0,
          maxAttempts: maxRetries,
        },
      },
      profileOverridesByMode: {
        ...next.profileOverridesByMode,
        task: {
          ...next.profileOverridesByMode.task,
          defaultGateIds: sanitizeQualityGateIds('task', taskGateIds),
        },
      },
      customGates:
        customScript.length > 0
          ? [
              {
                id: 'legacy-custom-gate',
                name: 'Legacy Custom Gate',
                command: customScript,
                modes: ['chat', 'plan', 'task', 'debug'],
                blocking: true,
              },
            ]
          : [],
    };
  }

  return next;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      ...defaultSettings,

      setBackend: (backend) => set({ backend }),

      setProvider: (provider) =>
        set((state) => {
          const canonical = normalizeProviderKey(provider);
          return {
            provider,
            model: state.modelByProvider[canonical] ?? '',
          };
        }),

      setModel: (model) =>
        set((state) => {
          const canonical = normalizeProviderKey(state.provider);
          return {
            model,
            modelByProvider: {
              ...state.modelByProvider,
              [canonical]: model,
            },
          };
        }),

      setModelByProvider: (provider, model) =>
        set((state) => {
          const canonical = normalizeProviderKey(provider);
          const nextModelByProvider = {
            ...state.modelByProvider,
            [canonical]: model,
          };
          const shouldUpdateCurrentModel = normalizeProviderKey(state.provider) === canonical;
          return {
            modelByProvider: nextModelByProvider,
            ...(shouldUpdateCurrentModel ? { model } : {}),
          };
        }),

      setApiKey: (apiKey) => set({ apiKey }),

      setTheme: (theme) => {
        set({ theme });
        // Apply theme to document
        applyTheme(theme);
      },

      setLanguage: (language) => {
        set({ language });
        // Apply language to i18n
        i18n.changeLanguage(language);
        localStorage.setItem('plan-cascade-language', language);
      },

      setDefaultMode: (defaultMode) => set({ defaultMode }),
      setStandaloneContextTurns: (standaloneContextTurns) => set({ standaloneContextTurns }),

      updateAgent: (name, updates) =>
        set((state) => ({
          agents: state.agents.map((a) => (a.name === name ? { ...a, ...updates } : a)),
        })),

      updateQualitySettings: (updates) =>
        set((state) => ({
          quality: {
            ...state.quality,
            ...updates,
            defaultBehaviorByMode: {
              ...state.quality.defaultBehaviorByMode,
              ...(updates.defaultBehaviorByMode ?? {}),
            },
            retryPolicyByMode: {
              ...state.quality.retryPolicyByMode,
              ...(updates.retryPolicyByMode ?? {}),
            },
            profileOverridesByMode: {
              ...state.quality.profileOverridesByMode,
              ...(updates.profileOverridesByMode ?? {}),
              chat: {
                ...state.quality.profileOverridesByMode.chat,
                ...(updates.profileOverridesByMode?.chat ?? {}),
                defaultGateIds: sanitizeQualityGateIds(
                  'chat',
                  updates.profileOverridesByMode?.chat?.defaultGateIds ??
                    state.quality.profileOverridesByMode.chat.defaultGateIds,
                ),
              },
              plan: {
                ...state.quality.profileOverridesByMode.plan,
                ...(updates.profileOverridesByMode?.plan ?? {}),
                defaultGateIds: sanitizeQualityGateIds(
                  'plan',
                  updates.profileOverridesByMode?.plan?.defaultGateIds ??
                    state.quality.profileOverridesByMode.plan.defaultGateIds,
                ),
              },
              task: {
                ...state.quality.profileOverridesByMode.task,
                ...(updates.profileOverridesByMode?.task ?? {}),
                defaultGateIds: sanitizeQualityGateIds(
                  'task',
                  updates.profileOverridesByMode?.task?.defaultGateIds ??
                    state.quality.profileOverridesByMode.task.defaultGateIds,
                ),
              },
              debug: {
                ...state.quality.profileOverridesByMode.debug,
                ...(updates.profileOverridesByMode?.debug ?? {}),
                defaultGateIds: sanitizeQualityGateIds(
                  'debug',
                  updates.profileOverridesByMode?.debug?.defaultGateIds ??
                    state.quality.profileOverridesByMode.debug.defaultGateIds,
                ),
              },
            },
            pluginPolicy: {
              ...state.quality.pluginPolicy,
              ...(updates.pluginPolicy ?? {}),
            },
            customGates:
              updates.customGates != null ? sanitizeQualityCustomGates(updates.customGates) : state.quality.customGates,
          },
        })),

      resetToDefaults: () => set(defaultSettings),

      setEnableContextCompaction: (enableContextCompaction) => set({ enableContextCompaction }),
      setShowReasoningOutput: (showReasoningOutput) => set({ showReasoningOutput }),
      setEnableThinking: (enableThinking) => set({ enableThinking }),
      setShowSubAgentEvents: (showSubAgentEvents) => set({ showSubAgentEvents }),

      setShowLineNumbers: (showLineNumbers) => set({ showLineNumbers }),
      setMaxFileAttachmentSize: (maxFileAttachmentSize) => set({ maxFileAttachmentSize }),
      setEnableMarkdownMath: (enableMarkdownMath) => set({ enableMarkdownMath }),
      setEnableCodeBlockCopy: (enableCodeBlockCopy) => set({ enableCodeBlockCopy }),

      setMaxTotalTokens: (maxTotalTokens) => set({ maxTotalTokens }),
      setMaxConcurrentSubagents: (maxConcurrentSubagents) => set({ maxConcurrentSubagents }),

      setGlmEndpoint: (glmEndpoint) => set({ glmEndpoint }),
      setMinimaxEndpoint: (minimaxEndpoint) => set({ minimaxEndpoint }),
      setQwenEndpoint: (qwenEndpoint) => set({ qwenEndpoint }),
      setSearchProvider: (searchProvider) => set({ searchProvider }),
      setDebugDefaultEnvironment: (debugDefaultEnvironment) => set({ debugDefaultEnvironment }),
      setDebugBrowserProfile: (debugBrowserProfile) => set({ debugBrowserProfile }),
      setDebugViewportPresets: (debugViewportPresets) =>
        set({ debugViewportPresets: ensureDebugViewportPresets(debugViewportPresets) }),
      setDebugProductionDiagnosticsAllowlist: (debugProductionDiagnosticsAllowlist) =>
        set({
          debugProductionDiagnosticsAllowlist: ensureDebugProductionDiagnosticsAllowlist(
            debugProductionDiagnosticsAllowlist,
          ),
        }),

      setPhaseConfigs: (phaseConfigs) => set({ phaseConfigs }),
      updatePhaseConfig: (phaseId, config) =>
        set((state) => ({
          phaseConfigs: {
            ...state.phaseConfigs,
            [phaseId]: { ...state.phaseConfigs[phaseId], ...config },
          },
        })),
      setMemorySettings: (memorySettings) => set({ memorySettings: ensureMemorySettings({ memorySettings }) }),
      updateMemorySettings: (patch) =>
        set((state) => ({
          memorySettings: ensureMemorySettings({
            memorySettings: {
              ...state.memorySettings,
              ...patch,
            },
          }),
        })),

      setOnboardingCompleted: (onboardingCompleted) => set({ onboardingCompleted }),
      setTourCompleted: (tourCompleted) => set({ tourCompleted }),
      setWorkspacePath: (workspacePath) => {
        set({ workspacePath });
        // Auto-add non-empty workspace path to pinned directories
        if (workspacePath) {
          const normalized = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
          set((state) => {
            const alreadyExists = state.pinnedDirectories.some(
              (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() === normalized.toLowerCase(),
            );
            if (alreadyExists) return state;
            return { pinnedDirectories: [...state.pinnedDirectories, normalized] };
          });
        }
        // Sync to backend StandaloneState for tool executor working directory
        import('@tauri-apps/api/core').then(({ invoke }) => {
          invoke('set_working_directory', { path: workspacePath }).catch((error) => {
            console.warn('[settings] Failed to sync working directory', error);
          });
        });
      },
      setKnowledgeAutoEnsureDocsCollection: (enabled: boolean) => set({ knowledgeAutoEnsureDocsCollection: enabled }),
      setKbQueryRunsV2: (enabled: boolean) => set({ kbQueryRunsV2: enabled }),
      setKbPickerServerSearch: (enabled: boolean) => set({ kbPickerServerSearch: enabled }),
      setKbIngestJobScopedProgress: (enabled: boolean) => set({ kbIngestJobScopedProgress: enabled }),

      addPinnedDirectory: (path) =>
        set((state) => {
          const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
          const alreadyExists = state.pinnedDirectories.some(
            (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() === normalized.toLowerCase(),
          );
          if (alreadyExists) return state;
          return { pinnedDirectories: [...state.pinnedDirectories, normalized] };
        }),

      removePinnedDirectory: (path) =>
        set((state) => {
          const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
          return {
            pinnedDirectories: state.pinnedDirectories.filter(
              (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() !== normalized,
            ),
          };
        }),

      setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
      setAutoPanelHoverEnabled: (autoPanelHoverEnabled) => set({ autoPanelHoverEnabled }),
      setCloseToBackgroundEnabled: (closeToBackgroundEnabled) => set({ closeToBackgroundEnabled }),
      setWorktreeAutoCleanupOnSessionDelete: (worktreeAutoCleanupOnSessionDelete) =>
        set({ worktreeAutoCleanupOnSessionDelete }),
      setSessionPathSort: (sessionPathSort) => set({ sessionPathSort }),
      setShowArchivedSessions: (showArchivedSessions) => set({ showArchivedSessions }),
      setDeveloperModeEnabled: (developerModeEnabled) => set({ developerModeEnabled }),
      setDeveloperPanels: (patch) =>
        set((state) => ({
          developerPanels: ensureDeveloperPanels({
            developerPanels: {
              ...state.developerPanels,
              ...patch,
            },
          }),
        })),
      setDeveloperSettingsInitialized: (developerSettingsInitialized) => set({ developerSettingsInitialized }),
      setUpdateChannel: (updateChannel) =>
        set((state) => ({
          updatePreferences: ensureUpdatePreferences({
            updatePreferences: {
              ...state.updatePreferences,
              updateChannel,
            },
          }),
        })),
      setAutoCheckForUpdates: (autoCheckForUpdates) =>
        set((state) => ({
          updatePreferences: ensureUpdatePreferences({
            updatePreferences: {
              ...state.updatePreferences,
              autoCheckForUpdates,
            },
          }),
        })),
      setIgnoredUpdateVersion: (channel, version) =>
        set((state) => ({
          updatePreferences: ensureUpdatePreferences({
            updatePreferences: {
              ...state.updatePreferences,
              ignoredUpdateVersionByChannel: {
                ...state.updatePreferences.ignoredUpdateVersionByChannel,
                [channel]: version,
              },
            },
          }),
        })),
      setLastUpdateCheckAt: (lastUpdateCheckAt) =>
        set((state) => ({
          updatePreferences: ensureUpdatePreferences({
            updatePreferences: {
              ...state.updatePreferences,
              lastUpdateCheckAt,
            },
          }),
        })),
    }),
    {
      name: 'plan-cascade-settings',
      version: SETTINGS_PERSIST_VERSION,
      migrate: (persistedState, version) => {
        const state = (persistedState ?? {}) as Partial<SettingsState>;
        if (version < 2) {
          const migrated = applyV3ForcedDefaults(state);
          return {
            ...migrated,
            phaseConfigs: resetPlanModePhaseConfigs(ensurePhaseConfigs(migrated.phaseConfigs)),
            memorySettings: ensureMemorySettings(state),
          };
        }
        if (version < 3) {
          const migrated = applyV3ForcedDefaults(state);
          return {
            ...migrated,
            phaseConfigs: resetPlanModePhaseConfigs(ensurePhaseConfigs(migrated.phaseConfigs)),
            memorySettings: ensureMemorySettings(state),
          };
        }
        const nextState = { ...state } as Record<string, unknown>;
        delete nextState.simpleKernelSot;
        delete nextState.typedCardPipeline;
        const migrated = nextState as Partial<SettingsState>;
        let phaseConfigs = ensurePhaseConfigs(migrated.phaseConfigs);
        if (version < 8) {
          phaseConfigs = resetPlanModePhaseConfigs(phaseConfigs);
        }
        return {
          ...migrated,
          quality: ensureQualitySettings(nextState as Partial<SettingsState> & Record<string, unknown>),
          phaseConfigs,
          memorySettings: ensureMemorySettings(state),
          developerPanels: ensureDeveloperPanels(state),
          updatePreferences: ensureUpdatePreferences(state),
          debugDefaultEnvironment:
            state.debugDefaultEnvironment === 'staging' || state.debugDefaultEnvironment === 'prod'
              ? state.debugDefaultEnvironment
              : defaultSettings.debugDefaultEnvironment,
          debugBrowserProfile:
            state.debugBrowserProfile === 'mobile' ? state.debugBrowserProfile : defaultSettings.debugBrowserProfile,
          debugViewportPresets: ensureDebugViewportPresets(state.debugViewportPresets),
          debugProductionDiagnosticsAllowlist: ensureDebugProductionDiagnosticsAllowlist(
            state.debugProductionDiagnosticsAllowlist,
          ),
          developerSettingsInitialized:
            typeof state.developerSettingsInitialized === 'boolean' ? state.developerSettingsInitialized : false,
        };
      },
      partialize: (state) => {
        return Object.fromEntries(Object.entries(state).filter(([key]) => key !== 'apiKey')) as Partial<SettingsState>;
      },
      merge: (persisted, current) => {
        const merged = { ...current, ...(persisted as object) };
        delete (merged as Record<string, unknown>).simpleKernelSot;
        delete (merged as Record<string, unknown>).typedCardPipeline;
        const mergedState = merged as SettingsState;
        // API keys are not persisted in frontend state.
        mergedState.apiKey = '';
        const modelByProvider = { ...(mergedState.modelByProvider || {}) };
        const canonicalProvider = normalizeProviderKey(mergedState.provider);
        if (!(canonicalProvider in modelByProvider)) {
          modelByProvider[canonicalProvider] = mergedState.model || '';
        }
        mergedState.modelByProvider = modelByProvider;
        if (!mergedState.model && typeof modelByProvider[canonicalProvider] === 'string') {
          mergedState.model = modelByProvider[canonicalProvider];
        }
        mergedState.phaseConfigs = ensurePhaseConfigs(mergedState.phaseConfigs);
        mergedState.quality = ensureQualitySettings(mergedState as SettingsState & Record<string, unknown>);
        mergedState.memorySettings = ensureMemorySettings(mergedState);
        mergedState.developerPanels = ensureDeveloperPanels(mergedState);
        mergedState.updatePreferences = ensureUpdatePreferences(mergedState);
        mergedState.debugDefaultEnvironment =
          mergedState.debugDefaultEnvironment === 'staging' || mergedState.debugDefaultEnvironment === 'prod'
            ? mergedState.debugDefaultEnvironment
            : defaultSettings.debugDefaultEnvironment;
        mergedState.debugBrowserProfile =
          mergedState.debugBrowserProfile === 'mobile'
            ? mergedState.debugBrowserProfile
            : defaultSettings.debugBrowserProfile;
        mergedState.debugViewportPresets = ensureDebugViewportPresets(mergedState.debugViewportPresets);
        mergedState.debugProductionDiagnosticsAllowlist = ensureDebugProductionDiagnosticsAllowlist(
          mergedState.debugProductionDiagnosticsAllowlist,
        );
        mergedState.developerSettingsInitialized =
          typeof mergedState.developerSettingsInitialized === 'boolean'
            ? mergedState.developerSettingsInitialized
            : false;
        return mergedState;
      },
      onRehydrateStorage: () => (state) => {
        // Drop legacy frontend API-key caches; keys live in backend keyring.
        localStorage.removeItem('plan-cascade-api-keys');
        localStorage.removeItem('plan-cascade-provider-api-key-cache');
        // Apply theme on rehydration
        if (state?.theme) {
          applyTheme(state.theme);
        }
        // Sync language: i18n LanguageDetector already picked the correct
        // language (from localStorage on return visits, or navigator.language
        // on first launch). Sync that result back to the store so the UI
        // language selector stays in sync.
        const detected = i18n.resolvedLanguage || i18n.language || 'en';
        const supportedLangs: Language[] = ['en', 'zh', 'ja'];
        const lang: Language = supportedLangs.includes(detected as Language) ? (detected as Language) : 'en';
        if (state && state.language !== lang) {
          useSettingsStore.setState({ language: lang });
        }
      },
    },
  ),
);

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

  if (theme === 'dark' || (theme === 'system' && systemDark)) {
    root.classList.add('dark');
    root.classList.remove('light');
  } else {
    root.classList.add('light');
    root.classList.remove('dark');
  }

  // Store theme preference for initial load script
  localStorage.setItem('plan-cascade-theme', theme === 'system' ? '' : theme);
}

export default useSettingsStore;
