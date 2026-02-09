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

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'title': 'Settings',
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
        'general.executionLimits.title': 'Execution Limits',
        'general.executionLimits.maxParallelStories': 'Max Parallel Stories',
        'general.executionLimits.maxIterations': 'Max Iterations',
        'general.executionLimits.timeout': 'Timeout (seconds)',
        'buttons.cancel': 'Cancel',
        'buttons.save': 'Save',
        'buttons.saving': 'Saving...',
      };
      return translations[key] || key;
    },
  }),
}));

const mockProviderKeys: Record<string, string> = {};

const mockInvoke = vi.fn(async (command: string, args?: { provider?: string; apiKey?: string; api_key?: string }) => {
  switch (command) {
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
      return { success: true, data: args?.provider ? (mockProviderKeys[args.provider] || null) : null, error: null };
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
const mockSetProvider = vi.fn();
const mockSetStandaloneContextTurns = vi.fn();
const mockSetEnableContextCompaction = vi.fn();
const mockSetShowReasoningOutput = vi.fn();
const mockSetShowSubAgentEvents = vi.fn();
const mockSetSearchProvider = vi.fn();

const mockSettingsState = {
  backend: 'claude-code' as string,
  provider: 'claude',
  model: '',
  apiKey: '',
  defaultMode: 'simple' as string,
  theme: 'system' as string,
  language: 'en' as string,
  standaloneContextTurns: 8 as number,
  enableContextCompaction: true,
  showReasoningOutput: false,
  showSubAgentEvents: true,
  searchProvider: 'duckduckgo' as string,
  agents: [
    { name: 'claude-code', enabled: true, command: 'claude', isDefault: true },
    { name: 'aider', enabled: false, command: 'aider', isDefault: false },
  ],
  maxParallelStories: 3,
  maxIterations: 50,
  timeoutSeconds: 300,
  onboardingCompleted: true,
  tourCompleted: true,
  workspacePath: '/home/user/projects',
  setDefaultMode: mockSetDefaultMode,
  setTheme: mockSetTheme,
  setBackend: mockSetBackend,
  setModel: mockSetModel,
  setProvider: mockSetProvider,
  setStandaloneContextTurns: mockSetStandaloneContextTurns,
  setEnableContextCompaction: mockSetEnableContextCompaction,
  setShowReasoningOutput: mockSetShowReasoningOutput,
  setShowSubAgentEvents: mockSetShowSubAgentEvents,
  setSearchProvider: mockSetSearchProvider,
};

vi.mock('../../store/settings', () => ({
  useSettingsStore: Object.assign(
    () => mockSettingsState,
    {
      getState: () => mockSettingsState,
      setState: vi.fn(),
    }
  ),
}));

// Mock settings API
vi.mock('../../lib/settingsApi', () => ({
  updateSettings: vi.fn().mockResolvedValue(undefined),
  isTauriAvailable: () => false,
}));

// Mock Radix Dialog
vi.mock('@radix-ui/react-dialog', () => ({
  Root: ({ children, open }: { children: React.ReactNode; open: boolean }) =>
    open ? <div data-testid="dialog-root">{children}</div> : null,
  Portal: ({ children }: { children: React.ReactNode }) => <div data-testid="dialog-portal">{children}</div>,
  Overlay: ({ className }: { className: string }) => <div data-testid="dialog-overlay" className={className} />,
  Content: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <div data-testid="dialog-content" className={className}>{children}</div>
  ),
  Title: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <h2 data-testid="dialog-title" className={className}>{children}</h2>
  ),
  Close: ({ children, asChild }: { children: React.ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <button>{children}</button>,
}));

// Mock Radix Tabs
vi.mock('@radix-ui/react-tabs', () => ({
  Root: ({ children, value, onValueChange: _onValueChange }: { children: React.ReactNode; value: string; onValueChange: (v: string) => void }) => (
    <div data-testid="tabs-root" data-active-tab={value}>{children}</div>
  ),
  List: ({ children, className }: { children: React.ReactNode; className: string }) => (
    <div data-testid="tabs-list" className={className} role="tablist">{children}</div>
  ),
  Trigger: ({ children, value }: { children: React.ReactNode; value: string }) => (
    <button data-testid={`tab-${value}`} role="tab">{children}</button>
  ),
  Content: ({ children, value, className }: { children: React.ReactNode; value: string; className: string }) => (
    <div data-testid={`tab-content-${value}`} role="tabpanel" className={className}>{children}</div>
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
  });

  it('renders language selector', () => {
    render(<GeneralSection />);

    expect(screen.getByTestId('language-selector')).toBeInTheDocument();
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

  it('renders LLM backend title and description', () => {
    render(<LLMBackendSection />);

    expect(screen.getByText('LLM Backend')).toBeInTheDocument();
    expect(screen.getByText(/Select your preferred LLM provider/)).toBeInTheDocument();
  });

  it('renders all backend provider options', () => {
    render(<LLMBackendSection />);

    expect(screen.getByText('Claude Code (Claude Max)')).toBeInTheDocument();
    expect(screen.getByText('Claude API')).toBeInTheDocument();
    expect(screen.getByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('DeepSeek')).toBeInTheDocument();
    expect(screen.getByText('Ollama (Local)')).toBeInTheDocument();
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

  it('shows API Key Required badge for providers that need keys', () => {
    render(<LLMBackendSection />);

    const apiKeyBadges = screen.getAllByText('API Key Required');
    // Claude API, OpenAI, DeepSeek, GLM, Qwen require keys
    expect(apiKeyBadges.length).toBe(5);
  });

  it('updates standalone context turns setting', () => {
    render(<LLMBackendSection />);

    const contextTurnsSelect = screen.getByDisplayValue('8');
    fireEvent.change(contextTurnsSelect, { target: { value: '20' } });

    expect(mockSetStandaloneContextTurns).toHaveBeenCalledWith(20);
  });

  it('renders streaming output toggles and updates preferences', () => {
    render(<LLMBackendSection />);

    const subAgentLabel = screen.getByText('Show sub-agent progress events');
    const reasoningLabel = screen.getByText('Show model reasoning/thinking traces');
    const subAgentToggle = subAgentLabel.closest('label')?.querySelector('input') as HTMLInputElement | null;
    const reasoningToggle = reasoningLabel.closest('label')?.querySelector('input') as HTMLInputElement | null;

    expect(subAgentToggle).toBeTruthy();
    expect(reasoningToggle).toBeTruthy();

    fireEvent.click(subAgentToggle!);
    fireEvent.click(reasoningToggle!);

    expect(mockSetShowSubAgentEvents).toHaveBeenCalledWith(false);
    expect(mockSetShowReasoningOutput).toHaveBeenCalledWith(true);
  });

  it('renders model selector and custom model input', () => {
    render(<LLMBackendSection />);

    expect(screen.getByText('Model')).toBeInTheDocument();
    expect(screen.getByText('Use provider default')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Model name')).toBeInTheDocument();
  });

  it('does not show API key configuration for non-key backends', () => {
    mockSettingsState.backend = 'claude-code';

    render(<LLMBackendSection />);

    expect(screen.queryByText(/API Key for/)).not.toBeInTheDocument();
  });

  it('keeps API key status scoped by provider', async () => {
    mockSettingsState.backend = 'glm';
    const { rerender } = render(<LLMBackendSection />);

    const glmInput = screen.getByPlaceholderText('Enter your API key');
    fireEvent.change(glmInput, { target: { value: 'glm-secret-key' } });
    fireEvent.click(screen.getByText('Save'));

    await waitFor(() => {
      expect(screen.getByText('API key saved successfully')).toBeInTheDocument();
    });

    mockSettingsState.backend = 'deepseek';
    rerender(<LLMBackendSection />);

    await waitFor(() => {
      expect(screen.getByText('API Key for DeepSeek')).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText('Enter your API key')).toBeInTheDocument();
    expect(screen.queryByText('Remove')).not.toBeInTheDocument();
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
});
