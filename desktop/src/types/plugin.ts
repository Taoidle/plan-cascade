/**
 * Plugin Types
 *
 * TypeScript types mirroring the Rust backend plugin models.
 * Used by the Zustand store and UI components.
 */

// ============================================================================
// Plugin Source
// ============================================================================

/** Where a plugin was discovered from */
export type PluginSource = 'claude_code' | 'installed' | 'project_local';

/** Human-readable label for a plugin source */
export function getPluginSourceLabel(source: PluginSource): string {
  switch (source) {
    case 'claude_code':
      return 'Project';
    case 'installed':
      return 'Installed';
    case 'project_local':
      return 'Plan Cascade';
    default:
      return 'Unknown';
  }
}

// ============================================================================
// Hook Types
// ============================================================================

/** Claude Code hook event types */
export type HookEvent =
  | 'SessionStart'
  | 'UserPromptSubmit'
  | 'PreCompact'
  | 'PostCompact'
  | 'PreToolUse'
  | 'PostToolUse'
  | 'Stop'
  | 'SubAgentSpawn'
  | 'SubAgentComplete'
  | 'PreLlmCall'
  | 'PostLlmCall'
  | 'Notification'
  | 'Error'
  | 'SessionEnd';

/** Hook execution type */
export type HookType = 'command' | 'prompt';

/** A plugin hook definition */
export interface PluginHook {
  event: HookEvent;
  matcher: string | null;
  hook_type: HookType;
  command: string;
  timeout: number;
  async_hook: boolean;
}

// ============================================================================
// Plugin Components
// ============================================================================

/** A skill from a plugin */
export interface PluginSkill {
  name: string;
  description: string;
  user_invocable: boolean;
  allowed_tools: string[];
  body: string;
  hooks: PluginHook[];
}

/** A command from a plugin */
export interface PluginCommand {
  name: string;
  description: string;
  body: string;
}

/** Plugin permission configuration */
export interface PluginPermissions {
  allow: string[];
  deny: string[];
  always_approve: string[];
}

/** Plugin manifest (from plugin.json) */
export interface PluginManifest {
  name: string;
  version: string;
  description: string;
  author: string | null;
  repository: string | null;
  license: string | null;
  keywords: string[];
}

// ============================================================================
// Loaded Plugin
// ============================================================================

/** A fully loaded plugin */
export interface LoadedPlugin {
  manifest: PluginManifest;
  source: PluginSource;
  enabled: boolean;
  root_path: string;
  skills: PluginSkill[];
  commands: PluginCommand[];
  hooks: PluginHook[];
  instructions: string | null;
  permissions: PluginPermissions;
}

// ============================================================================
// Response Types
// ============================================================================

/** Lightweight plugin info for listings */
export interface PluginInfo {
  name: string;
  version: string;
  description: string;
  source: PluginSource;
  enabled: boolean;
  skill_count: number;
  command_count: number;
  hook_count: number;
  has_instructions: boolean;
  author: string | null;
}

/** Full plugin detail */
export interface PluginDetail {
  plugin: LoadedPlugin;
  root_path: string;
}

// ============================================================================
// Registry & Marketplace Types
// ============================================================================

/** A plugin entry in the remote registry */
export interface RegistryEntry {
  name: string;
  version: string;
  description: string;
  author: string | null;
  repository: string | null;
  license: string | null;
  keywords: string[];
  category: string | null;
  git_url: string;
  stars: number;
  downloads: number;
}

/** A category in the plugin registry */
export interface RegistryCategory {
  id: string;
  label: string;
}

/** The full plugin registry */
export interface PluginRegistry {
  version: string;
  updated_at: string;
  plugins: RegistryEntry[];
  categories: RegistryCategory[];
}

/** A marketplace plugin enriched with local status */
export interface MarketplacePlugin extends RegistryEntry {
  installed: boolean;
  enabled: boolean;
}

/** Progress update during plugin installation */
export interface InstallProgress {
  plugin_name: string;
  phase: string;
  message: string;
  progress: number;
}
