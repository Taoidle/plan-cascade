/**
 * Codebase Index API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri codebase index management commands
 * defined in `src-tauri/src/commands/codebase.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface IndexedProjectEntry {
  project_path: string;
  file_count: number;
  last_indexed_at: string | null;
}

export interface LanguageBreakdown {
  language: string;
  count: number;
}

export interface ComponentSummary {
  name: string;
  count: number;
}

export interface ProjectIndexSummary {
  total_files: number;
  languages: string[];
  components: ComponentSummary[];
  key_entry_points: string[];
  total_symbols: number;
  embedding_chunks: number;
}

export interface EmbeddingMetadata {
  provider_type: string;
  provider_model: string;
  embedding_dimension: number;
}

export interface IndexStatusEvent {
  project_path: string;
  status: string;
  indexed_files: number;
  total_files: number;
  error_message: string | null;
  total_symbols: number;
  embedding_chunks: number;
  embedding_provider_name: string | null;
  lsp_enrichment: string;
}

export interface CodebaseProjectDetail {
  project_path: string;
  summary: ProjectIndexSummary;
  languages: LanguageBreakdown[];
  embedding_metadata: EmbeddingMetadata[];
  status: IndexStatusEvent;
}

export interface FileIndexRow {
  id: number;
  project_path: string;
  file_path: string;
  component: string;
  language: string;
  extension: string | null;
  size_bytes: number;
  line_count: number;
  is_test: boolean;
  content_hash: string;
  indexed_at: string | null;
}

export interface CodebaseFileListResult {
  files: FileIndexRow[];
  total: number;
}

export interface SemanticSearchResult {
  file_path: string;
  chunk_index: number;
  chunk_text: string;
  similarity: number;
}

// ---------------------------------------------------------------------------
// API Functions
// ---------------------------------------------------------------------------

export async function listCodebaseProjects(): Promise<CommandResponse<IndexedProjectEntry[]>> {
  try {
    return await invoke<CommandResponse<IndexedProjectEntry[]>>('codebase_list_projects');
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function getCodebaseDetail(projectPath: string): Promise<CommandResponse<CodebaseProjectDetail>> {
  try {
    return await invoke<CommandResponse<CodebaseProjectDetail>>('codebase_get_project_detail', {
      projectPath,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function listCodebaseFiles(
  projectPath: string,
  opts?: {
    languageFilter?: string | null;
    searchPattern?: string | null;
    offset?: number;
    limit?: number;
  },
): Promise<CommandResponse<CodebaseFileListResult>> {
  try {
    return await invoke<CommandResponse<CodebaseFileListResult>>('codebase_list_files', {
      projectPath,
      languageFilter: opts?.languageFilter ?? null,
      searchPattern: opts?.searchPattern ?? null,
      offset: opts?.offset ?? 0,
      limit: opts?.limit ?? 50,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function deleteCodebaseProject(projectPath: string): Promise<CommandResponse<number>> {
  try {
    return await invoke<CommandResponse<number>>('codebase_delete_project', {
      projectPath,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function searchCodebase(
  projectPath: string,
  query: string,
  topK?: number,
): Promise<CommandResponse<SemanticSearchResult[]>> {
  try {
    return await invoke<CommandResponse<SemanticSearchResult[]>>('codebase_search', {
      projectPath,
      query,
      topK: topK ?? 10,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function triggerReindex(projectPath: string): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('trigger_reindex', {
      projectPath,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function getIndexStatus(projectPath: string): Promise<CommandResponse<IndexStatusEvent>> {
  try {
    return await invoke<CommandResponse<IndexStatusEvent>>('get_index_status', {
      projectPath,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export interface ClassifyComponentsResult {
  source: string;
  mappings_count: number;
  files_updated: number;
}

export async function classifyComponents(projectPath: string): Promise<CommandResponse<ClassifyComponentsResult>> {
  try {
    return await invoke<CommandResponse<ClassifyComponentsResult>>('classify_codebase_components', {
      projectPath,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}
