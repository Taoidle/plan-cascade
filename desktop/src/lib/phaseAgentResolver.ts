/**
 * Phase Agent Resolver
 *
 * Resolves "llm:provider:model" agent references from phase configs into
 * concrete LLM parameters (provider, model, baseUrl) for planning phases.
 *
 * Priority: phase defaultAgent → phase fallbackChain → global settings.
 */

import { useSettingsStore } from '../store/settings';
import { resolveProviderBaseUrl, normalizeProvider } from './providers';

export interface ResolvedLlmParams {
  provider: string;
  model: string;
  baseUrl: string | undefined;
  source: 'phase_default' | 'phase_fallback' | 'global';
}

/** Parse "llm:provider:model" format. Returns null if not an LLM ref. */
export function parseLlmAgentRef(agentRef: string): { provider: string; model: string } | null {
  if (!agentRef?.startsWith('llm:')) return null;
  const parts = agentRef.split(':');
  if (parts.length < 3) return null;
  const provider = parts[1];
  const model = parts.slice(2).join(':'); // handles "llm:ollama:deepseek-r1:14b"
  return provider && model ? { provider: normalizeProvider(provider), model } : null;
}

/** Resolve phase agent config → LLM params. Priority: phase default > fallback > global. */
export function resolvePhaseAgent(phaseId: string): ResolvedLlmParams {
  const settings = useSettingsStore.getState();
  const {
    phaseConfigs,
    provider: globalProvider,
    model: globalModel,
    glmEndpoint,
    qwenEndpoint,
    minimaxEndpoint,
  } = settings;
  const endpointSettings = { glmEndpoint, qwenEndpoint, minimaxEndpoint };
  const config = phaseConfigs[phaseId];

  // 1. Phase default agent
  if (config?.defaultAgent) {
    const parsed = parseLlmAgentRef(config.defaultAgent);
    if (parsed) {
      return {
        ...parsed,
        baseUrl: resolveProviderBaseUrl(parsed.provider, endpointSettings),
        source: 'phase_default',
      };
    }
  }

  // 2. Fallback chain
  if (config?.fallbackChain) {
    for (const fb of config.fallbackChain) {
      const parsed = parseLlmAgentRef(fb);
      if (parsed) {
        return {
          ...parsed,
          baseUrl: resolveProviderBaseUrl(parsed.provider, endpointSettings),
          source: 'phase_fallback',
        };
      }
    }
  }

  // 3. Global settings
  return {
    provider: globalProvider,
    model: globalModel,
    baseUrl: globalProvider ? resolveProviderBaseUrl(globalProvider, endpointSettings) : undefined,
    source: 'global',
  };
}

/** Format model name for card display. */
export function formatModelDisplay(resolved: ResolvedLlmParams): string {
  if (!resolved.model && !resolved.provider) return '';
  return resolved.model || 'default';
}
