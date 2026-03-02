import { invoke } from '@tauri-apps/api/core';
import type { Backend } from '../store/settings';
import { normalizeProvider } from './providers';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface ProviderCatalogModel {
  id: string;
  context_window?: number;
}

interface ProviderCatalog {
  provider_type: string;
  models: ProviderCatalogModel[];
}

export const DEFAULT_PROMPT_TOKEN_BUDGET = 160_000;

const PROVIDER_FALLBACK_BUDGETS: Record<string, number> = {
  anthropic: 200_000,
  openai: 200_000,
  deepseek: 128_000,
  glm: 200_000,
  qwen: 262_144,
  minimax: 204_800,
  ollama: 8_192,
};

let modelBudgetMapCache: Map<string, Map<string, number>> | null = null;
let providerDefaultBudgetMapCache: Map<string, number> | null = null;
let loadCatalogPromise: Promise<void> | null = null;

function normalizeModel(model: string | null | undefined): string {
  return (model || '').trim().toLowerCase();
}

function normalizeContextWindow(value: unknown): number | null {
  if (typeof value !== 'number') return null;
  if (!Number.isFinite(value)) return null;
  if (value <= 0) return null;
  return Math.round(value);
}

function resolveCanonicalProvider(backend?: Backend | null, provider?: string | null): string {
  const normalizedProvider = normalizeProvider(provider || '');
  if (normalizedProvider) return normalizedProvider;
  if (backend === 'claude-code' || backend === 'claude-api') return 'anthropic';
  return '';
}

function isProviderDefaultPlaceholder(model: string): boolean {
  return model === '__provider_default__' || model === '__custom__';
}

async function ensureProviderCatalogLoaded(): Promise<void> {
  if (modelBudgetMapCache && providerDefaultBudgetMapCache) return;
  if (loadCatalogPromise) {
    await loadCatalogPromise;
    return;
  }

  loadCatalogPromise = (async () => {
    try {
      const result = await invoke<CommandResponse<ProviderCatalog[]>>('list_providers');
      if (!result.success || !result.data) return;

      const nextModelBudgetMap = new Map<string, Map<string, number>>();
      const nextProviderDefaultMap = new Map<string, number>();

      for (const providerInfo of result.data) {
        const provider = normalizeProvider(providerInfo.provider_type || '');
        if (!provider) continue;

        const modelMap = new Map<string, number>();
        let providerDefault: number | null = null;
        for (const model of providerInfo.models || []) {
          const modelId = normalizeModel(model.id);
          if (!modelId) continue;
          const contextWindow = normalizeContextWindow(model.context_window);
          if (contextWindow == null) continue;
          modelMap.set(modelId, contextWindow);
          if (providerDefault == null) {
            providerDefault = contextWindow;
          }
        }

        nextModelBudgetMap.set(provider, modelMap);
        if (providerDefault != null) {
          nextProviderDefaultMap.set(provider, providerDefault);
        }
      }

      modelBudgetMapCache = nextModelBudgetMap;
      providerDefaultBudgetMapCache = nextProviderDefaultMap;
    } catch {
      // Fall through to static fallback budgets.
    } finally {
      loadCatalogPromise = null;
    }
  })();

  await loadCatalogPromise;
}

export function resolvePromptTokenBudgetSync(params: {
  backend?: Backend | null;
  provider?: string | null;
  model?: string | null;
  fallbackBudget?: number;
}): number {
  const fallbackBudget = params.fallbackBudget ?? DEFAULT_PROMPT_TOKEN_BUDGET;
  const provider = resolveCanonicalProvider(params.backend, params.provider);
  const normalizedModel = normalizeModel(params.model);

  const modelBudgetMap = provider ? (modelBudgetMapCache?.get(provider) ?? null) : null;
  if (provider && modelBudgetMap && normalizedModel && !isProviderDefaultPlaceholder(normalizedModel)) {
    const matched = modelBudgetMap.get(normalizedModel);
    if (typeof matched === 'number' && matched > 0) {
      return matched;
    }
  }

  const providerDefaultBudget = provider ? providerDefaultBudgetMapCache?.get(provider) : undefined;
  if (typeof providerDefaultBudget === 'number' && providerDefaultBudget > 0) {
    return providerDefaultBudget;
  }

  if (provider && typeof PROVIDER_FALLBACK_BUDGETS[provider] === 'number') {
    return PROVIDER_FALLBACK_BUDGETS[provider];
  }

  return fallbackBudget;
}

export async function resolvePromptTokenBudget(params: {
  backend?: Backend | null;
  provider?: string | null;
  model?: string | null;
  fallbackBudget?: number;
}): Promise<number> {
  await ensureProviderCatalogLoaded();
  return resolvePromptTokenBudgetSync(params);
}
