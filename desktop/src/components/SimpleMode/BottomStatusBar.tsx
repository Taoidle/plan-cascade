/**
 * BottomStatusBar Component
 *
 * Bottom status bar that displays project selector, model switcher,
 * permission selector, index status, token usage, and effective context.
 * Migrated from the former top header bar.
 */

import { memo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ModelSwitcher } from './ModelSwitcher';
import { PermissionSelector } from './PermissionSelector';
import { ProjectSelector, IndexStatus, DocsIndexStatus } from '../shared';
import { EffectiveContextSummary } from '../shared/EffectiveContextSummary';
import type { PermissionLevel } from '../../types/permission';
import { formatTokenCount, type PromptTokenEstimateResult } from './tokenBudget';

interface BottomStatusBarProps {
  workspacePath: string | null;
  workspaceRootPath?: string | null;
  runtimeKind?: 'main' | 'managed_worktree' | 'legacy_worktree';
  runtimeBranch?: string | null;
  permissionLevel: PermissionLevel;
  onPermissionLevelChange: (level: PermissionLevel) => void;
  sessionId: string;
  turnUsage: { input_tokens: number; output_tokens: number } | null;
  sessionUsage: { input_tokens: number; output_tokens: number } | null;
  tokenEstimate: PromptTokenEstimateResult | null;
  isEstimatingTokenBudget: boolean;
  memoryStatus: {
    label: string;
    title: string;
    tone: 'neutral' | 'info' | 'success' | 'warning' | 'danger';
  } | null;
  onMemoryStatusClick: (() => void) | null;
}

function formatNumber(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '0';
  return value.toLocaleString();
}

function Divider() {
  return <span className="text-gray-300 dark:text-gray-700">|</span>;
}

export const BottomStatusBar = memo(function BottomStatusBar({
  workspacePath,
  workspaceRootPath,
  runtimeKind = 'main',
  runtimeBranch,
  permissionLevel,
  onPermissionLevelChange,
  sessionId,
  turnUsage,
  sessionUsage,
  tokenEstimate,
  isEstimatingTokenBudget,
  memoryStatus,
  onMemoryStatusClick,
}: BottomStatusBarProps) {
  const { t } = useTranslation('simpleMode');
  const projectWorkspacePath = workspaceRootPath ?? workspacePath;
  const turn = turnUsage || { input_tokens: 0, output_tokens: 0 };
  const total = sessionUsage || turn;
  const hasUsage = turnUsage || sessionUsage;
  const showTokenBudget = isEstimatingTokenBudget || !!tokenEstimate;

  return (
    <div
      className={clsx(
        'shrink-0 flex items-center gap-3 px-4 py-1.5',
        'border-t border-gray-200 dark:border-gray-700',
        'bg-white dark:bg-gray-900',
        'text-xs text-gray-600 dark:text-gray-400',
      )}
    >
      <ProjectSelector compact workspacePathOverride={projectWorkspacePath} />
      {runtimeKind !== 'main' && (
        <>
          <Divider />
          <div
            className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300 text-2xs"
            title={
              workspaceRootPath && workspacePath && workspaceRootPath !== workspacePath
                ? `${workspaceRootPath} -> ${workspacePath}`
                : workspacePath || undefined
            }
          >
            <span>{runtimeKind === 'managed_worktree' ? 'worktree' : 'legacy'}</span>
            {runtimeBranch ? <span className="font-mono">{runtimeBranch}</span> : null}
          </div>
        </>
      )}
      <Divider />
      <ModelSwitcher dropdownDirection="up" />
      <Divider />
      <PermissionSelector
        level={permissionLevel}
        onLevelChange={onPermissionLevelChange}
        sessionId={sessionId}
        dropdownDirection="up"
      />
      {projectWorkspacePath && (
        <>
          <Divider />
          <div className="flex items-center gap-2 min-w-0">
            <IndexStatus compact workspacePathOverride={projectWorkspacePath} />
            <DocsIndexStatus compact workspacePathOverride={projectWorkspacePath} />
          </div>
        </>
      )}
      {memoryStatus && (
        <>
          <Divider />
          <button
            type="button"
            onClick={onMemoryStatusClick ?? undefined}
            title={memoryStatus.title}
            className={clsx(
              'inline-flex items-center px-1.5 py-0.5 rounded text-2xs transition-colors',
              memoryStatus.tone === 'warning' && 'bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300',
              memoryStatus.tone === 'success' &&
                'bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-300',
              memoryStatus.tone === 'danger' && 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300',
              memoryStatus.tone === 'info' && 'bg-sky-50 dark:bg-sky-900/20 text-sky-700 dark:text-sky-300',
              memoryStatus.tone === 'neutral' && 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300',
              onMemoryStatusClick && 'hover:brightness-95',
            )}
          >
            {memoryStatus.label}
          </button>
        </>
      )}
      {hasUsage && (
        <>
          <Divider />
          <div className="flex items-center gap-2">
            <span className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-2xs">
              {formatNumber(turn.input_tokens)}&uarr; {formatNumber(turn.output_tokens)}&darr;
            </span>
            <span className="px-1.5 py-0.5 rounded bg-sky-50 dark:bg-sky-900/20 text-sky-700 dark:text-sky-300 text-2xs">
              &Sigma; {formatNumber(total.input_tokens)}&uarr; {formatNumber(total.output_tokens)}&darr;
            </span>
          </div>
        </>
      )}
      {showTokenBudget && (
        <div className="flex items-center">
          {isEstimatingTokenBudget ? (
            <span className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-2xs">
              {t('workflow.tokenBudget.estimating', { defaultValue: 'Estimating token budget...' })}
            </span>
          ) : tokenEstimate ? (
            <div className="group flex items-center gap-1">
              <span
                title={`${tokenEstimate.estimated_tokens} / ${tokenEstimate.budget_tokens}`}
                className={clsx(
                  'px-1.5 py-0.5 rounded text-2xs font-mono',
                  tokenEstimate.exceeds_budget
                    ? 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
                    : 'bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-300',
                )}
              >
                {formatTokenCount(tokenEstimate.estimated_tokens)} / {formatTokenCount(tokenEstimate.budget_tokens)}
              </span>
              <div
                className={clsx(
                  'flex items-center gap-1 overflow-hidden transition-all duration-150',
                  'max-w-0 opacity-0 translate-x-1',
                  'group-hover:max-w-[280px] group-hover:opacity-100 group-hover:translate-x-0',
                )}
              >
                <span
                  title={`${t('workflow.tokenBudget.attachments', { defaultValue: 'Attachments' })}: ${tokenEstimate.attachment_tokens}`}
                  className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-2xs font-mono whitespace-nowrap"
                >
                  {t('workflow.tokenBudget.attachments', { defaultValue: 'Attachments' })}:&nbsp;
                  {formatTokenCount(tokenEstimate.attachment_tokens)}
                </span>
                <span
                  title={`${t('workflow.tokenBudget.remaining', { defaultValue: 'Remaining' })}: ${tokenEstimate.remaining_tokens}`}
                  className={clsx(
                    'px-1.5 py-0.5 rounded text-2xs font-mono whitespace-nowrap',
                    tokenEstimate.remaining_tokens < 0
                      ? 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300',
                  )}
                >
                  {t('workflow.tokenBudget.remaining', { defaultValue: 'Remaining' })}:&nbsp;
                  {formatTokenCount(tokenEstimate.remaining_tokens)}
                </span>
              </div>
            </div>
          ) : null}
        </div>
      )}
      <EffectiveContextSummary className="ml-auto" />
    </div>
  );
});
