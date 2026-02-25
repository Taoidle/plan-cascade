/**
 * Spec Interview Store
 *
 * Manages spec interview state for the Expert Mode SpecInterviewPanel.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

/** Interview phase enum matching Rust InterviewPhase */
export type InterviewPhase = 'overview' | 'scope' | 'requirements' | 'interfaces' | 'stories' | 'review' | 'complete';

/** A question generated for the interview */
export interface InterviewQuestion {
  id: string;
  question: string;
  phase: InterviewPhase;
  hint: string | null;
  required: boolean;
  input_type: 'text' | 'textarea' | 'list' | 'boolean';
  field_name: string;
}

/** A single entry in the interview history */
export interface InterviewHistoryEntry {
  turn_number: number;
  phase: string;
  question: string;
  answer: string;
  timestamp: string;
}

/** Full interview session state from the backend */
export interface InterviewSession {
  id: string;
  status: 'in_progress' | 'finalized';
  phase: InterviewPhase;
  flow_level: string;
  description: string;
  question_cursor: number;
  max_questions: number;
  current_question: InterviewQuestion | null;
  progress: number;
  history: InterviewHistoryEntry[];
}

/** Configuration for starting a new interview */
export interface InterviewConfig {
  description: string;
  flow_level: string;
  max_questions: number;
  first_principles: boolean;
  project_path: string | null;
  exploration_context: string | null;
}

/** Compiled spec output from the backend */
export interface CompiledSpec {
  spec_json: Record<string, unknown>;
  spec_md: string;
  prd_json: Record<string, unknown>;
}

/** Compile options */
export interface CompileOptions {
  description: string;
  flow_level: string | null;
  tdd_mode: string | null;
  confirm: boolean;
  no_confirm: boolean;
}

/** LLM provider settings for BA-driven interviews */
export interface InterviewProviderSettings {
  provider?: string;
  model?: string;
  apiKey?: string;
  baseUrl?: string;
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

interface SpecInterviewState {
  /** Current interview session */
  session: InterviewSession | null;

  /** Compiled spec output (after compilation) */
  compiledSpec: CompiledSpec | null;

  /** Loading states */
  loading: {
    starting: boolean;
    submitting: boolean;
    fetching: boolean;
    compiling: boolean;
  };

  /** Error message */
  error: string | null;

  /** LLM provider settings for BA-driven mode */
  providerSettings: InterviewProviderSettings | null;

  /** Actions */
  setProviderSettings: (settings: InterviewProviderSettings | null) => void;
  startInterview: (config: InterviewConfig) => Promise<InterviewSession | null>;
  submitAnswer: (answer: string) => Promise<InterviewSession | null>;
  fetchState: (interviewId: string) => Promise<InterviewSession | null>;
  compileSpec: (options?: Partial<CompileOptions>) => Promise<CompiledSpec | null>;
  reset: () => void;
  clearError: () => void;
}

export const useSpecInterviewStore = create<SpecInterviewState>((set, get) => ({
  session: null,
  compiledSpec: null,
  loading: {
    starting: false,
    submitting: false,
    fetching: false,
    compiling: false,
  },
  error: null,
  providerSettings: null,

  setProviderSettings: (settings: InterviewProviderSettings | null) => {
    set({ providerSettings: settings });
  },

  startInterview: async (config: InterviewConfig) => {
    set((state) => ({
      loading: { ...state.loading, starting: true },
      error: null,
      compiledSpec: null,
    }));

    try {
      const { providerSettings } = get();
      const response = await invoke<CommandResponse<InterviewSession>>('start_spec_interview', {
        config,
        provider: providerSettings?.provider ?? null,
        model: providerSettings?.model ?? null,
        apiKey: providerSettings?.apiKey ?? null,
        baseUrl: providerSettings?.baseUrl ?? null,
      });

      if (response.success && response.data) {
        set((state) => ({
          session: response.data,
          loading: { ...state.loading, starting: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to start interview',
          loading: { ...state.loading, starting: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to start interview',
        loading: { ...state.loading, starting: false },
      }));
      return null;
    }
  },

  submitAnswer: async (answer: string) => {
    const { session, providerSettings } = get();
    if (!session) {
      set({ error: 'No active interview session' });
      return null;
    }

    set((state) => ({
      loading: { ...state.loading, submitting: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<InterviewSession>>('submit_interview_answer', {
        interviewId: session.id,
        answer,
        provider: providerSettings?.provider ?? null,
        model: providerSettings?.model ?? null,
        apiKey: providerSettings?.apiKey ?? null,
        baseUrl: providerSettings?.baseUrl ?? null,
      });

      if (response.success && response.data) {
        set((state) => ({
          session: response.data,
          loading: { ...state.loading, submitting: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to submit answer',
          loading: { ...state.loading, submitting: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to submit answer',
        loading: { ...state.loading, submitting: false },
      }));
      return null;
    }
  },

  fetchState: async (interviewId: string) => {
    set((state) => ({
      loading: { ...state.loading, fetching: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<InterviewSession>>('get_interview_state', { interviewId });

      if (response.success && response.data) {
        set((state) => ({
          session: response.data,
          loading: { ...state.loading, fetching: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch interview state',
          loading: { ...state.loading, fetching: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch interview state',
        loading: { ...state.loading, fetching: false },
      }));
      return null;
    }
  },

  compileSpec: async (options?: Partial<CompileOptions>) => {
    const { session } = get();
    if (!session) {
      set({ error: 'No active interview session' });
      return null;
    }

    set((state) => ({
      loading: { ...state.loading, compiling: true },
      error: null,
    }));

    const compileOptions: CompileOptions = {
      description: options?.description || '',
      flow_level: options?.flow_level || null,
      tdd_mode: options?.tdd_mode || null,
      confirm: options?.confirm || false,
      no_confirm: options?.no_confirm || false,
    };

    try {
      const response = await invoke<CommandResponse<CompiledSpec>>('compile_spec', {
        interviewId: session.id,
        options: compileOptions,
      });

      if (response.success && response.data) {
        set((state) => ({
          compiledSpec: response.data,
          loading: { ...state.loading, compiling: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to compile spec',
          loading: { ...state.loading, compiling: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to compile spec',
        loading: { ...state.loading, compiling: false },
      }));
      return null;
    }
  },

  reset: () => {
    set({
      session: null,
      compiledSpec: null,
      loading: {
        starting: false,
        submitting: false,
        fetching: false,
        compiling: false,
      },
      error: null,
      providerSettings: null,
    });
  },

  clearError: () => {
    set({ error: null });
  },
}));

/** Helper: Get phase display label */
export function getPhaseLabel(phase: InterviewPhase): string {
  const labels: Record<InterviewPhase, string> = {
    overview: 'Overview',
    scope: 'Scope',
    requirements: 'Requirements',
    interfaces: 'Interfaces',
    stories: 'Stories',
    review: 'Review',
    complete: 'Complete',
  };
  return labels[phase] || phase;
}

/** Helper: Get all phases in order */
export function getPhaseOrder(): InterviewPhase[] {
  return ['overview', 'scope', 'requirements', 'interfaces', 'stories', 'review', 'complete'];
}

export default useSpecInterviewStore;
