import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Command } from '@tauri-apps/plugin-shell';
import * as Dialog from '@radix-ui/react-dialog';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ArrowTopRightIcon, ChevronDownIcon, Link2Icon, OpenInNewWindowIcon } from '@radix-ui/react-icons';
import { useToast } from '../shared/Toast';
import { useSettingsStore } from '../../store/settings';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import {
  selectKernelChatRuntime,
  selectKernelDebugRuntime,
  selectKernelPlanRuntime,
  selectKernelTaskRuntime,
} from '../../store/workflowKernelSelectors';
import { useActiveSessionPaths } from './useActiveSessionPaths';
import type { CommandResponse } from '../../lib/tauri';
import type {
  BranchInfo,
  PreparePullRequestResult,
  PullRequestState,
  Worktree,
  WorktreeCleanupPolicy,
} from '../../types/git';
import type { WorkflowSession } from '../../types/workflowKernel';

interface MoveDialogState {
  mode: 'move' | 'recreate';
  open: boolean;
}

function BranchGlyph({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-hidden="true"
    >
      <circle cx="4" cy="3" r="1.75" />
      <circle cx="12" cy="6.5" r="1.75" />
      <circle cx="4" cy="13" r="1.75" />
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M5.75 3h1.5a3 3 0 0 1 3 3v0.5M10.25 6.5h-3a3 3 0 0 0-3 3V11.25"
      />
    </svg>
  );
}

function PullRequestGlyph({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-hidden="true"
    >
      <circle cx="4" cy="3" r="1.75" />
      <circle cx="12" cy="3" r="1.75" />
      <circle cx="12" cy="13" r="1.75" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M4 4.75v6a2.5 2.5 0 0 0 2.5 2.5H10.25M12 4.75v4.5" />
    </svg>
  );
}

function basename(path: string | null | undefined): string | null {
  if (!path) return null;
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function sanitizeBranchSlug(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9\-_]/g, '_')
    .replace(/-+/g, '-')
    .replace(/^[-_]+|[-_]+$/g, '');
  return normalized || 'session-runtime';
}

export function isValidWorktreeBranchName(value: string): boolean {
  const normalized = value.trim();
  return normalized.length > 0 && !/\s/.test(normalized) && !normalized.includes('..') && !normalized.startsWith('-');
}

export function buildDefaultWorktreeBranchName(
  session: Pick<WorkflowSession, 'sessionId' | 'displayTitle'> | null,
  workspaceRootPath: string | null,
): string {
  const source = session?.displayTitle?.trim() || basename(workspaceRootPath) || 'session-runtime';
  const shortId = session?.sessionId?.slice(0, 8) || 'runtime';
  return `pc/${sanitizeBranchSlug(source)}-${shortId}`;
}

function runtimeLabel(t: (key: string, options?: { defaultValue?: string }) => string, runtimeKind: string): string {
  if (runtimeKind === 'managed_worktree') {
    return t('topBar.runtime.worktree', { defaultValue: 'Worktree' });
  }
  if (runtimeKind === 'legacy_worktree') {
    return t('topBar.runtime.legacy', { defaultValue: 'Legacy' });
  }
  return t('topBar.runtime.main', { defaultValue: 'Main' });
}

function prStateLabel(
  t: (key: string, options?: { defaultValue?: string }) => string,
  state: PullRequestState | null | undefined,
): string | null {
  if (!state) return null;
  return t(`topBar.prStateValues.${state}`, {
    defaultValue: state.replace(/_/g, ' '),
  });
}

async function openPathInFileManager(path: string) {
  const platform = navigator.platform.toLowerCase();
  let cmdName = 'open-path-linux';
  if (platform.includes('mac')) {
    cmdName = 'open-path-macos';
  } else if (platform.includes('win')) {
    cmdName = 'open-path-windows';
  }
  await Command.create(cmdName, [path]).execute();
}

function RuntimeBadge({
  label,
  runtimeKind,
}: {
  label: string;
  runtimeKind: 'main' | 'managed_worktree' | 'legacy_worktree';
}) {
  return (
    <span
      className={clsx(
        'inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium',
        runtimeKind === 'main' && 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300',
        runtimeKind === 'managed_worktree' && 'bg-amber-50 text-amber-700 dark:bg-amber-900/20 dark:text-amber-300',
        runtimeKind === 'legacy_worktree' && 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
      )}
    >
      {label}
    </span>
  );
}

