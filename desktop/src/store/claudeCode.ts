/**
 * Claude Code Store (v5.0 Pure Rust Backend)
 *
 * Manages the Claude Code mode state including messages, tool calls,
 * connection status, and conversation history.
 *
 * Uses Tauri IPC instead of WebSocket for communication with Rust backend.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import {
  getClaudeCodeClient,
  initClaudeCodeClient,
  closeClaudeCodeClient,
  type ConnectionStatus,
  type StreamEventPayload,
  type ToolUpdateEvent,
  type SessionUpdateEvent,
  type UnifiedStreamEvent,
} from '../lib/claudeCodeClient';

// ============================================================================
// Module-level listener tracking (prevents duplicates from React StrictMode)
// ============================================================================

let _streamUnlisten: (() => void) | null = null;
let _toolUnlisten: (() => void) | null = null;
let _sessionUnlisten: (() => void) | null = null;
let _statusUnsubscribe: (() => void) | null = null;
let _lifecycleVersion = 0;

function cleanupEventListeners() {
  if (_statusUnsubscribe) {
    _statusUnsubscribe();
    _statusUnsubscribe = null;
  }
  if (_streamUnlisten) {
    _streamUnlisten();
    _streamUnlisten = null;
  }
  if (_toolUnlisten) {
    _toolUnlisten();
    _toolUnlisten = null;
  }
  if (_sessionUnlisten) {
    _sessionUnlisten();
    _sessionUnlisten = null;
  }
}

// ============================================================================
// Types
// ============================================================================

export type ToolCallStatus = 'pending' | 'executing' | 'completed' | 'failed';

export type ToolType = 'Read' | 'Write' | 'Edit' | 'Bash' | 'Glob' | 'Grep' | 'WebFetch' | 'WebSearch' | 'Unknown';

export interface ToolCallParameters {
  // Read tool
  file_path?: string;
  offset?: number;
  limit?: number;
  // Write tool
  content?: string;
  // Edit tool
  old_string?: string;
  new_string?: string;
  replace_all?: boolean;
  // Bash tool
  command?: string;
  timeout?: number;
  description?: string;
  // Glob tool
  pattern?: string;
  path?: string;
  // Grep tool
  output_mode?: string;
  // Generic
  [key: string]: unknown;
}

export interface ToolCallResult {
  success: boolean;
  output?: string;
  error?: string;
  files?: string[];
  content?: string;
  matches?: Array<{ file: string; line: number; content: string }>;
}

export interface ToolCall {
  id: string;
  name: ToolType;
  parameters: ToolCallParameters;
  result?: ToolCallResult;
  status: ToolCallStatus;
  startedAt?: string;
  completedAt?: string;
  duration?: number;
}

export type MessageRole = 'user' | 'assistant' | 'system';

export interface FileAttachment {
  id: string;
  name: string;
  path: string;
  size: number;
  type: 'text' | 'image' | 'pdf' | 'unknown';
  content?: string;
}

export interface FileReference {
  id: string;
  path: string;
  name: string;
}

export interface Message {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  toolCalls?: ToolCall[];
  isStreaming?: boolean;
  attachments?: FileAttachment[];
  references?: FileReference[];
  parentId?: string; // For conversation branching
  branchId?: string; // Branch identifier
  thinkingContent?: string; // Extended thinking content
}

export interface Conversation {
  id: string;
  title: string;
  messages: Message[];
  createdAt: string;
  updatedAt: string;
}

// ============================================================================
// State Interface
// ============================================================================

interface ClaudeCodeState {
  /** Current messages in the active conversation */
  messages: Message[];

  /** Connection status to Claude Code backend */
  connectionStatus: ConnectionStatus;

  /** Whether Claude is currently streaming a response */
  isStreaming: boolean;

  /** Current streaming message content */
  streamingContent: string;

  /** Current thinking content (extended thinking) */
  thinkingContent: string;

  /** Active tool calls being executed */
  activeToolCalls: ToolCall[];

  /** All tool calls from the current conversation */
  toolCallHistory: ToolCall[];

  /** Saved conversations */
  conversations: Conversation[];

  /** Active conversation ID */
  activeConversationId: string | null;

  /** Current session ID (from Rust backend) */
  currentSessionId: string | null;

  /** Is sending a message */
  isSending: boolean;

  /** Error message */
  error: string | null;

  /** Filter for tool history sidebar */
  toolFilter: ToolType | 'all';

  /** Session control state */
  sessionState: 'idle' | 'generating' | 'paused' | 'stopped';

  /** Buffered content when paused */
  bufferedContent: string[];

  /** Workspace files for @ references */
  workspaceFiles: FileReference[];

  // Actions
  /** Initialize Tauri event listeners */
  initialize: (projectPath?: string) => Promise<void>;

  /** Cleanup event listeners */
  cleanup: () => Promise<void>;

  /** Send a user message */
  sendMessage: (content: string) => Promise<void>;

  /** Add a message to the conversation */
  addMessage: (message: Message) => void;

  /** Update streaming content */
  updateStreamingContent: (content: string) => void;

  /** Update thinking content */
  updateThinkingContent: (content: string) => void;

  /** Complete streaming */
  completeStreaming: () => void;

  /** Add a tool call */
  addToolCall: (toolCall: ToolCall) => void;

  /** Update a tool call */
  updateToolCall: (id: string, updates: Partial<ToolCall>) => void;

  /** Clear the current conversation */
  clearConversation: () => void;

  /** Save current conversation */
  saveConversation: (title?: string) => void;

  /** Load a conversation */
  loadConversation: (id: string) => void;

  /** Delete a conversation */
  deleteConversation: (id: string) => void;

  /** Set tool filter */
  setToolFilter: (filter: ToolType | 'all') => void;

  /** Cancel current request */
  cancelRequest: () => Promise<void>;

  /** Reset error state */
  clearError: () => void;

  /** Export conversation */
  exportConversation: (format: 'json' | 'markdown' | 'html') => string;

  /** Update messages (for edit/regenerate) */
  updateMessages: (messages: Message[]) => void;

  /** Pause streaming */
  pauseStreaming: () => void;

  /** Resume streaming */
  resumeStreaming: () => void;

  /** Update a message */
  updateMessage: (id: string, updates: Partial<Message>) => void;

  /** Set workspace files */
  setWorkspaceFiles: (files: FileReference[]) => void;

  /** Fork conversation from a message */
  forkConversation: (messageId: string) => string | null;
}

