/**
 * BranchList Component
 *
 * Displays local and remote branches with collapsible sections.
 * Supports checkout, fetch, and context menu actions.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { BranchInfo, RemoteBranchInfo, CommandResponse } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface BranchListProps {
  localBranches: BranchInfo[];
  remoteBranches: RemoteBranchInfo[];
  currentBranch: BranchInfo | null;
  repoPath: string;
  onRefresh: () => Promise<void>;
  onMerge: (branchName: string) => void;
  onDelete: (branch: BranchInfo) => void;
  onRename: (branch: BranchInfo) => void;
}

interface BranchMenuState {
  branch: BranchInfo;
  x: number;
  y: number;
}

// ---------------------------------------------------------------------------
// CollapsibleSection
// ---------------------------------------------------------------------------

function CollapsibleSection({
  title,
  count,
  defaultOpen = true,
  actions,
  children,
}: {
  title: string;
  count: number;
  defaultOpen?: boolean;
  actions?: React.ReactNode;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center justify-between px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
      >
        <div className="flex items-center gap-1.5">
          <svg
            className={clsx('w-3 h-3 transition-transform', open && 'rotate-90')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          <span>{title}</span>
          <span className="text-2xs font-normal text-gray-400 dark:text-gray-500">
            ({count})
          </span>
        </div>
        {actions && (
          <div onClick={(e) => e.stopPropagation()}>
            {actions}
          </div>
        )}
      </button>
      {open && children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// AheadBehindBadge
// ---------------------------------------------------------------------------

function AheadBehindBadge({ ahead, behind }: { ahead: number; behind: number }) {
  const { t } = useTranslation('git');
  if (ahead === 0 && behind === 0) return null;

  return (
    <span className="flex items-center gap-1 text-2xs font-mono text-gray-500 dark:text-gray-400">
      {ahead > 0 && (
        <span className="text-green-600 dark:text-green-400" title={t('branchList.ahead', { count: ahead })}>
          {'\u2191'}{ahead}
        </span>
      )}
      {behind > 0 && (
        <span className="text-orange-600 dark:text-orange-400" title={t('branchList.behind', { count: behind })}>
          {'\u2193'}{behind}
        </span>
      )}
    </span>
  );
}

// ---------------------------------------------------------------------------
// BranchRow
// ---------------------------------------------------------------------------

function BranchRow({
  branch,
  repoPath,
  onRefresh,
  onMenuOpen,
}: {
  branch: BranchInfo;
  repoPath: string;
  onRefresh: () => Promise<void>;
  onMenuOpen: (branch: BranchInfo, e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation('git');
  const [checkingOut, setCheckingOut] = useState(false);

  const handleCheckout = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (branch.is_head || checkingOut) return;
      setCheckingOut(true);
      try {
        await invoke<CommandResponse<void>>('git_checkout_branch', {
          repoPath,
          name: branch.name,
        });
        await onRefresh();
      } catch {
        // Handle error silently for now
      } finally {
        setCheckingOut(false);
      }
    },
    [branch, repoPath, onRefresh, checkingOut]
  );

  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-3 py-1.5 group hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors',
        branch.is_head && 'bg-blue-50/50 dark:bg-blue-900/10'
      )}
    >
      {/* Current branch indicator */}
      <span className="w-2 shrink-0">
        {branch.is_head && (
          <span className="inline-block w-2 h-2 rounded-full bg-green-500" title={t('branchList.currentBranch')} />
        )}
      </span>

      {/* Branch name */}
      <span
        className={clsx(
          'flex-1 text-sm truncate',
          branch.is_head
            ? 'font-semibold text-gray-900 dark:text-gray-100'
            : 'text-gray-700 dark:text-gray-300'
        )}
        title={branch.name}
      >
        {branch.name}
      </span>

      {/* Upstream info */}
      {branch.upstream && (
        <AheadBehindBadge ahead={branch.ahead} behind={branch.behind} />
      )}

      {/* Actions */}
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
        {!branch.is_head && (
          <button
            onClick={handleCheckout}
            disabled={checkingOut}
            className="p-1 rounded text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
            title={t('branchList.checkout')}
          >
            {checkingOut ? (
              <div className="w-3.5 h-3.5 border border-gray-400 border-t-transparent rounded-full animate-spin" />
            ) : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 17l9.2-9.2M17 17V7H7" />
              </svg>
            )}
          </button>
        )}

        <button
          onClick={(e) => {
            e.stopPropagation();
            onMenuOpen(branch, e);
          }}
          className="p-1 rounded text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          title={t('branchList.moreActions')}
        >
          <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
            <circle cx="12" cy="5" r="1.5" />
            <circle cx="12" cy="12" r="1.5" />
            <circle cx="12" cy="19" r="1.5" />
          </svg>
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RemoteBranchRow
// ---------------------------------------------------------------------------

