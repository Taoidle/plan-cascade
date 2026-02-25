import { describe, it, expect, beforeEach } from 'vitest';
import {
  BACKEND_OPTIONS,
  FALLBACK_MODELS_BY_PROVIDER,
  DEFAULT_MODEL_BY_PROVIDER,
  PROVIDER_ALIASES,
  CUSTOM_MODELS_STORAGE_KEY,
  LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY,
  normalizeProvider,
  dedupeModels,
  getLocalProviderApiKeyStatuses,
  readLocalProviderApiKeyCache,
  writeLocalProviderApiKeyCache,
  getLocalProviderApiKey,
  setLocalProviderApiKey,
  getCustomModelsByProvider,
  setCustomModelsByProvider,
  type BackendOption,
  type ApiKeyStatus,
} from './providers';

describe('providers module', () => {
  // =========================================================================
  // Constants
  // =========================================================================
  describe('BACKEND_OPTIONS', () => {
    it('contains all expected backend options', () => {
      const ids = BACKEND_OPTIONS.map((o) => o.id);
      expect(ids).toContain('claude-code');
      expect(ids).toContain('claude-api');
      expect(ids).toContain('openai');
      expect(ids).toContain('deepseek');
      expect(ids).toContain('glm');
      expect(ids).toContain('qwen');
      expect(ids).toContain('minimax');
      expect(ids).toContain('ollama');
    });

    it('claude-code does not require API key', () => {
      const option = BACKEND_OPTIONS.find((o) => o.id === 'claude-code');
      expect(option).toBeDefined();
      expect(option!.requiresApiKey).toBe(false);
    });

    it('openai requires API key', () => {
      const option = BACKEND_OPTIONS.find((o) => o.id === 'openai');
      expect(option).toBeDefined();
      expect(option!.requiresApiKey).toBe(true);
    });

    it('every option has a provider field', () => {
      BACKEND_OPTIONS.forEach((option: BackendOption) => {
        expect(option.provider).toBeDefined();
        expect(typeof option.provider).toBe('string');
      });
    });
  });

  describe('FALLBACK_MODELS_BY_PROVIDER', () => {
    it('has entries for all major providers', () => {
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('anthropic');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('openai');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('deepseek');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('glm');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('qwen');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('minimax');
      expect(FALLBACK_MODELS_BY_PROVIDER).toHaveProperty('ollama');
    });

    it('each provider has at least one model', () => {
      Object.entries(FALLBACK_MODELS_BY_PROVIDER).forEach(([_provider, models]) => {
        expect(models.length).toBeGreaterThan(0);
      });
    });
  });

  describe('DEFAULT_MODEL_BY_PROVIDER', () => {
    it('has a default model for each provider with fallback models', () => {
      Object.keys(FALLBACK_MODELS_BY_PROVIDER).forEach((provider) => {
        expect(DEFAULT_MODEL_BY_PROVIDER).toHaveProperty(provider);
      });
    });

    it('default model is always the first fallback model', () => {
      Object.entries(DEFAULT_MODEL_BY_PROVIDER).forEach(([provider, defaultModel]) => {
        const fallbacks = FALLBACK_MODELS_BY_PROVIDER[provider];
        expect(fallbacks).toBeDefined();
        expect(fallbacks[0]).toBe(defaultModel);
      });
    });
  });

  describe('PROVIDER_ALIASES', () => {
    it('maps claude to anthropic', () => {
      expect(PROVIDER_ALIASES['claude']).toBe('anthropic');
      expect(PROVIDER_ALIASES['claude-api']).toBe('anthropic');
    });

    it('maps zhipu/zhipuai to glm', () => {
      expect(PROVIDER_ALIASES['zhipu']).toBe('glm');
      expect(PROVIDER_ALIASES['zhipuai']).toBe('glm');
    });

    it('maps dashscope/alibaba/aliyun to qwen', () => {
      expect(PROVIDER_ALIASES['dashscope']).toBe('qwen');
      expect(PROVIDER_ALIASES['alibaba']).toBe('qwen');
      expect(PROVIDER_ALIASES['aliyun']).toBe('qwen');
    });
  });

  // =========================================================================
  // normalizeProvider
  // =========================================================================
  describe('normalizeProvider', () => {
    it('returns canonical provider name from alias', () => {
      expect(normalizeProvider('claude')).toBe('anthropic');
      expect(normalizeProvider('Claude-API')).toBe('anthropic');
      expect(normalizeProvider('OPENAI')).toBe('openai');
    });

    it('trims and lowercases input', () => {
      expect(normalizeProvider('  DeepSeek  ')).toBe('deepseek');
    });

    it('returns input lowercase when no alias matches', () => {
      expect(normalizeProvider('some-unknown-provider')).toBe('some-unknown-provider');
    });

    it('handles empty string', () => {
      expect(normalizeProvider('')).toBe('');
    });
  });

  // =========================================================================
  // dedupeModels
  // =========================================================================
  describe('dedupeModels', () => {
    it('removes duplicate model ids', () => {
      expect(dedupeModels(['gpt-4o', 'gpt-4o', 'o1-preview'])).toEqual(['gpt-4o', 'o1-preview']);
    });

    it('trims model names', () => {
      expect(dedupeModels(['  gpt-4o  ', 'gpt-4o'])).toEqual(['gpt-4o']);
    });

    it('filters out empty strings', () => {
      expect(dedupeModels(['gpt-4o', '', '  ', 'o1-preview'])).toEqual(['gpt-4o', 'o1-preview']);
    });

    it('returns empty array for empty input', () => {
      expect(dedupeModels([])).toEqual([]);
    });
  });

  // =========================================================================
  // LocalStorage API Key helpers
  // =========================================================================
  describe('localStorage API key helpers', () => {
    beforeEach(() => {
      localStorage.clear();
    });

    describe('readLocalProviderApiKeyCache', () => {
      it('returns empty object when nothing stored', () => {
        expect(readLocalProviderApiKeyCache()).toEqual({});
      });

      it('normalizes provider names', () => {
        localStorage.setItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY, JSON.stringify({ Claude: 'sk-test' }));
        const cache = readLocalProviderApiKeyCache();
        expect(cache['anthropic']).toBe('sk-test');
      });

      it('ignores non-string values', () => {
        localStorage.setItem(
          LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY,
          JSON.stringify({ openai: 'sk-valid', deepseek: 123 }),
        );
        const cache = readLocalProviderApiKeyCache();
        expect(cache['openai']).toBe('sk-valid');
        expect(cache['deepseek']).toBeUndefined();
      });

      it('ignores empty/whitespace-only values', () => {
        localStorage.setItem(
          LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY,
          JSON.stringify({ openai: '  ', deepseek: 'sk-ok' }),
        );
        const cache = readLocalProviderApiKeyCache();
        expect(cache['openai']).toBeUndefined();
        expect(cache['deepseek']).toBe('sk-ok');
      });
    });

    describe('writeLocalProviderApiKeyCache', () => {
      it('writes cache to localStorage', () => {
        writeLocalProviderApiKeyCache({ openai: 'sk-test' });
        const stored = localStorage.getItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY);
        expect(JSON.parse(stored!)).toEqual({ openai: 'sk-test' });
      });
    });

    describe('getLocalProviderApiKey', () => {
      it('returns stored key for provider', () => {
        writeLocalProviderApiKeyCache({ anthropic: 'sk-ant-test' });
        expect(getLocalProviderApiKey('anthropic')).toBe('sk-ant-test');
      });

      it('normalizes provider name before lookup', () => {
        writeLocalProviderApiKeyCache({ anthropic: 'sk-ant-test' });
        expect(getLocalProviderApiKey('claude')).toBe('sk-ant-test');
      });

      it('returns empty string for missing provider', () => {
        expect(getLocalProviderApiKey('unknown-provider')).toBe('');
      });
    });

    describe('setLocalProviderApiKey', () => {
      it('sets API key for provider', () => {
        setLocalProviderApiKey('openai', 'sk-openai-test');
        expect(getLocalProviderApiKey('openai')).toBe('sk-openai-test');
      });

      it('normalizes provider name', () => {
        setLocalProviderApiKey('claude', 'sk-ant');
        expect(getLocalProviderApiKey('anthropic')).toBe('sk-ant');
      });

      it('removes provider when key is empty', () => {
        setLocalProviderApiKey('openai', 'sk-test');
        setLocalProviderApiKey('openai', '');
        expect(getLocalProviderApiKey('openai')).toBe('');
      });

      it('trims key before storing', () => {
        setLocalProviderApiKey('openai', '  sk-trimmed  ');
        expect(getLocalProviderApiKey('openai')).toBe('sk-trimmed');
      });
    });

    describe('getLocalProviderApiKeyStatuses', () => {
      it('returns empty object when no keys stored', () => {
        expect(getLocalProviderApiKeyStatuses()).toEqual({});
      });

      it('returns true for providers with keys', () => {
        writeLocalProviderApiKeyCache({ openai: 'sk-test', anthropic: 'sk-ant' });
        const statuses: ApiKeyStatus = getLocalProviderApiKeyStatuses();
        expect(statuses['openai']).toBe(true);
        expect(statuses['anthropic']).toBe(true);
      });
    });
  });

  // =========================================================================
  // Custom models localStorage helpers
  // =========================================================================
  describe('custom models localStorage helpers', () => {
    beforeEach(() => {
      localStorage.clear();
    });

    describe('getCustomModelsByProvider', () => {
      it('returns empty object when nothing stored', () => {
        expect(getCustomModelsByProvider()).toEqual({});
      });

      it('normalizes provider keys', () => {
        localStorage.setItem(CUSTOM_MODELS_STORAGE_KEY, JSON.stringify({ Claude: ['my-model'] }));
        const result = getCustomModelsByProvider();
        expect(result['anthropic']).toEqual(['my-model']);
      });

      it('deduplicates models', () => {
        localStorage.setItem(
          CUSTOM_MODELS_STORAGE_KEY,
          JSON.stringify({ openai: ['gpt-custom', 'gpt-custom', 'gpt-other'] }),
        );
        const result = getCustomModelsByProvider();
        expect(result['openai']).toEqual(['gpt-custom', 'gpt-other']);
      });
    });

    describe('setCustomModelsByProvider', () => {
      it('persists custom models to localStorage', () => {
        setCustomModelsByProvider({ openai: ['my-model'] });
        const stored = localStorage.getItem(CUSTOM_MODELS_STORAGE_KEY);
        expect(JSON.parse(stored!)).toEqual({ openai: ['my-model'] });
      });
    });
  });
});
