/**
 * MCP Server Types
 *
 * TypeScript interfaces for MCP server management.
 */

/** Type of MCP server connection */
export type McpServerType = 'stdio' | 'sse';

/** Status of an MCP server connection */
export type McpServerStatus = 'connected' | 'disconnected' | 'unknown' | { error: string };

/** MCP Server configuration */
export interface McpServer {
  id: string;
  name: string;
  server_type: McpServerType;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  url: string | null;
  headers: Record<string, string>;
  enabled: boolean;
  status: McpServerStatus;
  last_checked: string | null;
  created_at: string | null;
  updated_at: string | null;
}

/** Request to create a new MCP server */
export interface CreateMcpServerRequest {
  name: string;
  server_type: McpServerType;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
}

/** Request to update an existing MCP server */
export interface UpdateMcpServerRequest {
  name?: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}

/** Result of health check */
export interface HealthCheckResult {
  server_id: string;
  status: McpServerStatus;
  checked_at: string;
}

/** Result of importing servers from Claude Desktop */
export interface ImportResult {
  added: number;
  skipped: number;
  failed: number;
  servers: string[];
  errors: string[];
}

/** Generic command response from Tauri */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Helper to check if status is an error */
export function isStatusError(status: McpServerStatus): status is { error: string } {
  return typeof status === 'object' && 'error' in status;
}

/** Get status display string */
export function getStatusDisplay(status: McpServerStatus): string {
  if (status === 'connected') return 'Connected';
  if (status === 'disconnected') return 'Disconnected';
  if (status === 'unknown') return 'Unknown';
  if (isStatusError(status)) return `Error: ${status.error}`;
  return 'Unknown';
}

/** Get status color class */
export function getStatusColor(status: McpServerStatus): string {
  if (status === 'connected') return 'text-green-500';
  if (status === 'disconnected') return 'text-gray-500';
  if (status === 'unknown') return 'text-yellow-500';
  if (isStatusError(status)) return 'text-red-500';
  return 'text-gray-500';
}
