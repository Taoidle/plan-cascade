/**
 * KeyboardShortcuts Tests
 * Story 011-5: Keyboard Shortcuts Implementation
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      if (key === 'shortcuts.title') {
        return 'Keyboard Shortcuts';
      }
      return key;
    },
  }),
}));

import { formatShortcut, getPlatformModifier, KeyboardShortcutHint, ShortcutsHelpDialog } from '../KeyboardShortcuts';

describe('formatShortcut', () => {
  it('formats mod+key shortcuts', () => {
    const formatted = formatShortcut('mod+enter');
    expect(formatted).toMatch(/(Ctrl|Cmd) \+ enter/i);
  });

  it('handles multiple modifiers', () => {
    const formatted = formatShortcut('mod+shift+k');
    expect(formatted).toContain('Shift');
  });

  it('handles single keys', () => {
    const formatted = formatShortcut('escape');
    expect(formatted).toBe('escape');
  });
});

describe('getPlatformModifier', () => {
  it('returns platform-specific modifier', () => {
    const modifier = getPlatformModifier();
    expect(['Ctrl', 'Cmd']).toContain(modifier);
  });
});

describe('KeyboardShortcutHint', () => {
  it('renders keyboard shortcut with keys', () => {
    const { container } = render(<KeyboardShortcutHint shortcut="mod+enter" />);
    const keys = container.querySelectorAll('kbd');
    expect(keys.length).toBeGreaterThan(0);
  });

  it('applies custom className', () => {
    const { container } = render(<KeyboardShortcutHint shortcut="escape" className="custom" />);
    expect(container.querySelector('.custom')).toBeInTheDocument();
  });
});

describe('ShortcutsHelpDialog', () => {
  it('does not render when closed', () => {
    render(<ShortcutsHelpDialog isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText('Keyboard Shortcuts')).not.toBeInTheDocument();
  });

  it('renders when open', () => {
    render(<ShortcutsHelpDialog isOpen={true} onClose={vi.fn()} />);
    expect(screen.getByText('Keyboard Shortcuts')).toBeInTheDocument();
  });

  it('shows shortcut categories', () => {
    render(<ShortcutsHelpDialog isOpen={true} onClose={vi.fn()} />);
    expect(screen.getByText('Chat')).toBeInTheDocument();
    expect(screen.getByText('Navigation')).toBeInTheDocument();
    expect(screen.getByText('General')).toBeInTheDocument();
  });

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn();
    render(<ShortcutsHelpDialog isOpen={true} onClose={onClose} />);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onClose when backdrop is clicked', () => {
    const onClose = vi.fn();
    render(<ShortcutsHelpDialog isOpen={true} onClose={onClose} />);

    // Find the backdrop (fixed inset-0 element)
    const backdrop = document.querySelector('.fixed.inset-0');
    if (backdrop) {
      fireEvent.click(backdrop);
      expect(onClose).toHaveBeenCalled();
    }
  });

  it('shows help text about closing', () => {
    render(<ShortcutsHelpDialog isOpen={true} onClose={vi.fn()} />);
    expect(screen.getByText(/Esc/)).toBeInTheDocument();
  });
});
