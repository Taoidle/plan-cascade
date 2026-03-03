import type { FileAttachmentData } from '../../types/attachment';
import type { CardPayload } from '../../types/workflowCard';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';

export type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

export type Strategy = 'direct' | 'hybrid_auto' | 'hybrid_worktree' | 'mega_plan' | null;

/** Dimension scores from strategy analysis (0.0 - 1.0 each) */
export interface DimensionScores {
  scope: number;
  complexity: number;
  risk: number;
  parallelization: number;
}

/** Result of automatic strategy analysis from the Rust backend */
export interface StrategyAnalysis {
  strategy: string;
  confidence: number;
  reasoning: string;
  estimated_stories: number;
  estimated_features: number;
  estimated_duration_hours: number;
  complexity_indicators: string[];
  recommendations: string[];
  dimension_scores: DimensionScores;
}

/** A strategy option returned by get_strategy_options */
export interface StrategyOptionInfo {
  value: string;
  label: string;
  description: string;
  min_stories: number;
  max_stories: number;
}

export interface Story {
  id: string;
  title: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
  startedAt?: string;
  completedAt?: string;
  retryCount?: number;
}

// ============================================================================
// Streaming Output Types
// ============================================================================

export type StreamLineType =
  | 'text'
  | 'info'
  | 'error'
  | 'success'
  | 'warning'
  | 'tool'
  | 'tool_result'
  | 'sub_agent'
  | 'analysis'
  | 'thinking'
  | 'code'
  | 'card';

export interface StreamLine {
  id: number;
  content: string;
  type: StreamLineType;
  timestamp: number;
  /** Structured workflow card payload (v2 path). */
  cardPayload?: CardPayload;
  /** Sub-agent ID if this line originated from a sub-agent */
  subAgentId?: string;
  /** Sub-agent nesting depth (0 = root) */
  subAgentDepth?: number;
}

export interface HistoryConversationLine {
  type: StreamLineType;
  content: string;
  subAgentId?: string;
  subAgentDepth?: number;
}

export interface AnalysisCoverageSnapshot {
  runId?: string;
  status: 'idle' | 'running' | 'completed' | 'failed';
  successfulPhases: number;
  partialPhases: number;
  failedPhases: number;
  observedPaths: number;
  inventoryTotalFiles: number;
  sampledReadFiles: number;
  testFilesTotal: number;
  testFilesRead: number;
  coverageRatio: number;
  sampledReadRatio: number;
  testCoverageRatio: number;
  observedTestCoverageRatio: number;
  coverageTargetRatio?: number;
  sampledReadTargetRatio?: number;
  testCoverageTargetRatio?: number;
  validationIssues: string[];
  manifestPath?: string;
  reportPath?: string;
  updatedAt: number;
}

// ============================================================================
// Quality Gate Result Types
// ============================================================================

export type QualityGateStatus = 'pending' | 'running' | 'passed' | 'failed';

export interface QualityGateResult {
  gateId: string;
  gateName: string;
  storyId: string;
  status: QualityGateStatus;
  output?: string;
  duration?: number;
  startedAt?: number;
  completedAt?: number;
}

// ============================================================================
// Error State Types
// ============================================================================

export type ErrorSeverity = 'warning' | 'error' | 'critical';

export interface ExecutionError {
  id: string;
  storyId?: string;
  severity: ErrorSeverity;
  title: string;
  description: string;
  suggestedFix?: string;
  stackTrace?: string;
  timestamp: number;
  dismissed: boolean;
}

export interface Batch {
  batchNum: number;
  totalBatches: number;
  storyIds: string[];
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  startedAt?: string;
  completedAt?: string;
}

export interface ExecutionResult {
  success: boolean;
  message: string;
  completedStories: number;
  totalStories: number;
  duration: number;
  error?: string;
}

export interface ExecutionHistoryItem {
  id: string;
  title?: string;
  taskDescription: string;
  workspacePath?: string | null;
  strategy: Strategy;
  status: ExecutionStatus;
  startedAt: number;
  completedAt?: number;
  duration: number;
  completedStories: number;
  totalStories: number;
  success: boolean;
  error?: string;
  /** Serialized conversation content from streamingOutput */
  conversationContent?: string;
  /** Structured conversation lines for lossless restore */
  conversationLines?: HistoryConversationLine[];
  /** Session ID for potential reconnection */
  sessionId?: string;
  /** LLM backend used by this conversation session */
  llmBackend?: string;
  /** LLM provider used by this conversation session */
  llmProvider?: string;
  /** LLM model used by this conversation session */
  llmModel?: string;
}

export interface StandaloneTurn {
  user: string;
  assistant: string;
  createdAt: number;
  metadata?: {
    source?: string;
    type?: string;
  };
}