// ============================================================================
// Helper Functions
// ============================================================================

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function parseToolType(name: string): ToolType {
  const toolTypes: ToolType[] = ['Read', 'Write', 'Edit', 'Bash', 'Glob', 'Grep', 'WebFetch', 'WebSearch'];
  const found = toolTypes.find((t) => name.toLowerCase().includes(t.toLowerCase()));
  return found || 'Unknown';
}

function parseStreamEvent(event: UnifiedStreamEvent): { type: string; data: Record<string, unknown> } {
  switch (event.type) {
    case 'text_delta':
      return { type: 'text_delta', data: { content: event.content } };
    case 'thinking_delta':
      return { type: 'thinking_delta', data: { content: event.content } };
    case 'tool_start': {
      let input = {};
      if (event.arguments) {
        try {
          input = JSON.parse(event.arguments);
        } catch {
          /* keep empty */
        }
      }
      return { type: 'tool_use', data: { tool_use_id: event.tool_id, name: event.tool_name, input } };
    }
    case 'tool_result':
      return {
        type: 'tool_result',
        data: { tool_use_id: event.tool_id, success: !event.error, output: event.result || event.error || '' },
      };
    case 'usage':
      return { type: 'usage', data: { input_tokens: event.input_tokens, output_tokens: event.output_tokens } };
    case 'error':
      return { type: 'error', data: { message: event.message } };
    case 'complete':
      return { type: 'done', data: { stop_reason: event.stop_reason } };
    default:
      return { type: 'unknown', data: {} };
  }
}

