/**
 * Embedding Configuration Types
 *
 * TypeScript types mirroring the Rust embedding command request/response
 * structures defined in `src-tauri/src/commands/embedding.rs`.
 */

// ---------------------------------------------------------------------------
// Embedding Provider Types
// ---------------------------------------------------------------------------

/** Embedding provider type identifiers (matches Rust `EmbeddingProviderType` serde). */
export type EmbeddingProviderType = 'tf_idf' | 'ollama' | 'qwen' | 'glm' | 'open_ai';

/** Capability metadata for an embedding provider. */
export interface EmbeddingProviderCapability {
  provider_type: EmbeddingProviderType;
  display_name: string;
  is_local: boolean;
  requires_api_key: boolean;
  default_model: string;
  default_dimension: number;
  max_batch_size: number;
  supported_dimensions?: number[];
}

// ---------------------------------------------------------------------------
// IPC Request Types
// ---------------------------------------------------------------------------

/** Request payload for `set_embedding_config`. */
export interface SetEmbeddingConfigRequest {
  provider: string;
  model?: string;
  base_url?: string;
  dimension?: number;
  batch_size?: number;
  fallback_provider?: string;
}

/** Request payload for `check_embedding_provider_health`. */
export interface CheckEmbeddingHealthRequest {
  provider: string;
  model?: string;
  base_url?: string;
}

/** Request payload for `get_embedding_api_key`. */
export interface GetEmbeddingApiKeyRequest {
  provider: string;
}

/** Request payload for `set_embedding_api_key`. */
export interface SetEmbeddingApiKeyRequest {
  provider: string;
  api_key: string;
}

// ---------------------------------------------------------------------------
// IPC Response Types
// ---------------------------------------------------------------------------

/** Response from `get_embedding_config`. */
export interface EmbeddingConfigResponse {
  provider: string;
  model: string;
  base_url?: string;
  dimension: number;
  batch_size: number;
  fallback_provider?: string;
}

/** Response from `set_embedding_config`. */
export interface SetEmbeddingConfigResponse {
  provider: string;
  model: string;
  reindex_required: boolean;
}

/** Response from `check_embedding_provider_health`. */
export interface EmbeddingHealthResponse {
  healthy: boolean;
  message: string;
  latency_ms?: number;
}

/** Response from `set_embedding_api_key`. */
export interface SetEmbeddingApiKeyResponse {
  success: boolean;
}

// ---------------------------------------------------------------------------
// Keyring alias mapping for embedding providers
// ---------------------------------------------------------------------------

/** Maps EmbeddingProviderType to keyring alias for API key storage. */
export const EMBEDDING_KEYRING_ALIASES: Partial<Record<EmbeddingProviderType, string>> = {
  qwen: 'qwen_embedding',
  glm: 'glm_embedding',
  open_ai: 'openai_embedding',
};

/** Cloud embedding providers that require API keys. */
export const CLOUD_EMBEDDING_PROVIDERS: EmbeddingProviderType[] = ['qwen', 'glm', 'open_ai'];

/** Local embedding providers that do not require API keys. */
export const LOCAL_EMBEDDING_PROVIDERS: EmbeddingProviderType[] = ['tf_idf', 'ollama'];
