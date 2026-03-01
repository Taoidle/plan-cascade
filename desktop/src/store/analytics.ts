/**
 * Analytics Store
 *
 * Manages usage analytics state for the dashboard.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// Shared command envelope
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// Core usage stats
export interface UsageStats {
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_microdollars: number;
  request_count: number;
  avg_tokens_per_request: number;
  avg_cost_per_request: number;
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

export type CostStatus = 'exact' | 'estimated' | 'missing';

export interface DashboardFilterV2 {
  start_timestamp?: number;
  end_timestamp?: number;
  model_name?: string;
  provider?: string;
  session_id?: string;
  project_id?: string;
  cost_status?: CostStatus;
}

export interface UsageRecord {
  event_id: string;
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
  pricing_rule_id: string | null;
  currency: string;
  cost_status: CostStatus;
  cost_breakdown_json: string | null;
}

export interface PricingRule {
  id: string;
  provider: string;
  model_pattern: string;
  currency: string;
  input_per_million: number;
  output_per_million: number;
  cache_read_per_million: number;
  cache_write_per_million: number;
  thinking_per_million: number;
  effective_from: number;
  effective_to: number | null;
  status: 'active' | 'disabled';
  created_at: number;
  updated_at: number;
  note: string | null;
}

export type AggregationPeriod = 'hourly' | 'daily' | 'weekly' | 'monthly';
export type ExportFormat = 'csv' | 'json';

export interface ExportResult {
  data: string;
  record_count: number;
  summary: UsageStats | null;
  suggested_filename: string;
}

export type ExportJobStatus = 'running' | 'completed' | 'failed';

export interface ExportJob {
  id: string;
  status: ExportJobStatus;
  file_path: string | null;
  record_count: number;
  error: string | null;
}

export interface RecomputeCostsResult {
  scanned_records: number;
  recomputed_records: number;
  exact_records: number;
  estimated_records: number;
  missing_records: number;
}

// Period presets
export type PeriodPreset = 'last7days' | 'last30days' | 'last90days' | 'custom';

interface AnalyticsState {
  // Data
  summary: DashboardSummary | null;
  records: UsageRecord[];
  totalRecords: number;
  pricingRules: PricingRule[];

  // Loading states (split)
  summaryLoading: boolean;
  recordsLoading: boolean;
  pricingLoading: boolean;
  exportLoading: boolean;

  // Backward-compatible loading fields
  isLoading: boolean;
  isExporting: boolean;

  // Filter state
  filter: DashboardFilterV2;
  period: AggregationPeriod;
  periodPreset: PeriodPreset;

  // Error
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  fetchDashboardSummary: () => Promise<void>;
  fetchRecords: (limit?: number, offset?: number) => Promise<void>;
  fetchPricing: () => Promise<void>;

  setFilter: (filter: DashboardFilterV2) => void;
  setPeriod: (period: AggregationPeriod) => void;
  setPeriodPreset: (preset: PeriodPreset) => void;
  setDateRange: (start: Date, end: Date) => void;

  // Export (v1 compatibility + v2 streaming)
  exportData: (format: ExportFormat, includeSummary: boolean) => Promise<ExportResult | null>;
  exportByModel: (format: ExportFormat) => Promise<string | null>;
  exportByProject: (format: ExportFormat) => Promise<string | null>;
  exportStreamingJob: (format: ExportFormat, includeSummary: boolean, filePath?: string) => Promise<ExportJob | null>;

  // Pricing rule management (v2)
  upsertPricingRule: (rule: PricingRule) => Promise<PricingRule | null>;
  deletePricingRule: (ruleId: string) => Promise<boolean>;
  recomputeCosts: () => Promise<RecomputeCostsResult | null>;

  clearError: () => void;
}

function refreshCompositeLoading(state: Pick<AnalyticsState, 'summaryLoading' | 'recordsLoading' | 'pricingLoading'>) {
  return state.summaryLoading || state.recordsLoading || state.pricingLoading;
}

// Helper to convert preset to filter
function presetToFilter(preset: PeriodPreset): DashboardFilterV2 {
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

function csvEscape(value: string): string {
  if (value.includes(',') || value.includes('"') || value.includes('\n')) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function analyticsFilename(prefix: string, format: ExportFormat): string {
  const now = new Date();
  const pad = (v: number) => String(v).padStart(2, '0');
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `${prefix}_${stamp}.${format}`;
}

export const useAnalyticsStore = create<AnalyticsState>((set, get) => ({
  // Initial state
  summary: null,
  records: [],
  totalRecords: 0,
  pricingRules: [],

  summaryLoading: false,
  recordsLoading: false,
  pricingLoading: false,
  exportLoading: false,

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
      console.warn('Analytics initialization deferred:', error);
    }
  },

  // Fetch dashboard summary (v2)
  fetchDashboardSummary: async () => {
    set((state) => ({
      summaryLoading: true,
      isLoading: refreshCompositeLoading({ ...state, summaryLoading: true }),
      error: null,
    }));
    try {
      const { filter, period } = get();
      const response = await invoke<CommandResponse<DashboardSummary>>('get_dashboard_summary_v2', {
        filter,
        period,
      });

      if (response.success && response.data) {
        set((state) => ({
          summary: response.data,
          summaryLoading: false,
          isLoading: refreshCompositeLoading({ ...state, summaryLoading: false }),
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch dashboard data',
          summaryLoading: false,
          isLoading: refreshCompositeLoading({ ...state, summaryLoading: false }),
        }));
      }
    } catch (error) {
      set((state) => ({
        error: String(error),
        summaryLoading: false,
        isLoading: refreshCompositeLoading({ ...state, summaryLoading: false }),
      }));
    }
  },

  // Fetch usage records + total count (v2)
  fetchRecords: async (limit = 100, offset = 0) => {
    set((state) => ({
      recordsLoading: true,
      isLoading: refreshCompositeLoading({ ...state, recordsLoading: true }),
      error: null,
    }));
    try {
      const { filter } = get();

      const [recordsResp, countResp] = await Promise.all([
        invoke<CommandResponse<UsageRecord[]>>('list_usage_records_v2', {
          filter,
          limit,
          offset,
        }),
        invoke<CommandResponse<number>>('count_usage_records_v2', {
          filter,
        }),
      ]);

      if (recordsResp.success && recordsResp.data) {
        const rows = recordsResp.data ?? [];
        set((state) => ({
          records: rows,
          totalRecords: countResp.success && countResp.data !== null ? countResp.data : state.totalRecords,
          recordsLoading: false,
          isLoading: refreshCompositeLoading({ ...state, recordsLoading: false }),
          error: countResp.success ? state.error : countResp.error || state.error,
        }));
      } else {
        set((state) => ({
          error: recordsResp.error || 'Failed to fetch records',
          recordsLoading: false,
          isLoading: refreshCompositeLoading({ ...state, recordsLoading: false }),
        }));
      }
    } catch (error) {
      set((state) => ({
        error: String(error),
        recordsLoading: false,
        isLoading: refreshCompositeLoading({ ...state, recordsLoading: false }),
      }));
    }
  },

  // Fetch pricing rules (v2)
  fetchPricing: async () => {
    set((state) => ({
      pricingLoading: true,
      isLoading: refreshCompositeLoading({ ...state, pricingLoading: true }),
    }));
    try {
      const response = await invoke<CommandResponse<PricingRule[]>>('list_pricing_rules');
      if (response.success && response.data) {
        const rules = response.data ?? [];
        set((state) => ({
          pricingRules: rules,
          pricingLoading: false,
          isLoading: refreshCompositeLoading({ ...state, pricingLoading: false }),
        }));
      } else {
        set((state) => ({
          error: response.error || state.error,
          pricingLoading: false,
          isLoading: refreshCompositeLoading({ ...state, pricingLoading: false }),
        }));
      }
    } catch (error) {
      set((state) => ({
        error: String(error),
        pricingLoading: false,
        isLoading: refreshCompositeLoading({ ...state, pricingLoading: false }),
      }));
    }
  },

  setFilter: (filter) => {
    set({ filter, periodPreset: 'custom' });
  },

  setPeriod: (period) => {
    set({ period });
  },

  setPeriodPreset: (preset) => {
    const filter = presetToFilter(preset);
    set({ periodPreset: preset, filter });
  },

  // Inclusive end date (UI chooses day, backend uses end-exclusive timestamp)
  setDateRange: (start, end) => {
    const startBoundary = new Date(start.getFullYear(), start.getMonth(), start.getDate(), 0, 0, 0, 0);
    const endExclusive = new Date(end.getFullYear(), end.getMonth(), end.getDate() + 1, 0, 0, 0, 0);
    set({
      filter: {
        ...get().filter,
        start_timestamp: Math.floor(startBoundary.getTime() / 1000),
        end_timestamp: Math.floor(endExclusive.getTime() / 1000),
      },
      periodPreset: 'custom',
    });
  },

  // Compatibility export API now built on v2 data (no v1 backend dependency)
  exportData: async (format, includeSummary) => {
    set({ exportLoading: true, isExporting: true });
    try {
      const { filter, period } = get();
      const pageSize = 2000;
      let offset = 0;
      const rows: UsageRecord[] = [];

      while (true) {
        const resp = await invoke<CommandResponse<UsageRecord[]>>('list_usage_records_v2', {
          filter,
          limit: pageSize,
          offset,
        });
        if (!resp.success || !resp.data) {
          set({ exportLoading: false, isExporting: false, error: resp.error || 'Export failed' });
          return null;
        }
        rows.push(...resp.data);
        if (resp.data.length < pageSize) {
          break;
        }
        offset += pageSize;
      }

      let summary: UsageStats | null = null;
      if (includeSummary) {
        const summaryResp = await invoke<CommandResponse<DashboardSummary>>('get_dashboard_summary_v2', {
          filter,
          period,
        });
        if (summaryResp.success && summaryResp.data) {
          summary = summaryResp.data.current_period;
        }
      }

      let data = '';
      if (format === 'json') {
        data = JSON.stringify(
          {
            exported_at: new Date().toISOString(),
            record_count: rows.length,
            summary,
            records: rows,
          },
          null,
          2,
        );
      } else {
        const header =
          'event_id,session_id,project_id,provider,model,input_tokens,output_tokens,thinking_tokens,cache_read_tokens,cache_write_tokens,cost_microdollars,currency,cost_status,timestamp,metadata_json';
        const lines = rows.map((r) =>
          [
            csvEscape(r.event_id),
            csvEscape(r.session_id || ''),
            csvEscape(r.project_id || ''),
            csvEscape(r.provider),
            csvEscape(r.model_name),
            r.input_tokens,
            r.output_tokens,
            r.thinking_tokens,
            r.cache_read_tokens,
            r.cache_creation_tokens,
            r.cost_microdollars,
            csvEscape(r.currency),
            r.cost_status,
            r.timestamp,
            csvEscape(r.metadata || ''),
          ].join(','),
        );
        data = [header, ...lines].join('\n');
        if (summary) {
          data += `\n\n# summary\n# request_count=${summary.request_count}\n# total_input_tokens=${summary.total_input_tokens}\n# total_output_tokens=${summary.total_output_tokens}\n# total_cost_microdollars=${summary.total_cost_microdollars}\n`;
        }
      }

      set({ exportLoading: false, isExporting: false });
      return {
        data,
        record_count: rows.length,
        summary,
        suggested_filename: analyticsFilename('analytics_records', format),
      };
    } catch (error) {
      set({ error: String(error), exportLoading: false, isExporting: false });
      return null;
    }
  },

  exportByModel: async (format) => {
    set({ exportLoading: true, isExporting: true });
    try {
      const { filter, period } = get();
      const response = await invoke<CommandResponse<DashboardSummary>>('get_dashboard_summary_v2', {
        filter,
        period,
      });

      set({ exportLoading: false, isExporting: false });
      if (response.success && response.data) {
        const rows = response.data.by_model;
        if (format === 'json') {
          return JSON.stringify(rows, null, 2);
        }

        const header =
          'model_name,provider,total_input_tokens,total_output_tokens,total_cost_microdollars,request_count,avg_tokens_per_request,avg_cost_per_request';
        const lines = rows.map((r) =>
          [
            csvEscape(r.model_name),
            csvEscape(r.provider),
            r.stats.total_input_tokens,
            r.stats.total_output_tokens,
            r.stats.total_cost_microdollars,
            r.stats.request_count,
            r.stats.avg_tokens_per_request.toFixed(4),
            r.stats.avg_cost_per_request.toFixed(4),
          ].join(','),
        );
        return [header, ...lines].join('\n');
      }
      set({ error: response.error || 'Export failed' });
      return null;
    } catch (error) {
      set({ error: String(error), exportLoading: false, isExporting: false });
      return null;
    }
  },

  exportByProject: async (format) => {
    set({ exportLoading: true, isExporting: true });
    try {
      const { filter, period } = get();
      const response = await invoke<CommandResponse<DashboardSummary>>('get_dashboard_summary_v2', {
        filter,
        period,
      });

      set({ exportLoading: false, isExporting: false });
      if (response.success && response.data) {
        const rows = response.data.by_project;
        if (format === 'json') {
          return JSON.stringify(rows, null, 2);
        }

        const header =
          'project_id,project_name,total_input_tokens,total_output_tokens,total_cost_microdollars,request_count,avg_tokens_per_request,avg_cost_per_request';
        const lines = rows.map((r) =>
          [
            csvEscape(r.project_id),
            csvEscape(r.project_name || ''),
            r.stats.total_input_tokens,
            r.stats.total_output_tokens,
            r.stats.total_cost_microdollars,
            r.stats.request_count,
            r.stats.avg_tokens_per_request.toFixed(4),
            r.stats.avg_cost_per_request.toFixed(4),
          ].join(','),
        );
        return [header, ...lines].join('\n');
      }
      set({ error: response.error || 'Export failed' });
      return null;
    } catch (error) {
      set({ error: String(error), exportLoading: false, isExporting: false });
      return null;
    }
  },

  // v2 streaming export job (writes local file on backend)
  exportStreamingJob: async (format, includeSummary, filePath) => {
    set({ exportLoading: true, isExporting: true });
    try {
      const { filter } = get();
      const response = await invoke<CommandResponse<ExportJob>>('export_usage_streaming_job', {
        request: {
          filter,
          format,
          include_summary: includeSummary,
          file_path: filePath,
        },
      });
      set({ exportLoading: false, isExporting: false });

      if (response.success && response.data) {
        return response.data;
      }
      set({ error: response.error || 'Export failed' });
      return null;
    } catch (error) {
      set({ error: String(error), exportLoading: false, isExporting: false });
      return null;
    }
  },

  upsertPricingRule: async (rule) => {
    try {
      const response = await invoke<CommandResponse<PricingRule>>('upsert_pricing_rule', { rule });
      if (response.success && response.data) {
        await get().fetchPricing();
        return response.data;
      }
      set({ error: response.error || 'Failed to save pricing rule' });
      return null;
    } catch (error) {
      set({ error: String(error) });
      return null;
    }
  },

  deletePricingRule: async (ruleId) => {
    try {
      const response = await invoke<CommandResponse<boolean>>('delete_pricing_rule', { ruleId });
      if (response.success && response.data) {
        await get().fetchPricing();
        return true;
      }
      if (!response.success) {
        set({ error: response.error || 'Failed to delete pricing rule' });
      }
      return false;
    } catch (error) {
      set({ error: String(error) });
      return false;
    }
  },

  recomputeCosts: async () => {
    try {
      const response = await invoke<CommandResponse<RecomputeCostsResult>>('recompute_costs', {
        request: { filter: get().filter },
      });
      if (response.success && response.data) {
        await Promise.all([get().fetchDashboardSummary(), get().fetchRecords(), get().fetchPricing()]);
        return response.data;
      }
      set({ error: response.error || 'Failed to recompute costs' });
      return null;
    } catch (error) {
      set({ error: String(error) });
      return null;
    }
  },

  clearError: () => {
    set({ error: null });
  },
}));

export default useAnalyticsStore;
