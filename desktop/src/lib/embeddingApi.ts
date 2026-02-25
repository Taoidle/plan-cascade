/**
 * Embedding API (IPC Wrappers)
 *
 * Type-safe wrappers for the five Tauri embedding commands defined in
 * `src-tauri/src/commands/embedding.rs`. Each function follows the project
 * IPC pattern: `invoke<CommandResponse<T>>('command_name', { params })`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';
import type {
  EmbeddingConfigResponse,
  SetEmbeddingConfigRequest,
  SetEmbeddingConfigResponse,
  EmbeddingProviderCapability,
  CheckEmbeddingHealthRequest,
  EmbeddingHealthResponse,
  GetEmbeddingApiKeyRequest,
  SetEmbeddingApiKeyRequest,
  SetEmbeddingApiKeyResponse,
} from '../types/embedding';

// ---------------------------------------------------------------------------
// IPC-001: get_embedding_config
// ---------------------------------------------------------------------------

/**
 * Retrieve the current embedding provider configuration.
 *
 * Returns the persisted config from the database settings store, or defaults
 * to TF-IDF if no config has been explicitly saved.
 */
export async function getEmbeddingConfig(projectPath?: string): Promise<CommandResponse<EmbeddingConfigResponse>> {
  try {
    return await invoke<CommandResponse<EmbeddingConfigResponse>>('get_embedding_config', {
      project_path: projectPath ?? null,
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
// IPC-002: set_embedding_config
// ---------------------------------------------------------------------------

/**
 * Update the embedding provider configuration.
 *
 * Returns whether a reindex is required due to configuration changes.
 */
export async function setEmbeddingConfig(
  request: SetEmbeddingConfigRequest,
): Promise<CommandResponse<SetEmbeddingConfigResponse>> {
  try {
    return await invoke<CommandResponse<SetEmbeddingConfigResponse>>('set_embedding_config', {
      request,
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
// IPC-003: list_embedding_providers
// ---------------------------------------------------------------------------

/**
 * List all available embedding providers with their capabilities.
 */
export async function listEmbeddingProviders(): Promise<CommandResponse<EmbeddingProviderCapability[]>> {
  try {
    return await invoke<CommandResponse<EmbeddingProviderCapability[]>>('list_embedding_providers');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// IPC-004: check_embedding_provider_health
// ---------------------------------------------------------------------------

/**
 * Health check for a specific embedding provider.
 */
export async function checkEmbeddingProviderHealth(
  request: CheckEmbeddingHealthRequest,
): Promise<CommandResponse<EmbeddingHealthResponse>> {
  try {
    return await invoke<CommandResponse<EmbeddingHealthResponse>>('check_embedding_provider_health', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// IPC-006: get_embedding_api_key
// ---------------------------------------------------------------------------

/**
 * Retrieve an embedding API key from the OS keyring.
 *
 * Returns the stored key for the given provider alias, or `null` if no key
 * has been saved.
 */
export async function getEmbeddingApiKey(request: GetEmbeddingApiKeyRequest): Promise<CommandResponse<string | null>> {
  try {
    return await invoke<CommandResponse<string | null>>('get_embedding_api_key', {
      request,
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
// IPC-005: set_embedding_api_key
// ---------------------------------------------------------------------------

/**
 * Store an embedding API key in the OS keyring.
 *
 * Uses provider-specific keyring aliases (e.g., `qwen_embedding`).
 */
export async function setEmbeddingApiKey(
  request: SetEmbeddingApiKeyRequest,
): Promise<CommandResponse<SetEmbeddingApiKeyResponse>> {
  try {
    return await invoke<CommandResponse<SetEmbeddingApiKeyResponse>>('set_embedding_api_key', {
      request,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
