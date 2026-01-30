/**
 * Agent Types
 *
 * TypeScript interfaces matching the Rust models in desktop/src-tauri/src/models/agent.rs
 */

/** AI Agent with custom system prompt and tool configuration */
export interface Agent {
  /** Unique identifier (UUID) */
  id: string;
  /** Display name for the agent */
  name: string;
  /** Description of what the agent does */
  description: string | null;
  /** System prompt that defines the agent's behavior */
  system_prompt: string;
  /** Model to use (e.g., "claude-sonnet-4-20250514") */
  model: string;
  /** List of allowed tool names (empty means all tools allowed) */
  allowed_tools: string[];
  /** Creation timestamp (ISO 8601) */
  created_at: string | null;
  /** Last update timestamp (ISO 8601) */
  updated_at: string | null;
}

/** Request to create a new agent */
export interface AgentCreateRequest {
  /** Display name for the agent */
  name: string;
  /** Description of what the agent does */
  description?: string | null;
  /** System prompt that defines the agent's behavior */
  system_prompt: string;
  /** Model to use */
  model: string;
  /** List of allowed tool names */
  allowed_tools: string[];
}

/** Request to update an existing agent */
export interface AgentUpdateRequest {
  /** New display name (optional) */
  name?: string | null;
  /** New description (optional) */
  description?: string | null;
  /** New system prompt (optional) */
  system_prompt?: string | null;
  /** New model (optional) */
  model?: string | null;
  /** New allowed tools list (optional) */
  allowed_tools?: string[] | null;
}

/** Agent run status */
export type RunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

/** A single execution run of an agent */
export interface AgentRun {
  /** Unique run identifier */
  id: string;
  /** Agent that was executed */
  agent_id: string;
  /** User input that triggered the run */
  input: string;
  /** Output produced by the agent */
  output: string | null;
  /** Current status of the run */
  status: RunStatus;
  /** Execution duration in milliseconds */
  duration_ms: number | null;
  /** Number of input tokens used */
  input_tokens: number | null;
  /** Number of output tokens generated */
  output_tokens: number | null;
  /** Error message if the run failed */
  error: string | null;
  /** When the run was created */
  created_at: string | null;
  /** When the run completed */
  completed_at: string | null;
}

/** Statistics for an agent */
export interface AgentStats {
  /** Total number of runs */
  total_runs: number;
  /** Number of successful runs */
  completed_runs: number;
  /** Number of failed runs */
  failed_runs: number;
  /** Number of cancelled runs */
  cancelled_runs: number;
  /** Success rate (completed / total) as percentage */
  success_rate: number;
  /** Average execution duration in milliseconds */
  avg_duration_ms: number;
  /** Total input tokens used */
  total_input_tokens: number;
  /** Total output tokens generated */
  total_output_tokens: number;
  /** Timestamp of the last run */
  last_run_at: string | null;
}

/** Paginated list of agent runs */
export interface AgentRunList {
  /** List of runs */
  runs: AgentRun[];
  /** Total count of runs (for pagination) */
  total: number;
  /** Current offset */
  offset: number;
  /** Number of items per page */
  limit: number;
}

/** Agent with statistics (for UI display) */
export interface AgentWithStats extends Agent {
  stats: AgentStats;
}

/** Generic command response from Tauri */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Available Claude models */
export const CLAUDE_MODELS = [
  { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4', description: 'Balanced performance' },
  { id: 'claude-opus-4-20250514', name: 'Claude Opus 4', description: 'Most capable' },
  { id: 'claude-3-5-sonnet-20241022', name: 'Claude 3.5 Sonnet', description: 'Previous generation' },
  { id: 'claude-3-5-haiku-20241022', name: 'Claude 3.5 Haiku', description: 'Fast and efficient' },
] as const;

/** Default model for new agents */
export const DEFAULT_MODEL = 'claude-sonnet-4-20250514';

/** Available tools that can be assigned to agents */
export const AVAILABLE_TOOLS = [
  // File operations
  { name: 'read_file', category: 'File Operations', description: 'Read file contents' },
  { name: 'write_file', category: 'File Operations', description: 'Write to a file' },
  { name: 'edit_file', category: 'File Operations', description: 'Edit file contents' },
  { name: 'list_directory', category: 'File Operations', description: 'List directory contents' },
  { name: 'search_files', category: 'File Operations', description: 'Search for files' },
  // Code operations
  { name: 'execute_code', category: 'Code', description: 'Execute code snippets' },
  { name: 'run_tests', category: 'Code', description: 'Run test suites' },
  { name: 'lint_code', category: 'Code', description: 'Lint and format code' },
  // Git operations
  { name: 'git_status', category: 'Git', description: 'Get git status' },
  { name: 'git_diff', category: 'Git', description: 'Show git diff' },
  { name: 'git_commit', category: 'Git', description: 'Create git commits' },
  { name: 'git_push', category: 'Git', description: 'Push to remote' },
  // Web operations
  { name: 'web_search', category: 'Web', description: 'Search the web' },
  { name: 'web_fetch', category: 'Web', description: 'Fetch web content' },
  // System operations
  { name: 'execute_command', category: 'System', description: 'Execute shell commands' },
] as const;

/** Get tools grouped by category */
export function getToolsByCategory(): Record<string, typeof AVAILABLE_TOOLS[number][]> {
  const grouped: Record<string, typeof AVAILABLE_TOOLS[number][]> = {};

  for (const tool of AVAILABLE_TOOLS) {
    if (!grouped[tool.category]) {
      grouped[tool.category] = [];
    }
    grouped[tool.category].push(tool);
  }

  return grouped;
}

/** Format duration in a human-readable way */
export function formatDuration(ms: number | null): string {
  if (ms === null || ms === undefined) return '-';

  if (ms < 1000) {
    return `${ms}ms`;
  } else if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`;
  } else {
    const minutes = Math.floor(ms / 60000);
    const seconds = ((ms % 60000) / 1000).toFixed(0);
    return `${minutes}m ${seconds}s`;
  }
}

/** Format token count in a human-readable way */
export function formatTokens(count: number | null): string {
  if (count === null || count === undefined) return '-';

  if (count < 1000) {
    return count.toString();
  } else if (count < 1000000) {
    return `${(count / 1000).toFixed(1)}K`;
  } else {
    return `${(count / 1000000).toFixed(2)}M`;
  }
}

/** Get status color for UI display */
export function getStatusColor(status: RunStatus): string {
  switch (status) {
    case 'completed':
      return 'text-green-600 dark:text-green-400';
    case 'running':
      return 'text-blue-600 dark:text-blue-400';
    case 'pending':
      return 'text-yellow-600 dark:text-yellow-400';
    case 'failed':
      return 'text-red-600 dark:text-red-400';
    case 'cancelled':
      return 'text-gray-600 dark:text-gray-400';
    default:
      return 'text-gray-500';
  }
}

/** Get status badge background color */
export function getStatusBgColor(status: RunStatus): string {
  switch (status) {
    case 'completed':
      return 'bg-green-100 dark:bg-green-900/50';
    case 'running':
      return 'bg-blue-100 dark:bg-blue-900/50';
    case 'pending':
      return 'bg-yellow-100 dark:bg-yellow-900/50';
    case 'failed':
      return 'bg-red-100 dark:bg-red-900/50';
    case 'cancelled':
      return 'bg-gray-100 dark:bg-gray-900/50';
    default:
      return 'bg-gray-100 dark:bg-gray-800';
  }
}
