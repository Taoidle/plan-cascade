/**
 * CommandPalette Component Tests
 * Story 011-7: Command Palette with Fuzzy Search
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ReactNode } from 'react';
import {
  CommandPaletteProvider,
  useCommandPalette,
  createDefaultCommands,
  Command,
} from '../CommandPalette';

// Test wrapper component
function TestComponent() {
  const { isOpen, open, close, commands } = useCommandPalette();

  return (
    <div>
      <button onClick={open} data-testid="open-button">
        Open Palette
      </button>
      <button onClick={close} data-testid="close-button">
        Close Palette
      </button>
      <span data-testid="is-open">{isOpen.toString()}</span>
      <span data-testid="command-count">{commands.length}</span>
    </div>
  );
}

function renderWithProvider(
  ui: ReactNode,
  defaultCommands: Command[] = []
) {
  return render(
    <CommandPaletteProvider defaultCommands={defaultCommands}>
      {ui}
    </CommandPaletteProvider>
  );
}

describe('CommandPaletteProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('provides command palette context', () => {
    renderWithProvider(<TestComponent />);
    expect(screen.getByTestId('is-open')).toHaveTextContent('false');
  });

  it('opens palette when open is called', async () => {
    renderWithProvider(<TestComponent />);

    fireEvent.click(screen.getByTestId('open-button'));
    await waitFor(() => {
      expect(screen.getByTestId('is-open')).toHaveTextContent('true');
    });
  });

  it('closes palette when close is called', async () => {
    renderWithProvider(<TestComponent />);

    fireEvent.click(screen.getByTestId('open-button'));
    await waitFor(() => {
      expect(screen.getByTestId('is-open')).toHaveTextContent('true');
    });

    fireEvent.click(screen.getByTestId('close-button'));
    await waitFor(() => {
      expect(screen.getByTestId('is-open')).toHaveTextContent('false');
    });
  });

  it('includes default commands', () => {
    const commands: Command[] = [
      {
        id: 'test-command',
        title: 'Test Command',
        category: 'chat',
        action: vi.fn(),
      },
    ];

    renderWithProvider(<TestComponent />, commands);
    expect(screen.getByTestId('command-count')).toHaveTextContent('1');
  });
});

describe('createDefaultCommands', () => {
  const mockT = (key: string) => key;

  it('creates clear chat command when callback is provided', () => {
    const onClearChat = vi.fn();
    const commands = createDefaultCommands({ onClearChat }, mockT);

    const clearCommand = commands.find((c) => c.id === 'clear-chat');
    expect(clearCommand).toBeDefined();
    expect(clearCommand?.shortcut).toBe('mod+l');
  });

  it('creates new conversation command when callback is provided', () => {
    const onNewConversation = vi.fn();
    const commands = createDefaultCommands({ onNewConversation }, mockT);

    const newCommand = commands.find((c) => c.id === 'new-conversation');
    expect(newCommand).toBeDefined();
  });

  it('creates export command when callback is provided', () => {
    const onExportConversation = vi.fn();
    const commands = createDefaultCommands({ onExportConversation }, mockT);

    const exportCommand = commands.find((c) => c.id === 'export-conversation');
    expect(exportCommand).toBeDefined();
    expect(exportCommand?.category).toBe('file');
  });

  it('creates toggle sidebar command', () => {
    const onToggleSidebar = vi.fn();
    const commands = createDefaultCommands({ onToggleSidebar }, mockT);

    const toggleCommand = commands.find((c) => c.id === 'toggle-sidebar');
    expect(toggleCommand).toBeDefined();
    expect(toggleCommand?.shortcut).toBe('mod+b');
  });

  it('does not create commands when callbacks are not provided', () => {
    const commands = createDefaultCommands({}, mockT);
    expect(commands).toHaveLength(0);
  });

  it('executes callback when command action is called', () => {
    const onClearChat = vi.fn();
    const commands = createDefaultCommands({ onClearChat }, mockT);

    const clearCommand = commands.find((c) => c.id === 'clear-chat');
    clearCommand?.action();

    expect(onClearChat).toHaveBeenCalled();
  });

  it('sets correct keywords for commands', () => {
    const onClearChat = vi.fn();
    const commands = createDefaultCommands({ onClearChat }, mockT);

    const clearCommand = commands.find((c) => c.id === 'clear-chat');
    expect(clearCommand?.keywords).toContain('clear');
    expect(clearCommand?.keywords).toContain('delete');
  });
});
