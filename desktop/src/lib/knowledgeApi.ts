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
  chunk_text: string;
  document_id: string;
  collection_name: string;
  score: number;
  metadata: Record<string, string>;
}

/** Summary of a document within a collection. */
export interface DocumentSummary {
  document_id: string;
  chunk_count: number;
  preview: string;
}

/** Result of a RAG query. */
export interface RagQueryResult {
  results: SearchResult[];
  total_searched: number;
  collection_name: string;
}

/** Information about a document whose content changed or was deleted. */
export interface DocUpdateInfo {
  document_id: string;
  source_path: string;
  source_type: string;
  old_hash: string;
  /** `null` if file was deleted from disk. */
  new_hash: string | null;
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

// ---------------------------------------------------------------------------
// rag_ingest_documents
// ---------------------------------------------------------------------------

/**
 * Ingest documents into a knowledge collection.
 */
export async function ragIngestDocuments(
  collectionName: string,
  projectId: string,
  description: string | null,
  documents: DocumentInput[],
): Promise<CommandResponse<KnowledgeCollection>> {
  try {
    return await invoke<CommandResponse<KnowledgeCollection>>('rag_ingest_documents', {
      collectionName,
      projectId,
      description,
      documents,
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
export async function ragQuery(
  collectionName: string,
  projectId: string,
  query: string,
  topK?: number,
): Promise<CommandResponse<RagQueryResult>> {
  try {
    return await invoke<CommandResponse<RagQueryResult>>('rag_query', {
      collectionName,
      projectId,
      query,
      topK: topK ?? 10,
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
    return await invoke<CommandResponse<DocumentSummary[]>>('rag_list_documents', {
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
// rag_delete_document
// ---------------------------------------------------------------------------

/**
 * Delete a single document from a knowledge collection.
 */
export async function ragDeleteDocument(collectionId: string, documentId: string): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('rag_delete_document', {
      collectionId,
      documentId,
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
    return await invoke<CommandResponse<CollectionUpdateCheck>>('rag_check_collection_updates', {
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
