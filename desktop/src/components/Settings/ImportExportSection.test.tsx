import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ImportExportSection } from './ImportExportSection';

const mockClearAllData = vi.fn();
const mockResetAllSettings = vi.fn();
const mockExportAllSettings = vi.fn();
const mockImportAllSettings = vi.fn();
const mockFetchConfig = vi.fn();
const mockFetchIndexConfig = vi.fn();

const mockSettingsState = {
  backend: 'claude-code',
  provider: 'anthropic',
  model: '',
  modelByProvider: { anthropic: '' },
  theme: 'system',
  defaultMode: 'expert',
  agents: [{ name: 'claude-code', enabled: true, command: 'claude', isDefault: true }],
  agentSelection: 'prefer_default',
  defaultAgent: 'claude-code',
  qualityGates: { typecheck: true, test: true, lint: true, custom: false, customScript: '', maxRetries: 3 },
  maxParallelStories: 3,
  maxIterations: 50,
  maxTotalTokens: 1_000_000,
  timeoutSeconds: 300,
  standaloneContextTurns: -1,
  language: 'en',
  showLineNumbers: true,
  maxFileAttachmentSize: 10 * 1024 * 1024,
  enableMarkdownMath: true,
  enableCodeBlockCopy: true,
  enableContextCompaction: true,
  showReasoningOutput: true,
  enableThinking: true,
  showSubAgentEvents: true,
  glmEndpoint: 'standard',
  minimaxEndpoint: 'international',
  qwenEndpoint: 'china',
  searchProvider: 'duckduckgo',
  maxConcurrentSubagents: 0,
  phaseConfigs: {},
  pinnedDirectories: [],
  sidebarCollapsed: false,
  autoPanelHoverEnabled: false,
  workspacePath: '',
  resetToDefaults: vi.fn(),
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const map: Record<string, string> = {
        'importExport.title': 'Import & Export',
        'importExport.description': 'desc',
        'importExport.export.title': 'Export',
        'importExport.export.description': 'export desc',
        'importExport.export.includeSecrets': 'include secrets',
        'importExport.export.button': 'Export',
        'importExport.export.exporting': 'Exporting...',
        'importExport.import.title': 'Import',
        'importExport.import.description': 'import desc',
        'importExport.import.button': 'Choose File',
        'importExport.reset.title': 'Reset',
        'importExport.reset.description': 'reset desc',
        'importExport.reset.button': 'Reset',
        'importExport.clearAllData.title': 'Clear App Data',
        'importExport.clearAllData.description': 'clear all data desc',
        'importExport.clearAllData.button': 'Clear All Data',
        'importExport.clearAllData.clearing': 'Clearing...',
        'importExport.clearAllData.confirmPrimary': 'confirm-1',
        'importExport.clearAllData.confirmSecondary': 'confirm-2',
        'importExport.clearAllData.success': 'clear success',
        'importExport.clearAllData.error': 'clear error',
      };
      return map[key] || key;
    },
  }),
}));

vi.mock('@radix-ui/react-dialog', () => ({
  Root: ({ children }: { children: ReactNode }) => <>{children}</>,
  Portal: ({ children }: { children: ReactNode }) => <>{children}</>,
  Overlay: ({ className }: { className?: string }) => <div className={className} />,
  Content: ({ children, className }: { children: ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
  Title: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  Description: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  Close: ({ children, asChild }: { children: ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <button>{children}</button>,
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: Object.assign(() => mockSettingsState, {
    getState: () => mockSettingsState,
    setState: vi.fn(),
  }),
}));

vi.mock('../../store/embedding', () => ({
  useEmbeddingStore: {
    getState: () => ({
      fetchConfig: mockFetchConfig,
      fetchIndexConfig: mockFetchIndexConfig,
    }),
  },
}));

vi.mock('../../lib/settingsApi', () => ({
  clearAllData: (...args: unknown[]) => mockClearAllData(...args),
  resetAllSettings: (...args: unknown[]) => mockResetAllSettings(...args),
  exportAllSettings: (...args: unknown[]) => mockExportAllSettings(...args),
  importAllSettings: (...args: unknown[]) => mockImportAllSettings(...args),
}));

describe('ImportExportSection clear-all-data', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockClearAllData.mockResolvedValue(true);
    mockResetAllSettings.mockResolvedValue(true);
    mockFetchConfig.mockResolvedValue(undefined);
    mockFetchIndexConfig.mockResolvedValue(undefined);
    mockExportAllSettings.mockResolvedValue({});
    mockImportAllSettings.mockResolvedValue({
      success: true,
      frontend: null,
      imported_sections: [],
      skipped_sections: [],
      warnings: [],
      errors: [],
    });
  });

  it('calls clearAllData, clears localStorage, dispatches reset event, and schedules reload', async () => {
    localStorage.setItem('plan-cascade-settings', 'persisted');

    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    const dispatchSpy = vi.spyOn(window, 'dispatchEvent');
    const timeoutSpy = vi.spyOn(window, 'setTimeout');

    render(<ImportExportSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Clear All Data' }));

    await waitFor(() => {
      expect(mockClearAllData).toHaveBeenCalledTimes(1);
    });

    expect(confirmSpy).toHaveBeenCalledTimes(2);
    expect(localStorage.getItem('plan-cascade-settings')).toBeNull();
    expect(dispatchSpy).toHaveBeenCalled();
    expect(timeoutSpy).toHaveBeenCalled();
  });

  it('shows error and does not clear frontend state when clearAllData fails', async () => {
    localStorage.setItem('plan-cascade-settings', 'persisted');

    vi.spyOn(window, 'confirm').mockReturnValue(true);
    mockClearAllData.mockRejectedValueOnce(new Error('backend failure'));

    render(<ImportExportSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Clear All Data' }));

    await waitFor(() => {
      expect(screen.getByText('clear error')).toBeInTheDocument();
    });

    expect(localStorage.getItem('plan-cascade-settings')).toBe('persisted');
  });
});
