/**
 * IconRail Component Tests
 *
 * Tests mode icon rendering, selection, click handling,
 * running status dot, search/settings buttons.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { IconRail } from '../IconRail';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'modeSwitch.simple.name': 'Simple',
        'modeSwitch.simple.description': 'One-click execution',
        'modeSwitch.expert.name': 'Expert',
        'modeSwitch.expert.description': 'Full control',
        'modeSwitch.claudeCode.name': 'Claude Code',
        'modeSwitch.claudeCode.description': 'Interactive chat',
        'modeSwitch.projects.name': 'Projects',
        'modeSwitch.projects.description': 'Browse sessions',
        'modeSwitch.analytics.name': 'Analytics',
        'modeSwitch.analytics.description': 'Track usage',
        'modeSwitch.knowledge.name': 'Knowledge',
        'modeSwitch.knowledge.description': 'Knowledge base',
        'modeSwitch.artifacts.name': 'Artifacts',
        'modeSwitch.artifacts.description': 'Browse artifacts',
        'iconRail.searchTooltip': 'Search commands',
        'iconRail.settingsTooltip': 'Settings',
      };
      return translations[key] || key;
    },
  }),
}));

const mockSetMode = vi.fn();
let mockModeStoreState = {
  mode: 'simple' as string,
  setMode: mockSetMode,
};

vi.mock('../../store/mode', () => ({
  useModeStore: () => mockModeStoreState,
  MODES: ['simple', 'expert', 'claude-code', 'projects', 'analytics', 'knowledge', 'artifacts'],
}));

let mockExecutionStoreState = {
  status: 'idle' as string,
};

vi.mock('../../store/execution', () => ({
  useExecutionStore: () => mockExecutionStoreState,
}));

// Mock Radix Tooltip
vi.mock('@radix-ui/react-tooltip', () => ({
  Provider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Root: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Trigger: ({ children, asChild }: { children: React.ReactNode; asChild?: boolean }) =>
    asChild ? <>{children}</> : <button>{children}</button>,
  Portal: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Content: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Arrow: () => null,
}));

// Mock SettingsDialog
vi.mock('../Settings', () => ({
  SettingsDialog: ({ open }: { open: boolean; onOpenChange: (v: boolean) => void }) =>
    open ? <div data-testid="settings-dialog">Settings</div> : null,
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('IconRail', () => {
  const mockOpenCommandPalette = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockModeStoreState = {
      mode: 'simple',
      setMode: mockSetMode,
    };
    mockExecutionStoreState = {
      status: 'idle',
    };
  });

  it('renders 7 mode icon buttons', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    expect(screen.getByTestId('rail-icon-simple')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-expert')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-claude-code')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-projects')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-analytics')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-knowledge')).toBeInTheDocument();
    expect(screen.getByTestId('rail-icon-artifacts')).toBeInTheDocument();
  });

  it('highlights the selected mode with aria-current', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    const simpleBtn = screen.getByTestId('rail-icon-simple');
    expect(simpleBtn).toHaveAttribute('aria-current', 'page');

    const expertBtn = screen.getByTestId('rail-icon-expert');
    expect(expertBtn).not.toHaveAttribute('aria-current');
  });

  it('calls setMode when a mode icon is clicked', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    fireEvent.click(screen.getByTestId('rail-icon-expert'));
    expect(mockSetMode).toHaveBeenCalledWith('expert');
  });

  it('disables non-selected modes when running', () => {
    mockExecutionStoreState = { status: 'running' };

    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    // Selected mode should not be disabled
    expect(screen.getByTestId('rail-icon-simple')).not.toBeDisabled();

    // Other modes should be disabled
    expect(screen.getByTestId('rail-icon-expert')).toBeDisabled();
    expect(screen.getByTestId('rail-icon-claude-code')).toBeDisabled();
  });

  it('calls onOpenCommandPalette when search button is clicked', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    fireEvent.click(screen.getByTestId('rail-search'));
    expect(mockOpenCommandPalette).toHaveBeenCalledTimes(1);
  });

  it('opens settings dialog when settings button is clicked', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    // Settings dialog should not be visible initially
    expect(screen.queryByTestId('settings-dialog')).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId('rail-settings'));

    expect(screen.getByTestId('settings-dialog')).toBeInTheDocument();
  });

  it('renders the navigation element with correct aria-label', () => {
    render(<IconRail onOpenCommandPalette={mockOpenCommandPalette} />);

    expect(screen.getByRole('navigation')).toHaveAttribute('aria-label', 'Main navigation');
  });
});
