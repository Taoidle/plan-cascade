/**
 * Artifacts Store
 *
 * Zustand store for versioned artifact management. Manages artifact
 * listing, version history, preview content, and scope filtering
 * via Tauri IPC commands.
 */

import { create } from 'zustand';
import type {
  ArtifactMeta,
  ArtifactVersion,
} from '../lib/artifactsApi';
import {
  artifactList,
  artifactVersions,
  artifactLoad,
  artifactSave,
  artifactDelete,
} from '../lib/artifactsApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Scope filter level for artifact browsing. */
export type ScopeFilter = 'project' | 'session' | 'user';

export interface ArtifactsState {
  /** All artifacts matching the current scope. */
  artifacts: ArtifactMeta[];

  /** Currently selected artifact. */
  selectedArtifact: ArtifactMeta | null;

  /** Version history of the selected artifact. */
  versionHistory: ArtifactVersion[];

  /** Preview content (as Uint8Array converted to number[]). */
  previewContent: number[] | null;

  /** Current scope filter. */
  scopeFilter: ScopeFilter;

  /** Search text filter. */
  searchText: string;

  /** Loading states. */
  isLoading: boolean;
  isLoadingVersions: boolean;
  isLoadingPreview: boolean;
  isSaving: boolean;
  isDeleting: boolean;

  /** Error message. */
  error: string | null;

  /** Actions. */
  fetchArtifacts: (projectId: string, sessionId?: string, userId?: string) => Promise<void>;
  selectArtifact: (artifact: ArtifactMeta | null) => void;
  fetchVersionHistory: (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
  ) => Promise<void>;
  loadPreview: (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
    version?: number,
  ) => Promise<void>;
  saveArtifact: (
    name: string,
    projectId: string,
    sessionId: string | null,
    userId: string | null,
    contentType: string,
    data: number[],
  ) => Promise<boolean>;
  deleteArtifact: (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
  ) => Promise<boolean>;
  setScopeFilter: (scope: ScopeFilter) => void;
  setSearchText: (text: string) => void;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  artifacts: [],
  selectedArtifact: null,
  versionHistory: [],
  previewContent: null,
  scopeFilter: 'project' as ScopeFilter,
  searchText: '',
  isLoading: false,
  isLoadingVersions: false,
  isLoadingPreview: false,
  isSaving: false,
  isDeleting: false,
  error: null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useArtifactsStore = create<ArtifactsState>()((set, get) => ({
  ...DEFAULT_STATE,

  fetchArtifacts: async (projectId: string, sessionId?: string, userId?: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await artifactList(projectId, sessionId, userId);
      if (result.success && result.data) {
        set({ artifacts: result.data, isLoading: false });
      } else {
        set({
          isLoading: false,
          error: result.error ?? 'Failed to fetch artifacts',
        });
      }
    } catch (err) {
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  selectArtifact: (artifact: ArtifactMeta | null) => {
    set({
      selectedArtifact: artifact,
      versionHistory: [],
      previewContent: null,
    });
  },

  fetchVersionHistory: async (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
  ) => {
    set({ isLoadingVersions: true, error: null });
    try {
      const result = await artifactVersions(name, projectId, sessionId, userId);
      if (result.success && result.data) {
        set({ versionHistory: result.data, isLoadingVersions: false });
      } else {
        set({
          isLoadingVersions: false,
          error: result.error ?? 'Failed to fetch version history',
        });
      }
    } catch (err) {
      set({
        isLoadingVersions: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  loadPreview: async (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
    version?: number,
  ) => {
    set({ isLoadingPreview: true, error: null });
    try {
      const result = await artifactLoad(name, projectId, sessionId ?? null, userId ?? null, version);
      if (result.success && result.data) {
        set({ previewContent: result.data, isLoadingPreview: false });
      } else {
        set({
          isLoadingPreview: false,
          error: result.error ?? 'Failed to load artifact preview',
        });
      }
    } catch (err) {
      set({
        isLoadingPreview: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  saveArtifact: async (
    name: string,
    projectId: string,
    sessionId: string | null,
    userId: string | null,
    contentType: string,
    data: number[],
  ) => {
    set({ isSaving: true, error: null });
    try {
      const result = await artifactSave(name, projectId, sessionId, userId, contentType, data);
      if (result.success && result.data) {
        // Update or add the artifact in the list
        set((state) => {
          const exists = state.artifacts.some((a) => a.name === name);
          const artifacts = exists
            ? state.artifacts.map((a) => (a.name === name ? result.data! : a))
            : [...state.artifacts, result.data!];
          return {
            artifacts,
            selectedArtifact:
              state.selectedArtifact?.name === name ? result.data! : state.selectedArtifact,
            isSaving: false,
          };
        });
        return true;
      } else {
        set({
          isSaving: false,
          error: result.error ?? 'Failed to save artifact',
        });
        return false;
      }
    } catch (err) {
      set({
        isSaving: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  deleteArtifact: async (
    name: string,
    projectId: string,
    sessionId?: string,
    userId?: string,
  ) => {
    set({ isDeleting: true, error: null });
    try {
      const result = await artifactDelete(name, projectId, sessionId, userId);
      if (result.success) {
        set((state) => ({
          artifacts: state.artifacts.filter((a) => a.name !== name),
          selectedArtifact:
            state.selectedArtifact?.name === name ? null : state.selectedArtifact,
          versionHistory: state.selectedArtifact?.name === name ? [] : state.versionHistory,
          previewContent: state.selectedArtifact?.name === name ? null : state.previewContent,
          isDeleting: false,
        }));
        return true;
      } else {
        set({
          isDeleting: false,
          error: result.error ?? 'Failed to delete artifact',
        });
        return false;
      }
    } catch (err) {
      set({
        isDeleting: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  setScopeFilter: (scope: ScopeFilter) => {
    set({ scopeFilter: scope });
  },

  setSearchText: (text: string) => {
    set({ searchText: text });
  },

  clearError: () => set({ error: null }),
}));

export default useArtifactsStore;
