/**
 * Knowledge Base API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri RAG pipeline commands defined in
 * `src-tauri/src/commands/knowledge.rs`. Each function follows the project
 * IPC pattern: `invoke<CommandResponse<T>>('command_name', { params })`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Scoped document reference used by knowledge filters. */
export interface ScopedDocumentRef {
  collection_id: string;
  document_uid: string;
}

/** A knowledge collection containing indexed documents. */
export interface KnowledgeCollection {
  id: string;
  name: string;
  project_id: string;
  description: string;
  chunk_count: number;
  created_at: string;
  updated_at: string;
  /** Optional workspace path associating this collection with a project directory. */
  workspace_path?: string;
}

/** A document to ingest into a collection. */
export interface DocumentInput {
  id: string;
  content: string;
  /** Base64-encoded binary content (for PDF/DOCX/XLSX files). */
  content_base64?: string;
  source_path?: string;
  source_type?: string;
}

/** A search result from a RAG query. */
export interface SearchResult {
  collection_id: string;
  document_uid: string;
  chunk_text: string;
  document_id: string;
  collection_name: string;
  score: number;
  metadata: Record<string, string>;
}

/** Summary of a document within a collection. */
export interface DocumentSummary {
  document_uid: string;
  display_name: string;
  source_kind: string;
  source_locator: string;
  source_type: string;
  trackable: boolean;
  last_indexed_at: string;
  chunk_count: number;
  preview: string;
  /** Legacy compatibility alias (display_name). */
  document_id?: string;
  /** Legacy compatibility alias (source_locator). */
  source_path?: string;
}

/** Result of a RAG query. */
export interface RagQueryResult {
  results: SearchResult[];
  total_searched: number;
  collection_name: string;
}

/** Information about a document whose content changed or was deleted. */
export interface DocUpdateInfo {
  document_uid: string;
  display_name: string;
  source_kind: string;
  source_locator: string;
  source_type: string;
  old_hash: string;
  /** `null` if file was deleted from disk. */
  new_hash: string | null;
  /** Legacy compatibility alias (display_name). */
  document_id?: string;
  /** Legacy compatibility alias (source_locator). */
  source_path?: string;
}

/** Result of comparing stored hashes with disk state. */
export interface CollectionUpdateCheck {
  collection_id: string;
  modified: DocUpdateInfo[];
  deleted: DocUpdateInfo[];
  new_files: string[];
  unchanged: number;
}

/** Status of a docs knowledge base for a workspace. */
export interface DocsKbStatus {
  collection_id: string | null;
  collection_name: string | null;
  total_docs: number;
  pending_changes: string[];
  /** "none" | "indexing" | "indexed" | "changes_pending" */
  status: string;
}

export interface RagIngestRequest {
  projectId: string;
  documents: DocumentInput[];
  collectionId?: string;
  collectionName?: string;
  description?: string | null;
}

export interface RagQueryRequest {
  projectId: string;
  query: string;
  topK?: number;
  collectionName?: string;
  collectionIds?: string[];
  documentFilters?: ScopedDocumentRef[];
  retrievalProfile?: string;
}

/** Recorded retrieval execution metadata for observability. */
export interface QueryRunSummary {
  id: number;
  project_id: string;
  query: string;
  collection_scope: string;
  retrieval_profile: string;
  top_k: number;
  vector_candidates: number;
  bm25_candidates: number;
  merged_candidates: number;
  rerank_ms: number;
  total_ms: number;
  result_count: number;
  created_at: string;
}

/** Document match result for picker search. */
export interface DocumentSearchMatch {
  collection_id: string;
  document_uid: string;
  display_name: string;
}

/** Aggregated monitoring metrics for knowledge subsystem behavior. */
export interface KnowledgeObservabilityMetrics {
  query_run_scope_checks_total: number;
  query_run_scope_hits_total: number;
  query_run_scope_hit_rate: number;
  ingest_crosstalk_alert_total: number;
  picker_search_total: number;
  picker_search_empty_total: number;
  picker_search_empty_rate: number;
  plan_knowledge_attempt_total: number;
  plan_knowledge_hit_total: number;
  plan_knowledge_hit_rate: number;
}

