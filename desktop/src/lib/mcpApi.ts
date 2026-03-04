/**
 * MCP API (IPC Wrappers)
 *
 * Type-safe wrappers for MCP related Tauri commands.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  CommandResponse,
  ConnectedMcpToolDetail,
  ConnectedServerInfo,
  HealthCheckResult,
  ImportResult,
  McpCatalogListResponse,
  McpCatalogRefreshResult,
  McpExportPayload,
  McpExportSecretMode,
  McpImportConflictPolicy,
  McpInstallPreview,
  McpInstallRequest,
  McpInstallResult,
  McpRuntimeInfo,
  McpRuntimeRepairResult,
  McpServer,
} from '../types/mcp';

async function invokeMcp<T>(command: string, payload?: Record<string, unknown>): Promise<CommandResponse<T>> {
  try {
    return await invoke<CommandResponse<T>>(command, payload);
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export const mcpApi = {
  listServers(): Promise<CommandResponse<McpServer[]>> {
    return invokeMcp('list_mcp_servers');
  },

  addServer(payload: {
    name: string;
    serverType: string;
    command: string | null;
    args: string[] | null;
    env: Record<string, string> | null;
    url: string | null;
    headers: Record<string, string> | null;
    autoConnect: boolean;
  }): Promise<CommandResponse<McpServer>> {
    return invokeMcp('add_mcp_server', payload);
  },

  updateServer(payload: {
    id: string;
    name: string;
    serverType: string;
    command: string | null;
    clearCommand: boolean;
    args: string[];
    env: Record<string, string>;
    url: string | null;
    clearUrl: boolean;
    headers: Record<string, string>;
    autoConnect: boolean;
  }): Promise<CommandResponse<McpServer>> {
    return invokeMcp('update_mcp_server', payload);
  },

  toggleServer(id: string, enabled: boolean): Promise<CommandResponse<McpServer>> {
    return invokeMcp('toggle_mcp_server', { id, enabled });
  },

  removeServer(id: string): Promise<CommandResponse<void>> {
    return invokeMcp('remove_mcp_server', { id });
  },

  testServer(id: string): Promise<CommandResponse<HealthCheckResult>> {
    return invokeMcp('test_mcp_server', { id });
  },

  connectServer(id: string): Promise<CommandResponse<ConnectedServerInfo>> {
    return invokeMcp('connect_mcp_server', { id });
  },

  disconnectServer(id: string): Promise<CommandResponse<void>> {
    return invokeMcp('disconnect_mcp_server', { id });
  },

  listConnectedServers(): Promise<CommandResponse<ConnectedServerInfo[]>> {
    return invokeMcp('list_connected_mcp_servers');
  },

  getConnectedServerTools(serverId: string): Promise<CommandResponse<ConnectedMcpToolDetail[]>> {
    return invokeMcp('get_connected_mcp_server_tools', { serverId });
  },

  getServerDetail(id: string, includeSecrets = true): Promise<CommandResponse<McpServer>> {
    return invokeMcp('get_mcp_server_detail', { id, includeSecrets });
  },

  exportServers(secretMode: McpExportSecretMode = 'redacted'): Promise<CommandResponse<McpExportPayload>> {
    return invokeMcp('export_mcp_servers', {
      options: {
        secret_mode: secretMode,
        format_version: '2',
      },
    });
  },

  importFromClaudeDesktop(
    dryRun: boolean,
    conflictPolicy: McpImportConflictPolicy,
  ): Promise<CommandResponse<ImportResult>> {
    return invokeMcp('import_from_claude_desktop', {
      dryRun,
      conflictPolicy,
    });
  },

  importFromFile(payload: {
    path: string;
    dryRun: boolean;
    conflictPolicy: McpImportConflictPolicy;
  }): Promise<CommandResponse<ImportResult>> {
    return invokeMcp('import_mcp_from_file', payload);
  },

  listCatalog(): Promise<CommandResponse<McpCatalogListResponse>> {
    return invokeMcp('list_mcp_catalog');
  },

  refreshCatalog(force = true): Promise<CommandResponse<McpCatalogRefreshResult>> {
    return invokeMcp('refresh_mcp_catalog', { force });
  },

  previewCatalogInstall(itemId: string, preferredStrategy?: string): Promise<CommandResponse<McpInstallPreview>> {
    return invokeMcp('preview_install_mcp_catalog_item', {
      itemId,
      preferredStrategy,
    });
  },

  installCatalogItem(request: McpInstallRequest): Promise<CommandResponse<McpInstallResult>> {
    return invokeMcp('install_mcp_catalog_item', { request });
  },

  retryInstall(jobId: string): Promise<CommandResponse<McpInstallResult>> {
    return invokeMcp('retry_mcp_install', { jobId });
  },

  listRuntimeInventory(): Promise<CommandResponse<McpRuntimeInfo[]>> {
    return invokeMcp('list_mcp_runtime_inventory');
  },

  repairRuntime(runtimeKind: string): Promise<CommandResponse<McpRuntimeRepairResult>> {
    return invokeMcp('repair_mcp_runtime', { runtimeKind });
  },
};
