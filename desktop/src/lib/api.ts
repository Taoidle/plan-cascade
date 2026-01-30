/**
 * API Client (DEPRECATED - v5.0 Pure Rust Backend)
 *
 * This file is deprecated. The v5.0 architecture uses Tauri IPC instead of HTTP API.
 *
 * For Claude Code functionality, use:
 * - import { ... } from './claudeCodeClient'
 *
 * For execution functionality, use:
 * - The execution store uses Tauri invoke directly
 *
 * For settings, use:
 * - import { ... } from './settingsApi'
 *
 * @deprecated Use Tauri commands instead
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types (kept for backwards compatibility)
// ============================================================================

export interface ExecuteRequest {
  description: string;
  mode: 'simple' | 'expert';
  project_path?: string;
  use_worktree?: boolean;
  strategy?: 'direct' | 'hybrid_auto' | 'mega_plan';
  prd?: PRD;
}

export interface ExecuteResponse {
  task_id: string;
  status: string;
  message: string;
}

export interface PRD {
  metadata?: {
    version?: string;
    title?: string;
    description?: string;
  };
  goal?: string;
  stories: PRDStory[];
}

export interface PRDStory {
  id: string;
  title: string;
  description?: string;
  priority?: 'high' | 'medium' | 'low';
  dependencies?: string[];
  status?: string;
  acceptance_criteria?: string[];
}

export interface PRDRequest {
  description: string;
  context?: string;
}

export interface PRDResponse {
  prd_path: string;
  stories: PRDStory[];
  metadata?: Record<string, unknown>;
}

export interface StatusResponse {
  status: string;
  task_description: string;
  current_story_id: string | null;
  stories: StoryStatus[];
  progress: number;
}

export interface StoryStatus {
  id: string;
  title: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  started_at?: string;
  completed_at?: string;
  error?: string;
}

export interface HealthResponse {
  status: string;
  version: string;
}

export interface AnalyzeResponse {
  recommended_strategy: string;
  reasoning: string;
  estimated_stories: number;
  complexity: 'low' | 'medium' | 'high';
}

// Claude Code types
export interface ClaudeCodeSession {
  session_id: string;
  status: string;
  message: string;
}

export interface ClaudeCodeSessionInfo {
  id: string;
  working_dir: string;
  model?: string;
  status: string;
  messages: Array<{ role: string; content: string }>;
  tool_calls: Array<{
    id: string;
    name: string;
    parameters: Record<string, unknown>;
    status: string;
  }>;
}

// ============================================================================
// Error Handling
// ============================================================================

export class ApiError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    public detail?: string
  ) {
    super(detail || `API Error: ${status} ${statusText}`);
    this.name = 'ApiError';
  }
}

// ============================================================================
// Response Type
// ============================================================================

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// API Client (Tauri-based)
// ============================================================================

export const api = {
  // --------------------------------------------------------------------------
  // Health
  // --------------------------------------------------------------------------

  /**
   * Check backend health via Tauri
   */
  async health(): Promise<HealthResponse> {
    try {
      const result = await invoke<CommandResponse<HealthResponse>>('get_health');
      if (result.success && result.data) {
        return result.data;
      }
      // Fallback for when health command doesn't exist
      return { status: 'ok', version: '5.0.0' };
    } catch {
      return { status: 'ok', version: '5.0.0' };
    }
  },

  // --------------------------------------------------------------------------
  // Execution (via Tauri)
  // --------------------------------------------------------------------------

  /**
   * Start task execution via Tauri
   */
  async execute(request: ExecuteRequest): Promise<ExecuteResponse> {
    try {
      const result = await invoke<CommandResponse<{ task_id: string }>>('execute_standalone', {
        message: request.description,
        provider: 'anthropic',
        model: 'claude-sonnet-4-20250514',
        project_path: request.project_path || '.',
      });

      if (result.success && result.data) {
        return {
          task_id: result.data.task_id,
          status: 'started',
          message: 'Execution started',
        };
      }
      throw new ApiError(500, 'Execution failed', result.error || 'Unknown error');
    } catch (error) {
      if (error instanceof ApiError) throw error;
      throw new ApiError(500, 'Execution failed', error instanceof Error ? error.message : 'Unknown error');
    }
  },

  /**
   * Cancel current execution via Tauri
   */
  async cancelExecution(): Promise<{ status: string; message: string }> {
    // Cancellation is handled directly in the execution store
    return { status: 'cancelled', message: 'Use execution store cancel() method' };
  },

  /**
   * Pause current execution (not implemented in v5.0)
   */
  async pauseExecution(): Promise<{ status: string; message: string }> {
    return { status: 'paused', message: 'Pause handled in execution store' };
  },

  /**
   * Resume paused execution (not implemented in v5.0)
   */
  async resumeExecution(): Promise<{ status: string; message: string }> {
    return { status: 'resumed', message: 'Resume handled in execution store' };
  },

  // --------------------------------------------------------------------------
  // Status (use execution store instead)
  // --------------------------------------------------------------------------

  /**
   * @deprecated Use execution store state
   */
  async getStatus(): Promise<StatusResponse> {
    console.warn('api.getStatus() is deprecated. Use useExecutionStore state');
    return {
      status: 'idle',
      task_description: '',
      current_story_id: null,
      stories: [],
      progress: 0,
    };
  },

  /**
   * @deprecated Use execution store state
   */
  async getStoriesStatus(): Promise<StoryStatus[]> {
    console.warn('api.getStoriesStatus() is deprecated. Use useExecutionStore stories');
    return [];
  },

  /**
   * @deprecated Use execution store state
   */
  async getStoryStatus(_storyId: string): Promise<StoryStatus> {
    console.warn('api.getStoryStatus() is deprecated. Use useExecutionStore stories');
    throw new ApiError(404, 'Not Found', 'Use execution store');
  },

  // --------------------------------------------------------------------------
  // PRD (not implemented in v5.0 standalone mode)
  // --------------------------------------------------------------------------

  /**
   * @deprecated PRD generation not available in standalone mode
   */
  async generatePRD(_request: PRDRequest): Promise<PRDResponse> {
    console.warn('api.generatePRD() is not available in v5.0 standalone mode');
    throw new ApiError(501, 'Not Implemented', 'PRD generation not available in standalone mode');
  },

  /**
   * @deprecated PRD not available in standalone mode
   */
  async getPRD(): Promise<PRD> {
    console.warn('api.getPRD() is not available in v5.0 standalone mode');
    return { stories: [] };
  },

  /**
   * @deprecated PRD not available in standalone mode
   */
  async updatePRD(_update: {
    stories?: PRDStory[];
    metadata?: Record<string, unknown>;
  }): Promise<{ status: string; prd: PRD }> {
    console.warn('api.updatePRD() is not available in v5.0 standalone mode');
    return { status: 'not_implemented', prd: { stories: [] } };
  },

  /**
   * @deprecated PRD not available in standalone mode
   */
  async deletePRD(): Promise<{ status: string; message: string }> {
    console.warn('api.deletePRD() is not available in v5.0 standalone mode');
    return { status: 'not_implemented', message: 'PRD not available in standalone mode' };
  },

  /**
   * @deprecated PRD not available in standalone mode
   */
  async approvePRD(): Promise<{
    status: string;
    task_id: string;
    message: string;
  }> {
    console.warn('api.approvePRD() is not available in v5.0 standalone mode');
    return { status: 'not_implemented', task_id: '', message: 'PRD not available in standalone mode' };
  },

  // --------------------------------------------------------------------------
  // Analysis (not implemented in v5.0 standalone mode)
  // --------------------------------------------------------------------------

  /**
   * @deprecated Analysis not available in standalone mode
   */
  async analyzeTask(_description: string): Promise<AnalyzeResponse> {
    console.warn('api.analyzeTask() is not available in v5.0 standalone mode');
    return {
      recommended_strategy: 'direct',
      reasoning: 'Analysis not available in standalone mode',
      estimated_stories: 0,
      complexity: 'low',
    };
  },

  // --------------------------------------------------------------------------
  // Claude Code (use claudeCodeClient instead)
  // --------------------------------------------------------------------------

  /**
   * @deprecated Use claudeCodeClient.startChat()
   */
  async createClaudeCodeSession(
    workingDir?: string,
    _model?: string
  ): Promise<ClaudeCodeSession> {
    console.warn('api.createClaudeCodeSession() is deprecated. Use claudeCodeClient');
    const { getClaudeCodeClient } = await import('./claudeCodeClient');
    const client = getClaudeCodeClient();
    const response = await client.startChat({ project_path: workingDir || '.' });
    return {
      session_id: response.session_id,
      status: 'created',
      message: 'Session created',
    };
  },

  /**
   * @deprecated Use claudeCodeClient.getSessionInfo()
   */
  async getClaudeCodeSession(sessionId: string): Promise<ClaudeCodeSessionInfo> {
    console.warn('api.getClaudeCodeSession() is deprecated. Use claudeCodeClient');
    const { getClaudeCodeClient } = await import('./claudeCodeClient');
    const client = getClaudeCodeClient();
    const info = await client.getSessionInfo(sessionId);
    return {
      id: info.session.id,
      working_dir: info.session.project_path,
      model: info.session.model || undefined,
      status: info.session.state,
      messages: [],
      tool_calls: [],
    };
  },

  /**
   * @deprecated Use claudeCodeClient.cancelExecution()
   */
  async cancelClaudeCodeSession(sessionId: string): Promise<{ status: string; session_id: string }> {
    console.warn('api.cancelClaudeCodeSession() is deprecated. Use claudeCodeClient');
    const { getClaudeCodeClient } = await import('./claudeCodeClient');
    const client = getClaudeCodeClient();
    await client.cancelExecution(sessionId);
    return { status: 'cancelled', session_id: sessionId };
  },

  /**
   * @deprecated Use claudeCodeClient.listActiveSessions()
   */
  async listClaudeCodeSessions(): Promise<ClaudeCodeSessionInfo[]> {
    console.warn('api.listClaudeCodeSessions() is deprecated. Use claudeCodeClient');
    const { getClaudeCodeClient } = await import('./claudeCodeClient');
    const client = getClaudeCodeClient();
    const sessions = await client.listActiveSessions();
    return sessions.map((s) => ({
      id: s.session.id,
      working_dir: s.session.project_path,
      model: s.session.model || undefined,
      status: s.session.state,
      messages: [],
      tool_calls: [],
    }));
  },

  /**
   * @deprecated Use claudeCodeClient.sendMessage()
   */
  async sendClaudeCodeMessage(
    sessionId: string,
    content: string
  ): Promise<{ content: string; tool_calls: unknown[] }> {
    console.warn('api.sendClaudeCodeMessage() is deprecated. Use claudeCodeClient');
    const { getClaudeCodeClient } = await import('./claudeCodeClient');
    const client = getClaudeCodeClient();
    await client.sendMessage(sessionId, content);
    return { content: 'Message sent via events', tool_calls: [] };
  },
};

export default api;
