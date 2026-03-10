/**
 * Phase Agent Resolver
 *
 * Task/workflow planning phases remain LLM-only. Plan mode phases use a
 * generic agent-ref model so execution/retry can eventually support CLI
 * backends without changing the settings schema again.
 */

import { useSettingsStore } from '../store/settings';
import { resolveProviderBaseUrl, normalizeProvider } from './providers';

export interface ResolvedLlmParams {
  provider: string;
  model: string;
  baseUrl: string | undefined;
  source: 'phase_default' | 'phase_fallback' | 'global';
}

export type ResolvedPlanPhaseAgentSource = ResolvedLlmParams['source'];

export type ResolvedPlanPhaseAgent =
  | {
      kind: 'llm';
      agentRef: string | null;
      source: ResolvedPlanPhaseAgentSource;
      provider: string;
      model: string;
      baseUrl: string | undefined;
    }
  | {
      kind: 'cli';
      agentRef: string;
      source: Exclude<ResolvedPlanPhaseAgentSource, 'global'>;
      agentName: string;
    };

/** Parse "llm:provider:model" format. Returns null if not an LLM ref. */
export function parseLlmAgentRef(agentRef: string): { provider: string; model: string } | null {
  if (!agentRef?.startsWith('llm:')) return null;
  const parts = agentRef.split(':');
  if (parts.length < 3) return null;
  const provider = parts[1];
  const model = parts.slice(2).join(':'); // handles "llm:ollama:deepseek-r1:14b"
  return provider && model ? { provider: normalizeProvider(provider), model } : null;
}

export function parseCliAgentRef(agentRef: string): { agentName: string } | null {
  if (!agentRef?.startsWith('cli:')) return null;
  const agentName = agentRef.slice('cli:'.length).trim();
  return agentName ? { agentName } : null;
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

export function resolvePlanPhaseAgent(phaseId: string): ResolvedPlanPhaseAgent {
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

  const resolveConfiguredRef = (
    agentRef: string | undefined,
    source: Exclude<ResolvedPlanPhaseAgentSource, 'global'>,
  ): ResolvedPlanPhaseAgent | null => {
    if (!agentRef) return null;
    const parsedLlm = parseLlmAgentRef(agentRef);
    if (parsedLlm) {
      return {
        kind: 'llm',
        agentRef,
        source,
        provider: parsedLlm.provider,
        model: parsedLlm.model,
        baseUrl: resolveProviderBaseUrl(parsedLlm.provider, endpointSettings),
      };
    }
    const parsedCli = parseCliAgentRef(agentRef);
    if (parsedCli) {
      return {
        kind: 'cli',
        agentRef,
        source,
        agentName: parsedCli.agentName,
      };
    }
    return null;
  };

  const phaseDefault = resolveConfiguredRef(config?.defaultAgent, 'phase_default');
  if (phaseDefault) return phaseDefault;

  for (const fallback of config?.fallbackChain ?? []) {
    const resolved = resolveConfiguredRef(fallback, 'phase_fallback');
    if (resolved) return resolved;
  }

  return {
    kind: 'llm',
    agentRef: null,
    source: 'global',
    provider: globalProvider,
    model: globalModel,
    baseUrl: globalProvider ? resolveProviderBaseUrl(globalProvider, endpointSettings) : undefined,
  };
}

/** Format model name for card display. */
export function formatModelDisplay(resolved: ResolvedLlmParams): string {
  if (!resolved.model && !resolved.provider) return '';
  return resolved.model || 'default';
}

export function formatResolvedPlanAgentDisplay(resolved: ResolvedPlanPhaseAgent): string {
  if (resolved.kind === 'cli') {
    return resolved.agentName;
  }
  return formatModelDisplay(resolved);
}
