/**
 * RemotesTab Component
 *
 * Manage Git remotes and tags in one place.
 * Supports listing/adding/removing/updating remotes and creating/deleting tags.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { CommandResponse } from '../../../../lib/tauri';
import type { RemoteInfo, TagInfo } from '../../../../types/git';
import { useSettingsStore } from '../../../../store/settings';
import { useGitStore } from '../../../../store/git';

interface RemoteEditorState {
  name: string;
  url: string;
}

export function RemotesTab() {
  const { t } = useTranslation('git');
  const repoPath = useSettingsStore((s) => s.workspacePath);
  const setGlobalError = useGitStore((s) => s.setError);

  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [activeRemoteAction, setActiveRemoteAction] = useState<string | null>(null);

  const [newRemoteName, setNewRemoteName] = useState('');
  const [newRemoteUrl, setNewRemoteUrl] = useState('');

  const [remoteEditor, setRemoteEditor] = useState<RemoteEditorState | null>(null);

  const [newTagName, setNewTagName] = useState('');
  const [newTagTarget, setNewTagTarget] = useState('');
  const [newTagMessage, setNewTagMessage] = useState('');

  const getRemoteActionKey = useCallback((name: string, action: string) => `${name}:${action}`, []);

  const sortedTags = useMemo(() => [...tags].sort((a, b) => a.name.localeCompare(b.name)), [tags]);

  const refresh = useCallback(async () => {
    if (!repoPath) {
      setRemotes([]);
      setTags([]);
      return;
    }

    setIsLoading(true);
    try {
      const [remotesRes, tagsRes] = await Promise.all([
        invoke<CommandResponse<RemoteInfo[]>>('git_get_remotes', { repoPath }),
        invoke<CommandResponse<TagInfo[]>>('git_list_tags', { repoPath }),
      ]);

      if (remotesRes.success && remotesRes.data) {
        setRemotes(remotesRes.data);
      } else {
        setRemotes([]);
        setGlobalError(remotesRes.error || 'Failed to load remotes');
      }

      if (tagsRes.success && tagsRes.data) {
        setTags(tagsRes.data);
      } else {
        setTags([]);
        setGlobalError(tagsRes.error || 'Failed to load tags');
      }
    } catch (err) {
      setGlobalError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [repoPath, setGlobalError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleAddRemote = useCallback(async () => {
    if (!repoPath) return;
    const name = newRemoteName.trim();
    const url = newRemoteUrl.trim();
    if (!name || !url) {
      setGlobalError(
        t('remotesTab.validation.remoteNameUrlRequired', { defaultValue: 'Remote name and URL are required' }),
      );
      return;
    }

    setIsSaving(true);
    try {
      const res = await invoke<CommandResponse<void>>('git_remote_add', {
        repoPath,
        name,
        url,
      });
      if (!res.success) {
        setGlobalError(res.error || 'Failed to add remote');
        return;
      }
      setNewRemoteName('');
      setNewRemoteUrl('');
      await refresh();
    } catch (err) {
      setGlobalError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSaving(false);
    }
  }, [repoPath, newRemoteName, newRemoteUrl, setGlobalError, refresh, t]);

  const handleRemoveRemote = useCallback(
    async (name: string) => {
      if (!repoPath) return;
      const confirmed = window.confirm(
        t('remotesTab.confirmRemoveRemote', {
          name,
          defaultValue: `Remove remote "${name}"?`,
        }),
      );
      if (!confirmed) return;

      setIsSaving(true);
      try {
        const res = await invoke<CommandResponse<void>>('git_remote_remove', { repoPath, name });
        if (!res.success) {
          setGlobalError(res.error || 'Failed to remove remote');
          return;
        }
        await refresh();
      } catch (err) {
        setGlobalError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsSaving(false);
      }
    },
    [repoPath, setGlobalError, refresh, t],
  );

  const handleSaveRemoteUrl = useCallback(async () => {
    if (!repoPath || !remoteEditor) return;
    const nextUrl = remoteEditor.url.trim();
    if (!nextUrl) {
      setGlobalError(t('remotesTab.validation.remoteUrlRequired', { defaultValue: 'Remote URL is required' }));
      return;
    }

    setIsSaving(true);
    try {
      const res = await invoke<CommandResponse<void>>('git_remote_set_url', {
        repoPath,
        name: remoteEditor.name,
        url: nextUrl,
      });
      if (!res.success) {
        setGlobalError(res.error || 'Failed to update remote URL');
        return;
      }
      setRemoteEditor(null);
      await refresh();
    } catch (err) {
      setGlobalError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSaving(false);
    }
  }, [repoPath, remoteEditor, setGlobalError, refresh, t]);

  const handleFetchRemote = useCallback(
    async (remoteName: string, prune: boolean) => {
      if (!repoPath) return;
      const action = prune ? 'fetch-prune' : 'fetch';
      setActiveRemoteAction(getRemoteActionKey(remoteName, action));
      try {
        const res = await invoke<CommandResponse<void>>('git_fetch', {
          repoPath,
          remote: remoteName,
          prune,
        });
        if (!res.success) {
          setGlobalError(
            res.error ||
              t('remotesTab.fetchRemoteFailed', {
                name: remoteName,
                defaultValue: `Failed to fetch ${remoteName}`,
              }),
          );
          return;
        }
        await refresh();
      } catch (err) {
        setGlobalError(err instanceof Error ? err.message : String(err));
      } finally {
        setActiveRemoteAction(null);
      }
    },
    [repoPath, getRemoteActionKey, setGlobalError, refresh, t],
  );

  const handlePushTags = useCallback(
    async (remoteName: string) => {
      if (!repoPath) return;
      setActiveRemoteAction(getRemoteActionKey(remoteName, 'push-tags'));
      try {
        const res = await invoke<CommandResponse<void>>('git_push_tags', {
          repoPath,
          remote: remoteName,
        });
        if (!res.success) {
          setGlobalError(
            res.error ||
              t('remotesTab.pushTagsFailed', {
                name: remoteName,
                defaultValue: `Failed to push tags to ${remoteName}`,
              }),
          );
          return;
        }
      } catch (err) {
        setGlobalError(err instanceof Error ? err.message : String(err));
      } finally {
        setActiveRemoteAction(null);
      }
    },
    [repoPath, getRemoteActionKey, setGlobalError, t],
  );

  const handleCreateTag = useCallback(async () => {
    if (!repoPath) return;
    const name = newTagName.trim();
    if (!name) {
      setGlobalError(t('remotesTab.validation.tagNameRequired', { defaultValue: 'Tag name is required' }));
      return;
    }

    setIsSaving(true);
    try {
      const res = await invoke<CommandResponse<void>>('git_create_tag', {
        repoPath,
        name,
        target: newTagTarget.trim() || null,
        message: newTagMessage.trim() || null,
      });
      if (!res.success) {
        setGlobalError(res.error || 'Failed to create tag');
        return;
      }
      setNewTagName('');
      setNewTagTarget('');
      setNewTagMessage('');
      await refresh();
    } catch (err) {
      setGlobalError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSaving(false);
    }
  }, [repoPath, newTagName, newTagTarget, newTagMessage, setGlobalError, refresh, t]);

  const handleDeleteTag = useCallback(
    async (name: string) => {
      if (!repoPath) return;
      const confirmed = window.confirm(
        t('remotesTab.confirmDeleteTag', {
          name,
          defaultValue: `Delete tag "${name}"?`,
        }),
      );
      if (!confirmed) return;

      setIsSaving(true);
      try {
        const res = await invoke<CommandResponse<void>>('git_delete_tag', {
          repoPath,
          name,
        });
        if (!res.success) {
          setGlobalError(res.error || 'Failed to delete tag');
          return;
        }
        await refresh();
      } catch (err) {
        setGlobalError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsSaving(false);
      }
    },
    [repoPath, setGlobalError, refresh, t],
  );

  if (!repoPath) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-gray-500 dark:text-gray-400">
        {t('remotesTab.noWorkspace', { defaultValue: 'No workspace selected' })}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">
          {t('remotesTab.title', { defaultValue: 'Remotes & Tags' })}
        </h3>
        <button
          onClick={() => void refresh()}
          disabled={isLoading || isSaving || activeRemoteAction !== null}
          className={clsx(
            'px-2 py-1 text-xs rounded border transition-colors',
            'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300',
            'hover:bg-gray-50 dark:hover:bg-gray-700',
            (isLoading || isSaving || activeRemoteAction !== null) && 'opacity-50 cursor-not-allowed',
          )}
        >
          {t('remotesTab.refresh', { defaultValue: 'Refresh' })}
        </button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto p-3 space-y-4">
        <section className="space-y-2">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            {t('remotesTab.remotes', { defaultValue: 'Remotes' })}
          </h4>

          <div className="grid grid-cols-1 gap-2">
            <input
              type="text"
              value={newRemoteName}
              onChange={(e) => setNewRemoteName(e.target.value)}
              placeholder={t('remotesTab.remoteName', { defaultValue: 'Remote name (e.g. origin)' })}
              className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
            />
            <input
              type="text"
              value={newRemoteUrl}
              onChange={(e) => setNewRemoteUrl(e.target.value)}
              placeholder={t('remotesTab.remoteUrl', { defaultValue: 'Remote URL' })}
              className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
            />
            <button
              onClick={() => void handleAddRemote()}
              disabled={isSaving}
              className={clsx(
                'px-2 py-1.5 text-xs rounded text-white transition-colors',
                isSaving ? 'bg-blue-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700',
              )}
            >
              {t('remotesTab.addRemote', { defaultValue: 'Add Remote' })}
            </button>
          </div>

          {remotes.length === 0 && !isLoading && (
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('remotesTab.noRemotes', { defaultValue: 'No remotes configured' })}
            </div>
          )}

          {remotes.map((remote) => (
            <div
              key={remote.name}
              className="rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800/50 p-2"
            >
              {(() => {
                const fetchKey = getRemoteActionKey(remote.name, 'fetch');
                const fetchPruneKey = getRemoteActionKey(remote.name, 'fetch-prune');
                const pushTagsKey = getRemoteActionKey(remote.name, 'push-tags');
                const isFetching = activeRemoteAction === fetchKey || activeRemoteAction === fetchPruneKey;
                const isPushingTags = activeRemoteAction === pushTagsKey;
                const isRemoteBusy = isSaving || activeRemoteAction !== null;

                return (
                  <>
                    <div className="flex items-center justify-between gap-2">
                      <div className="min-w-0">
                        <div className="text-sm font-medium text-gray-800 dark:text-gray-200">{remote.name}</div>
                        <div className="text-xs text-gray-600 dark:text-gray-400 truncate">{remote.fetch_url}</div>
                      </div>
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => void handleFetchRemote(remote.name, false)}
                          disabled={isRemoteBusy}
                          className={clsx(
                            'px-2 py-0.5 text-2xs rounded border transition-colors',
                            'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300',
                            'hover:bg-gray-50 dark:hover:bg-gray-700',
                            isRemoteBusy && 'opacity-50 cursor-not-allowed',
                          )}
                        >
                          {isFetching
                            ? t('remotesTab.syncing', { defaultValue: 'Syncing...' })
                            : t('remotesTab.fetch', { defaultValue: 'Fetch' })}
                        </button>
                        <button
                          onClick={() => void handleFetchRemote(remote.name, true)}
                          disabled={isRemoteBusy}
                          className={clsx(
                            'px-2 py-0.5 text-2xs rounded border transition-colors',
                            'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300',
                            'hover:bg-gray-50 dark:hover:bg-gray-700',
                            isRemoteBusy && 'opacity-50 cursor-not-allowed',
                          )}
                        >
                          {t('remotesTab.fetchPrune', { defaultValue: 'Fetch + Prune' })}
                        </button>
                        <button
                          onClick={() => void handlePushTags(remote.name)}
                          disabled={isRemoteBusy}
                          className={clsx(
                            'px-2 py-0.5 text-2xs rounded border transition-colors',
                            'border-blue-300 dark:border-blue-700 text-blue-600 dark:text-blue-400',
                            'hover:bg-blue-50 dark:hover:bg-blue-900/20',
                            isRemoteBusy && 'opacity-50 cursor-not-allowed',
                          )}
                        >
                          {isPushingTags
                            ? t('remotesTab.pushingTags', { defaultValue: 'Pushing tags...' })
                            : t('remotesTab.pushTags', { defaultValue: 'Push Tags' })}
                        </button>
                        <button
                          onClick={() =>
                            setRemoteEditor({ name: remote.name, url: remote.push_url || remote.fetch_url })
                          }
                          disabled={isRemoteBusy}
                          className={clsx(
                            'px-2 py-0.5 text-2xs rounded border transition-colors',
                            'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300',
                            'hover:bg-gray-50 dark:hover:bg-gray-700',
                            isRemoteBusy && 'opacity-50 cursor-not-allowed',
                          )}
                        >
                          {t('remotesTab.setUrl', { defaultValue: 'Set URL' })}
                        </button>
                        <button
                          onClick={() => void handleRemoveRemote(remote.name)}
                          disabled={isRemoteBusy}
                          className={clsx(
                            'px-2 py-0.5 text-2xs rounded border transition-colors',
                            'border-red-300 dark:border-red-700 text-red-600 dark:text-red-400',
                            'hover:bg-red-50 dark:hover:bg-red-900/20',
                            isRemoteBusy && 'opacity-50 cursor-not-allowed',
                          )}
                        >
                          {t('remotesTab.remove', { defaultValue: 'Remove' })}
                        </button>
                      </div>
                    </div>
                  </>
                );
              })()}
            </div>
          ))}
        </section>

        <section className="space-y-2">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            {t('remotesTab.tags', { defaultValue: 'Tags' })}
          </h4>

          <div className="grid grid-cols-1 gap-2">
            <input
              type="text"
              value={newTagName}
              onChange={(e) => setNewTagName(e.target.value)}
              placeholder={t('remotesTab.tagName', { defaultValue: 'Tag name (e.g. v1.0.0)' })}
              className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
            />
            <input
              type="text"
              value={newTagTarget}
              onChange={(e) => setNewTagTarget(e.target.value)}
              placeholder={t('remotesTab.tagTarget', { defaultValue: 'Target commit (optional, default HEAD)' })}
              className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
            />
            <input
              type="text"
              value={newTagMessage}
              onChange={(e) => setNewTagMessage(e.target.value)}
              placeholder={t('remotesTab.tagMessage', { defaultValue: 'Annotated message (optional)' })}
              className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
            />
            <button
              onClick={() => void handleCreateTag()}
              disabled={isSaving}
              className={clsx(
                'px-2 py-1.5 text-xs rounded text-white transition-colors',
                isSaving ? 'bg-blue-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700',
              )}
            >
              {t('remotesTab.createTag', { defaultValue: 'Create Tag' })}
            </button>
          </div>

          {sortedTags.length === 0 && !isLoading && (
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('remotesTab.noTags', { defaultValue: 'No tags found' })}
            </div>
          )}

          {sortedTags.map((tag) => (
            <div
              key={tag.name}
              className="rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800/50 p-2"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{tag.name}</span>
                    {tag.is_annotated && (
                      <span className="text-[10px] px-1 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300">
                        annotated
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-gray-600 dark:text-gray-400 truncate">{tag.sha}</div>
                </div>
                <button
                  onClick={() => void handleDeleteTag(tag.name)}
                  disabled={isSaving}
                  className="px-2 py-0.5 text-2xs rounded border border-red-300 dark:border-red-700 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
                >
                  {t('remotesTab.deleteTag', { defaultValue: 'Delete' })}
                </button>
              </div>
            </div>
          ))}
        </section>
      </div>

      {remoteEditor && (
        <div className="shrink-0 border-t border-gray-200 dark:border-gray-700 p-3 bg-gray-50 dark:bg-gray-800/40 space-y-2">
          <div className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('remotesTab.editRemoteUrl', {
              name: remoteEditor.name,
              defaultValue: `Set URL for ${remoteEditor.name}`,
            })}
          </div>
          <input
            type="text"
            value={remoteEditor.url}
            onChange={(e) => setRemoteEditor({ ...remoteEditor, url: e.target.value })}
            className="w-full px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-800 dark:text-gray-200"
          />
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setRemoteEditor(null)}
              className="px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
            >
              {t('remotesTab.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              onClick={() => void handleSaveRemoteUrl()}
              disabled={isSaving}
              className={clsx(
                'px-2 py-1 text-xs rounded text-white',
                isSaving ? 'bg-blue-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700',
              )}
            >
              {t('remotesTab.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default RemotesTab;
