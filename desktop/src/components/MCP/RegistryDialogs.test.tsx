import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { McpExportDialog } from './RegistryDialogs';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const map: Record<string, string> = {
        'mcp.export': 'Export',
        'mcp.exportRedactedDefault': 'Sensitive fields are redacted by default.',
        'mcp.exportOnlyRedacted': 'Only redacted exports are allowed.',
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
}));

describe('McpExportDialog', () => {
  it('shows redacted-only export messaging', () => {
    render(<McpExportDialog open={true} onOpenChange={vi.fn()} onConfirm={vi.fn()} />);
    expect(screen.getByText('Sensitive fields are redacted by default.')).toBeInTheDocument();
    expect(screen.getByText('Only redacted exports are allowed.')).toBeInTheDocument();
    expect(screen.queryByText(/include secrets/i)).not.toBeInTheDocument();
  });
});
