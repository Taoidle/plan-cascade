/**
 * ProjectSelector Component Tests
 * Story 001: Add Open File Manager Button to Project Selector
 *
 * Tests for the "Open in File Manager" button functionality.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

// Use vi.hoisted to define mock functions that can be referenced in vi.mock factories
const { mockShellOpen } = vi.hoisted(() => ({
  mockShellOpen: vi.fn().mockResolvedValue(undefined),
}));

// Mock react-i18next (must include initReactI18next for i18n module import)
vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, defaultValueOrOpts?: string | { defaultValue?: string }) => {
      const translations: Record<string, string> = {
        'projectSelector.openInFileManager': 'Open in file manager',
      };
      if (translations[key]) return translations[key];
      if (typeof defaultValueOrOpts === 'string') return defaultValueOrOpts;
      if (typeof defaultValueOrOpts === 'object' && defaultValueOrOpts?.defaultValue) return defaultValueOrOpts.defaultValue;
      return key;
    },
    i18n: { language: 'en' },
  }),
}));

// Mock @tauri-apps/plugin-shell
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: mockShellOpen,
}));

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
}));

// Import after mocks
import { ProjectSelector } from './ProjectSelector';
import { useSettingsStore } from '../../store/settings';

// ============================================================================
// Helper to set workspacePath in the store
// ============================================================================
function setWorkspacePath(path: string) {
  useSettingsStore.setState({ workspacePath: path });
}

// ============================================================================
// Open File Manager Button Tests
// ============================================================================

describe('ProjectSelector - Open File Manager Button', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockShellOpen.mockResolvedValue(undefined);
    // Reset store to defaults
    useSettingsStore.setState({ workspacePath: '' });
  });

  it('should NOT render the open-in-file-manager button when workspacePath is empty', () => {
    setWorkspacePath('');
    render(<ProjectSelector />);

    const openButton = screen.queryByTitle('Open in file manager');
    expect(openButton).not.toBeInTheDocument();
  });

  it('should render the open-in-file-manager button when workspacePath is set', () => {
    setWorkspacePath('/home/user/projects/my-app');
    render(<ProjectSelector />);

    const openButton = screen.getByTitle('Open in file manager');
    expect(openButton).toBeInTheDocument();
  });

  it('should be positioned between the directory picker button and the clear button', () => {
    setWorkspacePath('/home/user/projects/my-app');
    const { container } = render(<ProjectSelector />);

    // Get all buttons in the component
    const buttons = container.querySelectorAll('button');

    // Expect 3 buttons: directory picker, open file manager, clear
    expect(buttons.length).toBe(3);

    // The open-in-file-manager button should be the second button (index 1)
    expect(buttons[1]).toHaveAttribute('title', 'Open in file manager');
  });

  it('should use compact styling when compact prop is true', () => {
    setWorkspacePath('/home/user/projects/my-app');
    render(<ProjectSelector compact />);

    const openButton = screen.getByTitle('Open in file manager');
    // In compact mode the button should have compact sizing class (w-5 h-5)
    expect(openButton.className).toContain('w-5');
    expect(openButton.className).toContain('h-5');
  });

  it('should use normal styling when compact prop is false', () => {
    setWorkspacePath('/home/user/projects/my-app');
    render(<ProjectSelector />);

    const openButton = screen.getByTitle('Open in file manager');
    // In normal mode the button should have normal sizing class (w-6 h-6)
    expect(openButton.className).toContain('w-6');
    expect(openButton.className).toContain('h-6');
  });

  it('should stop event propagation when clicked', () => {
    setWorkspacePath('/home/user/projects/my-app');
    const parentClickHandler = vi.fn();

    const { container } = render(
      <div onClick={parentClickHandler}>
        <ProjectSelector />
      </div>
    );

    const openButton = container.querySelector('[title="Open in file manager"]')!;
    fireEvent.click(openButton);

    // The parent should not receive the click event because stopPropagation is called
    expect(parentClickHandler).not.toHaveBeenCalled();
  });
});
