import { describe, expect, it } from 'vitest';
import enUpdates from '../locales/en/updates.json';
import zhUpdates from '../locales/zh/updates.json';
import jaUpdates from '../locales/ja/updates.json';

type LocaleTree = Record<string, unknown>;

function flattenLeafKeys(tree: unknown, prefix = ''): Map<string, string> {
  const result = new Map<string, string>();
  if (!tree || typeof tree !== 'object' || Array.isArray(tree)) return result;
  for (const [key, value] of Object.entries(tree as Record<string, unknown>)) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') {
      result.set(next, value);
      continue;
    }
    const nested = flattenLeafKeys(value, next);
    for (const [nestedKey, nestedValue] of nested) {
      result.set(nestedKey, nestedValue);
    }
  }
  return result;
}

function extractPlaceholders(input: string): string[] {
  const placeholders = new Set<string>();
  const pattern = /\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g;
  let match: RegExpExecArray | null = null;
  while (true) {
    match = pattern.exec(input);
    if (!match) break;
    if (match[1]) placeholders.add(match[1]);
  }
  return [...placeholders].sort();
}

describe('updates i18n parity', () => {
  it('keeps en/zh/ja update locale keys and placeholders aligned', () => {
    const enMap = flattenLeafKeys(enUpdates as LocaleTree);
    const zhMap = flattenLeafKeys(zhUpdates as LocaleTree);
    const jaMap = flattenLeafKeys(jaUpdates as LocaleTree);

    const allKeys = new Set<string>([...enMap.keys(), ...zhMap.keys(), ...jaMap.keys()]);
    for (const key of allKeys) {
      expect(enMap.has(key), `[updates] missing key '${key}' in en`).toBe(true);
      expect(zhMap.has(key), `[updates] missing key '${key}' in zh`).toBe(true);
      expect(jaMap.has(key), `[updates] missing key '${key}' in ja`).toBe(true);
    }

    for (const key of enMap.keys()) {
      const enVars = extractPlaceholders(enMap.get(key) ?? '');
      const zhVars = extractPlaceholders(zhMap.get(key) ?? '');
      const jaVars = extractPlaceholders(jaMap.get(key) ?? '');
      expect(zhVars, `[updates] interpolation mismatch on '${key}' (zh vs en)`).toEqual(enVars);
      expect(jaVars, `[updates] interpolation mismatch on '${key}' (ja vs en)`).toEqual(enVars);
    }
  });
});
