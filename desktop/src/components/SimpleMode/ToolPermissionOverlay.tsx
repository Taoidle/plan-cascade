/**
 * ToolPermissionOverlay
 *
 * Inline approval UI displayed when a tool execution requires user permission.
 * Replaces the chat input area while a permission request is pending.
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { ToolPermissionRequest, PermissionResponseType, ToolRiskLevel } from '../../types/permission';

interface ToolPermissionOverlayProps {
  request: ToolPermissionRequest;
  onRespond: (requestId: string, response: PermissionResponseType) => void;
  loading: boolean;
  queueSize: number;
}

const RISK_STYLE: Record<ToolRiskLevel, { color: string; bg: string }> = {
  ReadOnly: {
    color: 'text-green-700 dark:text-green-300',
    bg: 'bg-green-100 dark:bg-green-900/30',
  },
  SafeWrite: {
    color: 'text-amber-700 dark:text-amber-300',
    bg: 'bg-amber-100 dark:bg-amber-900/30',
  },
  Dangerous: {
    color: 'text-red-700 dark:text-red-300',
    bg: 'bg-red-100 dark:bg-red-900/30',
  },
};

const RISK_I18N_KEY: Record<ToolRiskLevel, string> = {
  ReadOnly: 'permission.risk.readOnly',
  SafeWrite: 'permission.risk.safeWrite',
  Dangerous: 'permission.risk.dangerous',
};

function truncateArgs(args: string, maxLines = 4): { text: string; truncated: boolean } {
  try {
    const parsed = JSON.parse(args);
    const pretty = JSON.stringify(parsed, null, 2);
    const lines = pretty.split('\n');
    if (lines.length > maxLines) {
      return { text: lines.slice(0, maxLines).join('\n') + '\n...', truncated: true };
    }
    return { text: pretty, truncated: false };
  } catch {
    const lines = args.split('\n');
    if (lines.length > maxLines) {
      return { text: lines.slice(0, maxLines).join('\n') + '\n...', truncated: true };
    }
    return { text: args, truncated: false };
  }
}

export function ToolPermissionOverlay({
  request,
  onRespond,
  loading,
  queueSize,
}: ToolPermissionOverlayProps) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);
  const riskStyle = RISK_STYLE[request.risk] ?? RISK_STYLE.Dangerous;
  const riskKey = RISK_I18N_KEY[request.risk] ?? RISK_I18N_KEY.Dangerous;
  const argsPreview = truncateArgs(request.arguments);

  const handleAllow = useCallback(() => {
    onRespond(request.requestId, 'allow');
  }, [request.requestId, onRespond]);

  const handleDeny = useCallback(() => {
    onRespond(request.requestId, 'deny');
  }, [request.requestId, onRespond]);

  const handleAlwaysAllow = useCallback(() => {
    onRespond(request.requestId, 'allow_always');
  }, [request.requestId, onRespond]);

  return (
    <div className="px-4 py-3">
      {/* Header with tool name and risk badge */}
      <div className="flex items-center gap-2 mb-2">
        <span className="text-xs font-medium text-gray-600 dark:text-gray-300">
          {t('permission.toolRequiresApproval')}
        </span>
        <span className="text-xs font-semibold text-gray-900 dark:text-gray-100">
          {request.toolName}
        </span>
        <span
          className={clsx(
            'px-1.5 py-0.5 rounded text-[10px] font-medium',
            riskStyle.bg,
            riskStyle.color
          )}
        >
          {t(riskKey)}
        </span>
        {queueSize > 0 && (
          <span className="text-[10px] text-gray-400 dark:text-gray-500 ml-auto">
            {t('permission.pending', { count: queueSize })}
          </span>
        )}
      </div>

      {/* Arguments preview */}
      {request.arguments && request.arguments !== '{}' && (
        <div className="mb-2">
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-[10px] text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 mb-1"
          >
            {expanded ? t('permission.hideArguments') : t('permission.showArguments')}
          </button>
          {expanded && (
            <pre className="text-[11px] leading-tight bg-gray-50 dark:bg-gray-800 rounded p-2 overflow-x-auto max-h-24 overflow-y-auto text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700">
              {argsPreview.text}
            </pre>
          )}
        </div>
      )}

      {/* Action buttons */}
      <div className="flex items-center gap-2">
        <button
          onClick={handleAllow}
          disabled={loading}
          className={clsx(
            'px-4 py-1.5 rounded-lg text-xs font-medium transition-colors',
            'bg-green-600 text-white hover:bg-green-700',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          {t('permission.allow')}
        </button>
        <button
          onClick={handleDeny}
          disabled={loading}
          className={clsx(
            'px-4 py-1.5 rounded-lg text-xs font-medium transition-colors',
            'bg-red-600 text-white hover:bg-red-700',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          {t('permission.deny')}
        </button>
        <button
          onClick={handleAlwaysAllow}
          disabled={loading}
          className="px-3 py-1.5 text-[11px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors disabled:opacity-50"
        >
          {t('permission.alwaysAllow')}
        </button>
      </div>
    </div>
  );
}
