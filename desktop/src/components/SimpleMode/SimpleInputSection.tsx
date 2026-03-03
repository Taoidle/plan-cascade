import type { ReactNode } from 'react';
import { ToolPermissionOverlay } from './ToolPermissionOverlay';
import type { PermissionResponseType, ToolPermissionRequest } from '../../types/permission';

interface SimpleInputSectionProps {
  permissionRequest: ToolPermissionRequest | null;
  isPermissionResponding: boolean;
  permissionQueueSize: number;
  onRespondPermission: (requestId: string, action: PermissionResponseType) => void | Promise<void>;
  children: ReactNode;
  apiError: string | null;
}

export function SimpleInputSection({
  permissionRequest,
  isPermissionResponding,
  permissionQueueSize,
  onRespondPermission,
  children,
  apiError,
}: SimpleInputSectionProps) {
  return (
    <div className="shrink-0 border-t border-gray-200 dark:border-gray-700">
      {permissionRequest ? (
        <ToolPermissionOverlay
          request={permissionRequest}
          onRespond={onRespondPermission}
          loading={isPermissionResponding}
          queueSize={permissionQueueSize}
        />
      ) : (
        children
      )}
      {apiError && (
        <div className="mx-4 mb-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
          <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
        </div>
      )}
    </div>
  );
}

export default SimpleInputSection;
