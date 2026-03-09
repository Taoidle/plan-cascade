/**
 * Settings Component Tests
 *
 * Tests SettingsDialog rendering, GeneralSection fields,
 * LLMBackendSection configuration, and SetupWizard existence.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within, waitFor } from '@testing-library/react';
import { GeneralSection } from '../Settings/GeneralSection';
import { LLMBackendSection } from '../Settings/LLMBackendSection';
import { SettingsDialog } from '../Settings/SettingsDialog';
import { useSettingsStore } from '../../store/settings';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        title: 'Settings',
        'tabs.general': 'General',
        'tabs.llm': 'LLM Backend',
        'tabs.agents': 'Agents',
        'tabs.quality': 'Quality Gates',
        'tabs.phases': 'Phase Agents',
        'tabs.importExport': 'Import/Export',
        'general.title': 'General Settings',
        'general.description': 'Configure your general preferences',
        'general.workingMode.title': 'Working Mode',
        'general.workingMode.simple.name': 'Simple Mode',
        'general.workingMode.simple.description': 'One-click execution',
        'general.workingMode.expert.name': 'Expert Mode',
        'general.workingMode.expert.description': 'Full control over execution',
        'general.theme.title': 'Theme',
        'general.theme.system': 'System',
        'general.theme.light': 'Light',
        'general.theme.dark': 'Dark',
        'general.theme.description': 'Choose your preferred color theme',
        'general.knowledgeBase.title': 'Knowledge Base',
        'general.knowledgeBase.autoEnsureDocs': 'Auto-create docs collection for workspace',
        'general.knowledgeBase.autoEnsureDocsDescription': 'Auto create docs collection when entering workspace',
        'general.knowledgeBase.kbQueryRunsV2': 'Use precise query-run scope filtering (v2)',
        'general.knowledgeBase.kbQueryRunsV2Description': 'Use structured filtering',
        'general.knowledgeBase.kbPickerServerSearch': 'Use server-side document search in picker',
        'general.knowledgeBase.kbPickerServerSearchDescription': 'Search unexpanded collections',
        'general.knowledgeBase.kbIngestJobScopedProgress': 'Use job-scoped ingest progress events',
        'general.knowledgeBase.kbIngestJobScopedProgressDescription': 'Isolate upload progress by job',
        'general.developerMode.title': 'Developer Mode',
        'general.developerMode.enable': 'Enable Developer Mode',
        'general.developerMode.description': 'Control developer-facing panels in Simple mode.',
        'general.developerMode.panels.contextInspector.title': 'Show Context Inspector tab',
        'general.developerMode.panels.contextInspector.description': 'Show the Context tab in the right panel.',
        'general.developerMode.panels.workflowReliability.title': 'Show Workflow Reliability',
        'general.developerMode.panels.workflowReliability.description': 'Show workflow observability metrics.',
        'general.developerMode.panels.executionLogs.title': 'Show execution logs',
        'general.developerMode.panels.executionLogs.description': 'Show the execution logs card.',
        'general.developerMode.panels.streamingOutput.title': 'Show output stream',
        'general.developerMode.panels.streamingOutput.description': 'Show streaming output in the right panel.',
        'general.executionLimits.title': 'Execution Limits',
        'general.executionLimits.maxParallelStories': 'Max Parallel Stories',
        'general.executionLimits.maxIterations': 'Max Iterations',
        'general.executionLimits.maxTotalTokens': 'Max Token Budget',
        'general.executionLimits.timeout': 'Timeout (seconds)',
        'general.executionLimits.maxConcurrentSubagents': 'Max Concurrent Subagents',
        'general.executionLimits.maxConcurrentSubagentsHelp':
          'Maximum number of subagents that can run at the same time.',
        'buttons.cancel': 'Cancel',
        'buttons.save': 'Save',
        'buttons.saving': 'Saving...',
      };
      return translations[key] || key;
    },
  }),
}));

const mockProviderKeys: Record<string, string> = {};
const { mockUpdateSettings, mockGetKnowledgeFeatureFlags, mockSetKnowledgeFeatureFlags, mockIsTauriAvailable } =
  vi.hoisted(() => ({
    mockUpdateSettings: vi.fn().mockResolvedValue(undefined),
    mockGetKnowledgeFeatureFlags: vi.fn().mockResolvedValue({
      kbQueryRunsV2: true,
      kbPickerServerSearch: true,
      kbIngestJobScopedProgress: true,
    }),
    mockSetKnowledgeFeatureFlags: vi.fn().mockResolvedValue(undefined),
    mockIsTauriAvailable: vi.fn(() => false),
  }));

const mockInvoke = vi.fn(async (command: string, args?: { provider?: string; apiKey?: string; api_key?: string }) => {
  switch (command) {
    case 'get_context_policy':
      return {
        success: true,
        data: {
          context_v2_pipeline: true,
          memory_v2_ranker: true,
          context_inspector_ui: false,
          pinned_sources: [],
          excluded_sources: [],
          soft_threshold_ratio: 0.85,
          hard_threshold_ratio: 0.95,
        },
        error: null,
      };
    case 'set_context_policy':
      return { success: true, data: { key: 'context_policy_v2', updated_at: '2026-03-02T00:00:00Z' }, error: null };
    case 'list_configured_api_key_providers':
      return { success: true, data: Object.keys(mockProviderKeys), error: null };
    case 'list_providers':
      return {
        success: true,
        data: [
          { provider_type: 'anthropic', models: [{ id: 'claude-3-5-sonnet-20241022' }] },
          { provider_type: 'glm', models: [{ id: 'glm-4.7' }, { id: 'glm-4.6' }] },
        ],
        error: null,
      };
    case 'configure_provider':
      {
        const provider = args?.provider || '';
        const key = args?.apiKey ?? args?.api_key ?? '';
        if (provider) {
          if (typeof key === 'string' && key.trim().length > 0) {
            mockProviderKeys[provider] = key.trim();
          } else {
            delete mockProviderKeys[provider];
          }
        }
      }
      return { success: true, data: true, error: null };
    case 'get_provider_api_key':
      return { success: true, data: args?.provider ? mockProviderKeys[args.provider] || null : null, error: null };
    default:
      return { success: true, data: null, error: null };
  }
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...(args as [string])),
}));

// Mock settings store
const mockSetDefaultMode = vi.fn();
const mockSetTheme = vi.fn();
const mockSetBackend = vi.fn();
const mockSetModel = vi.fn();
const mockSetModelByProvider = vi.fn();
const mockSetProvider = vi.fn();
const mockSetStandaloneContextTurns = vi.fn();
const mockSetEnableContextCompaction = vi.fn();
const mockSetShowReasoningOutput = vi.fn();
const mockSetShowSubAgentEvents = vi.fn();
const mockSetSearchProvider = vi.fn();
const mockSetKnowledgeAutoEnsureDocsCollection = vi.fn();
const mockSetKbQueryRunsV2 = vi.fn();
const mockSetKbPickerServerSearch = vi.fn();
const mockSetKbIngestJobScopedProgress = vi.fn();
const mockSetDeveloperModeEnabled = vi.fn();
const mockSetDeveloperPanels = vi.fn();
const mockSetDeveloperSettingsInitialized = vi.fn();

const mockSettingsState = {
  backend: 'claude-code' as string,
  provider: 'claude',
  model: '',
  modelByProvider: { anthropic: '' },
  apiKey: '',
  defaultMode: 'simple' as string,
  theme: 'system' as string,
  language: 'en' as string,
  standaloneContextTurns: 8 as number,
  enableContextCompaction: true,
  showReasoningOutput: false,
  showSubAgentEvents: true,
  searchProvider: 'duckduckgo' as string,
  knowledgeAutoEnsureDocsCollection: false,
  kbQueryRunsV2: true,
  kbPickerServerSearch: true,
  kbIngestJobScopedProgress: true,
  developerModeEnabled: false,
  developerPanels: {
    contextInspector: false,
    workflowReliability: false,
    executionLogs: false,
    streamingOutput: true,
  },
  developerSettingsInitialized: false,
  agents: [
    { name: 'claude-code', enabled: true, command: 'claude', isDefault: true },
    { name: 'aider', enabled: false, command: 'aider', isDefault: false },
  ],
  maxParallelStories: 3,
  maxIterations: 50,
  maxTotalTokens: 1000000,
  timeoutSeconds: 300,
  onboardingCompleted: true,
  tourCompleted: true,
  workspacePath: '/home/user/projects',
  setDefaultMode: mockSetDefaultMode,
  setTheme: mockSetTheme,
  setBackend: mockSetBackend,
  setModel: mockSetModel,
  setModelByProvider: mockSetModelByProvider,
  setProvider: mockSetProvider,
  setStandaloneContextTurns: mockSetStandaloneContextTurns,
  setEnableContextCompaction: mockSetEnableContextCompaction,
  setShowReasoningOutput: mockSetShowReasoningOutput,
  setShowSubAgentEvents: mockSetShowSubAgentEvents,
  setSearchProvider: mockSetSearchProvider,
  setKnowledgeAutoEnsureDocsCollection: mockSetKnowledgeAutoEnsureDocsCollection,
  setKbQueryRunsV2: mockSetKbQueryRunsV2,
  setKbPickerServerSearch: mockSetKbPickerServerSearch,
  setKbIngestJobScopedProgress: mockSetKbIngestJobScopedProgress,
  setDeveloperModeEnabled: mockSetDeveloperModeEnabled,
  setDeveloperPanels: mockSetDeveloperPanels,
  setDeveloperSettingsInitialized: mockSetDeveloperSettingsInitialized,
};

vi.mock('../../store/settings', () => ({
  useSettingsStore: Object.assign(() => mockSettingsState, {
    getState: () => mockSettingsState,
    setState: vi.fn(),
  }),
}));

// Mock settings API
vi.mock('../../lib/settingsApi', () => ({
  updateSettings: mockUpdateSettings,
  getKnowledgeFeatureFlags: mockGetKnowledgeFeatureFlags,
  setKnowledgeFeatureFlags: mockSetKnowledgeFeatureFlags,
  isTauriAvailable: mockIsTauriAvailable,
}));

// Mock Radix Dialog
vi.mock('@radix-ui/react-dialog', () => ({
  Root: ({ children, open }: { children: React.ReactNode; open: boolean }) =>
    open ? <div data-testid="dialog-root">{children}</div> : null,
  Portal: ({ children }: { children: React.ReactNode }) => <div data-testid="dialog-portal">{children}</div>,
  Overlay: ({ className }: { className: string }) => <div data-testid="dialog-overlay" className={className} />,
  Content: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <div data-testid="dialog-content" className={className}>
      {children}
    </div>
  ),
  Title: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <h2 data-testid="dialog-title" className={className}>
      {children}
    </h2>
  ),
  Close: ({ children, asChild }: { children: React.ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <button>{children}</button>,
}));

// Mock Radix Tabs
vi.mock('@radix-ui/react-tabs', () => ({
  Root: ({
    children,
    value,
    onValueChange: _onValueChange,
  }: {
    children: React.ReactNode;
    value: string;
    onValueChange: (v: string) => void;
  }) => (
    <div data-testid="tabs-root" data-active-tab={value}>
      {children}
    </div>
  ),
  List: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <div data-testid="tabs-list" className={className} role="tablist">
      {children}
    </div>
  ),
  Trigger: ({ children, value }: { children: React.ReactNode; value: string }) => (
    <button data-testid={`tab-${value}`} role="tab">
      {children}
    </button>
  ),
  Content: ({ children, value, className }: { children: React.ReactNode; value: string; className: string }) => (
    <div data-testid={`tab-content-${value}`} role="tabpanel" className={className}>
      {children}
    </div>
  ),
}));

// Mock sub-sections that are not the focus of these tests
vi.mock('../Settings/AgentConfigSection', () => ({
  AgentConfigSection: () => <div data-testid="agent-config-section">Agent Config</div>,
}));

vi.mock('../Settings/QualityGatesSection', () => ({
  QualityGatesSection: () => <div data-testid="quality-gates-section">Quality Gates</div>,
}));

vi.mock('../Settings/PhaseAgentSection', () => ({
  PhaseAgentSection: () => <div data-testid="phase-agent-section">Phase Agents</div>,
}));

vi.mock('../Settings/ImportExportSection', () => ({
  ImportExportSection: () => <div data-testid="import-export-section">Import/Export</div>,
}));

vi.mock('../Settings/LanguageSelector', () => ({
  LanguageSelector: () => <div data-testid="language-selector">Language Selector</div>,
}));

// --------------------------------------------------------------------------
// GeneralSection Tests
// --------------------------------------------------------------------------

describe('GeneralSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockSettingsState.defaultMode = 'simple';
    mockSettingsState.theme = 'system';
    mockSettingsState.developerModeEnabled = false;
    mockSettingsState.developerPanels = {
      contextInspector: false,
      workflowReliability: false,
      executionLogs: false,
      streamingOutput: true,
    };
    mockSettingsState.developerSettingsInitialized = false;
  });

  it('renders the general settings title and description', () => {
    render(<GeneralSection />);

    expect(screen.getByText('General Settings')).toBeInTheDocument();
    expect(screen.getByText('Configure your general preferences')).toBeInTheDocument();
  });

  it('renders working mode radio options (Simple and Expert)', () => {
    render(<GeneralSection />);

    expect(screen.getByText('Simple Mode')).toBeInTheDocument();
    expect(screen.getByText('Expert Mode')).toBeInTheDocument();
    expect(screen.getByText('One-click execution')).toBeInTheDocument();
    expect(screen.getByText('Full control over execution')).toBeInTheDocument();
  });

  it('selects the current working mode', () => {
    render(<GeneralSection />);

    const simpleRadio = screen.getByDisplayValue('simple');
    expect(simpleRadio).toBeChecked();

    const expertRadio = screen.getByDisplayValue('expert');
    expect(expertRadio).not.toBeChecked();
  });

  it('calls setDefaultMode when working mode is changed', () => {
    render(<GeneralSection />);

    const expertRadio = screen.getByDisplayValue('expert');
    fireEvent.click(expertRadio);

    expect(mockSetDefaultMode).toHaveBeenCalledWith('expert');
  });

  it('renders theme selector with system/light/dark options', () => {
    render(<GeneralSection />);

    expect(screen.getByText('Theme')).toBeInTheDocument();
    const select = screen.getByDisplayValue('System');
    expect(select).toBeInTheDocument();

    // Check options exist
    const options = within(select as HTMLSelectElement).getAllByRole('option');
    expect(options).toHaveLength(3);
    expect(options[0]).toHaveTextContent('System');
    expect(options[1]).toHaveTextContent('Light');
    expect(options[2]).toHaveTextContent('Dark');
  });

  it('calls setTheme when theme is changed', () => {
    render(<GeneralSection />);

    const select = screen.getByDisplayValue('System');
    fireEvent.change(select, { target: { value: 'dark' } });

    expect(mockSetTheme).toHaveBeenCalledWith('dark');
  });

  it('renders execution limits fields', () => {
    render(<GeneralSection />);

    expect(screen.getByText('Execution Limits')).toBeInTheDocument();
    expect(screen.getByText('Max Parallel Stories')).toBeInTheDocument();
    expect(screen.getByText('Max Iterations')).toBeInTheDocument();
    expect(screen.getByText('Timeout (seconds)')).toBeInTheDocument();
    expect(screen.getByText('Max Concurrent Subagents')).toBeInTheDocument();
  });

  it('calls setKnowledgeAutoEnsureDocsCollection when auto docs option is toggled', () => {
    render(<GeneralSection />);

    const title = screen.getByText('Auto-create docs collection for workspace');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    expect(mockSetKnowledgeAutoEnsureDocsCollection).toHaveBeenCalledWith(true);
  });

  it('calls setKbQueryRunsV2 when query-runs v2 option is toggled', () => {
    render(<GeneralSection />);

    const title = screen.getByText('Use precise query-run scope filtering (v2)');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    expect(mockSetKbQueryRunsV2).toHaveBeenCalledWith(false);
  });

  it('calls setKbPickerServerSearch when picker server search option is toggled', () => {
    render(<GeneralSection />);

    const title = screen.getByText('Use server-side document search in picker');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    expect(mockSetKbPickerServerSearch).toHaveBeenCalledWith(false);
  });

  it('calls setKbIngestJobScopedProgress when ingest progress scope option is toggled', () => {
    render(<GeneralSection />);

    const title = screen.getByText('Use job-scoped ingest progress events');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    expect(mockSetKbIngestJobScopedProgress).toHaveBeenCalledWith(false);
  });

  it('renders max parallel stories input with default value', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const storiesInput = inputs.find((el) => (el as HTMLInputElement).value === '3');
    expect(storiesInput).toBeDefined();
  });

  it('renders max iterations input with default value', () => {
    render(<GeneralSection />);

    // The maxIterations field should display the default value (50)
    const inputs = screen.getAllByRole('spinbutton');
    const iterationsInput = inputs.find((el) => (el as HTMLInputElement).value === '50');
    expect(iterationsInput).toBeDefined();
  });

  it('updates maxParallelStories via setState when changed', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const storiesInput = inputs.find((el) => (el as HTMLInputElement).value === '3') as HTMLInputElement;

    fireEvent.change(storiesInput, { target: { value: '5' } });

    expect(useSettingsStore.setState).toHaveBeenCalledWith({ maxParallelStories: 5 });
  });

  it('enforces min constraint on maxParallelStories input', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const storiesInput = inputs.find((el) => (el as HTMLInputElement).value === '3') as HTMLInputElement;

    expect(storiesInput).toBeDefined();
    expect(storiesInput.min).toBe('1');
  });

  it('enforces max constraint on maxParallelStories input', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const storiesInput = inputs.find((el) => (el as HTMLInputElement).value === '3') as HTMLInputElement;

    expect(storiesInput).toBeDefined();
    expect(storiesInput.max).toBe('10');
  });

  it('enforces min constraint on maxIterations input', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const iterationsInput = inputs.find((el) => (el as HTMLInputElement).value === '50') as HTMLInputElement;

    expect(iterationsInput).toBeDefined();
    expect(iterationsInput.min).toBe('10');
  });

  it('enforces max constraint on maxIterations input', () => {
    render(<GeneralSection />);

    const inputs = screen.getAllByRole('spinbutton');
    const iterationsInput = inputs.find((el) => (el as HTMLInputElement).value === '50') as HTMLInputElement;

    expect(iterationsInput).toBeDefined();
    expect(iterationsInput.max).toBe('200');
  });

  it('renders language selector', () => {
    render(<GeneralSection />);

    expect(screen.getByTestId('language-selector')).toBeInTheDocument();
  });

  it('updates developer mode through settings store', () => {
    render(<GeneralSection />);

    const title = screen.getByText('Enable Developer Mode');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    expect(mockSetDeveloperModeEnabled).toHaveBeenCalledWith(true);
  });

  it('disables developer panel toggles when developer mode is off', () => {
    render(<GeneralSection />);

    const workflowReliability = screen.getByText('Show Workflow Reliability').closest('label')?.querySelector('input');
    expect(workflowReliability).toBeDisabled();
  });

  it('persists developer context inspector toggle via context policy API', async () => {
    mockSettingsState.developerModeEnabled = true;
    render(<GeneralSection />);

    const title = await screen.findByText('Show Context Inspector tab');
    const toggle = title.closest('label')?.querySelector('input[type="checkbox"]');
    expect(toggle).toBeTruthy();
    fireEvent.click(toggle!);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'set_context_policy',
        expect.objectContaining({
          policy: expect.objectContaining({ context_inspector_ui: true }),
        }),
      ),
    );
  });
});

// --------------------------------------------------------------------------
// LLMBackendSection Tests
// --------------------------------------------------------------------------

describe('LLMBackendSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockInvoke.mockClear();
    Object.keys(mockProviderKeys).forEach((key) => delete mockProviderKeys[key]);
    mockSettingsState.backend = 'claude-code';
    mockSettingsState.model = '';
    mockSettingsState.standaloneContextTurns = 8;
  });

  it('selects the current backend', () => {
    render(<LLMBackendSection />);

    const claudeCodeRadio = screen.getByDisplayValue('claude-code');
    expect(claudeCodeRadio).toBeChecked();
  });

  it('calls setBackend and setProvider when backend is changed', () => {
    render(<LLMBackendSection />);

    const openaiRadio = screen.getByDisplayValue('openai');
    fireEvent.click(openaiRadio);

    expect(mockSetBackend).toHaveBeenCalledWith('openai');
    expect(mockSetProvider).toHaveBeenCalledWith('openai');
  });

  it('updates standalone context turns setting', () => {
    render(<LLMBackendSection />);

    const contextTurnsSelect = screen.getByDisplayValue('8');
    fireEvent.change(contextTurnsSelect, { target: { value: '20' } });

    expect(mockSetStandaloneContextTurns).toHaveBeenCalledWith(20);
  });

  it('does not show API key configuration for non-key backends', () => {
    mockSettingsState.backend = 'claude-code';

    render(<LLMBackendSection />);

    expect(screen.queryByText(/API Key for/)).not.toBeInTheDocument();
  });
});

// --------------------------------------------------------------------------
// SettingsDialog Tests
// --------------------------------------------------------------------------

describe('SettingsDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockSettingsState.backend = 'claude-code';
    mockIsTauriAvailable.mockReturnValue(false);
  });

  it('renders dialog with title when open', () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByTestId('dialog-title')).toHaveTextContent('Settings');
  });

  it('does not render when closed', () => {
    render(<SettingsDialog open={false} onOpenChange={vi.fn()} />);

    expect(screen.queryByTestId('dialog-root')).not.toBeInTheDocument();
  });

  it('renders all settings tabs', () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByTestId('tab-general')).toHaveTextContent('General');
    expect(screen.getByTestId('tab-llm')).toHaveTextContent('LLM Backend');
    expect(screen.getByTestId('tab-agents')).toHaveTextContent('Agents');
    expect(screen.getByTestId('tab-quality')).toHaveTextContent('Quality Gates');
    expect(screen.getByTestId('tab-phases')).toHaveTextContent('Phase Agents');
    expect(screen.getByTestId('tab-import-export')).toHaveTextContent('Import/Export');
  });

  it('renders Save and Cancel buttons', () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByText('Save')).toBeInTheDocument();
    expect(screen.getByText('Cancel')).toBeInTheDocument();
  });

  it('renders tab content sections', () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    // The general section should be rendered
    expect(screen.getByText('General Settings')).toBeInTheDocument();
    // Other sections should be in the DOM (all tab contents are rendered, just may not be visible)
    expect(screen.getByTestId('agent-config-section')).toBeInTheDocument();
    expect(screen.getByTestId('quality-gates-section')).toBeInTheDocument();
  });

  it('persists knowledge feature flags to backend on save when Tauri is available', async () => {
    mockIsTauriAvailable.mockReturnValue(true);
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    fireEvent.click(screen.getByTestId('settings-save-button'));

    await waitFor(() =>
      expect(mockSetKnowledgeFeatureFlags).toHaveBeenCalledWith({
        kbQueryRunsV2: true,
        kbPickerServerSearch: true,
        kbIngestJobScopedProgress: true,
      }),
    );
  });
});
