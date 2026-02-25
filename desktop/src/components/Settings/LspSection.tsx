/**
 * LspSection Component
 *
 * Settings section for LSP server detection and enrichment management.
 * Shows detected language servers, install hints for missing ones,
 * and controls for triggering enrichment passes.
 */

import { useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { CheckCircledIcon, CrossCircledIcon, ReloadIcon, InfoCircledIcon } from '@radix-ui/react-icons';
import { useLspStore } from '../../store/lsp';
import { useSettingsStore } from '../../store/settings';
import { LSP_LANGUAGES } from '../../types/lsp';
import type { LspServerStatus } from '../../types/lsp';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Find the status entry for a given language from the servers list. */
function findStatus(servers: LspServerStatus[], language: string): LspServerStatus | undefined {
  return servers.find((s) => s.language === language);
}

/** Format duration in milliseconds to a human-readable string. */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = (ms / 1000).toFixed(1);
  return `${seconds}s`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function LspSection() {
  const { t } = useTranslation('settings');

  const {
    servers,
    isDetecting,
    enrichmentReport,
    isEnriching,
    autoEnrich,
    error,
    detect,
    fetchStatus,
    enrich,
    fetchReport,
    setAutoEnrich,
    clearError,
  } = useLspStore();

  const workspacePath = useSettingsStore((s) => s.workspacePath);

  // Load status and report on mount
  useEffect(() => {
    void fetchStatus();
    void fetchReport();
  }, [fetchStatus, fetchReport]);

  // Handle detect
  const handleDetect = useCallback(async () => {
    await detect();
  }, [detect]);

  // Handle manual enrich
  const handleEnrich = useCallback(async () => {
    if (!workspacePath) return;
    await enrich(workspacePath);
  }, [enrich, workspacePath]);

  const detectedCount = servers.filter((s) => s.detected).length;

  return (
    <div className="space-y-8">
      {/* Section header */}
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('lsp.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('lsp.description')}</p>
      </div>

      {/* Server Detection */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('lsp.servers.title')}</h3>
          <button
            onClick={handleDetect}
            disabled={isDetecting}
            className={clsx(
              'inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            <ReloadIcon className={clsx('w-3.5 h-3.5', isDetecting && 'animate-spin')} />
            {isDetecting ? t('lsp.servers.detecting') : t('lsp.servers.detectButton')}
          </button>
        </div>

        {/* Summary */}
        {servers.length > 0 && (
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {t('lsp.servers.summary', { detected: detectedCount, total: LSP_LANGUAGES.length })}
          </p>
        )}

        {/* Server Table */}
        <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 dark:bg-gray-800/50">
                <th className="text-left px-4 py-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('lsp.servers.colLanguage')}
                </th>
                <th className="text-left px-4 py-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('lsp.servers.colServer')}
                </th>
                <th className="text-left px-4 py-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('lsp.servers.colStatus')}
                </th>
                <th className="text-left px-4 py-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('lsp.servers.colInfo')}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
              {LSP_LANGUAGES.map((lang) => {
                const serverStatus = findStatus(servers, lang.id);
                const detected = serverStatus?.detected ?? false;
                const serverName = serverStatus?.server_name ?? lang.serverName;
                const version = serverStatus?.version;
                const installHint = serverStatus?.install_hint;

                return (
                  <tr key={lang.id} className="hover:bg-gray-50 dark:hover:bg-gray-800/30">
                    <td className="px-4 py-3 font-medium text-gray-900 dark:text-white">{lang.displayName}</td>
                    <td className="px-4 py-3 text-gray-600 dark:text-gray-400">{serverName}</td>
                    <td className="px-4 py-3">
                      {detected ? (
                        <span className="inline-flex items-center gap-1.5">
                          <CheckCircledIcon className="w-4 h-4 text-green-500" />
                          <span className="text-green-600 dark:text-green-400">{t('lsp.servers.detected')}</span>
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5">
                          <CrossCircledIcon className="w-4 h-4 text-gray-400 dark:text-gray-500" />
                          <span className="text-gray-500 dark:text-gray-400">{t('lsp.servers.notFound')}</span>
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-500 dark:text-gray-400">
                      {detected && version ? (
                        <span>{t('lsp.servers.version', { version })}</span>
                      ) : !detected && installHint ? (
                        <code className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-xs">
                          {installHint}
                        </code>
                      ) : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>

      {/* Auto-Enrich Toggle */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('lsp.enrichment.title')}</h3>
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={autoEnrich}
            onChange={(e) => setAutoEnrich(e.target.checked)}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500"
          />
          <div>
            <span className="text-sm text-gray-900 dark:text-white">{t('lsp.enrichment.autoEnrich')}</span>
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.enrichment.autoEnrichHelp')}</p>
          </div>
        </label>
      </section>

      {/* Manual Enrichment */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('lsp.enrichment.manualTitle')}</h3>
        <div className="flex items-center gap-3">
          <button
            onClick={handleEnrich}
            disabled={isEnriching || !workspacePath || detectedCount === 0}
            className={clsx(
              'inline-flex items-center gap-2 px-4 py-2 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            <ReloadIcon className={clsx('w-4 h-4', isEnriching && 'animate-spin')} />
            {isEnriching ? t('lsp.enrichment.enriching') : t('lsp.enrichment.enrichButton')}
          </button>
          {!workspacePath && (
            <span className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.enrichment.noWorkspace')}</span>
          )}
          {workspacePath && detectedCount === 0 && (
            <span className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.enrichment.noServers')}</span>
          )}
        </div>
      </section>

      {/* Enrichment Report */}
      {enrichmentReport && (
        <section className="space-y-3">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('lsp.report.title')}</h3>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.report.languages')}</p>
              <p className="text-lg font-semibold text-gray-900 dark:text-white">
                {enrichmentReport.languages_enriched.length}
              </p>
              <p className="text-xs text-gray-400 dark:text-gray-500 truncate">
                {enrichmentReport.languages_enriched.join(', ') || '-'}
              </p>
            </div>
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.report.symbols')}</p>
              <p className="text-lg font-semibold text-gray-900 dark:text-white">{enrichmentReport.symbols_enriched}</p>
            </div>
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.report.references')}</p>
              <p className="text-lg font-semibold text-gray-900 dark:text-white">{enrichmentReport.references_found}</p>
            </div>
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('lsp.report.duration')}</p>
              <p className="text-lg font-semibold text-gray-900 dark:text-white">
                {formatDuration(enrichmentReport.duration_ms)}
              </p>
            </div>
          </div>
        </section>
      )}

      {/* Error display */}
      {error && (
        <div className="flex items-start gap-2 p-3 rounded-lg border border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20">
          <CrossCircledIcon className="w-5 h-5 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm text-red-700 dark:text-red-300">{error}</p>
            <button
              onClick={clearError}
              className="text-xs text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 mt-1 underline"
            >
              {t('lsp.dismiss')}
            </button>
          </div>
        </div>
      )}

      {/* Info notice */}
      <div className="flex items-start gap-2 p-3 rounded-lg border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-900/20">
        <InfoCircledIcon className="w-5 h-5 text-blue-600 dark:text-blue-400 shrink-0 mt-0.5" />
        <div>
          <p className="text-sm text-blue-700 dark:text-blue-300">{t('lsp.info.title')}</p>
          <p className="text-xs text-blue-600 dark:text-blue-400 mt-0.5">{t('lsp.info.description')}</p>
        </div>
      </div>
    </div>
  );
}

export default LspSection;