function RemoteBranchRow({ branch }: { branch: RemoteBranchInfo }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
      <span className="w-2 shrink-0" />
      <span className="flex-1 text-sm text-gray-600 dark:text-gray-400 truncate" title={branch.name}>
        {branch.name}
      </span>
      <span className="text-2xs font-mono text-gray-400 dark:text-gray-500 truncate max-w-[60px]">
        {branch.tip_sha}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// BranchContextMenu
// ---------------------------------------------------------------------------

function BranchContextMenu({
  menu,
  repoPath,
  currentBranch,
  onClose,
  onRefresh,
  onMerge,
  onDelete,
  onRename,
}: {
  menu: BranchMenuState;
  repoPath: string;
  currentBranch: BranchInfo | null;
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onMerge: (branchName: string) => void;
  onDelete: (branch: BranchInfo) => void;
  onRename: (branch: BranchInfo) => void;
}) {
  const { t } = useTranslation('git');
  const [loading, setLoading] = useState(false);

  const handlePush = useCallback(async () => {
    setLoading(true);
    try {
      await invoke<CommandResponse<void>>('git_push', {
        repoPath,
        branch: menu.branch.name,
        setUpstream: !menu.branch.upstream,
      });
      await onRefresh();
    } catch {
      // Silently fail
    } finally {
      setLoading(false);
      onClose();
    }
  }, [repoPath, menu.branch, onRefresh, onClose]);

  const handlePull = useCallback(async () => {
    setLoading(true);
    try {
      await invoke<CommandResponse<void>>('git_pull', { repoPath });
      await onRefresh();
    } catch {
      // Silently fail
    } finally {
      setLoading(false);
      onClose();
    }
  }, [repoPath, onRefresh, onClose]);

  const items = [
    {
      label: t('branchList.mergeInto', { branch: currentBranch?.name || 'current' }),
      action: () => {
        onMerge(menu.branch.name);
        onClose();
      },
      disabled: menu.branch.is_head,
      show: true,
    },
    {
      label: t('branchList.push'),
      action: handlePush,
      disabled: loading,
      show: menu.branch.is_head || !!menu.branch.upstream,
    },
    {
      label: t('branchList.pull'),
      action: handlePull,
      disabled: loading || !menu.branch.is_head,
      show: menu.branch.is_head,
    },
    { label: 'divider', action: () => {}, disabled: false, show: true },
    {
      label: t('branchList.rename'),
      action: () => {
        onRename(menu.branch);
        onClose();
      },
      disabled: false,
      show: true,
    },
    {
      label: t('branchList.delete'),
      action: () => {
        onDelete(menu.branch);
        onClose();
      },
      disabled: menu.branch.is_head,
      show: true,
      danger: true,
    },
  ];

  return (
    <>
      {/* Backdrop */}
      <div className="fixed inset-0 z-40" onClick={onClose} />

      {/* Menu */}
      <div
        className="fixed z-50 min-w-[180px] py-1 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700"
        style={{ left: menu.x, top: menu.y }}
      >
        {items
          .filter((item) => item.show)
          .map((item, idx) => {
            if (item.label === 'divider') {
              return (
                <div
                  key={`divider-${idx}`}
                  className="my-1 border-t border-gray-200 dark:border-gray-700"
                />
              );
            }
            return (
              <button
                key={item.label}
                onClick={item.action}
                disabled={item.disabled}
                className={clsx(
                  'w-full text-left px-3 py-1.5 text-sm transition-colors',
                  item.disabled
                    ? 'text-gray-400 dark:text-gray-600 cursor-not-allowed'
                    : (item as { danger?: boolean }).danger
                      ? 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20'
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
                )}
              >
                {item.label}
              </button>
            );
          })}
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// BranchList Component
// ---------------------------------------------------------------------------

export function BranchList({
  localBranches,
  remoteBranches,
  currentBranch,
  repoPath,
  onRefresh,
  onMerge,
  onDelete,
  onRename,
}: BranchListProps) {
  const { t } = useTranslation('git');
  const [menu, setMenu] = useState<BranchMenuState | null>(null);
  const [isFetching, setIsFetching] = useState(false);

  const handleMenuOpen = useCallback((branch: BranchInfo, e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({
      branch,
      x: Math.min(rect.right, window.innerWidth - 200),
      y: Math.min(rect.bottom, window.innerHeight - 250),
    });
  }, []);

  const handleFetch = useCallback(async () => {
    if (isFetching) return;
    setIsFetching(true);
    try {
      await invoke<CommandResponse<void>>('git_fetch', { repoPath });
      await onRefresh();
    } catch {
      // Silently fail
    } finally {
      setIsFetching(false);
    }
  }, [repoPath, onRefresh, isFetching]);

  // Sort: current branch first, then alphabetically
  const sortedLocal = [...localBranches].sort((a, b) => {
    if (a.is_head) return -1;
    if (b.is_head) return 1;
    return a.name.localeCompare(b.name);
  });

  return (
    <div className="flex-1 min-h-0 overflow-y-auto">
      {/* Local Branches */}
      <CollapsibleSection title={t('branchList.local')} count={localBranches.length}>
        <div>
          {sortedLocal.map((branch) => (
            <BranchRow
              key={branch.name}
              branch={branch}
              repoPath={repoPath}
              onRefresh={onRefresh}
              onMenuOpen={handleMenuOpen}
            />
          ))}
          {localBranches.length === 0 && (
            <div className="px-3 py-4 text-center text-sm text-gray-500 dark:text-gray-400">
              {t('branchList.noLocalBranches')}
            </div>
          )}
        </div>
      </CollapsibleSection>

      {/* Remote Branches */}
      <CollapsibleSection
        title={t('branchList.remote')}
        count={remoteBranches.length}
        defaultOpen={false}
        actions={
          <button
            onClick={handleFetch}
            disabled={isFetching}
            className={clsx(
              'px-2 py-0.5 text-2xs rounded transition-colors',
              'text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700',
              isFetching && 'opacity-50 cursor-not-allowed'
            )}
            title={t('branchList.fetchAll')}
          >
            {isFetching ? (
              <div className="inline-block w-3 h-3 border border-gray-400 border-t-transparent rounded-full animate-spin" />
            ) : (
              t('branchList.fetch')
            )}
          </button>
        }
      >
        <div>
          {remoteBranches.map((branch) => (
            <RemoteBranchRow key={branch.name} branch={branch} />
          ))}
          {remoteBranches.length === 0 && (
            <div className="px-3 py-4 text-center text-sm text-gray-500 dark:text-gray-400">
              {t('branchList.noRemoteBranches')}
            </div>
          )}
        </div>
      </CollapsibleSection>

      {/* Context Menu */}
      {menu && (
        <BranchContextMenu
          menu={menu}
          repoPath={repoPath}
          currentBranch={currentBranch}
          onClose={() => setMenu(null)}
          onRefresh={onRefresh}
          onMerge={onMerge}
          onDelete={onDelete}
          onRename={onRename}
        />
      )}
    </div>
  );
}
