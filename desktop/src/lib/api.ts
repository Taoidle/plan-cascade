/**
 * API Client
 *
 * HTTP client for communicating with the Plan Cascade FastAPI backend.
 * Provides typed methods for all API endpoints.
 */

const API_BASE_URL = 'http://127.0.0.1:8765/api';

// ============================================================================
// Types
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

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let detail: string | undefined;
    try {
      const data = await response.json();
      detail = data.detail;
    } catch {
      // Ignore JSON parse errors
    }
    throw new ApiError(response.status, response.statusText, detail);
  }
  return response.json();
}

// ============================================================================
// API Client
// ============================================================================

export const api = {
  // --------------------------------------------------------------------------
  // Health
  // --------------------------------------------------------------------------

  /**
   * Check server health
   */
  async health(): Promise<HealthResponse> {
    const response = await fetch(`${API_BASE_URL}/health`);
    return handleResponse<HealthResponse>(response);
  },

  // --------------------------------------------------------------------------
  // Execution
  // --------------------------------------------------------------------------

  /**
   * Start task execution
   */
  async execute(request: ExecuteRequest): Promise<ExecuteResponse> {
    const response = await fetch(`${API_BASE_URL}/execute`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return handleResponse<ExecuteResponse>(response);
  },

  /**
   * Cancel current execution
   */
  async cancelExecution(): Promise<{ status: string; message: string }> {
    const response = await fetch(`${API_BASE_URL}/execute/cancel`, {
      method: 'POST',
    });
    return handleResponse(response);
  },

  /**
   * Pause current execution
   */
  async pauseExecution(): Promise<{ status: string; message: string }> {
    const response = await fetch(`${API_BASE_URL}/execute/pause`, {
      method: 'POST',
    });
    return handleResponse(response);
  },

  /**
   * Resume paused execution
   */
  async resumeExecution(): Promise<{ status: string; message: string }> {
    const response = await fetch(`${API_BASE_URL}/execute/resume`, {
      method: 'POST',
    });
    return handleResponse(response);
  },

  // --------------------------------------------------------------------------
  // Status
  // --------------------------------------------------------------------------

  /**
   * Get current execution status
   */
  async getStatus(): Promise<StatusResponse> {
    const response = await fetch(`${API_BASE_URL}/status`);
    return handleResponse<StatusResponse>(response);
  },

  /**
   * Get all stories status
   */
  async getStoriesStatus(): Promise<StoryStatus[]> {
    const response = await fetch(`${API_BASE_URL}/status/stories`);
    return handleResponse<StoryStatus[]>(response);
  },

  /**
   * Get specific story status
   */
  async getStoryStatus(storyId: string): Promise<StoryStatus> {
    const response = await fetch(`${API_BASE_URL}/status/story/${storyId}`);
    return handleResponse<StoryStatus>(response);
  },

  // --------------------------------------------------------------------------
  // PRD
  // --------------------------------------------------------------------------

  /**
   * Generate PRD from description
   */
  async generatePRD(request: PRDRequest): Promise<PRDResponse> {
    const response = await fetch(`${API_BASE_URL}/prd/generate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return handleResponse<PRDResponse>(response);
  },

  /**
   * Get current PRD
   */
  async getPRD(): Promise<PRD> {
    const response = await fetch(`${API_BASE_URL}/prd`);
    return handleResponse<PRD>(response);
  },

  /**
   * Update current PRD
   */
  async updatePRD(update: {
    stories?: PRDStory[];
    metadata?: Record<string, unknown>;
  }): Promise<{ status: string; prd: PRD }> {
    const response = await fetch(`${API_BASE_URL}/prd`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(update),
    });
    return handleResponse(response);
  },

  /**
   * Delete current PRD
   */
  async deletePRD(): Promise<{ status: string; message: string }> {
    const response = await fetch(`${API_BASE_URL}/prd`, {
      method: 'DELETE',
    });
    return handleResponse(response);
  },

  /**
   * Approve PRD and start execution
   */
  async approvePRD(): Promise<{
    status: string;
    task_id: string;
    message: string;
  }> {
    const response = await fetch(`${API_BASE_URL}/prd/approve`, {
      method: 'POST',
    });
    return handleResponse(response);
  },

  // --------------------------------------------------------------------------
  // Analysis
  // --------------------------------------------------------------------------

  /**
   * Analyze task and recommend strategy
   */
  async analyzeTask(description: string): Promise<AnalyzeResponse> {
    const response = await fetch(`${API_BASE_URL}/analyze`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ description }),
    });
    return handleResponse<AnalyzeResponse>(response);
  },

  // --------------------------------------------------------------------------
  // Claude Code
  // --------------------------------------------------------------------------

  /**
   * Create a new Claude Code session
   */
  async createClaudeCodeSession(
    workingDir?: string,
    model?: string
  ): Promise<ClaudeCodeSession> {
    const params = new URLSearchParams();
    if (workingDir) params.append('working_dir', workingDir);
    if (model) params.append('model', model);

    const url = `${API_BASE_URL}/claude-code/session${params.toString() ? '?' + params.toString() : ''}`;
    const response = await fetch(url, { method: 'POST' });
    return handleResponse<ClaudeCodeSession>(response);
  },

  /**
   * Get Claude Code session info
   */
  async getClaudeCodeSession(sessionId: string): Promise<ClaudeCodeSessionInfo> {
    const response = await fetch(`${API_BASE_URL}/claude-code/session/${sessionId}`);
    return handleResponse<ClaudeCodeSessionInfo>(response);
  },

  /**
   * Cancel Claude Code session
   */
  async cancelClaudeCodeSession(sessionId: string): Promise<{ status: string; session_id: string }> {
    const response = await fetch(`${API_BASE_URL}/claude-code/session/${sessionId}`, {
      method: 'DELETE',
    });
    return handleResponse(response);
  },

  /**
   * List all Claude Code sessions
   */
  async listClaudeCodeSessions(): Promise<ClaudeCodeSessionInfo[]> {
    const response = await fetch(`${API_BASE_URL}/claude-code/sessions`);
    return handleResponse<ClaudeCodeSessionInfo[]>(response);
  },

  /**
   * Send message to Claude Code (non-streaming)
   */
  async sendClaudeCodeMessage(
    sessionId: string,
    content: string
  ): Promise<{ content: string; tool_calls: unknown[] }> {
    const response = await fetch(`${API_BASE_URL}/claude-code/session/${sessionId}/message`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
    return handleResponse(response);
  },
};

export default api;