function normalizeDocumentSummary(document: DocumentSummary): DocumentSummary {
  return {
    ...document,
    document_id: document.document_id ?? document.display_name,
    source_path: document.source_path ?? document.source_locator,
  };
}

function normalizeDocUpdateInfo(info: DocUpdateInfo): DocUpdateInfo {
  return {
    ...info,
    document_id: info.document_id ?? info.display_name,
    source_path: info.source_path ?? info.source_locator,
  };
}

// ---------------------------------------------------------------------------
// rag_ingest_documents
// ---------------------------------------------------------------------------

/**
 * Ingest documents into a knowledge collection.
 */
export async function ragIngestDocuments(request: RagIngestRequest): Promise<CommandResponse<KnowledgeCollection>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection>>('rag_ingest_documents', {
      collectionId: request.collectionId ?? null,
      collection_id: request.collectionId ?? null,
      collectionName: request.collectionName ?? null,
      collection_name: request.collectionName ?? null,
      projectId: request.projectId,
      project_id: request.projectId,
      description: request.description ?? null,
      documents: request.documents,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_query
// ---------------------------------------------------------------------------

/**
 * Query a knowledge collection for relevant documents.
 */
export async function ragQuery(request: RagQueryRequest): Promise<CommandResponse<RagQueryResult>> {
  try {
    return await invoke<CommandResponse<RagQueryResult>>('rag_query', {
      projectId: request.projectId,
      project_id: request.projectId,
      query: request.query,
      topK: request.topK ?? 10,
      top_k: request.topK ?? 10,
      collectionName: request.collectionName ?? null,
      collection_name: request.collectionName ?? null,
      collectionIds: request.collectionIds ?? null,
      collection_ids: request.collectionIds ?? null,
      documentFilters: request.documentFilters ?? null,
      document_filters: request.documentFilters ?? null,
      retrievalProfile: request.retrievalProfile ?? null,
      retrieval_profile: request.retrievalProfile ?? null,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_list_query_runs
// ---------------------------------------------------------------------------

/**
 * List recent retrieval runs for observability.
 */
export async function ragListQueryRuns(
  projectId: string,
  collectionIds?: string[],
  limit?: number,
): Promise<CommandResponse<QueryRunSummary[]>> {
  try {
    return await invoke<CommandResponse<QueryRunSummary[]>>('rag_list_query_runs', {
      projectId,
      project_id: projectId,
      collectionIds: collectionIds ?? null,
      collection_ids: collectionIds ?? null,
      limit: limit ?? null,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Search documents by display name across project collections.
 */
export async function ragSearchDocuments(
  projectId: string,
  query: string,
  collectionIds?: string[],
  limit?: number,
): Promise<CommandResponse<DocumentSearchMatch[]>> {
  try {
    return await invoke<CommandResponse<DocumentSearchMatch[]>>('rag_search_documents', {
      projectId,
      project_id: projectId,
      query,
      collectionIds: collectionIds ?? null,
      collection_ids: collectionIds ?? null,
      limit: limit ?? null,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Read knowledge observability metrics snapshot.
 */
export async function ragGetObservabilityMetrics(): Promise<CommandResponse<KnowledgeObservabilityMetrics>> {
  try {
    return await invoke<CommandResponse<KnowledgeObservabilityMetrics>>('rag_get_observability_metrics');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Record picker search event for empty-result rate monitoring.
 */
export async function ragRecordPickerSearch(empty: boolean): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('rag_record_picker_search', { empty });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Record a potential ingest progress crosstalk alert.
 */
export async function ragRecordIngestCrosstalkAlert(): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('rag_record_ingest_crosstalk_alert');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_list_collections
// ---------------------------------------------------------------------------

/**
 * List all knowledge collections for a project.
 */
export async function ragListCollections(projectId: string): Promise<CommandResponse<KnowledgeCollection[]>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection[]>>('rag_list_collections', {
      projectId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_delete_collection
// ---------------------------------------------------------------------------

/**
 * Delete a knowledge collection and all its documents.
 */
export async function ragDeleteCollection(
  collectionName: string,
  projectId: string,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('rag_delete_collection', {
      collectionName,
      projectId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_update_collection
// ---------------------------------------------------------------------------

/**
 * Update a knowledge collection's metadata (name, description, workspace path).
 */
export async function ragUpdateCollection(
  collectionId: string,
  name?: string,
  description?: string,
  workspacePath?: string | null,
): Promise<CommandResponse<KnowledgeCollection>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection>>('rag_update_collection', {
      collectionId,
      name: name ?? null,
      description: description ?? null,
      workspacePath: workspacePath === undefined ? null : workspacePath,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_list_documents
// ---------------------------------------------------------------------------

/**
 * List all documents in a knowledge collection.
 */
export async function ragListDocuments(collectionId: string): Promise<CommandResponse<DocumentSummary[]>> {
  try {
    const response = await invoke<CommandResponse<DocumentSummary[]>>('rag_list_documents', {
      collectionId,
      collection_id: collectionId,
    });
    if (!response.success || !response.data) return response;
    return {
      ...response,
      data: response.data.map(normalizeDocumentSummary),
    };
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_delete_document
// ---------------------------------------------------------------------------

/**
 * Delete a single document from a knowledge collection.
 */
export async function ragDeleteDocument(collectionId: string, documentUid: string): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('rag_delete_document', {
      collectionId,
      collection_id: collectionId,
      documentUid,
      document_uid: documentUid,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_check_collection_updates
// ---------------------------------------------------------------------------

/**
 * Check a collection for changed/deleted documents by comparing content hashes.
 */
export async function ragCheckCollectionUpdates(collectionId: string): Promise<CommandResponse<CollectionUpdateCheck>> {
  try {
    const response = await invoke<CommandResponse<CollectionUpdateCheck>>('rag_check_collection_updates', {
      collectionId,
      collection_id: collectionId,
    });
    if (!response.success || !response.data) return response;
    return {
      ...response,
      data: {
        ...response.data,
        modified: response.data.modified.map(normalizeDocUpdateInfo),
        deleted: response.data.deleted.map(normalizeDocUpdateInfo),
      },
    };
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_apply_collection_updates
// ---------------------------------------------------------------------------

/**
 * Apply detected updates to a collection (reingest modified, delete removed).
 */
export async function ragApplyCollectionUpdates(collectionId: string): Promise<CommandResponse<KnowledgeCollection>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection>>('rag_apply_collection_updates', {
      collectionId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_ensure_docs_collection
// ---------------------------------------------------------------------------

/**
 * Ensure a docs-only knowledge collection exists for a workspace.
 */
export async function ragEnsureDocsCollection(
  workspacePath: string,
  projectId: string,
): Promise<CommandResponse<KnowledgeCollection | null>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection | null>>('rag_ensure_docs_collection', {
      workspacePath,
      projectId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_sync_docs_collection
// ---------------------------------------------------------------------------

/**
 * Sync a docs collection: check for changes and apply them.
 */
export async function ragSyncDocsCollection(
  workspacePath: string,
  projectId: string,
): Promise<CommandResponse<KnowledgeCollection | null>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection | null>>('rag_sync_docs_collection', {
      workspacePath,
      projectId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// rag_get_docs_status
// ---------------------------------------------------------------------------

/**
 * Get the status of a docs knowledge base for a workspace.
 */
export async function ragGetDocsStatus(
  workspacePath: string,
  projectId: string,
): Promise<CommandResponse<DocsKbStatus>> {
  try {
    return await invoke<CommandResponse<DocsKbStatus>>('rag_get_docs_status', {
      workspacePath,
      projectId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
