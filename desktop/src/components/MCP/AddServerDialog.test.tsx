import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { AddServerDialog } from './AddServerDialog';

const mockAddServer = vi.fn();
const mockUpdateServer = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const map: Record<string, string> = {
        'mcp.addServerTitle': 'Add MCP Server',
        'mcp.addEnvVar': 'Add Variable',
        'mcp.placeholders.envKey': 'env-key',
        'mcp.placeholders.envValue': 'env-value',
        'mcp.placeholders.serverName': 'server-name',
        'mcp.placeholders.command': 'command',
        'mcp.placeholders.arguments': 'arguments',
        'mcp.show': 'Show',
        'mcp.hide': 'Hide',
        'buttons.save': 'Save',
        'common.adding': 'Adding',
        'mcp.errors.addServer': 'failed',
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

vi.mock('../../lib/mcpApi', () => ({
  mcpApi: {
    addServer: (...args: unknown[]) => mockAddServer(...args),
    updateServer: (...args: unknown[]) => mockUpdateServer(...args),
  },
}));

describe('AddServerDialog secrets masking', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockAddServer.mockResolvedValue({ success: true, data: null, error: null });
    mockUpdateServer.mockResolvedValue({ success: true, data: null, error: null });
  });

  it('masks env values by default and toggles only the selected row', () => {
    render(<AddServerDialog open={true} onOpenChange={vi.fn()} onServerAdded={vi.fn()} onServerUpdated={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Add Variable' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add Variable' }));

    const valueInputs = screen.getAllByPlaceholderText('env-value') as HTMLInputElement[];
    expect(valueInputs).toHaveLength(2);
    expect(valueInputs[0].type).toBe('password');
    expect(valueInputs[1].type).toBe('password');

    const showButtons = screen.getAllByRole('button', { name: 'Show' });
    fireEvent.click(showButtons[0]);

    const updatedInputs = screen.getAllByPlaceholderText('env-value') as HTMLInputElement[];
    expect(updatedInputs[0].type).toBe('text');
    expect(updatedInputs[1].type).toBe('password');
  });
});