function BranchSwitcher({
  repoPath,
  fallbackBranch,
  disabledReason,
}: {
  repoPath: string | null;
  fallbackBranch: string | null;
  disabledReason: string | null;
}) {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const [open, setOpen] = useState(false);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [search, setSearch] = useState('');
  const [newBranchName, setNewBranchName] = useState('');
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadBranches = useCallback(async () => {
    if (!repoPath) {
      setBranches([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<CommandResponse<BranchInfo[]>>('git_list_branches', { repoPath });
      if (result.success && result.data) {
        setBranches(result.data);
      } else {
        setBranches([]);
        setError(result.error || t('topBar.branchFailed', { defaultValue: 'Failed to load branches.' }));
      }
    } catch (loadError) {
      setBranches([]);
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, [repoPath, t]);

  useEffect(() => {
    if (!open) return;
    void loadBranches();
  }, [loadBranches, open]);

  const currentBranch =
    branches.find((branch) => branch.is_head)?.name ??
    fallbackBranch ??
    t('topBar.branchUnknown', { defaultValue: 'no-branch' });
  const normalizedSearch = search.trim().toLowerCase();
  const filteredBranches = branches.filter((branch) => branch.name.toLowerCase().includes(normalizedSearch));

  const handleCheckout = useCallback(
    async (branchName: string) => {
      if (!repoPath) return;
      setSubmitting(true);
      setError(null);
      try {
        const result = await invoke<CommandResponse<void>>('git_checkout_branch', {
          repoPath,
          name: branchName,
        });
        if (!result.success) {
          setError(result.error || t('topBar.branchCheckoutFailed', { defaultValue: 'Failed to switch branch.' }));
          return;
        }
        await loadBranches();
        showToast(t('topBar.toast.branchSwitched', { defaultValue: 'Checked out branch.' }), 'success');
        setOpen(false);
      } catch (checkoutError) {
        setError(checkoutError instanceof Error ? checkoutError.message : String(checkoutError));
      } finally {
        setSubmitting(false);
      }
    },
    [loadBranches, repoPath, showToast, t],
  );

  const handleCreateAndCheckout = useCallback(async () => {
    const normalized = newBranchName.trim();
    if (!repoPath) return;
    if (!normalized) {
      setError(t('topBar.createBranchRequired', { defaultValue: 'New branch name is required.' }));
      return;
    }
    if (!isValidWorktreeBranchName(normalized)) {
      setError(t('topBar.dialog.invalidBranch', { defaultValue: 'Branch name is invalid.' }));
      return;
    }
    const baseBranch = branches.find((branch) => branch.is_head)?.name || branches[0]?.name || 'HEAD';
    setSubmitting(true);
    setError(null);
    try {
      const createResult = await invoke<CommandResponse<void>>('git_create_branch', {
        repoPath,
        name: normalized,
        base: baseBranch,
      });
      if (!createResult.success) {
        setError(createResult.error || t('topBar.createBranchFailed', { defaultValue: 'Failed to create branch.' }));
        return;
      }
      const checkoutResult = await invoke<CommandResponse<void>>('git_checkout_branch', {
        repoPath,
        name: normalized,
      });
      if (!checkoutResult.success) {
        setError(
          checkoutResult.error || t('topBar.branchCheckoutFailed', { defaultValue: 'Failed to switch branch.' }),
        );
        return;
      }
      await loadBranches();
      setNewBranchName('');
      showToast(t('topBar.toast.branchCreated', { defaultValue: 'Created and checked out branch.' }), 'success');
      setOpen(false);
    } catch (createError) {
      setError(createError instanceof Error ? createError.message : String(createError));
    } finally {
      setSubmitting(false);
    }
  }, [branches, loadBranches, newBranchName, repoPath, showToast, t]);

  return (
    <DropdownMenu.Root open={open} onOpenChange={setOpen}>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          title={disabledReason ?? repoPath ?? undefined}
          className={clsx(
            'inline-flex items-center gap-1 rounded-full bg-gray-100 px-2 py-0.5 font-mono text-[11px] text-gray-700 dark:bg-gray-800 dark:text-gray-300',
            repoPath && !disabledReason && 'hover:bg-gray-200 dark:hover:bg-gray-700',
            disabledReason && 'cursor-not-allowed opacity-60',
          )}
          disabled={!repoPath || !!disabledReason}
        >
          <BranchGlyph className="h-3 w-3" />
          <span className="max-w-[180px] truncate">{currentBranch}</span>
          <ChevronDownIcon className="h-3 w-3" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="start"
          sideOffset={8}
          className="w-[320px] rounded-xl border border-gray-200 bg-white p-2 shadow-lg dark:border-gray-700 dark:bg-gray-800"
          onCloseAutoFocus={(event) => event.preventDefault()}
        >
          <div className="space-y-2">
            <div>
              <input
                type="text"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => event.stopPropagation()}
                placeholder={t('topBar.searchBranches', { defaultValue: 'Search branches...' })}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-100"
              />
            </div>

            <div className="max-h-56 space-y-1 overflow-y-auto rounded-lg border border-gray-200 p-1 dark:border-gray-700">
              {loading ? (
                <div className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400">
                  {t('topBar.loadingBranches', { defaultValue: 'Loading branches...' })}
                </div>
              ) : filteredBranches.length === 0 ? (
                <div className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400">
                  {t('topBar.noBranches', { defaultValue: 'No branches found.' })}
                </div>
              ) : (
                filteredBranches.map((branch) => (
                  <button
                    key={branch.name}
                    type="button"
                    disabled={submitting || branch.is_head}
                    onClick={() => void handleCheckout(branch.name)}
                    className={clsx(
                      'flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-colors',
                      branch.is_head
                        ? 'bg-primary-50 text-primary-700 dark:bg-primary-900/20 dark:text-primary-300'
                        : 'text-gray-700 hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700',
                      submitting && 'cursor-not-allowed opacity-60',
                    )}
                  >
                    <span className="min-w-0 truncate font-mono">{branch.name}</span>
                    {branch.is_head ? (
                      <span className="ml-2 shrink-0 text-[11px] font-medium">
                        {t('topBar.currentBranch', { defaultValue: 'Current' })}
                      </span>
                    ) : null}
                  </button>
                ))
              )}
            </div>

            <div className="rounded-lg border border-gray-200 p-2 dark:border-gray-700">
              <div className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('topBar.createAndCheckout', { defaultValue: 'Create and checkout' })}
              </div>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={newBranchName}
                  onChange={(event) => setNewBranchName(event.target.value)}
                  onKeyDown={(event) => {
                    event.stopPropagation();
                    if (event.key === 'Enter') {
                      event.preventDefault();
                      void handleCreateAndCheckout();
                    }
                  }}
                  placeholder={t('topBar.newBranchPlaceholder', { defaultValue: 'feature/my-branch' })}
                  className="min-w-0 flex-1 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-100"
                />
                <button
                  type="button"
                  onClick={() => void handleCreateAndCheckout()}
                  disabled={submitting}
                  className={clsx(
                    'rounded-lg px-3 py-2 text-sm font-medium text-white transition-colors',
                    submitting ? 'cursor-not-allowed bg-primary-400' : 'bg-primary-600 hover:bg-primary-700',
                  )}
                >
                  {t('topBar.create', { defaultValue: 'Create' })}
                </button>
              </div>
            </div>

            {error ? <div className="px-1 text-sm text-red-600 dark:text-red-400">{error}</div> : null}
          </div>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function SimpleTopBarContext() {
  const { t } = useTranslation('simpleMode');
  const session = useWorkflowKernelStore((state) => state.session);
  const fallbackWorkspacePath = useSettingsStore((state) => state.workspacePath);
  const activeSessionPaths = useActiveSessionPaths(session, fallbackWorkspacePath);
  const chatRuntime = selectKernelChatRuntime(session);
  const taskRuntime = selectKernelTaskRuntime(session);
  const planRuntime = selectKernelPlanRuntime(session);
  const debugRuntime = selectKernelDebugRuntime(session);

  if (!session) {
    return (
      <div className="flex h-full items-center px-2 text-xs text-gray-500 dark:text-gray-400">
        {t('topBar.noActiveSession', { defaultValue: 'No active session' })}
      </div>
    );
  }

  const prState = prStateLabel(t, session.runtime?.prStatus?.state);
  const isBusy = chatRuntime.isBusy || taskRuntime.isBusy || planRuntime.isBusy || debugRuntime.isBusy;
  const branchDisabledReason = isBusy
    ? t('topBar.branchBusy', { defaultValue: 'Finish the current run before switching branches.' })
    : null;

  return (
    <div className="flex min-w-0 items-center gap-2 px-2">
      <span
        className="max-w-[420px] truncate text-sm font-medium text-gray-900 dark:text-gray-100"
        title={session.displayTitle}
      >
        {session.displayTitle}
      </span>
      <RuntimeBadge
        label={runtimeLabel(t, activeSessionPaths.runtimeKind)}
        runtimeKind={activeSessionPaths.runtimeKind}
      />
      <BranchSwitcher
        repoPath={activeSessionPaths.runtimePath}
        fallbackBranch={activeSessionPaths.runtimeBranch}
        disabledReason={branchDisabledReason}
      />
      {prState ? (
        <span className="inline-flex shrink-0 items-center rounded-full bg-sky-50 px-2 py-0.5 text-[11px] font-medium text-sky-700 dark:bg-sky-900/20 dark:text-sky-300">
          {t('topBar.prState', { defaultValue: 'PR' })}: {prState}
        </span>
      ) : null}
    </div>
  );
}

interface MoveToWorktreeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  session: WorkflowSession;
  workspaceRootPath: string;
  mode: 'move' | 'recreate';
  onCompleted: (message: string) => void;
}

function MoveToWorktreeDialog({
  open,
  onOpenChange,
  session,
  workspaceRootPath,
  mode,
  onCompleted,
}: MoveToWorktreeDialogProps) {
  const { t } = useTranslation('simpleMode');
  const moveSessionToManagedWorktree = useWorkflowKernelStore((state) => state.moveSessionToManagedWorktree);
  const runtimePathHint = '~/.plan-cascade/worktrees/...';
  const [branchName, setBranchName] = useState('');
  const [targetBranch, setTargetBranch] = useState('');
  const [cleanupPolicy, setCleanupPolicy] = useState<WorktreeCleanupPolicy>('manual');
  const [availableBranches, setAvailableBranches] = useState<BranchInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    const defaultBranch = buildDefaultWorktreeBranchName(session, workspaceRootPath);
    setBranchName(defaultBranch);
    setCleanupPolicy('manual');
    setError(null);

    let active = true;
    invoke<CommandResponse<BranchInfo[]>>('git_list_branches', { repoPath: workspaceRootPath })
      .then((result) => {
        if (!active) return;
        if (result.success && result.data) {
          setAvailableBranches(result.data);
          const current = result.data.find((branch) => branch.is_head)?.name;
          setTargetBranch(session.runtime?.targetBranch || current || session.runtime?.branch || 'main');
        } else {
          setAvailableBranches([]);
          setTargetBranch(session.runtime?.targetBranch || session.runtime?.branch || 'main');
        }
      })
      .catch((loadError) => {
        if (!active) return;
        setAvailableBranches([]);
        setTargetBranch(session.runtime?.targetBranch || session.runtime?.branch || 'main');
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      });

    return () => {
      active = false;
    };
  }, [mode, open, session, workspaceRootPath]);

  const handleSubmit = useCallback(async () => {
    const normalizedBranch = branchName.trim();
    if (!normalizedBranch) {
      setError(t('topBar.dialog.branchRequired', { defaultValue: 'Branch name is required.' }));
      return;
    }
    if (!isValidWorktreeBranchName(normalizedBranch)) {
      setError(t('topBar.dialog.invalidBranch', { defaultValue: 'Branch name is invalid.' }));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const updated = await moveSessionToManagedWorktree({
        sessionId: session.sessionId,
        repoPath: workspaceRootPath,
        branchName: normalizedBranch,
        targetBranch: targetBranch.trim() || 'main',
        cleanupPolicy,
      });
      if (!updated) {
        setError(t('topBar.dialog.submitFailed', { defaultValue: 'Failed to move session to worktree.' }));
        return;
      }
      onOpenChange(false);
      onCompleted(
        mode === 'recreate'
          ? t('topBar.toast.recreatedManaged', { defaultValue: 'Recreated runtime as a managed worktree.' })
          : t('topBar.toast.moved', { defaultValue: 'Moved session to managed worktree.' }),
      );
    } finally {
      setSubmitting(false);
    }
  }, [
    branchName,
    cleanupPolicy,
    mode,
    moveSessionToManagedWorktree,
    onCompleted,
    onOpenChange,
    session.sessionId,
    t,
    targetBranch,
    workspaceRootPath,
  ]);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,560px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
          <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {mode === 'recreate'
              ? t('topBar.dialog.recreateTitle', { defaultValue: 'Recreate as managed worktree' })
              : t('topBar.dialog.moveTitle', { defaultValue: 'Move to worktree' })}
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
            {t('topBar.dialog.moveDescription', {
              defaultValue: 'Migrate this session runtime into a managed worktree without creating a new session.',
            })}
          </Dialog.Description>

          <div className="mt-4 space-y-4">
            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('topBar.dialog.session', { defaultValue: 'Session' })}
              </label>
              <div className="mt-1 rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-800 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-200">
                {session.displayTitle}
              </div>
            </div>

            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('topBar.dialog.workspaceRoot', { defaultValue: 'Workspace root' })}
              </label>
              <div
                className="mt-1 rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-800 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-200"
                title={workspaceRootPath}
              >
                {workspaceRootPath}
              </div>
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('topBar.dialog.worktreeBranch', { defaultValue: 'Worktree branch' })}
              </label>
              <input
                type="text"
                value={branchName}
                onChange={(event) => setBranchName(event.target.value)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              />
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('topBar.dialog.targetBranch', { defaultValue: 'Target branch' })}
              </label>
              <input
                list="simple-topbar-target-branches"
                type="text"
                value={targetBranch}
                onChange={(event) => setTargetBranch(event.target.value)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              />
              <datalist id="simple-topbar-target-branches">
                {availableBranches.map((branch) => (
                  <option key={branch.name} value={branch.name} />
                ))}
              </datalist>
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('topBar.dialog.cleanupPolicy', { defaultValue: 'Cleanup policy' })}
              </label>
              <select
                value={cleanupPolicy}
                onChange={(event) => setCleanupPolicy(event.target.value as WorktreeCleanupPolicy)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              >
                <option value="manual">{t('topBar.dialog.manualCleanup', { defaultValue: 'Manual cleanup' })}</option>
                <option value="delete_on_session_delete">
                  {t('topBar.dialog.deleteOnSessionDelete', { defaultValue: 'Delete on session delete' })}
                </option>
              </select>
            </div>

            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('topBar.dialog.runtimeLocation', { defaultValue: 'Runtime location' })}
              </label>
              <div className="mt-1 rounded-lg border border-dashed border-gray-300 px-3 py-2 text-xs text-gray-600 dark:border-gray-600 dark:text-gray-300">
                {runtimePathHint}
              </div>
            </div>

            {error ? <div className="text-sm text-red-600 dark:text-red-400">{error}</div> : null}
          </div>

          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              {t('topBar.dialog.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              onClick={() => void handleSubmit()}
              disabled={submitting}
              className={clsx(
                'rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors',
                submitting ? 'cursor-not-allowed bg-primary-400' : 'bg-primary-600 hover:bg-primary-700',
              )}
            >
              {submitting
                ? t('topBar.dialog.continuing', { defaultValue: 'Continuing...' })
                : t('topBar.dialog.continue', { defaultValue: 'Continue' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

interface SwitchWorktreeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  session: WorkflowSession;
  repoPath: string;
  currentRuntimePath: string | null;
  onCompleted: (message: string) => void;
}

function SwitchWorktreeDialog({
  open,
  onOpenChange,
  session,
  repoPath,
  currentRuntimePath,
  onCompleted,
}: SwitchWorktreeDialogProps) {
  const { t } = useTranslation('simpleMode');
  const listRepoWorktrees = useWorkflowKernelStore((state) => state.listRepoWorktrees);
  const attachSessionWorktree = useWorkflowKernelStore((state) => state.attachSessionWorktree);
  const [worktrees, setWorktrees] = useState<Worktree[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setError(null);
    setSelectedPath('');
    listRepoWorktrees(repoPath)
      .then((items) => {
        if (!active) return;
        const available = items.filter((item) => item.path !== currentRuntimePath);
        setWorktrees(available);
        setSelectedPath(available[0]?.path ?? '');
      })
      .catch((loadError) => {
        if (!active) return;
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      });
    return () => {
      active = false;
    };
  }, [currentRuntimePath, listRepoWorktrees, open, repoPath]);

  const handleSubmit = useCallback(async () => {
    if (!selectedPath) {
      setError(t('topBar.dialog.noWorktrees', { defaultValue: 'No compatible worktrees were found.' }));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const updated = await attachSessionWorktree({
        sessionId: session.sessionId,
        repoPath,
        worktreePath: selectedPath,
      });
      if (!updated) {
        setError(t('topBar.dialog.switchFailed', { defaultValue: 'Failed to switch worktree.' }));
        return;
      }
      onOpenChange(false);
      onCompleted(t('topBar.toast.switched', { defaultValue: 'Switched session runtime.' }));
    } finally {
      setSubmitting(false);
    }
  }, [attachSessionWorktree, onCompleted, onOpenChange, repoPath, selectedPath, session.sessionId, t]);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,560px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
          <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {t('topBar.dialog.switchTitle', { defaultValue: 'Switch worktree' })}
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
            {t('topBar.dialog.switchDescription', {
              defaultValue: 'Bind this session to another managed or legacy worktree for the same repository.',
            })}
          </Dialog.Description>

          <div className="mt-4 space-y-3">
            {worktrees.length === 0 ? (
              <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-sm text-gray-500 dark:border-gray-600 dark:text-gray-400">
                {t('topBar.dialog.noWorktrees', { defaultValue: 'No compatible worktrees were found.' })}
              </div>
            ) : (
              worktrees.map((worktree) => (
                <label
                  key={worktree.path}
                  className={clsx(
                    'flex cursor-pointer items-start gap-3 rounded-lg border px-3 py-3 text-sm transition-colors',
                    selectedPath === worktree.path
                      ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                      : 'border-gray-200 hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800',
                  )}
                >
                  <input
                    type="radio"
                    name="simple-topbar-switch-worktree"
                    className="mt-1"
                    checked={selectedPath === worktree.path}
                    onChange={() => setSelectedPath(worktree.path)}
                  />
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-gray-900 dark:text-gray-100">
                        {worktree.display_label || worktree.name}
                      </span>
                      <RuntimeBadge
                        label={
                          worktree.runtime_kind === 'managed'
                            ? t('topBar.runtime.worktree', { defaultValue: 'Worktree' })
                            : t('topBar.runtime.legacy', { defaultValue: 'Legacy' })
                        }
                        runtimeKind={worktree.runtime_kind === 'managed' ? 'managed_worktree' : 'legacy_worktree'}
                      />
                    </div>
                    <div className="mt-1 font-mono text-xs text-gray-500 dark:text-gray-400">{worktree.branch}</div>
                    <div className="mt-1 truncate text-xs text-gray-500 dark:text-gray-400" title={worktree.path}>
                      {worktree.path}
                    </div>
                  </div>
                </label>
              ))
            )}
            {error ? <div className="text-sm text-red-600 dark:text-red-400">{error}</div> : null}
          </div>

          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              {t('topBar.dialog.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              disabled={submitting || !selectedPath}
              onClick={() => void handleSubmit()}
              className={clsx(
                'rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors',
                submitting || !selectedPath
                  ? 'cursor-not-allowed bg-primary-400'
                  : 'bg-primary-600 hover:bg-primary-700',
              )}
            >
              {submitting
                ? t('topBar.dialog.switching', { defaultValue: 'Switching...' })
                : t('topBar.dialog.continue', { defaultValue: 'Continue' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

interface CreatePullRequestDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  prepareResult: PreparePullRequestResult | null;
  sessionId: string;
  onCompleted: (message: string) => void;
}

function CreatePullRequestDialog({
  open,
  onOpenChange,
  prepareResult,
  sessionId,
  onCompleted,
}: CreatePullRequestDialogProps) {
  const { t } = useTranslation('simpleMode');
  const createSessionPr = useWorkflowKernelStore((state) => state.createSessionPr);
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [draft, setDraft] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !prepareResult) return;
    setTitle(`${prepareResult.head_branch} -> ${prepareResult.base_branch}`);
    setBody('');
    setDraft(false);
    setError(null);
  }, [open, prepareResult]);

  const handleSubmit = useCallback(async () => {
    if (!prepareResult) return;
    setSubmitting(true);
    setError(null);
    try {
      const result = await createSessionPr({
        sessionId,
        provider: prepareResult.forge_provider,
        remoteName: prepareResult.remote_name,
        title: title.trim() || `${prepareResult.head_branch} -> ${prepareResult.base_branch}`,
        body,
        draft,
      });
      if (!result) {
        setError(t('topBar.dialog.createPrFailed', { defaultValue: 'Failed to create pull request.' }));
        return;
      }
      onOpenChange(false);
      if (result.url) {
        window.open(result.url, '_blank', 'noopener,noreferrer');
      }
      onCompleted(t('topBar.toast.prCreated', { defaultValue: 'Pull request created.' }));
    } finally {
      setSubmitting(false);
    }
  }, [body, createSessionPr, draft, onCompleted, onOpenChange, prepareResult, sessionId, t, title]);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,640px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
          <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {t('topBar.dialog.createPrTitle', { defaultValue: 'Create pull request' })}
          </Dialog.Title>
          {prepareResult ? (
            <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
              {prepareResult.forge_provider} · {prepareResult.head_branch} → {prepareResult.base_branch}
            </Dialog.Description>
          ) : null}
          <div className="mt-4 space-y-4">
            <div>
              <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('topBar.dialog.prTitle', { defaultValue: 'PR title' })}
              </label>
              <input
                type="text"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              />
            </div>
            <div>
              <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('topBar.dialog.prBody', { defaultValue: 'PR body' })}
              </label>
              <textarea
                value={body}
                onChange={(event) => setBody(event.target.value)}
                rows={8}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              />
            </div>
            <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
              <input type="checkbox" checked={draft} onChange={(event) => setDraft(event.target.checked)} />
              {t('topBar.dialog.prDraft', { defaultValue: 'Create as draft' })}
            </label>
            {error ? <div className="text-sm text-red-600 dark:text-red-400">{error}</div> : null}
          </div>

          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              {t('topBar.dialog.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              onClick={() => void handleSubmit()}
              disabled={submitting || !prepareResult}
              className={clsx(
                'rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors',
                submitting || !prepareResult
                  ? 'cursor-not-allowed bg-primary-400'
                  : 'bg-primary-600 hover:bg-primary-700',
              )}
            >
              {submitting
                ? t('topBar.dialog.creatingPr', { defaultValue: 'Creating PR...' })
                : t('topBar.dialog.createPr', { defaultValue: 'Create PR' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function SessionRuntimeCapsule() {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const session = useWorkflowKernelStore((state) => state.session);
  const fallbackWorkspacePath = useSettingsStore((state) => state.workspacePath);
  const detachSessionWorktree = useWorkflowKernelStore((state) => state.detachSessionWorktree);
  const cleanupSessionWorktree = useWorkflowKernelStore((state) => state.cleanupSessionWorktree);
  const prepareSessionPr = useWorkflowKernelStore((state) => state.prepareSessionPr);
  const activeSessionPaths = useActiveSessionPaths(session, fallbackWorkspacePath);
  const chatRuntime = selectKernelChatRuntime(session);
  const taskRuntime = selectKernelTaskRuntime(session);
  const planRuntime = selectKernelPlanRuntime(session);
  const debugRuntime = selectKernelDebugRuntime(session);
  const [moveDialog, setMoveDialog] = useState<MoveDialogState>({ mode: 'move', open: false });
  const [switchDialogOpen, setSwitchDialogOpen] = useState(false);
  const [createPrDialogOpen, setCreatePrDialogOpen] = useState(false);
  const [prepareResult, setPrepareResult] = useState<PreparePullRequestResult | null>(null);

  const isBusy = chatRuntime.isBusy || taskRuntime.isBusy || planRuntime.isBusy || debugRuntime.isBusy;
  const mutationDisabledReason = isBusy
    ? t('topBar.runtimeBusy', { defaultValue: 'This session is currently running, so runtime changes are disabled.' })
    : null;

  const onCompleted = useCallback(
    (message: string) => {
      showToast(message, 'success');
    },
    [showToast],
  );

  const handleOpenRuntimeFolder = useCallback(async () => {
    if (!activeSessionPaths.runtimePath) return;
    try {
      await openPathInFileManager(activeSessionPaths.runtimePath);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  }, [activeSessionPaths.runtimePath, showToast]);

  const handlePreparePr = useCallback(async () => {
    if (!session) return;
    const result = await prepareSessionPr(session.sessionId);
    if (!result) {
      showToast(t('topBar.toast.preparePrFailed', { defaultValue: 'Failed to prepare pull request.' }), 'error');
      return;
    }
    setPrepareResult(result);
    window.open(result.create_url, '_blank', 'noopener,noreferrer');
    showToast(t('topBar.toast.prPrepared', { defaultValue: 'Opened pull request page.' }), 'success');
  }, [prepareSessionPr, session, showToast, t]);

  const handleCreatePr = useCallback(async () => {
    if (!session) return;
    let result = prepareResult;
    if (!result) {
      result = await prepareSessionPr(session.sessionId);
      if (!result) {
        showToast(t('topBar.toast.preparePrFailed', { defaultValue: 'Failed to prepare pull request.' }), 'error');
        return;
      }
      setPrepareResult(result);
    }
    setCreatePrDialogOpen(true);
  }, [prepareResult, prepareSessionPr, session, showToast, t]);

  const handleDetach = useCallback(async () => {
    if (!session) return;
    const updated = await detachSessionWorktree(session.sessionId);
    if (!updated) {
      showToast(t('topBar.toast.detachFailed', { defaultValue: 'Failed to detach worktree.' }), 'error');
      return;
    }
    showToast(t('topBar.toast.detached', { defaultValue: 'Detached runtime back to the main workspace.' }), 'success');
  }, [detachSessionWorktree, session, showToast, t]);

  const handleCleanup = useCallback(async () => {
    if (!session) return;
    const confirmed = window.confirm(
      t('topBar.dialog.cleanupConfirm', {
        defaultValue: 'Delete this managed worktree and move the session back to the main workspace?',
      }),
    );
    if (!confirmed) return;
    const updated = await cleanupSessionWorktree(session.sessionId, true);
    if (!updated) {
      showToast(t('topBar.toast.cleanupFailed', { defaultValue: 'Failed to clean up worktree.' }), 'error');
      return;
    }
    showToast(t('topBar.toast.cleaned', { defaultValue: 'Cleaned up managed worktree.' }), 'success');
  }, [cleanupSessionWorktree, session, showToast, t]);

  if (!session || !activeSessionPaths.workspaceRootPath) {
    return null;
  }

  const capsuleLabel =
    activeSessionPaths.runtimeKind === 'main'
      ? t('topBar.moveToWorktree', { defaultValue: 'Move to Worktree' })
      : activeSessionPaths.runtimeKind === 'legacy_worktree'
        ? t('topBar.legacyCapsule', { defaultValue: 'Legacy Worktree' })
        : `${t('topBar.runtime.worktree', { defaultValue: 'Worktree' })}: ${activeSessionPaths.runtimeBranch ?? 'unknown'}`;

  if (activeSessionPaths.runtimeKind === 'main') {
    return (
      <>
        <button
          type="button"
          title={mutationDisabledReason ?? undefined}
          disabled={!!mutationDisabledReason}
          onClick={() => setMoveDialog({ mode: 'move', open: true })}
          className={clsx(
            'inline-flex h-8 items-center gap-2 rounded-full border px-3 text-sm font-medium transition-colors',
            mutationDisabledReason
              ? 'cursor-not-allowed border-gray-200 text-gray-400 dark:border-gray-700 dark:text-gray-500'
              : 'border-primary-200 bg-primary-50 text-primary-700 hover:bg-primary-100 dark:border-primary-800 dark:bg-primary-900/20 dark:text-primary-300',
          )}
        >
          <BranchGlyph className="h-4 w-4" />
          {capsuleLabel}
        </button>
        <MoveToWorktreeDialog
          open={moveDialog.open}
          onOpenChange={(open) => setMoveDialog((current) => ({ ...current, open }))}
          session={session}
          workspaceRootPath={activeSessionPaths.workspaceRootPath}
          mode={moveDialog.mode}
          onCompleted={onCompleted}
        />
      </>
    );
  }

  return (
    <>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            className={clsx(
              'inline-flex h-8 items-center gap-2 rounded-full border px-3 text-sm font-medium transition-colors',
              activeSessionPaths.runtimeKind === 'managed_worktree'
                ? 'border-amber-200 bg-amber-50 text-amber-700 hover:bg-amber-100 dark:border-amber-800 dark:bg-amber-900/20 dark:text-amber-300'
                : 'border-slate-200 bg-slate-50 text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300',
            )}
            title={activeSessionPaths.runtimePath ?? undefined}
          >
            <BranchGlyph className="h-4 w-4" />
            <span className="max-w-[240px] truncate">{capsuleLabel}</span>
            <ChevronDownIcon className="h-4 w-4" />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="end"
            sideOffset={6}
            className="min-w-[220px] rounded-lg border border-gray-200 bg-white p-1 shadow-lg dark:border-gray-700 dark:bg-gray-800"
          >
            <DropdownMenu.Item
              onSelect={() => {
                void handleOpenRuntimeFolder();
              }}
              className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-gray-700 outline-none hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700"
            >
              <OpenInNewWindowIcon className="h-4 w-4" />
              {t('topBar.openRuntimeFolder', { defaultValue: 'Open Runtime Folder' })}
            </DropdownMenu.Item>

            {activeSessionPaths.runtimeKind === 'managed_worktree' ? (
              <>
                <DropdownMenu.Item
                  onSelect={() => {
                    void handlePreparePr();
                  }}
                  className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-gray-700 outline-none hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700"
                >
                  <ArrowTopRightIcon className="h-4 w-4" />
                  {t('topBar.preparePr', { defaultValue: 'Prepare PR' })}
                </DropdownMenu.Item>
                <DropdownMenu.Item
                  onSelect={() => {
                    void handleCreatePr();
                  }}
                  className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-gray-700 outline-none hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700"
                >
                  <PullRequestGlyph className="h-4 w-4" />
                  {t('topBar.createPr', { defaultValue: 'Create PR' })}
                </DropdownMenu.Item>
              </>
            ) : null}

            <DropdownMenu.Item
              disabled={!!mutationDisabledReason}
              title={mutationDisabledReason ?? undefined}
              onSelect={() => {
                setSwitchDialogOpen(true);
              }}
              className={clsx(
                'flex items-center gap-2 rounded-md px-3 py-2 text-sm outline-none',
                mutationDisabledReason
                  ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                  : 'cursor-pointer text-gray-700 hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700',
              )}
            >
              <Link2Icon className="h-4 w-4" />
              {t('topBar.switchWorktree', { defaultValue: 'Switch Worktree' })}
            </DropdownMenu.Item>

            {activeSessionPaths.runtimeKind === 'legacy_worktree' ? (
              <DropdownMenu.Item
                disabled={!!mutationDisabledReason}
                title={mutationDisabledReason ?? undefined}
                onSelect={() => {
                  setMoveDialog({ mode: 'recreate', open: true });
                }}
                className={clsx(
                  'flex items-center gap-2 rounded-md px-3 py-2 text-sm outline-none',
                  mutationDisabledReason
                    ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                    : 'cursor-pointer text-gray-700 hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700',
                )}
              >
                <BranchGlyph className="h-4 w-4" />
                {t('topBar.recreateManaged', { defaultValue: 'Recreate as Managed Worktree' })}
              </DropdownMenu.Item>
            ) : null}

            <DropdownMenu.Item
              disabled={!!mutationDisabledReason}
              title={mutationDisabledReason ?? undefined}
              onSelect={() => {
                void handleDetach();
              }}
              className={clsx(
                'flex items-center gap-2 rounded-md px-3 py-2 text-sm outline-none',
                mutationDisabledReason
                  ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                  : 'cursor-pointer text-gray-700 hover:bg-gray-100 dark:text-gray-200 dark:hover:bg-gray-700',
              )}
            >
              <ArrowTopRightIcon className="h-4 w-4" />
              {t('topBar.detachToMain', { defaultValue: 'Detach to Main Workspace' })}
            </DropdownMenu.Item>

            {activeSessionPaths.runtimeKind === 'managed_worktree' ? (
              <DropdownMenu.Item
                disabled={!!mutationDisabledReason}
                title={mutationDisabledReason ?? undefined}
                onSelect={() => {
                  void handleCleanup();
                }}
                className={clsx(
                  'flex items-center gap-2 rounded-md px-3 py-2 text-sm outline-none',
                  mutationDisabledReason
                    ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                    : 'cursor-pointer text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-900/20',
                )}
              >
                <OpenInNewWindowIcon className="h-4 w-4" />
                {t('topBar.cleanupWorktree', { defaultValue: 'Cleanup Worktree' })}
              </DropdownMenu.Item>
            ) : null}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <MoveToWorktreeDialog
        open={moveDialog.open}
        onOpenChange={(open) => setMoveDialog((current) => ({ ...current, open }))}
        session={session}
        workspaceRootPath={activeSessionPaths.workspaceRootPath}
        mode={moveDialog.mode}
        onCompleted={onCompleted}
      />
      <SwitchWorktreeDialog
        open={switchDialogOpen}
        onOpenChange={setSwitchDialogOpen}
        session={session}
        repoPath={activeSessionPaths.workspaceRootPath}
        currentRuntimePath={activeSessionPaths.runtimePath}
        onCompleted={onCompleted}
      />
      <CreatePullRequestDialog
        open={createPrDialogOpen}
        onOpenChange={setCreatePrDialogOpen}
        prepareResult={prepareResult}
        sessionId={session.sessionId}
        onCompleted={onCompleted}
      />
    </>
  );
}
