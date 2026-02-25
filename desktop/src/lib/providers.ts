/**
 * Shared Provider Module
 *
 * Extracts provider-related constants, types, and utility functions that are
 * shared across the application (Settings, Model Switcher, etc.).
 *
 * Originally defined inline in LLMBackendSection.tsx.
 */

import type { Backend } from '../store/settings';

// ============================================================================
// Types
// ============================================================================

export interface BackendOption {
  id: Backend;
  i18nKey: string;
  requiresApiKey: boolean;
  provider: string;
}

export interface ApiKeyStatus {
  [provider: string]: boolean;
}

// ============================================================================
// Constants
// ============================================================================

export const BACKEND_OPTIONS: BackendOption[] = [
  {
    id: 'claude-code',
    i18nKey: 'claude-code',
    requiresApiKey: false,
    provider: 'anthropic',
  },
  {
    id: 'claude-api',
    i18nKey: 'claude-api',
    requiresApiKey: true,
    provider: 'anthropic',
  },
  {
    id: 'openai',
    i18nKey: 'openai',
    requiresApiKey: true,
    provider: 'openai',
  },
  {
    id: 'deepseek',
    i18nKey: 'deepseek',
    requiresApiKey: true,
    provider: 'deepseek',
  },
  {
    id: 'glm',
    i18nKey: 'glm',
    requiresApiKey: true,
    provider: 'glm',
  },
  {
    id: 'qwen',
    i18nKey: 'qwen',
    requiresApiKey: true,
    provider: 'qwen',
  },
  {
    id: 'minimax',
    i18nKey: 'minimax',
    requiresApiKey: true,
    provider: 'minimax',
  },
  {
    id: 'ollama',
    i18nKey: 'ollama',
    requiresApiKey: false,
    provider: 'ollama',
  },
];

export const PROVIDER_ALIASES: Record<string, string> = {
  anthropic: 'anthropic',
  claude: 'anthropic',
  'claude-api': 'anthropic',
  openai: 'openai',
  deepseek: 'deepseek',
  glm: 'glm',
  'glm-api': 'glm',
  zhipu: 'glm',
  zhipuai: 'glm',
  qwen: 'qwen',
  'qwen-api': 'qwen',
  dashscope: 'qwen',
  alibaba: 'qwen',
  aliyun: 'qwen',
  minimax: 'minimax',
  'minimax-api': 'minimax',
  ollama: 'ollama',
};

export const FALLBACK_MODELS_BY_PROVIDER: Record<string, string[]> = {
  anthropic: ['claude-3-5-sonnet-20241022', 'claude-3-opus-20240229'],
  openai: ['gpt-4o', 'o1-preview', 'o3-mini'],
  deepseek: ['deepseek-chat', 'deepseek-r1'],
  glm: [
    'glm-5',
    'glm-4.7',
    'glm-4.6',
    'glm-4.6v',
    'glm-4.6v-flash',
    'glm-4.6v-flashx',
    'glm-4.5',
    'glm-4.5-air',
    'glm-4.5-x',
    'glm-4.5-flash',
    'glm-4.5v',
    'glm-4.1v-thinking-flashx',
    'glm-4.1v-thinking-flash',
    'glm-4-air-250414',
    'glm-4-flash-250414',
    'glm-4-plus',
    'glm-4-air',
    'glm-4-airx',
    'glm-4-flash',
    'glm-4-flashx',
    'glm-4v-plus-0111',
    'glm-4v-flash',
  ],
  qwen: ['qwen3-max', 'qwen3-plus', 'qwen3-coder', 'qwq-plus', 'qwen-max', 'qwen-plus', 'qwen-turbo'],
  minimax: ['MiniMax-M2.5', 'MiniMax-M2.5-highspeed', 'MiniMax-M2.1', 'MiniMax-M2.1-highspeed', 'MiniMax-M2'],
  ollama: ['llama3.2', 'deepseek-r1:14b', 'qwq:32b'],
};

/**
 * Default model for each provider (first entry in FALLBACK_MODELS_BY_PROVIDER).
 */
export const DEFAULT_MODEL_BY_PROVIDER: Record<string, string> = Object.fromEntries(
  Object.entries(FALLBACK_MODELS_BY_PROVIDER).map(([provider, models]) => [provider, models[0]]),
);

export const CUSTOM_MODELS_STORAGE_KEY = 'plan-cascade-custom-models';
export const LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY = 'plan-cascade-provider-api-key-cache';
export const MODEL_DEFAULT_VALUE = '__provider_default__';
export const MODEL_CUSTOM_VALUE = '__custom__';

// ============================================================================
// Functions
// ============================================================================

/**
 * Normalize a provider name string to its canonical form using PROVIDER_ALIASES.
 * Trims whitespace, lowercases, and resolves aliases.
 */
