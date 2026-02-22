/**
 * Settings Store
 *
 * Manages application settings with persistence.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import i18n from '../i18n';

export type Backend = 'claude-code' | 'claude-api' | 'openai' | 'deepseek' | 'glm' | 'qwen' | 'minimax' | 'ollama';
export type Theme = 'system' | 'light' | 'dark';
export type Language = 'en' | 'zh' | 'ja';
export type StandaloneContextTurns = 2 | 4 | 6 | 8 | 10 | 20 | 50 | 100 | 200 | 500 | -1;
export type GlmEndpoint = 'standard' | 'coding' | 'international' | 'international-coding';
export type MinimaxEndpoint = 'international' | 'china';
export type QwenEndpoint = 'china' | 'singapore' | 'us';

interface Agent {
  name: string;
  enabled: boolean;
  command: string;
  isDefault: boolean;
}

interface QualityGates {
  typecheck: boolean;
  test: boolean;
  lint: boolean;
  custom: boolean;
  customScript: string;
  maxRetries: number;
}

interface SettingsState {
  // Backend settings
  backend: Backend;
  provider: string;
  model: string;
  apiKey: string;

  // Agent settings
  agents: Agent[];
  agentSelection: 'smart' | 'prefer_default' | 'manual';
  defaultAgent: string;

  // Quality gates
  qualityGates: QualityGates;

  // Execution settings
  maxParallelStories: number;
  maxIterations: number;
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

  // Sidebar settings
  pinnedDirectories: string[];
  sidebarCollapsed: boolean;

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

  // Actions
  setBackend: (backend: Backend) => void;
  setProvider: (provider: string) => void;
  setModel: (model: string) => void;
  setApiKey: (apiKey: string) => void;
  setTheme: (theme: Theme) => void;
  setLanguage: (language: Language) => void;
  setDefaultMode: (mode: 'simple' | 'expert') => void;
  setStandaloneContextTurns: (turns: StandaloneContextTurns) => void;
  updateAgent: (name: string, updates: Partial<Agent>) => void;
  updateQualityGates: (updates: Partial<QualityGates>) => void;
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

  // Chat UI actions
  setShowLineNumbers: (show: boolean) => void;
  setMaxFileAttachmentSize: (size: number) => void;
  setEnableMarkdownMath: (enable: boolean) => void;
  setEnableCodeBlockCopy: (enable: boolean) => void;

  // Onboarding actions
  setOnboardingCompleted: (completed: boolean) => void;
  setTourCompleted: (completed: boolean) => void;
  setWorkspacePath: (path: string) => void;

  // Sidebar actions
  addPinnedDirectory: (path: string) => void;
  removePinnedDirectory: (path: string) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
}

const defaultSettings = {
  // Backend
  backend: 'claude-code' as Backend,
  provider: 'anthropic',
  model: '',
  apiKey: '',

  // Agents
  agents: [
    { name: 'claude-code', enabled: true, command: 'claude', isDefault: true },
    { name: 'aider', enabled: false, command: 'aider', isDefault: false },
    { name: 'codex', enabled: false, command: 'codex', isDefault: false },
  ] as Agent[],
  agentSelection: 'prefer_default' as const,
  defaultAgent: 'claude-code',

  // Quality gates
  qualityGates: {
    typecheck: true,
    test: true,
    lint: true,
    custom: false,
    customScript: '',
    maxRetries: 3,
  },

  // Execution
  maxParallelStories: 3,
  maxIterations: 50,
  maxTotalTokens: 1_000_000,
  timeoutSeconds: 300,
  maxConcurrentSubagents: 0,

  // UI
  defaultMode: 'simple' as const,
  theme: 'system' as Theme,
  language: 'en' as Language,
  standaloneContextTurns: 8 as StandaloneContextTurns,

  // Chat UI
  showLineNumbers: true,
  maxFileAttachmentSize: 10 * 1024 * 1024, // 10MB
  enableMarkdownMath: true,
  enableCodeBlockCopy: true,

  // Onboarding
  onboardingCompleted: false,
  tourCompleted: false,
  workspacePath: '',

  // Sidebar
  pinnedDirectories: [] as string[],
  sidebarCollapsed: false,

  // Context compaction
  enableContextCompaction: true,
  showReasoningOutput: false,
  enableThinking: false,
  showSubAgentEvents: true,

  // GLM endpoint
  glmEndpoint: 'standard' as GlmEndpoint,

  // MiniMax endpoint
  minimaxEndpoint: 'international' as MinimaxEndpoint,

  // Qwen endpoint
  qwenEndpoint: 'china' as QwenEndpoint,

  // Search provider
  searchProvider: 'duckduckgo' as const,
};

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      ...defaultSettings,

      setBackend: (backend) => set({ backend }),

      setProvider: (provider) => set({ provider }),

      setModel: (model) => set({ model }),

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
          agents: state.agents.map((a) =>
            a.name === name ? { ...a, ...updates } : a
          ),
        })),

      updateQualityGates: (updates) =>
        set((state) => ({
          qualityGates: { ...state.qualityGates, ...updates },
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

      setOnboardingCompleted: (onboardingCompleted) => set({ onboardingCompleted }),
      setTourCompleted: (tourCompleted) => set({ tourCompleted }),
      setWorkspacePath: (workspacePath) => {
        set({ workspacePath });
        // Auto-add non-empty workspace path to pinned directories
        if (workspacePath) {
          const normalized = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
          set((state) => {
            const alreadyExists = state.pinnedDirectories.some(
              (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() === normalized.toLowerCase()
            );
            if (alreadyExists) return state;
            return { pinnedDirectories: [...state.pinnedDirectories, normalized] };
          });
        }
        // Sync to backend StandaloneState for tool executor working directory
        import('@tauri-apps/api/core').then(({ invoke }) => {
          invoke('set_working_directory', { path: workspacePath }).catch(() => {});
        });
      },

      addPinnedDirectory: (path) =>
        set((state) => {
          const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
          const alreadyExists = state.pinnedDirectories.some(
            (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() === normalized.toLowerCase()
          );
          if (alreadyExists) return state;
          return { pinnedDirectories: [...state.pinnedDirectories, normalized] };
        }),

      removePinnedDirectory: (path) =>
        set((state) => {
          const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
          return {
            pinnedDirectories: state.pinnedDirectories.filter(
              (p) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase() !== normalized
            ),
          };
        }),

      setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
    }),
    {
      name: 'plan-cascade-settings',
      onRehydrateStorage: () => (state) => {
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
        const lang: Language = supportedLangs.includes(detected as Language)
          ? (detected as Language)
          : 'en';
        if (state && state.language !== lang) {
          useSettingsStore.setState({ language: lang });
        }
      },
    }
  )
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
