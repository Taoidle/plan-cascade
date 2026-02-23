/**
 * Tool Permission Types
 *
 * Type definitions for the tool execution permission system.
 */

/** Session-level permission mode */
export type PermissionLevel = 'strict' | 'standard' | 'permissive';

/** Risk classification for a tool invocation */
export type ToolRiskLevel = 'ReadOnly' | 'SafeWrite' | 'Dangerous';

/** Tool permission request from backend */
export interface ToolPermissionRequest {
  requestId: string;
  sessionId: string;
  toolName: string;
  arguments: string;
  risk: ToolRiskLevel;
}

/** Response type for permission decisions */
export type PermissionResponseType = 'allow' | 'deny' | 'allow_always';