export interface BackendUsageStats {
  input_tokens: number;
  output_tokens: number;
  thinking_tokens?: number | null;
  cache_read_tokens?: number | null;
  cache_creation_tokens?: number | null;
}

/** Snapshot of per-session state for background session storage */
export interface SessionSnapshot {
  id: string;
  taskDescription: string;
  status: ExecutionStatus;
  streamingOutput: StreamLine[];
  streamLineCounter: number;
  currentTurnStartLineId: number;
  taskId: string | null;
  isChatSession: boolean;
  standaloneTurns: StandaloneTurn[];
  standaloneSessionId: string | null;
  latestUsage: BackendUsageStats | null;
  sessionUsageTotals: BackendUsageStats | null;
  startedAt: number | null;
  toolCallFilter: ToolCallStreamFilter;
  /** LLM backend active when this session was backgrounded */
  llmBackend: string;
  /** LLM provider active when this session was backgrounded */
  llmProvider: string;
  /** LLM model active when this session was backgrounded */
  llmModel: string;
  /** Parent session ID for fork hierarchy (undefined = root session) */
  parentSessionId?: string;
  /** Workspace path active when this session was backgrounded */
  workspacePath?: string;
  /** History entry ID this session originated from (for de-dup in sidebar) */
  originHistoryId?: string;
  /** Stable session identity from history (claude:/standalone:) */
  originSessionId?: string;
  /** Last mutation timestamp for sort/recovery heuristics */
  updatedAt?: number;
}

export interface ExecutionState {
  /** Current execution status */
  status: ExecutionStatus;

  /** Backend connection status (always connected in Tauri) */
  connectionStatus: ConnectionStatus;

  /** Task ID from server */
  taskId: string | null;

  /** Active Claude execution ID for the current turn (session-scoped) */
  activeExecutionId: string | null;

  /** True while waiting for backend cancel ACK */
  isCancelling: boolean;

  /** User requested cancel before Claude session_id became available */
  pendingCancelBeforeSessionReady: boolean;

  /** Task description */
  taskDescription: string;

  /** Selected strategy */
  strategy: Strategy;

  /** List of stories */
  stories: Story[];

  /** List of batches */
  batches: Batch[];

  /** Current batch number */
  currentBatch: number;

  /** Currently executing story ID */
  currentStoryId: string | null;

  /** Overall progress (0-100) */
  progress: number;

  /** Execution result */
  result: ExecutionResult | null;

  /** Start timestamp */
  startedAt: number | null;

  /** Execution logs */
  logs: string[];

  /** Execution history */
  history: ExecutionHistoryItem[];

  /** Is submitting (API call in progress) */
  isSubmitting: boolean;

  /** API error message */
  apiError: string | null;

  /** Strategy analysis result from auto-analyzer */
  strategyAnalysis: StrategyAnalysis | null;

  /** Whether strategy analysis is in progress */
  isAnalyzingStrategy: boolean;

  /** Available strategy options metadata */
  strategyOptions: StrategyOptionInfo[];

  /** Streaming output buffer for real-time display */
  streamingOutput: StreamLine[];

  /** Counter for unique stream line IDs */
  streamLineCounter: number;

  /** Line ID at the start of the current turn (for scoping text_replace) */
  currentTurnStartLineId: number;

  /** Structured analysis coverage snapshot for Simple mode visualization */
  analysisCoverage: AnalysisCoverageSnapshot | null;

  /** Quality gate results per story */
  qualityGateResults: QualityGateResult[];

  /** Actionable error states */
  executionErrors: ExecutionError[];

  /** Estimated time remaining in milliseconds */
  estimatedTimeRemaining: number | null;

  /** Whether we're in an active Claude Code chat session (supports multi-turn) */
  isChatSession: boolean;

  /** Local multi-turn context for standalone providers (glm/openai/deepseek/qwen/ollama) */
  standaloneTurns: StandaloneTurn[];

  /** Session identifier for standalone conversation-scoped analysis reuse */
  standaloneSessionId: string | null;

  /** Last usage payload reported by backend for current turn */
  latestUsage: BackendUsageStats | null;

  /** Accumulated token usage for current chat session */
  sessionUsageTotals: BackendUsageStats | null;

  /** Token usage accumulated for the current turn (reset on each new user message) */
  turnUsageTotals: BackendUsageStats | null;

  /** Filter for stripping tool_call code blocks from streaming text */
  toolCallFilter: ToolCallStreamFilter;

  /** File attachments pending to be sent with the next message */
  attachments: FileAttachmentData[];

  /** Background session snapshots keyed by session ID */
  backgroundSessions: Record<string, SessionSnapshot>;

  /** Currently active foreground session ID (for tracking which bg session was swapped in) */
  activeSessionId: string | null;

