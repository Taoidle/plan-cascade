/**
 * Analytics Store
 *
 * Manages usage analytics state for the dashboard.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// Types matching Rust models
export interface UsageStats {
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_microdollars: number;
  request_count: number;
  avg_tokens_per_request: number;
  avg_cost_per_request: number;
}

export interface UsageRecord {
  id: number;
  session_id: string | null;
  project_id: string | null;
  model_name: string;
  provider: string;
  input_tokens: number;
  output_tokens: number;
  thinking_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  cost_microdollars: number;
  timestamp: number;
  metadata: string | null;
}

export interface ModelUsage {
  model_name: string;
  provider: string;
  stats: UsageStats;
}

export interface ProjectUsage {
  project_id: string;
  project_name: string | null;
  stats: UsageStats;
}

export interface TimeSeriesPoint {
  timestamp: number;
  timestamp_formatted: string;
  stats: UsageStats;
}

export interface DashboardSummary {
  current_period: UsageStats;
  previous_period: UsageStats;
  cost_change_percent: number;
  tokens_change_percent: number;
  requests_change_percent: number;
  by_model: ModelUsage[];
  by_project: ProjectUsage[];
  time_series: TimeSeriesPoint[];
}

export interface ModelPricing {
  id: number;
  model_name: string;
  provider: string;
  input_price_per_million: number;
  output_price_per_million: number;
  is_custom: boolean;
  updated_at: number;
}

export interface UsageFilter {
  start_timestamp?: number;
  end_timestamp?: number;
  model_name?: string;
  provider?: string;
  session_id?: string;
  project_id?: string;
}

export type AggregationPeriod = 'hourly' | 'daily' | 'weekly' | 'monthly';

export type ExportFormat = 'csv' | 'json';

export interface ExportRequest {
  filter: UsageFilter;
  format: ExportFormat;
  include_summary: boolean;
}

export interface ExportResult {
  data: string;
  record_count: number;
  summary: UsageStats | null;
  suggested_filename: string;
}

// Period presets
export type PeriodPreset = 'last7days' | 'last30days' | 'last90days' | 'custom';

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface AnalyticsState {
  // Data
  summary: DashboardSummary | null;
  records: UsageRecord[];
  pricing: ModelPricing[];

  // Loading states
  isLoading: boolean;
  isExporting: boolean;

  // Filter state
  filter: UsageFilter;
  period: AggregationPeriod;
  periodPreset: PeriodPreset;

  // Error
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  fetchDashboardSummary: () => Promise<void>;
  fetchRecords: (limit?: number, offset?: number) => Promise<void>;
  fetchPricing: () => Promise<void>;

  setFilter: (filter: UsageFilter) => void;
  setPeriod: (period: AggregationPeriod) => void;
  setPeriodPreset: (preset: PeriodPreset) => void;
  setDateRange: (start: Date, end: Date) => void;

  exportData: (format: ExportFormat, includeSummary: boolean) => Promise<ExportResult | null>;
  exportByModel: (format: ExportFormat) => Promise<string | null>;
  exportByProject: (format: ExportFormat) => Promise<string | null>;

  setCustomPricing: (pricing: ModelPricing) => Promise<boolean>;
  removeCustomPricing: (provider: string, modelName: string) => Promise<boolean>;

  clearError: () => void;
}

// Helper to convert preset to filter
function presetToFilter(preset: PeriodPreset): UsageFilter {
  const now = Math.floor(Date.now() / 1000);
  const day = 24 * 60 * 60;

  switch (preset) {
    case 'last7days':
      return { start_timestamp: now - 7 * day, end_timestamp: now };
    case 'last30days':
      return { start_timestamp: now - 30 * day, end_timestamp: now };
    case 'last90days':
      return { start_timestamp: now - 90 * day, end_timestamp: now };
    case 'custom':
    default:
      return {};
  }
}

// Helper to format cost
export function formatCost(microdollars: number): string {
  const dollars = microdollars / 1_000_000;
  if (dollars < 0.01) {
    return `$${dollars.toFixed(6)}`;
  } else if (dollars < 1) {
    return `$${dollars.toFixed(4)}`;
  } else if (dollars < 100) {
    return `$${dollars.toFixed(2)}`;
  } else {
    return `$${dollars.toFixed(0)}`;
  }
}

// Helper to format tokens
export function formatTokens(tokens: number): string {
  if (tokens < 1000) {
    return tokens.toString();
  } else if (tokens < 1_000_000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  } else {
    return `${(tokens / 1_000_000).toFixed(2)}M`;
  }
}

// Helper to format change percentage
export function formatChange(percent: number): string {
  const sign = percent >= 0 ? '+' : '';
  return `${sign}${percent.toFixed(1)}%`;
}

export const useAnalyticsStore = create<AnalyticsState>((set, get) => ({
  // Initial state
  summary: null,
  records: [],
  pricing: [],
  isLoading: false,
  isExporting: false,
  filter: presetToFilter('last30days'),
  period: 'daily',
  periodPreset: 'last30days',
  error: null,

  // Initialize analytics
  initialize: async () => {
    try {
      const response = await invoke<CommandResponse<boolean>>('init_analytics');
      if (!response.success) {
        console.warn('Analytics initialization deferred:', response.error);
      }
    } catch (error) {
      // Backend may not be ready yet; analytics will retry on next interaction
      console.warn('Analytics initialization deferred:', error);
    }
  },

  // Fetch dashboard summary
  fetchDashboardSummary: async () => {
    set({ isLoading: true, error: null });
    try {
      const { filter, period } = get();
      const response = await invoke<CommandResponse<DashboardSummary>>('get_dashboard_summary', {
        filter,
        period,
      });

      if (response.success && response.data) {
        set({ summary: response.data, isLoading: false });
      } else {
        set({ error: response.error || 'Failed to fetch dashboard data', isLoading: false });
      }
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  // Fetch usage records
  fetchRecords: async (limit = 100, offset = 0) => {
    set({ isLoading: true, error: null });
    try {
      const { filter } = get();
      const response = await invoke<CommandResponse<UsageRecord[]>>('list_usage_records', {
        filter,
        limit,
        offset,
      });

      if (response.success && response.data) {
        set({ records: response.data, isLoading: false });
      } else {
        set({ error: response.error || 'Failed to fetch records', isLoading: false });
      }
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  // Fetch pricing
  fetchPricing: async () => {
    try {
      const response = await invoke<CommandResponse<ModelPricing[]>>('list_model_pricing');
      if (response.success && response.data) {
        set({ pricing: response.data });
      }
    } catch (error) {
      console.error('Failed to fetch pricing:', error);
    }
  },

  // Set filter
  setFilter: (filter) => {
    set({ filter, periodPreset: 'custom' });
  },

  // Set aggregation period
  setPeriod: (period) => {
    set({ period });
  },

  // Set period preset
  setPeriodPreset: (preset) => {
    const filter = presetToFilter(preset);
    set({ periodPreset: preset, filter });
  },

  // Set custom date range
  setDateRange: (start, end) => {
    set({
      filter: {
        ...get().filter,
        start_timestamp: Math.floor(start.getTime() / 1000),
        end_timestamp: Math.floor(end.getTime() / 1000),
      },
      periodPreset: 'custom',
    });
  },

  // Export data
  exportData: async (format, includeSummary) => {
    set({ isExporting: true });
    try {
      const { filter } = get();
      const request: ExportRequest = {
        filter,
        format,
        include_summary: includeSummary,
      };

      const response = await invoke<CommandResponse<ExportResult>>('export_usage', { request });

      set({ isExporting: false });

      if (response.success && response.data) {
        return response.data;
      } else {
        set({ error: response.error || 'Export failed' });
        return null;
      }
    } catch (error) {
      set({ error: String(error), isExporting: false });
      return null;
    }
  },

  // Export by model
  exportByModel: async (format) => {
    set({ isExporting: true });
    try {
      const { filter } = get();
      const response = await invoke<CommandResponse<string>>('export_by_model', { filter, format });

      set({ isExporting: false });

      if (response.success && response.data) {
        return response.data;
      } else {
        set({ error: response.error || 'Export failed' });
        return null;
      }
    } catch (error) {
      set({ error: String(error), isExporting: false });
      return null;
    }
  },

  // Export by project
  exportByProject: async (format) => {
    set({ isExporting: true });
    try {
      const { filter } = get();
      const response = await invoke<CommandResponse<string>>('export_by_project', { filter, format });

      set({ isExporting: false });

      if (response.success && response.data) {
        return response.data;
      } else {
        set({ error: response.error || 'Export failed' });
        return null;
      }
    } catch (error) {
      set({ error: String(error), isExporting: false });
      return null;
    }
  },

  // Set custom pricing
  setCustomPricing: async (pricing) => {
    try {
      const response = await invoke<CommandResponse<boolean>>('set_custom_pricing', { pricing });
      if (response.success) {
        await get().fetchPricing();
        return true;
      }
      return false;
    } catch (error) {
      console.error('Failed to set custom pricing:', error);
      return false;
    }
  },

  // Remove custom pricing
  removeCustomPricing: async (provider, modelName) => {
    try {
      const response = await invoke<CommandResponse<boolean>>('remove_custom_pricing', {
        provider,
        model_name: modelName,
      });
      if (response.success) {
        await get().fetchPricing();
        return true;
      }
      return false;
    } catch (error) {
      console.error('Failed to remove custom pricing:', error);
      return false;
    }
  },

  // Clear error
  clearError: () => {
    set({ error: null });
  },
}));

export default useAnalyticsStore;
