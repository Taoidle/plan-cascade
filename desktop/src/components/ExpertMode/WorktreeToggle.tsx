/**
 * Worktree Toggle Component
 *
 * Toggle for enabling Git worktree mode with branch name input
 * and validation.
 */

import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { usePRDStore } from '../../store/prd';
import * as Switch from '@radix-ui/react-switch';
import {
  GitBranchIcon,
  InfoCircledIcon,
  ExclamationTriangleIcon,
} from '@radix-ui/react-icons';
import * as Tooltip from '@radix-ui/react-tooltip';

// Branch name validation regex
const BRANCH_NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9-_/]*$/;

export function WorktreeToggle() {
  const { prd, setWorktreeConfig } = usePRDStore();
  const { worktree } = prd;
  const [branchError, setBranchError] = useState<string | null>(null);

  // Validate branch name
  useEffect(() => {
    if (worktree.enabled && worktree.branchName) {
      if (!BRANCH_NAME_REGEX.test(worktree.branchName)) {
        setBranchError('Branch name must start with a letter and contain only letters, numbers, hyphens, underscores, or slashes');
      } else if (worktree.branchName.length > 100) {
        setBranchError('Branch name must be less than 100 characters');
      } else {
        setBranchError(null);
      }
    } else {
      setBranchError(null);
    }
  }, [worktree.branchName, worktree.enabled]);

  // Generate suggested branch name from PRD title
  const suggestBranchName = () => {
    if (prd.title) {
      const suggested = prd.title
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '')
        .substring(0, 50);
      setWorktreeConfig({ branchName: `feature/${suggested}` });
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
            Git Worktree
          </label>
          <Tooltip.Provider>
            <Tooltip.Root>
              <Tooltip.Trigger asChild>
                <button className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                  <InfoCircledIcon className="w-4 h-4" />
                </button>
              </Tooltip.Trigger>
              <Tooltip.Portal>
                <Tooltip.Content
                  className={clsx(
                    'max-w-xs px-3 py-2 rounded-lg text-sm',
                    'bg-gray-900 dark:bg-gray-700 text-white',
                    'shadow-lg'
                  )}
                  sideOffset={5}
                >
                  Git worktrees allow isolated development without switching branches.
                  Each feature gets its own working directory, preventing conflicts
                  and enabling parallel development.
                  <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-700" />
                </Tooltip.Content>
              </Tooltip.Portal>
            </Tooltip.Root>
          </Tooltip.Provider>
        </div>

        <Switch.Root
          checked={worktree.enabled}
          onCheckedChange={(checked) => setWorktreeConfig({ enabled: checked })}
          className={clsx(
            'w-11 h-6 rounded-full relative transition-colors',
            worktree.enabled
              ? 'bg-primary-600'
              : 'bg-gray-200 dark:bg-gray-700'
          )}
        >
          <Switch.Thumb
            className={clsx(
              'block w-5 h-5 rounded-full bg-white shadow-sm transition-transform',
              worktree.enabled ? 'translate-x-[22px]' : 'translate-x-0.5'
            )}
          />
        </Switch.Root>
      </div>

      {/* Worktree configuration */}
      {worktree.enabled && (
        <div
          className={clsx(
            'space-y-4 p-4 rounded-lg',
            'bg-white dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'animate-in fade-in slide-in-from-top-1'
          )}
        >
          {/* Branch name input */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="block text-xs font-medium text-gray-600 dark:text-gray-400">
                Branch Name
              </label>
              {prd.title && (
                <button
                  onClick={suggestBranchName}
                  className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                >
                  Suggest from title
                </button>
              )}
            </div>
            <div className="relative">
              <GitBranchIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
              <input
                type="text"
                value={worktree.branchName}
                onChange={(e) => setWorktreeConfig({ branchName: e.target.value })}
                placeholder="feature/my-feature"
                className={clsx(
                  'w-full pl-10 pr-3 py-2 rounded-lg text-sm',
                  'bg-gray-50 dark:bg-gray-900',
                  'border transition-colors',
                  branchError
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-gray-300 dark:border-gray-600 focus:ring-primary-500',
                  'focus:outline-none focus:ring-2'
                )}
              />
            </div>
            {branchError && (
              <p className="mt-1 flex items-center gap-1 text-xs text-red-500">
                <ExclamationTriangleIcon className="w-3 h-3" />
                {branchError}
              </p>
            )}
          </div>

          {/* Base branch input */}
          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
              Base Branch
            </label>
            <select
              value={worktree.baseBranch}
              onChange={(e) => setWorktreeConfig({ baseBranch: e.target.value })}
              className={clsx(
                'w-full px-3 py-2 rounded-lg text-sm',
                'bg-gray-50 dark:bg-gray-900',
                'border border-gray-300 dark:border-gray-600',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
            >
              <option value="main">main</option>
              <option value="master">master</option>
              <option value="develop">develop</option>
            </select>
          </div>

          {/* Worktree path display */}
          {worktree.branchName && !branchError && (
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
              <p className="text-xs text-gray-500 dark:text-gray-400 mb-1">
                Worktree will be created at:
              </p>
              <code className="text-xs font-mono text-gray-700 dark:text-gray-300">
                .worktree/{worktree.branchName.replace(/\//g, '-')}
              </code>
            </div>
          )}
        </div>
      )}

      {/* Info banner when disabled */}
      {!worktree.enabled && (
        <p className="text-xs text-gray-500 dark:text-gray-400">
          Enable to develop in an isolated Git worktree branch
        </p>
      )}
    </div>
  );
}

export default WorktreeToggle;
