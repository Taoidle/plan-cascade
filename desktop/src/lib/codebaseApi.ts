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

export interface IndexedProjectStatusEntry extends IndexedProjectEntry {
  status: IndexStatusEvent;
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

export type CodebaseIndexStatus =
  | 'idle'
  | 'queued'
  | 'indexing'
  | 'indexed'
  | 'indexed_no_embedding'
  | 'error'
  | 'stale';

export type LspEnrichmentStatus = 'none' | 'enriching' | 'enriched';

export interface IndexStatusEvent {
  project_path: string;
  status: CodebaseIndexStatus;
  indexed_files: number;
  total_files: number;
  error_message: string | null;
  total_symbols: number;
  embedding_chunks: number;
  embedding_provider_name: string | null;
  lsp_enrichment: LspEnrichmentStatus;
  phase?: 'queued' | 'parse' | 'embedding' | 'lsp' | 'done';
  job_id?: string | null;
  updated_at?: string | null;
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

export type CodeSearchMode = 'symbol' | 'path' | 'semantic' | 'hybrid';

export interface CodeSearchFilters {
  component?: string | null;
  language?: string | null;
  file_path_prefix?: string | null;
}

export interface SearchChannelScore {
  channel: CodeSearchMode;
  rank: number;
  score: number;
}

export interface SearchHit {
  file_path: string;
  symbol_name?: string | null;
  snippet?: string | null;
  similarity?: number | null;
  score: number;
  score_breakdown: SearchChannelScore[];
  line_start?: number | null;
  line_end?: number | null;
  component?: string | null;
  language?: string | null;
  channels?: CodeSearchMode[];
  query_id?: string;
}

export interface CodeSearchDiagnostics {
  query_id: string;
  active_channels: CodeSearchMode[];
  semantic_degraded: boolean;
  semantic_error?: string | null;
  provider_display?: string | null;
  embedding_dimension: number;
  hnsw_used: boolean;
  hnsw_vector_count: number;
}

export interface CodeSearchRequest {
  project_path: string;
  query: string;
  modes?: CodeSearchMode[];
  limit?: number;
  offset?: number;
  include_snippet?: boolean;
  filters?: CodeSearchFilters;
}

export interface CodeSearchResponse {
  hits: SearchHit[];
  total: number;
  semantic_degraded: boolean;
  semantic_error?: string | null;
  query_id?: string;
  diagnostics?: CodeSearchDiagnostics | null;
}

export interface FileExcerptResult {
  file_path: string;
  line_start: number;
  line_end: number;
  total_lines: number;
  content: string;
}

export interface ContextItem {
  type: 'file' | 'symbol' | 'snippet' | 'search_result';
  project_path: string;
  file_path: string;
  symbol_name?: string | null;
  snippet?: string | null;
  line_start?: number | null;
  line_end?: number | null;
  score?: number | null;
  metadata?: Record<string, unknown> | null;
  source?: string | null;
  session_id?: string | null;
  target_mode?: 'chat' | 'plan' | 'task' | 'debug' | null;
  context_ref_id?: string | null;
}

export interface CodebaseContextAppendResult {
  appended_count: number;
  context_ref_ids: string[];
  session_id: string;
  target_mode: string;
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

export async function listCodebaseProjectsV2(): Promise<CommandResponse<IndexedProjectStatusEntry[]>> {
  try {
    return await invoke<CommandResponse<IndexedProjectStatusEntry[]>>('codebase_list_projects_v2');
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

export async function searchCodebase(request: CodeSearchRequest): Promise<CommandResponse<CodeSearchResponse>> {
  try {
    return await invoke<CommandResponse<CodeSearchResponse>>('codebase_search_v2', {
      request: {
        ...request,
        limit: request.limit ?? 20,
        offset: request.offset ?? 0,
        include_snippet: request.include_snippet ?? true,
      },
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

export async function getCodebaseFileExcerpt(
  projectPath: string,
  filePath: string,
  lineStart: number,
  lineEnd: number,
): Promise<CommandResponse<FileExcerptResult>> {
  try {
    return await invoke<CommandResponse<FileExcerptResult>>('codebase_get_file_excerpt', {
      projectPath,
      filePath,
      lineStart,
      lineEnd,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function openCodebaseFileInEditor(
  projectPath: string,
  filePath: string,
  line?: number,
  column?: number,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('codebase_open_in_editor', {
      projectPath,
      filePath,
      line,
      column,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}

export async function addCodebaseContext(
  targetMode: 'chat' | 'plan' | 'task' | 'debug',
  items: ContextItem[],
  sessionId?: string | null,
): Promise<CommandResponse<CodebaseContextAppendResult>> {
  try {
    return await invoke<CommandResponse<CodebaseContextAppendResult>>('codebase_add_context', {
      targetMode,
      items,
      sessionId: sessionId ?? null,
    });
  } catch (e) {
    return { success: false, data: null, error: String(e) };
  }
}
