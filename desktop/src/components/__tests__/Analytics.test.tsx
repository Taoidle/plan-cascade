/**
 * Analytics Component Tests
 *
 * Tests the Dashboard rendering, OverviewCards display, and PeriodSelector.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { OverviewCards } from '../Analytics/OverviewCards';
import { Dashboard } from '../Analytics/Dashboard';
import { createMockDashboardSummary, createMockUsageStats } from './test-utils';
import type { DashboardSummary } from '../../store/analytics';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback || key,
  }),
}));

// Mock analytics store
const mockInitialize = vi.fn().mockResolvedValue(undefined);
const mockFetchDashboardSummary = vi.fn().mockResolvedValue(undefined);
const mockFetchPricing = vi.fn().mockResolvedValue(undefined);
const mockClearError = vi.fn();
const mockSetPeriodPreset = vi.fn();
const mockSetPeriod = vi.fn();
const mockSetDateRange = vi.fn();

let mockAnalyticsState = {
  summary: null as DashboardSummary | null,
  isLoading: false,
  error: null as string | null,
  initialize: mockInitialize,
  fetchDashboardSummary: mockFetchDashboardSummary,
  fetchPricing: mockFetchPricing,
  clearError: mockClearError,
  periodPreset: 'last30days' as string,
  period: 'daily' as string,
  setPeriodPreset: mockSetPeriodPreset,
  setPeriod: mockSetPeriod,
  setDateRange: mockSetDateRange,
};

vi.mock('../../store/analytics', () => ({
  useAnalyticsStore: () => mockAnalyticsState,
  formatCost: (microdollars: number) => {
    const dollars = microdollars / 1_000_000;
    if (dollars < 0.01) return `$${dollars.toFixed(6)}`;
    if (dollars < 1) return `$${dollars.toFixed(4)}`;
    if (dollars < 100) return `$${dollars.toFixed(2)}`;
    return `$${dollars.toFixed(0)}`;
  },
  formatTokens: (tokens: number) => {
    if (tokens < 1000) return tokens.toString();
    if (tokens < 1_000_000) return `${(tokens / 1000).toFixed(1)}K`;
    return `${(tokens / 1_000_000).toFixed(2)}M`;
  },
  formatChange: (percent: number) => {
    const sign = percent >= 0 ? '+' : '';
    return `${sign}${percent.toFixed(1)}%`;
  },
}));

// Mock Radix Select for PeriodSelector
vi.mock('@radix-ui/react-select', () => ({
  Root: ({ children, value }: { children: React.ReactNode; value: string; onValueChange: (v: string) => void }) => (
    <div data-testid="select-root" data-value={value}>
      {children}
    </div>
  ),
  Trigger: ({ children }: { children: React.ReactNode }) => <button data-testid="period-trigger">{children}</button>,
  Value: () => <span>Value</span>,
  Icon: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Portal: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Content: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Viewport: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Item: ({ children, value }: { children: React.ReactNode; value: string }) => (
    <div data-testid={`period-item-${value}`}>{children}</div>
  ),
  ItemText: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  ItemIndicator: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
}));

// Mock sub-components for Dashboard isolation
vi.mock('../Analytics/CostChart', () => ({
  CostChart: () => <div data-testid="cost-chart">Cost Chart</div>,
}));

vi.mock('../Analytics/TokenBreakdown', () => ({
  TokenBreakdown: () => <div data-testid="token-breakdown">Token Breakdown</div>,
}));

vi.mock('../Analytics/ExportDialog', () => ({
  ExportDialog: ({ open }: { open: boolean }) => (open ? <div data-testid="export-dialog">Export</div> : null),
}));

vi.mock('../Analytics/UsageTable', () => ({
  UsageTable: () => <div data-testid="usage-table">Usage Table</div>,
}));

vi.mock('../Analytics/AnalyticsSkeleton', () => ({
  default: () => <div data-testid="analytics-skeleton">Loading Skeleton</div>,
}));

// --------------------------------------------------------------------------
// OverviewCards Tests
// --------------------------------------------------------------------------

describe('OverviewCards', () => {
  const summary = createMockDashboardSummary();

  it('renders all four overview cards', () => {
    render(<OverviewCards summary={summary} />);

    expect(screen.getByText('Total Cost')).toBeInTheDocument();
    expect(screen.getByText('Total Tokens')).toBeInTheDocument();
    expect(screen.getByText('Total Requests')).toBeInTheDocument();
    expect(screen.getByText('Avg Cost/Request')).toBeInTheDocument();
  });

  it('displays formatted cost value', () => {
    render(<OverviewCards summary={summary} />);

    // $2.50 from 2,500,000 microdollars
    expect(screen.getByText('$2.50')).toBeInTheDocument();
  });

  it('displays formatted token count', () => {
    render(<OverviewCards summary={summary} />);

    // 150K + 50K = 200K tokens
    expect(screen.getByText('200.0K')).toBeInTheDocument();
  });

  it('displays request count', () => {
    render(<OverviewCards summary={summary} />);

    expect(screen.getByText('42')).toBeInTheDocument();
  });

  it('shows change indicators with percentages', () => {
    render(<OverviewCards summary={summary} />);

    // cost_change_percent is 25.0 - should show +25.0%
    expect(screen.getByText('+25.0%')).toBeInTheDocument();
  });

  it('applies loading animation when isLoading is true', () => {
    const { container } = render(<OverviewCards summary={summary} isLoading={true} />);

    const pulsing = container.querySelectorAll('.animate-pulse');
    expect(pulsing.length).toBeGreaterThan(0);
  });

  it('shows "---" for values when loading', () => {
    render(<OverviewCards summary={summary} isLoading={true} />);

    const dashes = screen.getAllByText('---');
    expect(dashes.length).toBe(4);
  });
});

// --------------------------------------------------------------------------
// Dashboard Tests
// --------------------------------------------------------------------------

describe('Dashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockAnalyticsState = {
      summary: null,
      isLoading: false,
      error: null,
      initialize: mockInitialize,
      fetchDashboardSummary: mockFetchDashboardSummary,
      fetchPricing: mockFetchPricing,
      clearError: mockClearError,
      periodPreset: 'last30days',
      period: 'daily',
      setPeriodPreset: mockSetPeriodPreset,
      setPeriod: mockSetPeriod,
      setDateRange: mockSetDateRange,
    };
  });

  it('renders dashboard header with title and subtitle', () => {
    render(<Dashboard />);

    expect(screen.getByText('Usage Analytics')).toBeInTheDocument();
    expect(screen.getByText('Track your API usage and costs')).toBeInTheDocument();
  });

  it('shows loading skeleton when loading and no summary', () => {
    mockAnalyticsState.isLoading = true;
    mockAnalyticsState.summary = null;

    render(<Dashboard />);

    expect(screen.getByTestId('analytics-skeleton')).toBeInTheDocument();
  });

  it('renders overview tab with cards and charts when summary is loaded', () => {
    mockAnalyticsState.summary = createMockDashboardSummary();

    render(<Dashboard />);

    expect(screen.getByText('Total Cost')).toBeInTheDocument();
    expect(screen.getByTestId('cost-chart')).toBeInTheDocument();
    expect(screen.getByTestId('token-breakdown')).toBeInTheDocument();
  });

  it('displays error message with retry button', () => {
    mockAnalyticsState.error = 'Failed to fetch analytics data';

    render(<Dashboard />);

    expect(screen.getByText('Failed to fetch analytics data')).toBeInTheDocument();
    expect(screen.getByText('Retry')).toBeInTheDocument();
  });

  it('calls clearError and fetchDashboardSummary on retry', () => {
    mockAnalyticsState.error = 'Network error';

    render(<Dashboard />);

    fireEvent.click(screen.getByText('Retry'));

    expect(mockClearError).toHaveBeenCalled();
    expect(mockFetchDashboardSummary).toHaveBeenCalled();
  });

  it('renders Overview and Detailed Records tabs', () => {
    render(<Dashboard />);

    expect(screen.getByText('Overview')).toBeInTheDocument();
    expect(screen.getByText('Detailed Records')).toBeInTheDocument();
  });

  it('switches to Detailed Records tab showing UsageTable', () => {
    mockAnalyticsState.summary = createMockDashboardSummary();

    render(<Dashboard />);

    fireEvent.click(screen.getByText('Detailed Records'));

    expect(screen.getByTestId('usage-table')).toBeInTheDocument();
  });

  it('shows Export button and opens export dialog', () => {
    render(<Dashboard />);

    const exportButton = screen.getByText('Export');
    expect(exportButton).toBeInTheDocument();

    fireEvent.click(exportButton);

    expect(screen.getByTestId('export-dialog')).toBeInTheDocument();
  });

  it('calls initialize, fetchDashboardSummary, and fetchPricing on mount', async () => {
    render(<Dashboard />);

    await waitFor(() => {
      expect(mockInitialize).toHaveBeenCalled();
      expect(mockFetchDashboardSummary).toHaveBeenCalled();
      expect(mockFetchPricing).toHaveBeenCalled();
    });
  });

  it('renders model table when summary has model data', () => {
    mockAnalyticsState.summary = createMockDashboardSummary({
      by_model: [
        {
          model_name: 'claude-sonnet-4-20250514',
          provider: 'anthropic',
          stats: createMockUsageStats(),
        },
      ],
    });

    render(<Dashboard />);

    expect(screen.getByText('Top Models by Cost')).toBeInTheDocument();
    expect(screen.getByText('claude-sonnet-4-20250514')).toBeInTheDocument();
    expect(screen.getByText('anthropic')).toBeInTheDocument();
  });
});
