/**
 * MCP Server Types
 *
 * TypeScript interfaces for MCP server management.
 */

/** Type of MCP server connection */
export type McpServerType = 'stdio' | 'stream_http';
export type McpCatalogTrustLevel = 'official' | 'verified' | 'community';
export type McpRuntimeKind = 'node' | 'uv' | 'python' | 'docker';
export type McpInstallStrategyKind =
  | 'uv_tool'
  | 'python_venv'
  | 'node_managed_pkg'
  | 'docker'
  | 'stream_http_api_key'
  | 'stream_http_api_key_optional'
  | 'oauth_bridge_mcp_remote'
  | 'go_binary';

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
  managed_install?: boolean;
  catalog_item_id?: string | null;
  trust_level?: McpCatalogTrustLevel | null;
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
  version?: string;
  exported_at?: string;
  secrets_redacted?: boolean;
  mcpServers: Record<string, Record<string, unknown>>;
}

export type McpExportSecretMode = 'redacted' | 'include';

export interface McpExportOptions {
  secret_mode?: McpExportSecretMode;
  format_version?: '1' | '2';
}

export type McpImportConflictPolicy = 'skip';

export interface RuntimeRequirement {
  runtime: McpRuntimeKind;
  min_version?: string | null;
  optional?: boolean;
}

export interface McpInstallStrategy {
  id: string;
  kind: McpInstallStrategyKind;
  priority: number;
  requirements: RuntimeRequirement[];
  recipe: Record<string, unknown>;
}

export interface McpSecretSchemaField {
  key: string;
  label: string;
  required?: boolean;
  secret_type?: string | null;
}

export interface McpCatalogItem {
  id: string;
  name: string;
  vendor: string;
  trust_level: McpCatalogTrustLevel;
  tags: string[];
  docs_url?: string | null;
  maintained_by?: string | null;
  os_support: string[];
  strategies: McpInstallStrategy[];
  secrets_schema: McpSecretSchemaField[];
}

export interface McpCatalogFilter {
  trust_levels?: McpCatalogTrustLevel[];
  tags?: string[];
  query?: string;
}

export interface McpCatalogListResponse {
  items: McpCatalogItem[];
  source: string;
  fetched_at?: string | null;
  signature_valid: boolean;
}

export interface McpCatalogRefreshResult {
  source: string;
  fetched_at: string;
  item_count: number;
  updated: boolean;
  signature_valid: boolean;
  error?: string | null;
}

export interface McpInstallPreview {
  item_id: string;
  selected_strategy: string;
  missing_runtimes: McpRuntimeKind[];
  install_commands: string[];
  required_secrets: McpSecretSchemaField[];
  risk_flags: McpInstallRiskFlag[];
}

export interface McpInstallRequest {
  item_id: string;
  server_alias: string;
  selected_strategy?: string;
  secrets?: Record<string, string>;
  oauth_mode?: string;
  auto_connect?: boolean;
}

export type McpInstallRiskFlag =
  | 'review_commands'
  | 'community_caution'
  | 'community_item_confirmation_required'
  | 'unpinned_artifact'
  | string;

export type McpInstallStatus = 'running' | 'success' | 'failed';
export type McpInstallPhase =
  | 'PRECHECK'
  | 'ELEVATE'
  | 'INSTALL_RUNTIME'
  | 'INSTALL_PACKAGE'
  | 'WRITE_CONFIG'
  | 'VERIFY_PROTOCOL'
  | 'AUTO_CONNECT'
  | 'COMMIT'
  | 'ROLLBACK';

export interface McpInstallResult {
  job_id: string;
  server_id?: string | null;
  phase: McpInstallPhase;
  status: McpInstallStatus;
  diagnostics?: string | null;
}

export interface McpRuntimeInfo {
  runtime: McpRuntimeKind;
  version?: string | null;
  path?: string | null;
  source?: string | null;
  managed?: boolean;
  healthy?: boolean;
  last_error?: string | null;
  last_checked?: string | null;
}

export interface McpRuntimeRepairResult {
  runtime: McpRuntimeKind;
  status: string;
  message: string;
}

export interface McpInstallRecord {
  server_id: string;
  catalog_item_id: string;
  catalog_version?: string | null;
  strategy_id: string;
  trust_level: McpCatalogTrustLevel;
  package_lock_json?: Record<string, unknown> | null;
  runtime_snapshot_json?: Record<string, unknown> | null;
  installed_at?: string | null;
  updated_at?: string | null;
}

export interface McpInstallProgressEvent {
  job_id: string;
  phase: McpInstallPhase;
  progress: number;
  status: string;
  message: string;
  server_id?: string | null;
}

export interface McpInstallLogEvent {
  job_id: string;
  phase: McpInstallPhase;
  level: 'info' | 'warn' | 'error' | string;
  message: string;
}

export interface McpOauthEvent {
  job_id: string;
  state: string;
  message?: string | null;
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
