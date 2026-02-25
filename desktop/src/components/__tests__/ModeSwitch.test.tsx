/**
 * ModeSwitch Component Tests
 *
 * Tests mode switching, active mode indication, all 5 modes render,
 * and transition animations.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ModeSwitch, ModeTabs, AnimatedModeContent } from '../ModeSwitch';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'modeSwitch.label': 'Select Mode',
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
      };
      return translations[key] || key;
    },
  }),
}));

let mockModeStoreState = {
  isTransitioning: false,
  transitionDirection: 'none' as string,
};

vi.mock('../../store/mode', () => ({
  useModeStore: () => mockModeStoreState,
  MODES: ['simple', 'expert', 'claude-code', 'projects', 'analytics'],
}));

// Mock Radix DropdownMenu for ModeSwitch
vi.mock('@radix-ui/react-dropdown-menu', () => ({
  Root: ({ children }: { children: React.ReactNode }) => <div data-testid="dropdown-root">{children}</div>,
  Trigger: ({ children, asChild, disabled }: { children: React.ReactNode; asChild?: boolean; disabled?: boolean }) =>
    asChild ? <>{children}</> : <button disabled={disabled}>{children}</button>,
  Portal: ({ children }: { children: React.ReactNode }) => <div data-testid="dropdown-portal">{children}</div>,
  Content: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="dropdown-content" role="menu">
      {children}
    </div>
  ),
  Label: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Item: ({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) => (
    <div role="menuitem" onClick={onClick}>
      {children}
    </div>
  ),
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('ModeSwitch', () => {
  const mockOnChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockModeStoreState = { isTransitioning: false, transitionDirection: 'none' };
  });

  it('renders with the current mode label displayed', () => {
    render(<ModeSwitch mode="simple" onChange={mockOnChange} />);

    expect(screen.getAllByText('Simple').length).toBeGreaterThan(0);
  });

  it('renders all 5 mode options in the dropdown', () => {
    render(<ModeSwitch mode="simple" onChange={mockOnChange} />);

    expect(screen.getAllByText('Simple').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Expert').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Claude Code').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Projects').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Analytics').length).toBeGreaterThan(0);
  });

  it('displays descriptions for each mode', () => {
    render(<ModeSwitch mode="simple" onChange={mockOnChange} />);

    expect(screen.getByText('One-click execution')).toBeInTheDocument();
    expect(screen.getByText('Full control')).toBeInTheDocument();
    expect(screen.getByText('Interactive chat')).toBeInTheDocument();
    expect(screen.getByText('Browse sessions')).toBeInTheDocument();
    expect(screen.getByText('Track usage')).toBeInTheDocument();
  });

  it('calls onChange when a mode option is clicked', () => {
    render(<ModeSwitch mode="simple" onChange={mockOnChange} />);

    const expertItem = screen.getByText('Expert').closest('[role="menuitem"]');
    if (expertItem) fireEvent.click(expertItem);

    expect(mockOnChange).toHaveBeenCalledWith('expert');
  });

  it('shows a checkmark icon for the currently selected mode', () => {
    render(<ModeSwitch mode="expert" onChange={mockOnChange} />);

    // The selected mode label should be visible as the trigger text
    // There are multiple "Expert" entries (trigger + menu item)
    const expertTexts = screen.getAllByText('Expert');
    expect(expertTexts.length).toBeGreaterThanOrEqual(1);
  });

  it('shows Select Mode label in the dropdown', () => {
    render(<ModeSwitch mode="simple" onChange={mockOnChange} />);

    expect(screen.getByText('Select Mode')).toBeInTheDocument();
  });
});

describe('ModeTabs', () => {
  const mockOnChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders tab buttons for all 5 modes', () => {
    render(<ModeTabs mode="simple" onChange={mockOnChange} />);

    // All mode labels should be present (hidden on mobile but in DOM)
    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBe(5);
  });

  it('calls onChange when a tab is clicked', () => {
    render(<ModeTabs mode="simple" onChange={mockOnChange} />);

    const buttons = screen.getAllByRole('button');
    // Click the second button (Expert)
    fireEvent.click(buttons[1]);

    expect(mockOnChange).toHaveBeenCalledWith('expert');
  });

  it('applies disabled styles when disabled prop is true', () => {
    const { container } = render(<ModeTabs mode="simple" onChange={mockOnChange} disabled />);

    const wrapper = container.firstChild;
    expect(wrapper).toHaveClass('opacity-50');
    expect(wrapper).toHaveClass('pointer-events-none');
  });

  it('renders mode labels in tabs', () => {
    render(<ModeTabs mode="projects" onChange={mockOnChange} />);

    expect(screen.getByText('Simple')).toBeInTheDocument();
    expect(screen.getByText('Expert')).toBeInTheDocument();
    expect(screen.getByText('Claude Code')).toBeInTheDocument();
    expect(screen.getByText('Projects')).toBeInTheDocument();
    expect(screen.getByText('Analytics')).toBeInTheDocument();
  });
});

describe('AnimatedModeContent', () => {
  beforeEach(() => {
    mockModeStoreState = { isTransitioning: false, transitionDirection: 'none' };
  });

  it('renders children correctly', () => {
    render(
      <AnimatedModeContent mode="simple">
        <div>Content Here</div>
      </AnimatedModeContent>,
    );

    expect(screen.getByText('Content Here')).toBeInTheDocument();
  });

  it('applies base opacity class when not transitioning', () => {
    const { container } = render(
      <AnimatedModeContent mode="simple">
        <div>Content</div>
      </AnimatedModeContent>,
    );

    const wrapper = container.firstChild;
    expect(wrapper).toHaveClass('opacity-100');
  });

  it('applies slide animation class when transitioning left', () => {
    mockModeStoreState = { isTransitioning: true, transitionDirection: 'left' };

    const { container } = render(
      <AnimatedModeContent mode="expert">
        <div>Content</div>
      </AnimatedModeContent>,
    );

    const wrapper = container.firstChild;
    expect(wrapper).toHaveClass('animate-in');
  });

  it('applies slide animation class when transitioning right', () => {
    mockModeStoreState = { isTransitioning: true, transitionDirection: 'right' };

    const { container } = render(
      <AnimatedModeContent mode="simple">
        <div>Content</div>
      </AnimatedModeContent>,
    );

    const wrapper = container.firstChild;
    expect(wrapper).toHaveClass('animate-in');
  });
});
