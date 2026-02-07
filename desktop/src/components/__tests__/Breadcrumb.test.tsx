/**
 * Breadcrumb Component Tests
 *
 * Tests breadcrumb navigation path rendering, click navigation,
 * and responsive visibility.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Breadcrumb } from '../shared/Breadcrumb';
import type { BreadcrumbItem } from '../../store/mode';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

const mockNavigateToBreadcrumb = vi.fn();

let mockBreadcrumbs: BreadcrumbItem[] = [];

vi.mock('../../store/mode', () => ({
  useModeStore: () => ({
    breadcrumbs: mockBreadcrumbs,
    navigateToBreadcrumb: mockNavigateToBreadcrumb,
  }),
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('Breadcrumb', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBreadcrumbs = [];
  });

  it('renders nothing when breadcrumbs array is empty', () => {
    mockBreadcrumbs = [];

    const { container } = render(<Breadcrumb />);

    expect(container.querySelector('nav')).toBeNull();
  });

  it('renders a single breadcrumb item (last item, not clickable)', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
    ];

    render(<Breadcrumb />);

    // Single item is the last item, should be rendered as a span (current page)
    expect(screen.getByText('Plan Cascade')).toBeInTheDocument();
    expect(screen.getByLabelText('Breadcrumb')).toBeInTheDocument();
  });

  it('renders a two-level breadcrumb with home and mode', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-simple', label: 'Simple', mode: 'simple' as const, navigable: true },
    ];

    render(<Breadcrumb />);

    expect(screen.getByText('Plan Cascade')).toBeInTheDocument();
    expect(screen.getByText('Simple')).toBeInTheDocument();
  });

  it('renders clickable breadcrumb items that are not the last item', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-expert', label: 'Expert', mode: 'expert' as const, navigable: true },
      { id: 'subview', label: 'PRD Editor', navigable: false },
    ];

    render(<Breadcrumb />);

    // Home and Expert should be buttons (clickable)
    const homeButton = screen.getByText('Plan Cascade').closest('button');
    expect(homeButton).toBeInTheDocument();

    const expertButton = screen.getByText('Expert').closest('button');
    expect(expertButton).toBeInTheDocument();

    // PRD Editor is the last item, should be a span (not clickable)
    const subviewSpan = screen.getByText('PRD Editor');
    expect(subviewSpan.closest('button')).toBeNull();
  });

  it('calls navigateToBreadcrumb when a navigable item is clicked', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-projects', label: 'Projects', mode: 'projects' as const, navigable: true },
      { id: 'subview', label: 'Session Details', navigable: false },
    ];

    render(<Breadcrumb />);

    // Click on the home breadcrumb
    const homeButton = screen.getByText('Plan Cascade').closest('button')!;
    fireEvent.click(homeButton);

    expect(mockNavigateToBreadcrumb).toHaveBeenCalledWith('home');
  });

  it('calls navigateToBreadcrumb with mode id when mode breadcrumb is clicked', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-analytics', label: 'Analytics', mode: 'analytics' as const, navigable: true },
      { id: 'subview', label: 'Cost Report', navigable: false },
    ];

    render(<Breadcrumb />);

    const analyticsButton = screen.getByText('Analytics').closest('button')!;
    fireEvent.click(analyticsButton);

    expect(mockNavigateToBreadcrumb).toHaveBeenCalledWith('mode-analytics');
  });

  it('renders separators between breadcrumb items', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-simple', label: 'Simple', mode: 'simple' as const, navigable: true },
    ];

    const { container } = render(<Breadcrumb />);

    // ChevronRightIcon should be rendered as separator (it has aria-hidden="true")
    const separators = container.querySelectorAll('[aria-hidden="true"]');
    expect(separators.length).toBeGreaterThanOrEqual(1);
  });

  it('marks the last item with aria-current="page"', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-expert', label: 'Expert', mode: 'expert' as const, navigable: true },
    ];

    render(<Breadcrumb />);

    const lastItem = screen.getByText('Expert');
    expect(lastItem.closest('[aria-current="page"]')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
    ];

    const { container } = render(<Breadcrumb className="custom-class" />);

    expect(container.querySelector('.custom-class')).toBeInTheDocument();
  });

  it('renders home icon on the first breadcrumb item', () => {
    mockBreadcrumbs = [
      { id: 'home', label: 'Plan Cascade', navigable: true },
      { id: 'mode-simple', label: 'Simple', mode: 'simple' as const, navigable: true },
    ];

    const { container } = render(<Breadcrumb />);

    // The HomeIcon from Radix should be in the first item
    // Check that the first list item contains an SVG element (the icon)
    const firstItem = container.querySelector('li');
    expect(firstItem).toBeInTheDocument();
    const svg = firstItem?.querySelector('svg');
    expect(svg).toBeInTheDocument();
  });
});
