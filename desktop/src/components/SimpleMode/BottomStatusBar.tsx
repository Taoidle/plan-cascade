/**
 * BottomStatusBar Component
 *
 * Bottom status bar that displays connection status, project selector,
 * model switcher, permission selector, index status, and token usage.
 * Migrated from the former top header bar.
 */

import { clsx } from 'clsx';
import { ConnectionStatus } from './ConnectionStatus';
import { ModelSwitcher } from './ModelSwitcher';
import { PermissionSelector } from './PermissionSelector';
import { ProjectSelector, IndexStatus, DocsIndexStatus } from '../shared';
import type { ConnectionStatus as ConnectionStatusType } from '../../lib/claudeCodeClient';
import type { PermissionLevel } from '../../types/permission';

interface BottomStatusBarProps {
  connectionStatus: ConnectionStatusType;
  workspacePath: string | null;
  permissionLevel: PermissionLevel;
  onPermissionLevelChange: (level: PermissionLevel) => void;
  sessionId: string;
  turnUsage: { input_tokens: number; output_tokens: number } | null;
  sessionUsage: { input_tokens: number; output_tokens: number } | null;
}

function formatNumber(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '0';
  return value.toLocaleString();
}

function Divider() {
  return <span className="text-gray-300 dark:text-gray-700">|</span>;
}

export function BottomStatusBar({
  connectionStatus,
  workspacePath,
  permissionLevel,
  onPermissionLevelChange,
  sessionId,
  turnUsage,
  sessionUsage,
}: BottomStatusBarProps) {
  const turn = turnUsage || { input_tokens: 0, output_tokens: 0 };
  const total = sessionUsage || turn;
  const hasUsage = turnUsage || sessionUsage;

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
    </div>
  );
}
