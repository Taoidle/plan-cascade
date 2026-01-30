/**
 * Markdown Store
 *
 * Manages CLAUDE.md file state for the MarkdownEditor.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  ClaudeMdFile,
  ClaudeMdContent,
  SaveResult,
  ViewMode,
  SaveStatus,
  CommandResponse,
} from '../types/markdown';

interface MarkdownState {
  /** List of all discovered CLAUDE.md files */
  files: ClaudeMdFile[];

  /** Currently selected file */
  selectedFile: ClaudeMdFile | null;

  /** Content of the currently selected file */
  content: string;

  /** Original content when file was loaded (for dirty checking) */
  originalContent: string;

  /** Root path being scanned */
  rootPath: string;

  /** Current view mode */
  viewMode: ViewMode;

  /** Save status for auto-save indicator */
  saveStatus: SaveStatus;

  /** Loading states */
  loading: {
    files: boolean;
    content: boolean;
    saving: boolean;
  };

  /** Error messages */
  error: string | null;

  /** Auto-save enabled */
  autoSaveEnabled: boolean;

  /** Actions */
  fetchFiles: (rootPath: string) => Promise<void>;
  selectFile: (file: ClaudeMdFile | null) => Promise<void>;
  setContent: (content: string) => void;
  saveContent: () => Promise<SaveResult | null>;
  createFile: (path: string, templateContent: string) => Promise<SaveResult | null>;
  setViewMode: (mode: ViewMode) => void;
  setRootPath: (path: string) => void;
  setAutoSaveEnabled: (enabled: boolean) => void;
  setSaveStatus: (status: SaveStatus) => void;
  clearError: () => void;
  refreshFiles: () => Promise<void>;
  isDirty: () => boolean;
}

export const useMarkdownStore = create<MarkdownState>((set, get) => ({
  files: [],
  selectedFile: null,
  content: '',
  originalContent: '',
  rootPath: '',
  viewMode: 'split',
  saveStatus: 'saved',
  loading: {
    files: false,
    content: false,
    saving: false,
  },
  error: null,
  autoSaveEnabled: true,

  fetchFiles: async (rootPath: string) => {
    set((state) => ({
      loading: { ...state.loading, files: true },
      error: null,
      rootPath,
    }));

    try {
      const response = await invoke<CommandResponse<ClaudeMdFile[]>>('scan_claude_md', {
        rootPath,
      });

      if (response.success && response.data) {
        set((state) => ({
          files: response.data!,
          loading: { ...state.loading, files: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to scan for CLAUDE.md files',
          loading: { ...state.loading, files: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to scan for CLAUDE.md files',
        loading: { ...state.loading, files: false },
      }));
    }
  },

  selectFile: async (file: ClaudeMdFile | null) => {
    // Check if there are unsaved changes
    const { isDirty, saveContent } = get();
    if (isDirty()) {
      // Auto-save before switching
      await saveContent();
    }

    set({
      selectedFile: file,
      content: '',
      originalContent: '',
      saveStatus: 'saved',
    });

    if (file) {
      set((state) => ({
        loading: { ...state.loading, content: true },
        error: null,
      }));

      try {
        const response = await invoke<CommandResponse<ClaudeMdContent>>('read_claude_md', {
          path: file.path,
        });

        if (response.success && response.data) {
          set((state) => ({
            content: response.data!.content,
            originalContent: response.data!.content,
            loading: { ...state.loading, content: false },
          }));
        } else {
          set((state) => ({
            error: response.error || 'Failed to read file',
            loading: { ...state.loading, content: false },
          }));
        }
      } catch (err) {
        set((state) => ({
          error: err instanceof Error ? err.message : 'Failed to read file',
          loading: { ...state.loading, content: false },
        }));
      }
    }
  },

  setContent: (content: string) => {
    const { originalContent } = get();
    set({
      content,
      saveStatus: content !== originalContent ? 'unsaved' : 'saved',
    });
  },

  saveContent: async () => {
    const { selectedFile, content } = get();

    if (!selectedFile) {
      return null;
    }

    set((state) => ({
      loading: { ...state.loading, saving: true },
      saveStatus: 'saving',
    }));

    try {
      const response = await invoke<CommandResponse<SaveResult>>('save_claude_md', {
        path: selectedFile.path,
        content,
      });

      if (response.success && response.data) {
        if (response.data.success) {
          set((state) => ({
            originalContent: content,
            saveStatus: 'saved',
            loading: { ...state.loading, saving: false },
          }));
          return response.data;
        } else {
          set((state) => ({
            error: response.data!.error || 'Failed to save file',
            saveStatus: 'error',
            loading: { ...state.loading, saving: false },
          }));
          return response.data;
        }
      } else {
        set((state) => ({
          error: response.error || 'Failed to save file',
          saveStatus: 'error',
          loading: { ...state.loading, saving: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to save file',
        saveStatus: 'error',
        loading: { ...state.loading, saving: false },
      }));
      return null;
    }
  },

  createFile: async (path: string, templateContent: string) => {
    set((state) => ({
      loading: { ...state.loading, saving: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<SaveResult>>('create_claude_md', {
        path,
        templateContent,
      });

      if (response.success && response.data) {
        set((state) => ({
          loading: { ...state.loading, saving: false },
        }));

        // Refresh the file list
        const { rootPath, fetchFiles } = get();
        if (rootPath) {
          await fetchFiles(rootPath);
        }

        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to create file',
          loading: { ...state.loading, saving: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to create file',
        loading: { ...state.loading, saving: false },
      }));
      return null;
    }
  },

  setViewMode: (mode: ViewMode) => {
    set({ viewMode: mode });
  },

  setRootPath: (path: string) => {
    set({ rootPath: path });
  },

  setAutoSaveEnabled: (enabled: boolean) => {
    set({ autoSaveEnabled: enabled });
  },

  setSaveStatus: (status: SaveStatus) => {
    set({ saveStatus: status });
  },

  clearError: () => {
    set({ error: null });
  },

  refreshFiles: async () => {
    const { rootPath, fetchFiles } = get();
    if (rootPath) {
      await fetchFiles(rootPath);
    }
  },

  isDirty: () => {
    const { content, originalContent } = get();
    return content !== originalContent;
  },
}));

export default useMarkdownStore;
