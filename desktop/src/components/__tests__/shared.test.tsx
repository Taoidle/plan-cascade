/**
 * Shared Component Tests
 *
 * Tests for Skeleton, ContextualActions, and ShortcutOverlay components.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { Skeleton, SkeletonGroup, SettingsSkeleton, ListItemSkeleton, TableSkeleton } from '../shared/Skeleton';
import { ContextualActions } from '../shared/ContextualActions';
import { ShortcutOverlay } from '../shared/ShortcutOverlay';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

// Mock for ContextualActions
let mockModeStoreState = {
  mode: 'simple' as string,
};

let mockExecutionStoreState = {
  status: 'idle' as string,
};

vi.mock('../../store/mode', () => ({
  useModeStore: () => mockModeStoreState,
}));

vi.mock('../../store/execution', () => ({
  useExecutionStore: () => mockExecutionStoreState,
}));

// Mock react-hotkeys-hook
vi.mock('react-hotkeys-hook', () => ({
  useHotkeys: vi.fn(),
}));

// --------------------------------------------------------------------------
// Skeleton Tests
// --------------------------------------------------------------------------

describe('Skeleton', () => {
  it('renders a rect skeleton by default', () => {
    const { container } = render(<Skeleton />);

    const skeleton = container.firstChild as HTMLElement;
    expect(skeleton).toBeInTheDocument();
    expect(skeleton).toHaveAttribute('aria-hidden', 'true');
  });

  it('renders with custom width and height', () => {
    const { container } = render(<Skeleton width="200px" height="2rem" />);

    const skeleton = container.firstChild as HTMLElement;
    expect(skeleton).toHaveStyle({ width: '200px', height: '2rem' });
  });

  it('applies rounded classes based on rounded prop', () => {
    const { container } = render(<Skeleton rounded="xl" />);

    expect(container.firstChild).toHaveClass('rounded-xl');
  });

  it('renders circle variant with correct size', () => {
    const { container } = render(<Skeleton variant="circle" size="3rem" />);

    const skeleton = container.firstChild as HTMLElement;
    expect(skeleton).toHaveStyle({ width: '3rem', height: '3rem' });
    expect(skeleton).toHaveClass('rounded-full');
  });

  it('renders text variant with correct number of lines', () => {
    const { container } = render(<Skeleton variant="text" lines={4} />);

    // The wrapper div contains lines
    const lines = container.querySelectorAll('.h-4');
    expect(lines.length).toBe(4);
  });

  it('renders text variant with shorter last line', () => {
    const { container } = render(<Skeleton variant="text" lines={3} lastLineWidth="50%" />);

    const lines = container.querySelectorAll('.h-4');
    const lastLine = lines[lines.length - 1] as HTMLElement;
    expect(lastLine).toHaveStyle({ width: '50%' });
  });

  it('renders card variant with title and body lines', () => {
    const { container } = render(<Skeleton variant="card" lines={3} />);

    // Card should have border
    const card = container.firstChild as HTMLElement;
    expect(card).toHaveAttribute('aria-hidden', 'true');
    // Should contain title bar and body lines
    const innerElements = card.querySelectorAll('[aria-hidden="true"]');
    // Title (h-5) + 3 body lines
    expect(innerElements.length).toBeGreaterThanOrEqual(3);
  });

  it('renders card variant with image area when showImage is true', () => {
    const { container } = render(<Skeleton variant="card" showImage={true} />);

    // Should have the image placeholder (h-40)
    const imageArea = container.querySelector('.h-40');
    expect(imageArea).toBeInTheDocument();
  });

  it('renders button variant with correct size class', () => {
    const { container } = render(<Skeleton variant="button" size="lg" />);

    expect(container.firstChild).toHaveClass('h-12', 'w-32');
  });

  it('renders avatar variant with correct dimensions', () => {
    const { container } = render(<Skeleton variant="avatar" size="lg" />);

    const avatar = container.firstChild as HTMLElement;
    expect(avatar).toHaveStyle({ width: '3rem', height: '3rem' });
    expect(avatar).toHaveClass('rounded-full');
  });

  it('renders badge variant', () => {
    const { container } = render(<Skeleton variant="badge" />);

    expect(container.firstChild).toHaveClass('h-5', 'w-16', 'rounded-full');
  });

  it('disables animation when animate is false', () => {
    const { container } = render(<Skeleton animate={false} />);

    expect(container.firstChild).not.toHaveClass('animate-skeleton');
  });

  it('applies custom className', () => {
    const { container } = render(<Skeleton className="my-custom-class" />);

    expect(container.firstChild).toHaveClass('my-custom-class');
  });
});

describe('SkeletonGroup', () => {
  it('renders the correct number of children', () => {
    render(
      <SkeletonGroup count={5}>
        {(i) => (
          <div key={i} data-testid={`item-${i}`}>
            Item {i}
          </div>
        )}
      </SkeletonGroup>,
    );

    for (let i = 0; i < 5; i++) {
      expect(screen.getByTestId(`item-${i}`)).toBeInTheDocument();
    }
  });

  it('applies container className', () => {
    const { container } = render(
      <SkeletonGroup count={2} className="gap-4">
        {(i) => <div key={i}>Item</div>}
      </SkeletonGroup>,
    );

    expect(container.firstChild).toHaveClass('gap-4');
  });

  it('is hidden from assistive technology', () => {
    const { container } = render(<SkeletonGroup count={1}>{(i) => <div key={i}>Item</div>}</SkeletonGroup>);

    expect(container.firstChild).toHaveAttribute('aria-hidden', 'true');
  });
});

describe('SettingsSkeleton', () => {
  it('renders three settings sections', () => {
    const { container } = render(<SettingsSkeleton />);

    expect(container.firstChild).toHaveAttribute('aria-hidden', 'true');
    // Should have 3 groups (each with label, input, description)
    const groups = container.querySelectorAll('.space-y-2');
    expect(groups.length).toBe(3);
  });
});

describe('ListItemSkeleton', () => {
  it('renders with circle avatar, text lines, and badge', () => {
    const { container } = render(<ListItemSkeleton />);

    expect(container.firstChild).toHaveAttribute('aria-hidden', 'true');
    // Should contain a circle (avatar), text lines, and a badge
    const circles = container.querySelectorAll('.rounded-full');
    expect(circles.length).toBeGreaterThanOrEqual(1);
  });
});

describe('TableSkeleton', () => {
  it('renders correct number of rows and columns', () => {
    const { container } = render(<TableSkeleton rows={3} cols={4} />);

    expect(container.firstChild).toHaveAttribute('aria-hidden', 'true');
    // Header + 3 rows = 4 flex containers
    const rowDivs = container.querySelectorAll('.flex.gap-4');
    expect(rowDivs.length).toBe(4); // 1 header + 3 data rows
  });
});

// --------------------------------------------------------------------------
// ContextualActions Tests
// --------------------------------------------------------------------------

describe('ContextualActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockModeStoreState = { mode: 'simple' };
    mockExecutionStoreState = { status: 'idle' };
  });

  it('renders nothing when no actions are applicable', () => {
    mockModeStoreState = { mode: 'simple' };
    mockExecutionStoreState = { status: 'idle' };

    const { container } = render(<ContextualActions />);

    // No callbacks provided, so no actions should render
    expect(container.firstChild).toBeNull();
  });
});

// --------------------------------------------------------------------------
// ShortcutOverlay Tests
// --------------------------------------------------------------------------

describe('ShortcutOverlay', () => {
  it('renders nothing when isOpen is false', () => {
    const { container } = render(<ShortcutOverlay isOpen={false} />);

    expect(container.querySelector('[role="dialog"]')).not.toBeInTheDocument();
  });
});