export function normalizeProvider(provider: string): string {
  const normalized = provider.trim().toLowerCase();
  return PROVIDER_ALIASES[normalized] || normalized;
}

/**
 * Deduplicate a list of model ID strings.
 * Trims each entry and removes empty strings.
 */
export function dedupeModels(models: string[]): string[] {
  return Array.from(new Set(models.map((m) => m.trim()).filter(Boolean)));
}

/**
 * Compute the list of providers that require an API key (deduplicated, normalized).
 */
export function getApiKeyRequiredProviders(): string[] {
  return dedupeModels(
    BACKEND_OPTIONS.filter((option) => option.requiresApiKey).map((option) => normalizeProvider(option.provider)),
  );
}

// ============================================================================
// localStorage helpers: API key cache
// ============================================================================

export function readLocalProviderApiKeyCache(): Record<string, string> {
  try {
    const raw = localStorage.getItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const normalized: Record<string, string> = {};
    Object.entries(parsed).forEach(([provider, value]) => {
      if (typeof value !== 'string') return;
      const trimmed = value.trim();
      if (!trimmed) return;
      normalized[normalizeProvider(provider)] = trimmed;
    });
    return normalized;
  } catch {
    return {};
  }
}

export function writeLocalProviderApiKeyCache(nextValue: Record<string, string>): void {
  localStorage.setItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY, JSON.stringify(nextValue));
}

export function getLocalProviderApiKey(provider: string): string {
  const cache = readLocalProviderApiKeyCache();
  return cache[normalizeProvider(provider)] || '';
}

export function setLocalProviderApiKey(provider: string, apiKey: string): void {
  const normalizedProvider = normalizeProvider(provider);
  const cache = readLocalProviderApiKeyCache();
  const trimmed = apiKey.trim();
  if (trimmed) {
    cache[normalizedProvider] = trimmed;
  } else {
    delete cache[normalizedProvider];
  }
  writeLocalProviderApiKeyCache(cache);
}

export function getLocalProviderApiKeyStatuses(): ApiKeyStatus {
  const cache = readLocalProviderApiKeyCache();
  const statuses: ApiKeyStatus = {};
  Object.entries(cache).forEach(([provider, value]) => {
    if (value.trim()) {
      statuses[provider] = true;
    }
  });
  return statuses;
}

// ============================================================================
// localStorage helpers: Custom models
// ============================================================================

export function getCustomModelsByProvider(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(CUSTOM_MODELS_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, string[]>;
    const normalized: Record<string, string[]> = {};
    Object.entries(parsed).forEach(([provider, models]) => {
      normalized[normalizeProvider(provider)] = dedupeModels(Array.isArray(models) ? models : []);
    });
    return normalized;
  } catch {
    return {};
  }
}

export function setCustomModelsByProvider(value: Record<string, string[]>): void {
  localStorage.setItem(CUSTOM_MODELS_STORAGE_KEY, JSON.stringify(value));
}

// ============================================================================
// Provider endpoint → base URL resolution
// ============================================================================

const GLM_CODING_BASE_URL = 'https://open.bigmodel.cn/api/coding/paas/v4/chat/completions';
const GLM_INTL_BASE_URL = 'https://api.z.ai/api/paas/v4/chat/completions';
const GLM_INTL_CODING_BASE_URL = 'https://api.z.ai/api/coding/paas/v4/chat/completions';
const MINIMAX_CHINA_BASE_URL = 'https://api.minimaxi.com/v1/chat/completions';
const QWEN_SINGAPORE_BASE_URL = 'https://dashscope-intl.aliyuncs.com/api/v1';
const QWEN_US_BASE_URL = 'https://dashscope-us.aliyuncs.com/api/v1';

/**
 * Resolve provider-specific base URL override from user endpoint settings.
 * Returns `undefined` when the default endpoint should be used.
 */
export function resolveProviderBaseUrl(
  provider: string,
  settings: { glmEndpoint?: string; minimaxEndpoint?: string; qwenEndpoint?: string },
): string | undefined {
  const normalized = normalizeProvider(provider);
  if (normalized === 'glm') {
    if (settings.glmEndpoint === 'coding') return GLM_CODING_BASE_URL;
    if (settings.glmEndpoint === 'international') return GLM_INTL_BASE_URL;
    if (settings.glmEndpoint === 'international-coding') return GLM_INTL_CODING_BASE_URL;
  }
  if (normalized === 'minimax' && settings.minimaxEndpoint === 'china') {
    return MINIMAX_CHINA_BASE_URL;
  }
  if (normalized === 'qwen') {
    if (settings.qwenEndpoint === 'singapore') return QWEN_SINGAPORE_BASE_URL;
    if (settings.qwenEndpoint === 'us') return QWEN_US_BASE_URL;
  }
  return undefined;
}
