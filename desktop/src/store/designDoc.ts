/**
 * Design Document Store
 *
 * Manages design document state for the Expert Mode DesignDocPanel.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types (matching Rust serialization)
// ============================================================================

/** Design document level */
export type DesignDocLevel = 'project' | 'feature';

/** Decision status enum */
export type DecisionStatus = 'proposed' | 'accepted' | 'deprecated' | 'superseded';

/** Design document metadata */
export interface DesignDocMetadata {
  created_at: string | null;
  version: string;
  source: string | null;
  level: DesignDocLevel;
  mega_plan_reference: string | null;
}

/** Overview section */
export interface Overview {
  title: string;
  summary: string;
  goals: string[];
  non_goals: string[];
}

/** Architecture component */
export interface DesignComponent {
  name: string;
  description: string;
  responsibilities: string[];
  dependencies: string[];
  features: string[];
}

/** Design pattern */
export interface DesignPattern {
  name: string;
  description: string;
  rationale: string;
  applies_to: string[];
}

/** Infrastructure info */
export interface Infrastructure {
  existing_services: string[];
  new_services: string[];
}

/** Architecture section */
export interface Architecture {
  system_overview: string;
  components: DesignComponent[];
  data_flow: string;
  patterns: DesignPattern[];
  infrastructure: Infrastructure;
}

/** API standards */
export interface ApiStandards {
  style: string;
  error_handling: string;
  async_pattern: string;
}

/** Shared data model */
export interface SharedDataModel {
  name: string;
  location: string;
  description: string | null;
  changes: string | null;
}

/** Interfaces section */
export interface Interfaces {
  api_standards: ApiStandards;
  shared_data_models: SharedDataModel[];
}

/** Architecture Decision Record */
export interface DesignDecision {
  id: string;
  title: string;
  context: string;
  decision: string;
  rationale: string;
  alternatives_considered: string[];
  status: DecisionStatus;
  applies_to: string[];
}

/** Feature mapping */
export interface FeatureMapping {
  components: string[];
  patterns: string[];
  decisions: string[];
  description: string;
}

/** Complete design document */
export interface DesignDoc {
  metadata: DesignDocMetadata;
  overview: Overview;
  architecture: Architecture;
  interfaces: Interfaces;
  decisions: DesignDecision[];
  feature_mappings: Record<string, FeatureMapping>;
}

/** Generation info metadata */
export interface GenerationInfo {
  stories_processed: number;
  components_generated: number;
  patterns_identified: number;
  decisions_created: number;
  feature_mappings_created: number;
}

/** Result from generate_design_doc command */
export interface GenerateResult {
  design_doc: DesignDoc;
  saved_path: string | null;
  generation_info: GenerationInfo;
}

/** Import warning */
export interface ImportWarning {
  message: string;
  field: string | null;
  severity: 'info' | 'low' | 'medium' | 'high';
}

/** Result from import_design_doc command */
export interface ImportResult {
  design_doc: DesignDoc;
  warnings: ImportWarning[];
  source_format: 'markdown' | 'json';
  clean_import: boolean;
}

/** Generation options */
export interface GenerateOptions {
  level?: DesignDocLevel;
  mega_plan_reference?: string;
  additional_context?: string;
}

/** Standard command response from Tauri */
interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// Store
// ============================================================================

interface DesignDocState {
  /** Current design document */
  designDoc: DesignDoc | null;

  /** Generation info (if generated) */
  generationInfo: GenerationInfo | null;

  /** Import warnings (if imported) */
  importWarnings: ImportWarning[] | null;

  /** Loading states */
  loading: {
    generating: boolean;
    importing: boolean;
    loading: boolean;
  };

  /** Error message */
  error: string | null;

  /** Actions */
  generateDesignDoc: (prdPath: string, options?: GenerateOptions) => Promise<DesignDoc | null>;
  importDesignDoc: (filePath: string, format?: string) => Promise<DesignDoc | null>;
  loadDesignDoc: (projectPath?: string) => Promise<DesignDoc | null>;
  reset: () => void;
  clearError: () => void;
}

export const useDesignDocStore = create<DesignDocState>((set) => ({
  designDoc: null,
  generationInfo: null,
  importWarnings: null,
  loading: {
    generating: false,
    importing: false,
    loading: false,
  },
  error: null,

  generateDesignDoc: async (prdPath: string, options?: GenerateOptions) => {
    set((state) => ({
      loading: { ...state.loading, generating: true },
      error: null,
      importWarnings: null,
    }));

    try {
      const response = await invoke<CommandResponse<GenerateResult>>(
        'generate_design_doc',
        { prdPath, options: options || null }
      );

      if (response.success && response.data) {
        set((state) => ({
          designDoc: response.data!.design_doc,
          generationInfo: response.data!.generation_info,
          loading: { ...state.loading, generating: false },
        }));
        return response.data.design_doc;
      } else {
        set((state) => ({
          error: response.error || 'Failed to generate design document',
          loading: { ...state.loading, generating: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to generate design document',
        loading: { ...state.loading, generating: false },
      }));
      return null;
    }
  },

  importDesignDoc: async (filePath: string, format?: string) => {
    set((state) => ({
      loading: { ...state.loading, importing: true },
      error: null,
      generationInfo: null,
    }));

    try {
      const response = await invoke<CommandResponse<ImportResult>>(
        'import_design_doc',
        { filePath, format: format || null }
      );

      if (response.success && response.data) {
        set((state) => ({
          designDoc: response.data!.design_doc,
          importWarnings: response.data!.warnings,
          loading: { ...state.loading, importing: false },
        }));
        return response.data.design_doc;
      } else {
        set((state) => ({
          error: response.error || 'Failed to import design document',
          loading: { ...state.loading, importing: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to import design document',
        loading: { ...state.loading, importing: false },
      }));
      return null;
    }
  },

  loadDesignDoc: async (projectPath?: string) => {
    set((state) => ({
      loading: { ...state.loading, loading: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<DesignDoc>>(
        'get_design_doc',
        { projectPath: projectPath || null }
      );

      if (response.success && response.data) {
        set((state) => ({
          designDoc: response.data,
          loading: { ...state.loading, loading: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to load design document',
          loading: { ...state.loading, loading: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to load design document',
        loading: { ...state.loading, loading: false },
      }));
      return null;
    }
  },

  reset: () => {
    set({
      designDoc: null,
      generationInfo: null,
      importWarnings: null,
      loading: {
        generating: false,
        importing: false,
        loading: false,
      },
      error: null,
    });
  },

  clearError: () => {
    set({ error: null });
  },
}));

export default useDesignDocStore;