  /** Parent session ID of the current foreground session (set when created via fork) */
  foregroundParentSessionId: string | null;

  /** bg session ID representing the foreground in the tree (ghost). null if foreground was created fresh (fork/new). */
  foregroundBgId: string | null;

  /** Source history item ID for the current foreground (used for de-dup and switch heuristics). */
  foregroundOriginHistoryId: string | null;

  /** Source logical session ID for the current foreground (claude:/standalone:). */
  foregroundOriginSessionId: string | null;

  /** Whether foreground diverged from its source snapshot/history and should be persisted on switch. */
  foregroundDirty: boolean;

  /** Active agent ID for the current session */
  activeAgentId: string | null;

  /** Active agent name for display */
  activeAgentName: string | null;

  /** Pending task context to inject into the next sendFollowUp (Claude Code backend) */
  _pendingTaskContext: string | null;

  // Actions
  /** Add a file attachment */
  addAttachment: (file: FileAttachmentData) => void;

  /** Remove a file attachment by ID */
  removeAttachment: (id: string) => void;

  /** Clear all file attachments */
  clearAttachments: () => void;

  /** Snapshot the current foreground session into backgroundSessions and reset foreground */
  backgroundCurrentSession: () => void;

  /** Swap the foreground session with a background session by ID */
  switchToSession: (id: string) => void;

  /** Remove a background session by ID */
  removeBackgroundSession: (id: string) => void;

  /** Initialize Tauri event listeners */
  initialize: () => void;

  /** Cleanup event listeners */
  cleanup: () => void;

  /** Start execution */
  start: (description: string, mode: 'simple' | 'expert') => Promise<void>;

  /** Pause execution */
  pause: () => Promise<void>;

  /** Resume execution */
  resume: () => Promise<void>;

  /** Cancel execution */
  cancel: () => Promise<void>;

  /** Send a follow-up message in an existing Claude Code chat session */
  sendFollowUp: (prompt: string) => Promise<void>;

  /** Reset state */
  reset: () => void;

  /** Update story status */
  updateStory: (storyId: string, updates: Partial<Story>) => void;

  /** Add log entry */
  addLog: (message: string) => void;

  /** Set stories from server */
  setStories: (stories: Story[]) => void;

  /** Set strategy */
  setStrategy: (strategy: Strategy) => void;

  /** Load history from localStorage */
  loadHistory: () => void;

  /** Save to history */
  saveToHistory: () => void;

  /** Clear history */
  clearHistory: () => void;

  /** Delete a single history item */
  deleteHistory: (historyId: string) => void;

  /** Rename a history item */
  renameHistory: (historyId: string, title: string) => void;

  /** Restore a conversation from history into the streaming output view */
  restoreFromHistory: (historyId: string) => void;

  /** Analyze task strategy via Rust backend */
  analyzeStrategy: (description: string) => Promise<StrategyAnalysis | null>;

  /** Load available strategy options */
  loadStrategyOptions: () => Promise<void>;

  /** Clear strategy analysis */
  clearStrategyAnalysis: () => void;

  /** Append a streaming output line */
  appendStreamLine: (content: string, type: StreamLineType, subAgentId?: string, subAgentDepth?: number) => void;

  /** Append a structured workflow card line */
  appendCard: (payload: CardPayload, subAgentId?: string, subAgentDepth?: number) => void;

  /** Clear streaming output buffer */
  clearStreamingOutput: () => void;

  /** Update quality gate result for a story */
  updateQualityGate: (result: QualityGateResult) => void;

  /** Add an execution error */
  addExecutionError: (error: Omit<ExecutionError, 'id' | 'timestamp' | 'dismissed'>) => void;

  /** Dismiss an execution error */
  dismissError: (errorId: string) => void;

  /** Clear all execution errors */
  clearExecutionErrors: () => void;

  /** Retry a failed story */
  retryStory: (storyId: string) => Promise<void>;

  /** Rollback conversation to a specific user turn, removing all subsequent turns */
  rollbackToTurn: (userLineId: number) => void;

  /** Regenerate the assistant response for a given user turn */
  regenerateResponse: (userLineId: number) => Promise<void>;

  /** Edit a user message and resend it */
  editAndResend: (userLineId: number, newContent: string) => Promise<void>;

  /** Append a synthetic StandaloneTurn (used by contextBridge for Task→Chat writeback) */
  appendStandaloneTurn: (turn: StandaloneTurn) => void;

  /** Set pending task context to inject into next sendFollowUp (Claude Code backend) */
  setPendingTaskContext: (context: string) => void;

  /** Clear pending task context */
  clearPendingTaskContext: () => void;

  /** Fork conversation at a turn: background original session, create truncated foreground copy */
  forkSessionAtTurn: (userLineId: number) => void;
}
