const PROVIDER_ALIASES: Record<string, string> = {
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

const GLM_CODING_BASE_URL = 'https://open.bigmodel.cn/api/coding/paas/v4/chat/completions';
const GLM_INTL_BASE_URL = 'https://api.z.ai/api/paas/v4/chat/completions';
const GLM_INTL_CODING_BASE_URL = 'https://api.z.ai/api/coding/paas/v4/chat/completions';
const MINIMAX_CHINA_BASE_URL = 'https://api.minimaxi.com/v1/chat/completions';
const QWEN_SINGAPORE_BASE_URL = 'https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions';
const QWEN_US_BASE_URL = 'https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions';

/** Default model per provider, used when user selects "Provider default". */
export const DEFAULT_MODEL_BY_PROVIDER: Record<string, string> = {
  anthropic: 'claude-sonnet-4-6-20260219',
  openai: 'gpt-5.1',
  deepseek: 'deepseek-chat',
  glm: 'glm-5',
  qwen: 'qwen3-max',
  minimax: 'MiniMax-M2.5',
  ollama: 'llama3.2',
};

export function normalizeProviderName(value: string | null | undefined): string | null {
  if (!value) return null;
  const normalized = value.trim().toLowerCase();
  return PROVIDER_ALIASES[normalized] || null;
}

export function isClaudeCodeBackend(value: string | null | undefined): boolean {
  if (!value) return false;
  const normalized = value.trim().toLowerCase();
  return normalized === 'claude-code' || normalized === 'claude_code' || normalized === 'claudecode';
}

function inferProviderFromModel(model: string | null | undefined): string | null {
  if (!model) return null;
  const normalized = model.trim().toLowerCase();
  if (!normalized) return null;

  if (normalized.includes('glm')) return 'glm';
  if (normalized.includes('qwen') || normalized.includes('qwq')) return 'qwen';
  if (normalized.includes('deepseek')) return 'deepseek';
  if (normalized.includes('minimax')) return 'minimax';
  if (normalized.includes('claude')) return 'anthropic';
  if (normalized.startsWith('gpt') || normalized.startsWith('o1') || normalized.startsWith('o3')) return 'openai';
  return null;
}

export function resolveStandaloneProvider(
  rawBackend: string | null | undefined,
  rawProvider: string | null | undefined,
  rawModel: string | null | undefined,
): string {
  const backendCandidate = normalizeProviderName(rawBackend);
  const providerCandidate = normalizeProviderName(rawProvider);
  const modelCandidate = inferProviderFromModel(rawModel);

  // When backend/provider conflict, trust model hint first, then provider setting.
  if (backendCandidate && providerCandidate && backendCandidate !== providerCandidate) {
    if (modelCandidate === providerCandidate) return providerCandidate;
    if (modelCandidate === backendCandidate) return backendCandidate;
    return providerCandidate;
  }

  return backendCandidate || providerCandidate || modelCandidate || 'anthropic';
}

/**
 * Resolve provider-specific base URL override from user settings.
 * GLM has standard/coding endpoints; MiniMax has international/china endpoints;
 * Qwen has china/singapore/us endpoints.
 */
export function resolveProviderBaseUrl(
  provider: string,
  settings: {
    glmEndpoint?: string;
    minimaxEndpoint?: string;
    qwenEndpoint?: string;
    customProviderBaseUrls?: Record<string, string>;
  },
): string | undefined {
  const normalized = normalizeProviderName(provider);
  const customBaseUrl = normalized ? settings.customProviderBaseUrls?.[normalized]?.trim() : '';
  if (customBaseUrl) {
    return customBaseUrl;
  }
  if (normalized === 'glm' && settings.glmEndpoint === 'coding') {
    return GLM_CODING_BASE_URL;
  }
  if (normalized === 'glm' && settings.glmEndpoint === 'international') {
    return GLM_INTL_BASE_URL;
  }
  if (normalized === 'glm' && settings.glmEndpoint === 'international-coding') {
    return GLM_INTL_CODING_BASE_URL;
  }
  if (normalized === 'minimax' && settings.minimaxEndpoint === 'china') {
    return MINIMAX_CHINA_BASE_URL;
  }
  if (normalized === 'qwen' && settings.qwenEndpoint === 'singapore') {
    return QWEN_SINGAPORE_BASE_URL;
  }
  if (normalized === 'qwen' && settings.qwenEndpoint === 'us') {
    return QWEN_US_BASE_URL;
  }
  return undefined;
}

export function parseMemoryReviewAgentProvider(reviewAgentRef: string | null | undefined): string | null {
  if (!reviewAgentRef) return null;
  const trimmed = reviewAgentRef.trim();
  if (!trimmed.startsWith('llm:')) return null;
  const [, provider] = trimmed.split(':', 3);
  return normalizeProviderName(provider);
}
