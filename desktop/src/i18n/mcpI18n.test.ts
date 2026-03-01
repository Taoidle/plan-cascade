import { describe, expect, it } from 'vitest';
import enCommon from './locales/en/common.json';
import zhCommon from './locales/zh/common.json';
import jaCommon from './locales/ja/common.json';

function getByPath(obj: Record<string, unknown>, path: string): unknown {
  return path.split('.').reduce<unknown>((acc, key) => {
    if (acc && typeof acc === 'object' && key in (acc as Record<string, unknown>)) {
      return (acc as Record<string, unknown>)[key];
    }
    return undefined;
  }, obj);
}

const requiredMcpKeys = [
  'mcp.title',
  'mcp.description',
  'mcp.refresh',
  'mcp.import',
  'mcp.addServer',
  'mcp.noServers',
  'mcp.noServersDescription',
  'mcp.importFromClaude',
  'mcp.addManually',
  'mcp.test',
  'mcp.edit',
  'mcp.connect',
  'mcp.disconnect',
  'mcp.confirmDelete',
  'mcp.addServerTitle',
  'mcp.serverType',
  'mcp.serverTypeStdio',
  'mcp.serverTypeStreamHttp',
  'mcp.serverName',
  'mcp.command',
  'mcp.arguments',
  'mcp.envVariables',
  'mcp.addEnvVar',
  'mcp.serverUrl',
  'mcp.headers',
  'mcp.addHeader',
  'mcp.placeholders.serverName',
  'mcp.placeholders.command',
  'mcp.placeholders.arguments',
  'mcp.placeholders.envKey',
  'mcp.placeholders.envValue',
  'mcp.placeholders.serverUrl',
  'mcp.placeholders.headerKey',
  'mcp.placeholders.headerValue',
  'mcp.importTitle',
  'mcp.importDescription',
  'mcp.configPath',
  'mcp.importing',
  'mcp.importNow',
  'mcp.importJsonFile',
  'mcp.added',
  'mcp.skipped',
  'mcp.failed',
  'mcp.importedServers',
  'mcp.importErrors',
  'mcp.status.connected',
  'mcp.status.disconnected',
  'mcp.status.unknown',
  'mcp.status.error',
  'mcp.errors.fetchServers',
  'mcp.errors.testConnection',
  'mcp.errors.toggleServer',
  'mcp.errors.connectServer',
  'mcp.errors.disconnectServer',
  'mcp.errors.deleteServer',
  'mcp.errors.addServer',
  'mcp.errors.importServers',
] as const;

const requiredSharedKeys = ['common.retry', 'common.cancel', 'common.done', 'common.adding'] as const;

describe('MCP i18n completeness', () => {
  const locales = [
    { name: 'en', data: enCommon as Record<string, unknown> },
    { name: 'zh', data: zhCommon as Record<string, unknown> },
    { name: 'ja', data: jaCommon as Record<string, unknown> },
  ];

  it.each(locales)('includes all MCP keys for %s', ({ name, data }) => {
    for (const key of requiredMcpKeys) {
      const value = getByPath(data, key);
      expect(value, `missing key '${key}' in locale '${name}'`).toBeDefined();
      expect(typeof value, `invalid value type for '${key}' in locale '${name}'`).toBe('string');
      expect((value as string).trim().length, `empty value for '${key}' in locale '${name}'`).toBeGreaterThan(0);
    }
  });

  it.each(locales)('includes shared common keys for MCP dialogs in %s', ({ name, data }) => {
    for (const key of requiredSharedKeys) {
      const value = getByPath(data, key);
      expect(value, `missing key '${key}' in locale '${name}'`).toBeDefined();
      expect(typeof value, `invalid value type for '${key}' in locale '${name}'`).toBe('string');
      expect((value as string).trim().length, `empty value for '${key}' in locale '${name}'`).toBeGreaterThan(0);
    }
  });
});
