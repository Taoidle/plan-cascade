/**
 * Artifacts API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri artifact commands defined in
 * `src-tauri/src/commands/artifacts.rs`. Each function follows the project
 * IPC pattern: `invoke<CommandResponse<T>>('command_name', { params })`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Metadata for a stored artifact. */
export interface ArtifactMeta {
  id: string;
  name: string;
  project_id: string;
  session_id: string | null;
  user_id: string | null;
  content_type: string;
  current_version: number;
  size_bytes: number;
  checksum: string;
  created_at: string;
  updated_at: string;
}

/** A specific version of an artifact. */
export interface ArtifactVersion {
  id: string;
  artifact_id: string;
  version: number;
  size_bytes: number;
  checksum: string;
  storage_path: string;
  created_at: string;
}

/** Scope filter for artifact queries. */
export interface ArtifactScope {
  projectId: string;
  sessionId?: string;
  userId?: string;
}

// ---------------------------------------------------------------------------
// artifact_save
// ---------------------------------------------------------------------------

/**
 * Save an artifact (auto-increments version).
 */
export async function artifactSave(
  name: string,
  projectId: string,
  sessionId: string | null,
  userId: string | null,
  contentType: string,
  data: number[],
): Promise<CommandResponse<ArtifactMeta>> {
  try {
    return await invoke<CommandResponse<ArtifactMeta>>('artifact_save', {
      name,
      projectId,
      sessionId,
      userId,
      contentType,
      data,
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
// artifact_load
// ---------------------------------------------------------------------------

/**
 * Load an artifact (latest version by default, or specific version).
 */
export async function artifactLoad(
  name: string,
  projectId: string,
  sessionId: string | null,
  userId: string | null,
  version?: number,
): Promise<CommandResponse<number[]>> {
  try {
    return await invoke<CommandResponse<number[]>>('artifact_load', {
      name,
      projectId,
      sessionId,
      userId,
      version: version ?? null,
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
// artifact_list
// ---------------------------------------------------------------------------

/**
 * List artifacts filtered by scope.
 */
export async function artifactList(
  projectId: string,
  sessionId?: string,
  userId?: string,
): Promise<CommandResponse<ArtifactMeta[]>> {
  try {
    return await invoke<CommandResponse<ArtifactMeta[]>>('artifact_list', {
      projectId,
      sessionId: sessionId ?? null,
      userId: userId ?? null,
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
// artifact_versions
// ---------------------------------------------------------------------------

/**
 * List all versions of a named artifact.
 */
export async function artifactVersions(
  name: string,
  projectId: string,
  sessionId?: string,
  userId?: string,
): Promise<CommandResponse<ArtifactVersion[]>> {
  try {
    return await invoke<CommandResponse<ArtifactVersion[]>>('artifact_versions', {
      name,
      projectId,
      sessionId: sessionId ?? null,
      userId: userId ?? null,
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
// artifact_delete
// ---------------------------------------------------------------------------

/**
 * Delete an artifact and all its versions.
 */
export async function artifactDelete(
  name: string,
  projectId: string,
  sessionId?: string,
  userId?: string,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('artifact_delete', {
      name,
      projectId,
      sessionId: sessionId ?? null,
      userId: userId ?? null,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
