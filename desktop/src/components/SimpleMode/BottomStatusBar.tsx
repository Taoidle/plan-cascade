/**
 * BottomStatusBar Component
 *
 * Bottom status bar that displays connection status, project selector,
 * model switcher, permission selector, index status, and token usage.
 * Migrated from the former top header bar.
 */

import { memo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ConnectionStatus } from './ConnectionStatus';
import { ModelSwitcher } from './ModelSwitcher';
import { PermissionSelector } from './PermissionSelector';
import { ProjectSelector, IndexStatus, DocsIndexStatus } from '../shared';
import type { ConnectionStatus as ConnectionStatusType } from '../../lib/claudeCodeClient';
import type { PermissionLevel } from '../../types/permission';
import { formatTokenCount, type PromptTokenEstimateResult } from './tokenBudget';

interface BottomStatusBarProps {
  connectionStatus: ConnectionStatusType;
  workspacePath: string | null;
  permissionLevel: PermissionLevel;
  onPermissionLevelChange: (level: PermissionLevel) => void;
  sessionId: string;
  turnUsage: { input_tokens: number; output_tokens: number } | null;
  sessionUsage: { input_tokens: number; output_tokens: number } | null;
  tokenEstimate: PromptTokenEstimateResult | null;
  isEstimatingTokenBudget: boolean;
}

function formatNumber(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '0';
  return value.toLocaleString();
}

function Divider() {
  return <span className="text-gray-300 dark:text-gray-700">|</span>;
}

export const BottomStatusBar = memo(function BottomStatusBar({
  connectionStatus,
  workspacePath,
  permissionLevel,
  onPermissionLevelChange,
  sessionId,
  turnUsage,
  sessionUsage,
  tokenEstimate,
  isEstimatingTokenBudget,
}: BottomStatusBarProps) {
  const { t } = useTranslation('simpleMode');
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
      <ConnectionStatus status={connectionStatus} />
      <Divider />
      <ProjectSelector compact />
      <Divider />
      <ModelSwitcher dropdownDirection="up" />
      <Divider />
      <PermissionSelector
        level={permissionLevel}
        onLevelChange={onPermissionLevelChange}
        sessionId={sessionId}
        dropdownDirection="up"
      />
      {workspacePath && (
        <>
          <Divider />
          <div className="hidden lg:flex items-center gap-2">
            <IndexStatus compact />
            <DocsIndexStatus compact />
          </div>
        </>
      )}
      {hasUsage && (
        <>
          <Divider />
          <div className="hidden lg:flex items-center gap-2">
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
        <div className="ml-auto hidden lg:flex items-center">
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
    </div>
  );
});
