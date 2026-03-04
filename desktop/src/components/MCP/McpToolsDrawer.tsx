import { useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { CheckIcon, CopyIcon, Cross2Icon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import type { ConnectedMcpToolDetail, ConnectedServerInfo } from '../../types/mcp';

interface McpToolsDrawerProps {
  open: boolean;
  server: ConnectedServerInfo | null;
  tools: ConnectedMcpToolDetail[];
  query: string;
  onQueryChange: (value: string) => void;
  loading: boolean;
  onClose: () => void;
}

function schemaSummary(schema: Record<string, unknown>): { parameters: number; required: number } {
  const raw = schema && typeof schema === 'object' ? schema : {};
  const properties = (raw as { properties?: Record<string, unknown> }).properties ?? {};
  const required = (raw as { required?: unknown }).required;
  return {
    parameters: Object.keys(properties).length,
    required: Array.isArray(required) ? required.length : 0,
  };
}

export function McpToolsDrawer({ open, server, tools, query, onQueryChange, loading, onClose }: McpToolsDrawerProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState<string | null>(null);

  const toolCountLabel = useMemo(() => {
    if (!server) return '';
    return t('mcp.connectionMeta', {
      protocol: server.protocol_version || t('mcp.status.unknown'),
      count: tools.length,
    });
  }, [server, t, tools.length]);

  if (!open || !server) {
    return null;
  }

  const handleCopy = async (qualifiedName: string) => {
    try {
      await navigator.clipboard.writeText(qualifiedName);
      setCopied(qualifiedName);
      window.setTimeout(() => setCopied((prev) => (prev === qualifiedName ? null : prev)), 1200);
    } catch {
      setCopied(null);
    }
  };

  return (
    <div className="fixed inset-0 z-50">
      <button
        type="button"
        className="absolute inset-0 bg-black/40"
        onClick={onClose}
        aria-label={t('buttons.close')}
      />
      <div className="absolute right-0 top-0 h-full w-full max-w-xl bg-white dark:bg-gray-900 border-l border-gray-200 dark:border-gray-700 shadow-xl p-4 flex flex-col">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{t('mcp.toolsDrawerTitle')}</h3>
            <p className="text-xs text-gray-500 dark:text-gray-400">{server.server_name}</p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{toolCountLabel}</p>
            {server.connected_at && (
              <p className="text-xs text-gray-500 dark:text-gray-400">
                {t('mcp.connectedAtMeta', { value: server.connected_at })}
              </p>
            )}
          </div>
          <button type="button" onClick={onClose} className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800">
            <Cross2Icon className="w-4 h-4 text-gray-600 dark:text-gray-300" />
          </button>
        </div>

        <input
          type="text"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder={t('mcp.toolsSearchPlaceholder', { defaultValue: 'Search tools' })}
          className={clsx(
            'w-full px-3 py-2 rounded-md mb-3',
            'bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
            'text-sm text-gray-900 dark:text-white',
          )}
        />

        <div className="flex-1 overflow-y-auto space-y-2">
          {loading ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.loading')}</p>
          ) : tools.length === 0 ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.noTools')}</p>
          ) : (
            tools.map((tool) => {
              const summary = schemaSummary(tool.input_schema);
              return (
                <div
                  key={tool.qualified_name}
                  className="rounded border border-gray-200 dark:border-gray-700 p-3 space-y-2"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 break-all">{tool.tool_name}</p>
                      <p className="text-xs font-mono text-gray-500 dark:text-gray-400 break-all">
                        {tool.qualified_name}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleCopy(tool.qualified_name)}
                      className={clsx(
                        'inline-flex items-center gap-1 px-2 py-1 rounded text-[11px]',
                        'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700',
                      )}
                    >
                      {copied === tool.qualified_name ? (
                        <CheckIcon className="w-3 h-3" />
                      ) : (
                        <CopyIcon className="w-3 h-3" />
                      )}
                      {copied === tool.qualified_name
                        ? t('common.done')
                        : t('mcp.copyQualifiedName', { defaultValue: 'Copy name' })}
                    </button>
                  </div>
                  <p className="text-xs text-gray-700 dark:text-gray-300">
                    {tool.description || t('mcp.toolDescriptionMissing', { defaultValue: 'No description provided.' })}
                  </p>
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-[11px] px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300">
                      {t('mcp.toolSchemaSummary', {
                        defaultValue: '{{parameters}} params / {{required}} required',
                        parameters: summary.parameters,
                        required: summary.required,
                      })}
                    </span>
                    <span
                      className={clsx(
                        'text-[11px] px-2 py-0.5 rounded',
                        tool.is_parallel_safe
                          ? 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300'
                          : 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
                      )}
                    >
                      {tool.is_parallel_safe
                        ? t('mcp.parallelSafe', { defaultValue: 'Parallel-safe' })
                        : t('mcp.parallelUnsafe', { defaultValue: 'Sequential recommended' })}
                    </span>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
