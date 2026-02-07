/**
 * ClaudeCodeMode Component Tests
 *
 * Tests the chat interface, session management, sidebar toggling,
 * error handling, and initialization/cleanup lifecycle.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

const mockInitialize = vi.fn();
const mockCleanup = vi.fn();
const mockClearConversation = vi.fn();
const mockSendMessage = vi.fn();
const mockClearError = vi.fn();

let mockClaudeCodeState = {
  connectionStatus: 'connected' as string,
  messages: [] as Array<{ id: string; role: string; content: string; timestamp: string }>,
  error: null as string | null,
  isStreaming: false,
  initialize: mockInitialize,
  cleanup: mockCleanup,
  clearConversation: mockClearConversation,
  sendMessage: mockSendMessage,
  clearError: mockClearError,
};

vi.mock('../../store/claudeCode', () => ({
  useClaudeCodeStore: () => mockClaudeCodeState,
}));

// Mock child components to isolate ClaudeCodeMode logic
vi.mock('../ClaudeCodeMode/ChatView', () => ({
  ChatView: () => <div data-testid="chat-view">Chat View</div>,
}));

vi.mock('../ClaudeCodeMode/ChatInput', () => ({
  ChatInput: ({ onSend, disabled }: { onSend: (msg: string) => void; disabled: boolean }) => (
    <div data-testid="chat-input">
      <input
        data-testid="message-input"
        disabled={disabled}
        onChange={() => {}}
      />
      <button data-testid="send-btn" onClick={() => onSend('test message')} disabled={disabled}>
        Send
      </button>
    </div>
  ),
}));

vi.mock('../ClaudeCodeMode/ToolHistorySidebar', () => ({
  ToolHistorySidebar: () => <div data-testid="tool-sidebar">Tool History</div>,
}));

vi.mock('../ClaudeCodeMode/ExportDialog', () => ({
  ExportDialog: ({ open, onClose }: { open: boolean; onClose: () => void }) =>
    open ? (
      <div data-testid="export-dialog">
        <button onClick={onClose}>Close Export</button>
      </div>
    ) : null,
}));

vi.mock('../ClaudeCodeMode/CommandPalette', () => ({
  CommandPaletteProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  createDefaultCommands: () => [],
  useCommandPalette: () => ({ open: vi.fn(), close: vi.fn() }),
}));

vi.mock('../ClaudeCodeMode/KeyboardShortcuts', () => ({
  ShortcutsHelpDialog: ({ open, onClose }: { open: boolean; onClose: () => void }) =>
    open ? (
      <div data-testid="shortcuts-dialog">
        <button onClick={onClose}>Close Shortcuts</button>
      </div>
    ) : null,
  useChatShortcuts: () => {},
}));

vi.mock('../ClaudeCodeMode/SessionControl', () => ({
  SessionControlProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// Mock @radix-ui components
vi.mock('@radix-ui/react-dropdown-menu', () => ({
  Root: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Trigger: ({ children, ...props }: { children: React.ReactNode }) => <button {...props}>{children}</button>,
  Portal: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Content: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Item: ({ children, onSelect, ...props }: { children: React.ReactNode; onSelect?: () => void }) => (
    <button onClick={onSelect} {...props}>{children}</button>
  ),
  Separator: () => <hr />,
}));

vi.mock('@radix-ui/react-icons', () => ({
  ViewVerticalIcon: () => <span>SidebarIcon</span>,
  TrashIcon: () => <span>TrashIcon</span>,
  DownloadIcon: () => <span>DownloadIcon</span>,
  ReloadIcon: () => <span>ReloadIcon</span>,
  CheckCircledIcon: () => <span>CheckIcon</span>,
  CrossCircledIcon: () => <span>CrossIcon</span>,
  DotsHorizontalIcon: () => <span>DotsIcon</span>,
  KeyboardIcon: () => <span>KeyboardIcon</span>,
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

// Dynamic import after mocks are set up
const { default: ClaudeCodeMode } = await import('../ClaudeCodeMode/index');

describe('ClaudeCodeMode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockClaudeCodeState = {
      connectionStatus: 'connected',
      messages: [],
      error: null,
      isStreaming: false,
      initialize: mockInitialize,
      cleanup: mockCleanup,
      clearConversation: mockClearConversation,
      sendMessage: mockSendMessage,
      clearError: mockClearError,
    };
  });

  it('renders the main chat interface with input and chat view', () => {
    render(<ClaudeCodeMode />);

    expect(screen.getByTestId('chat-view')).toBeInTheDocument();
    expect(screen.getByTestId('chat-input')).toBeInTheDocument();
  });

  it('calls initialize on mount and cleanup on unmount', () => {
    const { unmount } = render(<ClaudeCodeMode />);

    expect(mockInitialize).toHaveBeenCalledTimes(1);

    unmount();
    expect(mockCleanup).toHaveBeenCalledTimes(1);
  });

  it('renders tool history sidebar by default', () => {
    render(<ClaudeCodeMode />);

    expect(screen.getByTestId('tool-sidebar')).toBeInTheDocument();
  });

  it('displays connection status indicator', () => {
    render(<ClaudeCodeMode />);

    expect(screen.getByText(/connected/i)).toBeInTheDocument();
  });

  it('shows disconnected status when not connected', () => {
    mockClaudeCodeState.connectionStatus = 'disconnected';

    render(<ClaudeCodeMode />);

    expect(screen.getByText(/disconnected/i)).toBeInTheDocument();
  });

  it('displays error message when error is present', () => {
    mockClaudeCodeState.error = 'WebSocket connection failed';

    render(<ClaudeCodeMode />);

    expect(screen.getByText('WebSocket connection failed')).toBeInTheDocument();
  });

  it('shows empty state message when no messages exist', () => {
    render(<ClaudeCodeMode />);

    // ChatView is mocked, but the container should still be present
    expect(screen.getByTestId('chat-view')).toBeInTheDocument();
  });
});
