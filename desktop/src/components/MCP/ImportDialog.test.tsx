import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ImportDialog } from './ImportDialog';

const mockImportFromClaudeDesktop = vi.fn();
const mockImportFromFile = vi.fn();
const mockOpenDialog = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const map: Record<string, string> = {
        'mcp.importTitle': 'Import',
        'mcp.importDescription': 'desc',
        'mcp.configPath': 'Config Path',
        'mcp.importJsonFile': 'Import JSON File',
        'mcp.importNow': 'Import Now',
        'mcp.previewImport': 'Preview Import',
        'mcp.previewOnly': 'Preview only',
        'mcp.conflictPolicy.label': 'Conflict policy',
        'mcp.conflictPolicy.skip': 'Skip duplicate',
        'mcp.conflictPolicy.rename': 'Rename imported',
        'mcp.conflictPolicy.replace': 'Replace existing',
        'common.cancel': 'Cancel',
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
  Close: ({ children, asChild }: { children: ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <button>{children}</button>,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (...args: unknown[]) => mockOpenDialog(...args),
}));

vi.mock('../../lib/mcpApi', () => ({
  mcpApi: {
    importFromClaudeDesktop: (...args: unknown[]) => mockImportFromClaudeDesktop(...args),
    importFromFile: (...args: unknown[]) => mockImportFromFile(...args),
  },
}));

describe('ImportDialog conflict policy', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOpenDialog.mockResolvedValue('/tmp/import.json');
    mockImportFromClaudeDesktop.mockResolvedValue({
      success: true,
      data: { added: 0, skipped: 0, failed: 0, servers: [], errors: [] },
      error: null,
    });
    mockImportFromFile.mockResolvedValue({
      success: true,
      data: { added: 0, skipped: 0, failed: 0, servers: [], errors: [] },
      error: null,
    });
  });

  it('passes selected conflict policy to file import', async () => {
    render(<ImportDialog open={true} onOpenChange={vi.fn()} onImportComplete={vi.fn()} />);

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'replace' } });
    fireEvent.click(screen.getByRole('button', { name: 'Import JSON File' }));

    await waitFor(() => {
      expect(mockImportFromFile).toHaveBeenCalledTimes(1);
    });

    expect(mockImportFromFile).toHaveBeenCalledWith({
      path: '/tmp/import.json',
      dryRun: false,
      conflictPolicy: 'replace',
    });
  });
});
