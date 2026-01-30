/**
 * Claude Code Store
 *
 * Manages the Claude Code mode state including messages, tool calls,
 * connection status, and conversation history.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { getWebSocketManager, initWebSocket, ConnectionStatus } from '../lib/websocket';

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

  /** Active tool calls being executed */
  activeToolCalls: ToolCall[];

  /** All tool calls from the current conversation */
  toolCallHistory: ToolCall[];

  /** Saved conversations */
  conversations: Conversation[];

  /** Active conversation ID */
  activeConversationId: string | null;

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
  /** Initialize WebSocket connection */
  initialize: () => void;

  /** Cleanup WebSocket connection */
  cleanup: () => void;

  /** Send a user message */
  sendMessage: (content: string) => Promise<void>;

  /** Add a message to the conversation */
  addMessage: (message: Message) => void;

  /** Update streaming content */
  updateStreamingContent: (content: string) => void;

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
  const found = toolTypes.find(t => name.toLowerCase().includes(t.toLowerCase()));
  return found || 'Unknown';
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
      activeToolCalls: [],
      toolCallHistory: [],
      conversations: [],
      activeConversationId: null,
      isSending: false,
      error: null,
      toolFilter: 'all',
      sessionState: 'idle',
      bufferedContent: [],
      workspaceFiles: [],

      initialize: () => {
        const wsManager = initWebSocket();

        // Subscribe to connection status changes
        wsManager.onStatusChange((status) => {
          set({ connectionStatus: status });
        });

        // Subscribe to Claude Code specific events
        wsManager.on('claude_code_response', (data) => {
          const content = data.content as string;
          if (data.streaming) {
            set((state) => {
              // If paused, buffer the content instead of appending
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
          }
        });

        wsManager.on('claude_code_complete', (data) => {
          const { completeStreaming, addMessage } = get();
          const content = data.content as string || get().streamingContent;

          completeStreaming();

          const assistantMessage: Message = {
            id: generateId(),
            role: 'assistant',
            content,
            timestamp: new Date().toISOString(),
            toolCalls: get().activeToolCalls,
          };

          addMessage(assistantMessage);
          set({
            activeToolCalls: [],
            isSending: false,
          });
        });

        wsManager.on('claude_code_tool_call', (data) => {
          const toolCall: ToolCall = {
            id: data.id as string || generateId(),
            name: parseToolType(data.name as string),
            parameters: data.parameters as ToolCallParameters,
            status: 'executing',
            startedAt: new Date().toISOString(),
          };

          get().addToolCall(toolCall);
        });

        wsManager.on('claude_code_tool_result', (data) => {
          const id = data.id as string;
          const result: ToolCallResult = {
            success: data.success as boolean,
            output: data.output as string,
            error: data.error as string,
            files: data.files as string[],
            content: data.content as string,
            matches: data.matches as Array<{ file: string; line: number; content: string }>,
          };

          get().updateToolCall(id, {
            result,
            status: result.success ? 'completed' : 'failed',
            completedAt: new Date().toISOString(),
          });
        });

        wsManager.on('claude_code_error', (data) => {
          set({
            error: data.message as string || 'An error occurred',
            isStreaming: false,
            isSending: false,
          });
        });
      },

      cleanup: () => {
        const wsManager = getWebSocketManager();
        wsManager.disconnect();
      },

      sendMessage: async (content: string) => {
        const { addMessage, connectionStatus } = get();

        if (connectionStatus !== 'connected') {
          set({ error: 'Not connected to Claude Code backend' });
          return;
        }

        set({ isSending: true, error: null, streamingContent: '' });

        // Add user message
        const userMessage: Message = {
          id: generateId(),
          role: 'user',
          content,
          timestamp: new Date().toISOString(),
        };

        addMessage(userMessage);

        // Send to backend via WebSocket
        const wsManager = getWebSocketManager();
        wsManager.send({
          type: 'claude_code_message',
          data: {
            content,
            conversation_id: get().activeConversationId,
          },
        });
      },

      addMessage: (message: Message) => {
        set((state) => ({
          messages: [...state.messages, message],
        }));
      },

      updateStreamingContent: (content: string) => {
        set({ streamingContent: content });
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
          activeToolCalls: state.activeToolCalls.map((tc) =>
            tc.id === id ? { ...tc, ...updates } : tc
          ),
          toolCallHistory: state.toolCallHistory.map((tc) =>
            tc.id === id ? { ...tc, ...updates } : tc
          ),
        }));
      },

      clearConversation: () => {
        set({
          messages: [],
          toolCallHistory: [],
          activeToolCalls: [],
          streamingContent: '',
          isStreaming: false,
          error: null,
          activeConversationId: null,
        });
      },

      saveConversation: (title?: string) => {
        const state = get();
        if (state.messages.length === 0) return;

        const conversationTitle =
          title || state.messages[0]?.content.slice(0, 50) || 'Untitled';

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
          const existing = prevState.conversations.findIndex(
            (c) => c.id === conversation.id
          );

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
          isStreaming: false,
          error: null,
        });
      },

      deleteConversation: (id: string) => {
        set((state) => ({
          conversations: state.conversations.filter((c) => c.id !== id),
          activeConversationId:
            state.activeConversationId === id ? null : state.activeConversationId,
        }));
      },

      setToolFilter: (filter: ToolType | 'all') => {
        set({ toolFilter: filter });
      },

      cancelRequest: async () => {
        const wsManager = getWebSocketManager();
        wsManager.send({
          type: 'claude_code_cancel',
          data: {},
        });

        set({
          isStreaming: false,
          isSending: false,
          streamingContent: '',
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
            2
          );
        }

        if (format === 'markdown') {
          let md = '# Claude Code Conversation\n\n';
          md += `*Exported: ${new Date().toLocaleString()}*\n\n---\n\n`;

          messages.forEach((msg) => {
            const roleLabel = msg.role === 'user' ? 'User' : 'Claude';
            md += `## ${roleLabel}\n\n`;
            md += `${msg.content}\n\n`;

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
    .tool-call { background: #fff3e0; padding: 10px; margin: 10px 0; border-radius: 4px; font-family: monospace; font-size: 12px; }
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
          messages: state.messages.map((msg) =>
            msg.id === id ? { ...msg, ...updates } : msg
          ),
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
    }
  )
);

export default useClaudeCodeStore;
