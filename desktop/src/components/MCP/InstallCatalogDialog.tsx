import { useEffect, useMemo, useRef, useState } from 'react';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, RocketIcon } from '@radix-ui/react-icons';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import type {
  CommandResponse,
  McpCatalogItem,
  McpInstallPreview,
  McpInstallRequest,
  McpInstallResult,
  McpInstallProgressEvent,
  McpInstallLogEvent,
  McpOauthEvent,
} from '../../types/mcp';

interface InstallCatalogDialogProps {
  open: boolean;
  item: McpCatalogItem | null;
  onOpenChange: (open: boolean) => void;
  onInstalled?: (result: McpInstallResult) => void;
}

export function InstallCatalogDialog({ open, item, onOpenChange, onInstalled }: InstallCatalogDialogProps) {
  const { t } = useTranslation();
  const [alias, setAlias] = useState('');
  const [strategy, setStrategy] = useState('');
  const [autoConnect, setAutoConnect] = useState(true);
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<McpInstallPreview | null>(null);
  const [installing, setInstalling] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [lastFailedJobId, setLastFailedJobId] = useState<string | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<McpInstallProgressEvent | null>(null);
  const [logs, setLogs] = useState<McpInstallLogEvent[]>([]);
  const [oauthState, setOauthState] = useState<McpOauthEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [communityConfirmed, setCommunityConfirmed] = useState(false);
  const installingRef = useRef(false);
  const activeJobIdRef = useRef<string | null>(null);

  const strategyOptions = useMemo(() => {
    return [...(item?.strategies || [])].sort((a, b) => a.priority - b.priority);
  }, [item]);
  const missingRequiredSecrets = useMemo(() => {
    return (preview?.required_secrets || [])
      .filter((field) => field.required)
      .filter((field) => !(secrets[field.key] || '').trim())
      .map((field) => field.key);
  }, [preview?.required_secrets, secrets]);
  const hasMissingRequiredSecrets = missingRequiredSecrets.length > 0;
  const riskFlags = useMemo(() => new Set(preview?.risk_flags || []), [preview?.risk_flags]);
  const artifactPinned = !riskFlags.has('unpinned_artifact');

  useEffect(() => {
    installingRef.current = installing;
  }, [installing]);

  useEffect(() => {
    activeJobIdRef.current = activeJobId;
  }, [activeJobId]);

  useEffect(() => {
    if (!open || !item) return;
    setAlias(item.name);
    setStrategy(strategyOptions[0]?.id || '');
    setAutoConnect(true);
    setInstalling(false);
    setRetrying(false);
    setLastFailedJobId(null);
    setActiveJobId(null);
    setSecrets({});
    setProgress(null);
    setPreview(null);
    setLogs([]);
    setOauthState(null);
    setError(null);
    setCommunityConfirmed(item.trust_level !== 'community');
  }, [item, open, strategyOptions]);

  useEffect(() => {
    if (!open || !item || !strategy) return;
    let cancelled = false;
    (async () => {
      try {
        const response = await invoke<CommandResponse<McpInstallPreview>>('preview_install_mcp_catalog_item', {
          itemId: item.id,
          preferredStrategy: strategy,
        });
        if (!cancelled) {
          if (response.success && response.data) {
            setPreview(response.data);
          } else {
            setError(response.error || t('mcp.errors.previewInstall'));
          }
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : t('mcp.errors.previewInstall'));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [item, open, strategy, t]);

  useEffect(() => {
    if (!open) return;
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenLog: UnlistenFn | null = null;
    let unlistenOauth: UnlistenFn | null = null;
    const shouldAcceptJob = (jobId: string) => {
      if (!jobId) return false;
      const current = activeJobIdRef.current;
      if (current) {
        return current === jobId;
      }
      if (!installingRef.current) {
        return false;
      }
      activeJobIdRef.current = jobId;
      setActiveJobId(jobId);
      return true;
    };
    (async () => {
      unlistenProgress = await listen<McpInstallProgressEvent>('mcp:install-progress', (event) => {
        if (!shouldAcceptJob(event.payload.job_id)) return;
        setProgress(event.payload);
      });
      unlistenLog = await listen<McpInstallLogEvent>('mcp:install-log', (event) => {
        if (!shouldAcceptJob(event.payload.job_id)) return;
        setLogs((prev) => [event.payload, ...prev].slice(0, 50));
      });
      unlistenOauth = await listen<McpOauthEvent>('mcp:oauth-state', (event) => {
        if (!shouldAcceptJob(event.payload.job_id)) return;
        setOauthState(event.payload);
      });
    })();
    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenLog) unlistenLog();
      if (unlistenOauth) unlistenOauth();
    };
  }, [open]);

  const handleInstall = async () => {
    if (!item || !alias.trim()) return;
    if (hasMissingRequiredSecrets) {
      setError(
        t('mcp.install.missingRequiredSecrets', {
          defaultValue: 'Missing required secrets: {{keys}}',
          keys: missingRequiredSecrets.join(', '),
        }),
      );
      return;
    }
    setInstalling(true);
    installingRef.current = true;
    setRetrying(false);
    setLastFailedJobId(null);
    setActiveJobId(null);
    activeJobIdRef.current = null;
    setError(null);
    setProgress(null);
    setLogs([]);
    setOauthState(null);
    try {
      const request: McpInstallRequest = {
        item_id: item.id,
        server_alias: alias.trim(),
        selected_strategy: strategy || undefined,
        secrets,
        auto_connect: autoConnect,
      };
      const response = await invoke<CommandResponse<McpInstallResult>>('install_mcp_catalog_item', { request });
      if (response.success && response.data) {
        if (response.data.status === 'success') {
          onInstalled?.(response.data);
          onOpenChange(false);
        } else {
          setLastFailedJobId(response.data.job_id || null);
          setError(response.data.diagnostics || t('mcp.install.installFailed'));
        }
      } else {
        setError(response.error || t('mcp.errors.installCatalog'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('mcp.errors.installCatalog'));
    } finally {
      setInstalling(false);
      installingRef.current = false;
    }
  };

  const handleRetry = async () => {
    if (!lastFailedJobId) return;
    setRetrying(true);
    setInstalling(true);
    installingRef.current = true;
    setActiveJobId(null);
    activeJobIdRef.current = null;
    setError(null);
    setProgress(null);
    setLogs([]);
    setOauthState(null);
    try {
      const response = await invoke<CommandResponse<McpInstallResult>>('retry_mcp_install', { jobId: lastFailedJobId });
      if (response.success && response.data) {
        if (response.data.status === 'success') {
          onInstalled?.(response.data);
          onOpenChange(false);
        } else {
          setLastFailedJobId(response.data.job_id || lastFailedJobId);
          setError(response.data.diagnostics || t('mcp.install.installFailed'));
        }
      } else {
        setError(response.error || t('mcp.errors.installCatalog'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('mcp.errors.installCatalog'));
    } finally {
      setRetrying(false);
      setInstalling(false);
      installingRef.current = false;
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0 z-40" />
        <Dialog.Content
          className={clsx(
            'fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[96vw] max-w-2xl max-h-[88vh] overflow-y-auto',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700',
          )}
        >
          <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-white">
              {t('mcp.install.title')}
              {item ? ` - ${item.name}` : ''}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button type="button" className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800">
                <Cross2Icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
              </button>
            </Dialog.Close>
          </div>

          <div className="p-4 space-y-4">
            <section className="space-y-2">
              <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('mcp.install.stepStrategy')}</h3>
              <select
                value={strategy}
                onChange={(e) => setStrategy(e.target.value)}
                className="w-full px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-gray-100 dark:bg-gray-800 text-sm text-gray-900 dark:text-white"
              >
                {strategyOptions.map((opt) => (
                  <option key={opt.id} value={opt.id}>
                    {opt.id} ({opt.kind})
                  </option>
                ))}
              </select>
            </section>

            <section className="space-y-2">
              <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('mcp.install.stepEnvironment')}</h3>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                {t('mcp.install.artifactPinStatus', {
                  defaultValue: 'Artifact pinning: {{status}}',
                  status: artifactPinned
                    ? t('mcp.install.pinned', { defaultValue: 'pinned' })
                    : t('mcp.install.unpinned', { defaultValue: 'unpinned' }),
                })}
              </p>
              {(preview?.risk_flags || []).length > 0 && (
                <div className="space-y-1">
                  {(preview?.risk_flags || []).map((flag) => {
                    const colorClass =
                      flag === 'unpinned_artifact' || flag === 'community_caution'
                        ? 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/40 dark:bg-amber-900/20 dark:text-amber-300'
                        : 'border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-900/40 dark:bg-blue-900/20 dark:text-blue-300';
                    const label =
                      flag === 'review_commands'
                        ? t('mcp.install.riskReviewCommands', {
                            defaultValue: 'Review install commands before continuing.',
                          })
                        : flag === 'community_caution'
                          ? t('mcp.install.riskCommunityCaution', {
                              defaultValue: 'Community-maintained entry. Validate source and permissions.',
                            })
                          : flag === 'community_item_confirmation_required'
                            ? t('mcp.install.riskCommunityConfirm', {
                                defaultValue: 'Extra confirmation required for community entries.',
                              })
                            : flag === 'unpinned_artifact'
                              ? t('mcp.install.riskUnpinnedArtifact', {
                                  defaultValue: 'Artifact is not pinned to a digest/version.',
                                })
                              : flag;
                    return (
                      <div key={flag} className={clsx('text-xs rounded-md border px-2 py-1.5', colorClass)}>
                        {label}
                      </div>
                    );
                  })}
                </div>
              )}
              {preview?.missing_runtimes.length ? (
                <div className="text-xs rounded-md border border-amber-200 bg-amber-50 dark:border-amber-900/40 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300 px-3 py-2">
                  {t('mcp.install.missingRuntimes')}: {preview.missing_runtimes.join(', ')}
                </div>
              ) : (
                <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.install.runtimeReady')}</p>
              )}
              {preview?.install_commands?.length ? (
                <div className="space-y-1">
                  {preview.install_commands.slice(0, 3).map((cmd) => (
                    <code
                      key={cmd}
                      className="block text-[11px] px-2 py-1 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 break-all"
                    >
                      {cmd}
                    </code>
                  ))}
                </div>
              ) : null}
            </section>

            <section className="space-y-2">
              <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('mcp.install.stepSecrets')}</h3>
              <input
                type="text"
                value={alias}
                onChange={(e) => setAlias(e.target.value)}
                placeholder={t('mcp.install.aliasPlaceholder')}
                className="w-full px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-gray-100 dark:bg-gray-800 text-sm text-gray-900 dark:text-white"
              />
              {(preview?.required_secrets || []).map((field) => (
                <div key={field.key} className="space-y-1">
                  <label className="text-xs text-gray-600 dark:text-gray-300">
                    {field.label} {field.required ? '*' : ''}
                  </label>
                  <input
                    type="password"
                    value={secrets[field.key] || ''}
                    onChange={(e) =>
                      setSecrets((prev) => ({
                        ...prev,
                        [field.key]: e.target.value,
                      }))
                    }
                    className="w-full px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-gray-100 dark:bg-gray-800 text-sm text-gray-900 dark:text-white"
                  />
                </div>
              ))}
              {hasMissingRequiredSecrets && (
                <p className="text-xs text-amber-700 dark:text-amber-300">
                  {t('mcp.install.missingRequiredSecrets', {
                    defaultValue: 'Missing required secrets: {{keys}}',
                    keys: missingRequiredSecrets.join(', '),
                  })}
                </p>
              )}
              <label className="inline-flex items-center gap-2 text-xs text-gray-600 dark:text-gray-300">
                <input type="checkbox" checked={autoConnect} onChange={(e) => setAutoConnect(e.target.checked)} />
                {t('mcp.autoConnect')}
              </label>
              {(item?.trust_level === 'community' ||
                riskFlags.has('community_caution') ||
                riskFlags.has('community_item_confirmation_required')) && (
                <label className="inline-flex items-start gap-2 text-xs text-amber-700 dark:text-amber-300 rounded border border-amber-200 dark:border-amber-900/40 bg-amber-50 dark:bg-amber-900/20 px-2 py-1.5">
                  <input
                    type="checkbox"
                    checked={communityConfirmed}
                    onChange={(e) => setCommunityConfirmed(e.target.checked)}
                    className="mt-0.5"
                  />
                  <span>{t('mcp.install.communityConfirm')}</span>
                </label>
              )}
            </section>

            <section className="space-y-2">
              <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('mcp.install.stepProgress')}</h3>
              {progress ? (
                <p className="text-xs text-gray-600 dark:text-gray-300">
                  {progress.phase} - {progress.message} ({Math.round(progress.progress * 100)}%)
                  {activeJobId ? ` [${activeJobId.slice(0, 8)}]` : ''}
                </p>
              ) : (
                <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.install.waiting')}</p>
              )}
              {logs.length > 0 && (
                <div className="max-h-32 overflow-y-auto space-y-1 rounded border border-gray-200 dark:border-gray-700 p-2">
                  {logs.map((log, index) => (
                    <p key={`${log.phase}-${index}`} className="text-[11px] text-gray-600 dark:text-gray-300">
                      [{log.phase}] {log.message}
                    </p>
                  ))}
                </div>
              )}
              {oauthState && (
                <div className="text-xs rounded border border-blue-200 bg-blue-50 dark:border-blue-900/40 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300 px-2 py-1.5">
                  OAuth: {oauthState.state}
                  {oauthState.message ? ` - ${oauthState.message}` : ''}
                </div>
              )}
            </section>

            {error && (
              <div className="text-sm rounded-md border border-red-200 bg-red-50 dark:border-red-900/40 dark:bg-red-900/20 text-red-700 dark:text-red-300 px-3 py-2">
                {error}
              </div>
            )}
          </div>

          <div className="p-4 border-t border-gray-200 dark:border-gray-700 flex items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-3 py-2 rounded-md text-sm bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
            >
              {t('buttons.cancel')}
            </button>
            {lastFailedJobId && (
              <button
                type="button"
                onClick={handleRetry}
                disabled={installing}
                className={clsx(
                  'inline-flex items-center gap-1.5 px-3 py-2 rounded-md text-sm',
                  'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
                  'hover:bg-amber-200 dark:hover:bg-amber-900/50 disabled:opacity-50',
                )}
              >
                {retrying ? t('mcp.install.retrying') : t('mcp.install.retry')}
              </button>
            )}
            <button
              type="button"
              onClick={handleInstall}
              disabled={installing || !item || !communityConfirmed || hasMissingRequiredSecrets}
              className={clsx(
                'inline-flex items-center gap-1.5 px-3 py-2 rounded-md text-sm text-white',
                'bg-primary-600 hover:bg-primary-700 disabled:opacity-50',
              )}
            >
              <RocketIcon className="w-4 h-4" />
              {installing ? t('mcp.install.installing') : t('mcp.install.installNow')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
