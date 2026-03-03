/**
 * MCP Server Types
 *
 * TypeScript interfaces for MCP server management.
 */

/** Type of MCP server connection */
export type McpServerType = 'stdio' | 'stream_http';

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
  has_env_secret?: boolean;
  has_headers_secret?: boolean;
  enabled: boolean;
  auto_connect?: boolean;
  status: McpServerStatus;
  last_error?: string | null;
  last_connected_at?: string | null;
  retry_count?: number;
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
  auto_connect?: boolean;
}

/** Request to update an existing MCP server */
export interface UpdateMcpServerRequest {
  name?: string;
  server_type?: McpServerType;
  command?: string;
  clear_command?: boolean;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  clear_url?: boolean;
  headers?: Record<string, string>;
  enabled?: boolean;
  auto_connect?: boolean;
}

/** Result of health check */
export interface HealthCheckResult {
  server_id: string;
  status: McpServerStatus;
  checked_at: string;
  latency_ms?: number | null;
  protocol_version?: string | null;
  tool_count?: number | null;
}

/** Result of importing servers from Claude Desktop */
export interface ImportResult {
  added: number;
  skipped: number;
  failed: number;
  servers: string[];
  errors: string[];
  will_add?: string[];
  will_skip?: string[];
  will_fail?: string[];
}

/** Connected MCP server runtime state */
export interface ConnectedServerInfo {
  server_id: string;
  server_name: string;
  connection_state: string;
  tool_names: string[];
  qualified_tool_names: string[];
  protocol_version: string;
  connected_at?: string | null;
  last_error?: string | null;
  retry_count?: number;
}

/** Auto-connect result returned on app startup/manual run */
export interface McpAutoConnectResult {
  connected: ConnectedServerInfo[];
  failed: string[];
}

/** Tool definition shape from backend */
export interface McpToolDefinition {
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
}

export interface McpExportPayload {
  mcpServers: Record<string, Record<string, unknown>>;
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