// ============================================================================
// Store Implementation
// ============================================================================

const CONVERSATIONS_KEY = 'claude-code-conversations';

export const useClaudeCodeStore = create<ClaudeCodeState>()(
  persist(
    (set, get) => ({
      messages: [],
      connectionStatus: 'disconnected',
      isStreaming: false,
      streamingContent: '',
      thinkingContent: '',
      activeToolCalls: [],
      toolCallHistory: [],
      conversations: [],
      activeConversationId: null,
      currentSessionId: null,
      isSending: false,
      error: null,
      toolFilter: 'all',
      sessionState: 'idle',
      bufferedContent: [],
      workspaceFiles: [],

      initialize: async (projectPath?: string) => {
        const initializeVersion = ++_lifecycleVersion;

        try {
          // Clean up any existing event listeners first
          // (prevents duplicates from React StrictMode double-mount)
          cleanupEventListeners();

          const client = await initClaudeCodeClient();
          if (initializeVersion !== _lifecycleVersion) {
            return;
          }

          // Subscribe to connection status changes
          _statusUnsubscribe = client.onStatusChange((status) => {
            if (initializeVersion !== _lifecycleVersion) {
              return;
            }
            set({ connectionStatus: status });
          });

          // Subscribe to stream events (tracked at module level)
          _streamUnlisten = await client.onStreamEvent((payload: StreamEventPayload) => {
            if (initializeVersion !== _lifecycleVersion) {
              return;
            }
            const { event, session_id } = payload;
            const state = get();

            // Only process events for current session
            if (state.currentSessionId && state.currentSessionId !== session_id) {
              return;
            }

            const parsed = parseStreamEvent(event);

            switch (parsed.type) {
              case 'text_delta': {
                const content = parsed.data.content as string;
                set((state) => {
                  if (state.sessionState === 'paused') {
                    return {
                      bufferedContent: [...state.bufferedContent, content],
                      isStreaming: true,
                      sessionState: 'paused',
                    };
                  }
                  return {
                    streamingContent: state.streamingContent + content,
                    isStreaming: true,
                    sessionState: 'generating',
                  };
                });
                break;
              }
              case 'thinking_delta': {
                const content = parsed.data.content as string;
                set((state) => ({
                  thinkingContent: state.thinkingContent + content,
                }));
                break;
              }
              case 'error': {
                set({
                  error: (parsed.data.message as string) || 'An error occurred',
                  isStreaming: false,
                  isSending: false,
                  sessionState: 'idle',
                });
                break;
              }
              case 'done': {
                const { completeStreaming, addMessage } = get();
                const content = get().streamingContent;
                const thinking = get().thinkingContent;

                completeStreaming();

                if (content || thinking) {
                  const assistantMessage: Message = {
                    id: generateId(),
                    role: 'assistant',
                    content,
                    timestamp: new Date().toISOString(),
                    toolCalls: get().activeToolCalls,
                    thinkingContent: thinking || undefined,
                  };

                  addMessage(assistantMessage);
                }

                set({
                  activeToolCalls: [],
                  isSending: false,
                  sessionState: 'idle',
                  thinkingContent: '',
                });
                break;
              }
            }
          });
          if (initializeVersion !== _lifecycleVersion) {
            cleanupEventListeners();
            return;
          }

          // Subscribe to tool events (tracked at module level)
          _toolUnlisten = await client.onToolEvent((payload: ToolUpdateEvent) => {
            if (initializeVersion !== _lifecycleVersion) {
              return;
            }
            const { execution, update_type, session_id } = payload;
            const state = get();

            if (state.currentSessionId && state.currentSessionId !== session_id) {
              return;
            }

            if (update_type === 'started') {
              const toolCall: ToolCall = {
                id: execution.id,
                name: parseToolType(execution.tool_name),
                parameters: execution.input as ToolCallParameters,
                status: 'executing',
                startedAt: execution.started_at,
              };
              get().addToolCall(toolCall);
            } else if (update_type === 'completed') {
              get().updateToolCall(execution.id, {
                result: {
                  success: execution.success ?? false,
                  output: execution.output ?? undefined,
                },
                status: execution.success ? 'completed' : 'failed',
                completedAt: execution.completed_at ?? new Date().toISOString(),
              });
            }
          });
          if (initializeVersion !== _lifecycleVersion) {
            cleanupEventListeners();
            return;
          }

          // Subscribe to session events (tracked at module level)
          _sessionUnlisten = await client.onSessionEvent((payload: SessionUpdateEvent) => {
            if (initializeVersion !== _lifecycleVersion) {
              return;
            }
            const { session, update_type } = payload;

            if (update_type === 'state_changed') {
              if (session.state === 'error') {
                set({
                  error: session.error_message || 'Session error',
                  isStreaming: false,
                  isSending: false,
                  sessionState: 'idle',
                });
              } else if (session.state === 'cancelled') {
                set({
                  isStreaming: false,
                  isSending: false,
                  sessionState: 'stopped',
                });
              }
            }
          });
          if (initializeVersion !== _lifecycleVersion) {
            cleanupEventListeners();
            return;
          }

          // Start a chat session if project path is provided
          if (projectPath && initializeVersion === _lifecycleVersion) {
            try {
              const response = await client.startChat({ project_path: projectPath });
              if (initializeVersion !== _lifecycleVersion) {
                return;
              }
              set({ currentSessionId: response.session_id });
            } catch (err) {
              console.error('Failed to start chat session:', err);
            }
          }
        } catch (err) {
          if (initializeVersion !== _lifecycleVersion) {
            return;
          }
          console.error('Failed to initialize Claude Code client:', err);
          set({
            connectionStatus: 'disconnected',
            error: err instanceof Error ? err.message : 'Failed to connect',
          });
        }
      },

      cleanup: async () => {
        const cleanupVersion = ++_lifecycleVersion;
        cleanupEventListeners();
        await closeClaudeCodeClient();
        if (cleanupVersion !== _lifecycleVersion) {
          return;
        }
        set({ connectionStatus: 'disconnected', currentSessionId: null });
      },

      sendMessage: async (content: string) => {
        const { connectionStatus, currentSessionId } = get();

        if (connectionStatus !== 'connected') {
          set({ error: 'Not connected to Claude Code backend' });
          return;
        }

        if (!currentSessionId) {
          set({ error: 'No active session' });
          return;
        }

        set({ isSending: true, error: null, streamingContent: '', thinkingContent: '' });

        // Add user message
        const userMessage: Message = {
          id: generateId(),
          role: 'user',
          content,
          timestamp: new Date().toISOString(),
        };

        get().addMessage(userMessage);

        // Send to backend via Tauri IPC
        try {
          const client = getClaudeCodeClient();
          await client.sendMessage(currentSessionId, content);
        } catch (err) {
          set({
            error: err instanceof Error ? err.message : 'Failed to send message',
            isSending: false,
          });
        }
      },

      addMessage: (message: Message) => {
        set((state) => ({
          messages: [...state.messages, message],
        }));
      },

      updateStreamingContent: (content: string) => {
        set({ streamingContent: content });
      },

      updateThinkingContent: (content: string) => {
        set({ thinkingContent: content });
      },

      completeStreaming: () => {
        set({
          isStreaming: false,
          streamingContent: '',
        });
      },

      addToolCall: (toolCall: ToolCall) => {
        set((state) => ({
          activeToolCalls: [...state.activeToolCalls, toolCall],
          toolCallHistory: [...state.toolCallHistory, toolCall],
        }));
      },

      updateToolCall: (id: string, updates: Partial<ToolCall>) => {
        set((state) => ({
          activeToolCalls: state.activeToolCalls.map((tc) => (tc.id === id ? { ...tc, ...updates } : tc)),
          toolCallHistory: state.toolCallHistory.map((tc) => (tc.id === id ? { ...tc, ...updates } : tc)),
        }));
      },

      clearConversation: () => {
        // Auto-save current conversation before clearing
        const state = get();
        if (state.messages.length > 0) {
          state.saveConversation();
        }

        set({
          messages: [],
          toolCallHistory: [],
          activeToolCalls: [],
          streamingContent: '',
          thinkingContent: '',
          isStreaming: false,
          error: null,
          activeConversationId: null,
        });
      },

      saveConversation: (title?: string) => {
        const state = get();
        if (state.messages.length === 0) return;

        const conversationTitle = title || state.messages[0]?.content.slice(0, 50) || 'Untitled';

        const conversation: Conversation = {
          id: state.activeConversationId || generateId(),
          title: conversationTitle,
          messages: state.messages,
          createdAt: state.activeConversationId
            ? state.conversations.find((c) => c.id === state.activeConversationId)?.createdAt ||
              new Date().toISOString()
            : new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };

        set((prevState) => {
          const existing = prevState.conversations.findIndex((c) => c.id === conversation.id);

          let newConversations: Conversation[];
          if (existing >= 0) {
            newConversations = [...prevState.conversations];
            newConversations[existing] = conversation;
          } else {
            newConversations = [conversation, ...prevState.conversations];
          }

          return {
            conversations: newConversations,
            activeConversationId: conversation.id,
          };
        });
      },

      loadConversation: (id: string) => {
        const conversation = get().conversations.find((c) => c.id === id);
        if (!conversation) return;

        // Rebuild tool call history from messages
        const toolCallHistory: ToolCall[] = [];
        conversation.messages.forEach((msg) => {
          if (msg.toolCalls) {
            toolCallHistory.push(...msg.toolCalls);
          }
        });

        set({
          messages: conversation.messages,
          activeConversationId: id,
          toolCallHistory,
          activeToolCalls: [],
          streamingContent: '',
          thinkingContent: '',
          isStreaming: false,
          error: null,
        });
      },

      deleteConversation: (id: string) => {
        set((state) => ({
          conversations: state.conversations.filter((c) => c.id !== id),
          activeConversationId: state.activeConversationId === id ? null : state.activeConversationId,
        }));
      },

      setToolFilter: (filter: ToolType | 'all') => {
        set({ toolFilter: filter });
      },

      cancelRequest: async () => {
        const { currentSessionId } = get();
        if (!currentSessionId) return;

        try {
          const client = getClaudeCodeClient();
          await client.cancelExecution(currentSessionId);
        } catch (err) {
          console.error('Failed to cancel execution:', err);
        }

        set({
          isStreaming: false,
          isSending: false,
          streamingContent: '',
          sessionState: 'stopped',
        });
      },

      clearError: () => {
        set({ error: null });
      },

      exportConversation: (format: 'json' | 'markdown' | 'html') => {
        const { messages, toolCallHistory } = get();

        if (format === 'json') {
          return JSON.stringify(
            {
              messages,
              toolCalls: toolCallHistory,
              exportedAt: new Date().toISOString(),
            },
            null,
            2,
          );
        }

        if (format === 'markdown') {
          let md = '# Claude Code Conversation\n\n';
          md += `*Exported: ${new Date().toLocaleString()}*\n\n---\n\n`;

          messages.forEach((msg) => {
            const roleLabel = msg.role === 'user' ? 'User' : 'Claude';
            md += `## ${roleLabel}\n\n`;
            md += `${msg.content}\n\n`;

            if (msg.thinkingContent) {
              md += '### Thinking\n\n';
              md += `> ${msg.thinkingContent.replace(/\n/g, '\n> ')}\n\n`;
            }

            if (msg.toolCalls && msg.toolCalls.length > 0) {
              md += '### Tool Calls\n\n';
              msg.toolCalls.forEach((tc) => {
                md += `- **${tc.name}**: ${tc.status}\n`;
                if (tc.parameters.file_path) {
                  md += `  - File: \`${tc.parameters.file_path}\`\n`;
                }
                if (tc.parameters.command) {
                  md += `  - Command: \`${tc.parameters.command}\`\n`;
                }
              });
              md += '\n';
            }

            md += '---\n\n';
          });

          return md;
        }

        if (format === 'html') {
          let html = `<!DOCTYPE html>
<html>
<head>
  <title>Claude Code Conversation</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
    .message { margin: 20px 0; padding: 15px; border-radius: 8px; }
    .user { background: #e3f2fd; }
    .assistant { background: #f5f5f5; }
    .role { font-weight: bold; margin-bottom: 10px; }
    .thinking { background: #fff3e0; padding: 10px; margin: 10px 0; border-radius: 4px; font-style: italic; }
    .tool-call { background: #e8f5e9; padding: 10px; margin: 10px 0; border-radius: 4px; font-family: monospace; font-size: 12px; }
    pre { background: #263238; color: #fff; padding: 10px; border-radius: 4px; overflow-x: auto; }
  </style>
</head>
<body>
  <h1>Claude Code Conversation</h1>
  <p><em>Exported: ${new Date().toLocaleString()}</em></p>
  <hr>
`;

          messages.forEach((msg) => {
            const roleLabel = msg.role === 'user' ? 'User' : 'Claude';
            html += `  <div class="message ${msg.role}">
    <div class="role">${roleLabel}</div>
    <div class="content">${msg.content.replace(/\n/g, '<br>')}</div>
`;

            if (msg.thinkingContent) {
              html += `    <div class="thinking">
      <strong>Thinking:</strong><br>
      ${msg.thinkingContent.replace(/\n/g, '<br>')}
    </div>\n`;
            }

            if (msg.toolCalls && msg.toolCalls.length > 0) {
              html += '    <div class="tool-calls">\n';
              msg.toolCalls.forEach((tc) => {
                html += `      <div class="tool-call">
        <strong>${tc.name}</strong> - ${tc.status}
        ${tc.parameters.file_path ? `<br>File: ${tc.parameters.file_path}` : ''}
        ${tc.parameters.command ? `<br>Command: ${tc.parameters.command}` : ''}
      </div>\n`;
              });
              html += '    </div>\n';
            }

            html += '  </div>\n';
          });

          html += `</body>
</html>`;

          return html;
        }

        return '';
      },

      updateMessages: (messages: Message[]) => {
        set({ messages });
      },

      pauseStreaming: () => {
        set({ sessionState: 'paused' });
      },

      resumeStreaming: () => {
        set((state) => ({
          sessionState: 'generating',
          streamingContent: state.streamingContent + state.bufferedContent.join(''),
          bufferedContent: [],
        }));
      },

      updateMessage: (id: string, updates: Partial<Message>) => {
        set((state) => ({
          messages: state.messages.map((msg) => (msg.id === id ? { ...msg, ...updates } : msg)),
        }));
      },

      setWorkspaceFiles: (files: FileReference[]) => {
        set({ workspaceFiles: files });
      },

      forkConversation: (messageId: string) => {
        const state = get();
        const messageIndex = state.messages.findIndex((m) => m.id === messageId);
        if (messageIndex === -1) return null;

        // Create a new branch with messages up to this point
        const branchId = generateId();
        const branchMessages = state.messages.slice(0, messageIndex + 1).map((m) => ({
          ...m,
          branchId,
        }));

        // Create a new conversation for the branch
        const conversation: Conversation = {
          id: branchId,
          title: `Branch from: ${branchMessages[0]?.content.slice(0, 30) || 'Conversation'}...`,
          messages: branchMessages,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };

        set((prevState) => ({
          conversations: [conversation, ...prevState.conversations],
        }));

        return branchId;
      },
    }),
    {
      name: CONVERSATIONS_KEY,
      partialize: (state) => ({
        conversations: state.conversations,
      }),
    },
  ),
);

// Re-export ConnectionStatus for backwards compatibility
export type { ConnectionStatus };

export default useClaudeCodeStore;
