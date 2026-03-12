import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type {
  GatewayStatus,
  RemoteGatewayConfig,
  RemoteSessionMapping,
  TelegramAdapterConfig,
} from '../../lib/remoteApi';
import { RemoteSection } from './RemoteSection';

let mockStoreState: ReturnType<typeof createMockStoreState>;
let mockWorkflowKernelState: ReturnType<typeof createMockWorkflowKernelState>;
let mockSettingsState: ReturnType<typeof createMockSettingsState>;

function createGatewayStatus(overrides?: Partial<GatewayStatus>): GatewayStatus {
  return {
    running: false,
    adapter_type: 'Telegram',
    connected_since: null,
    active_remote_sessions: 0,
    total_commands_processed: 0,
    last_command_at: null,
    error: null,
    reconnect_attempts: 0,
    last_error_at: null,
    reconnecting: false,
    ...overrides,
  };
}

function createRemoteConfig(overrides?: Partial<RemoteGatewayConfig>): RemoteGatewayConfig {
  return {
    enabled: true,
    adapter: 'Telegram',
    auto_start: false,
    allowed_project_roots: [{ path: '/tmp/project-a', label: 'Project A' }],
    ...overrides,
  };
}

function createTelegramConfig(overrides?: Partial<TelegramAdapterConfig>): TelegramAdapterConfig {
  return {
    bot_token: '***',
    allowed_chat_ids: [123],
    allowed_user_ids: [456],
    require_password: false,
    access_password: null,
    max_message_length: 4000,
    streaming_mode: 'WaitForComplete',
    ...overrides,
  };
}

function createMockStoreState() {
  return {
    gatewayStatus: createGatewayStatus(),
    remoteConfig: createRemoteConfig(),
    telegramConfig: createTelegramConfig(),
    remoteSessions: [] as RemoteSessionMapping[],
    auditLog: { entries: [], total: 0 },
    saving: false,
    error: null,
    fetchGatewayStatus: vi.fn().mockResolvedValue(undefined),
    startGateway: vi.fn().mockResolvedValue(true),
    stopGateway: vi.fn().mockResolvedValue(true),
    fetchConfig: vi.fn().mockResolvedValue(undefined),
    saveConfig: vi.fn().mockResolvedValue({ applied: true, restart_required: false }),
    fetchTelegramConfig: vi.fn().mockResolvedValue(undefined),
    saveTelegramConfig: vi.fn().mockResolvedValue({ applied: true, restart_required: false }),
    fetchSessions: vi.fn().mockResolvedValue(undefined),
    disconnectSession: vi.fn().mockResolvedValue(true),
    fetchAuditLog: vi.fn().mockResolvedValue(undefined),
    clearError: vi.fn(),
  };
}

function createMockWorkflowKernelState() {
  return {
    sessionCatalog: [
      {
        sessionId: 'session-1',
        sessionKind: 'live',
        displayTitle: 'Session Project',
        workspacePath: '/tmp/project-b',
        activeMode: 'chat',
        status: 'idle',
        backgroundState: 'foreground',
        updatedAt: '2026-03-12T12:00:00Z',
        createdAt: '2026-03-12T11:00:00Z',
        lastError: null,
        contextLedger: {
          conversationTurnCount: 0,
          artifactRefCount: 0,
          contextSourceKinds: [],
          lastCompactionAt: null,
          ledgerVersion: 1,
        },
        modeSnapshots: {
          chat: null,
          plan: null,
          task: null,
          debug: null,
        },
        modeRuntimeMeta: {},
      },
    ],
    getSessionCatalogState: vi.fn().mockResolvedValue({
      activeSessionId: 'session-1',
      sessions: [
        {
          sessionId: 'session-1',
          sessionKind: 'live',
          displayTitle: 'Session Project',
          workspacePath: '/tmp/project-b',
          activeMode: 'chat',
          status: 'idle',
          backgroundState: 'foreground',
          updatedAt: '2026-03-12T12:00:00Z',
          createdAt: '2026-03-12T11:00:00Z',
          lastError: null,
          contextLedger: {
            conversationTurnCount: 0,
            artifactRefCount: 0,
            contextSourceKinds: [],
            lastCompactionAt: null,
            ledgerVersion: 1,
          },
          modeSnapshots: {
            chat: null,
            plan: null,
            task: null,
            debug: null,
          },
          modeRuntimeMeta: {},
        },
      ],
    }),
  };
}

function createMockSettingsState() {
  return {
    workspacePath: '/tmp/project-current',
  };
}

vi.mock('../../store/remote', () => ({
  useRemoteStore: () => mockStoreState,
}));

vi.mock('../../store/workflowKernel', () => ({
  useWorkflowKernelStore: (selector: (state: ReturnType<typeof createMockWorkflowKernelState>) => unknown) =>
    selector(mockWorkflowKernelState),
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector: (state: ReturnType<typeof createMockSettingsState>) => unknown) =>
    selector(mockSettingsState),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValueOrOptions?: unknown) => {
      if (
        defaultValueOrOptions &&
        typeof defaultValueOrOptions === 'object' &&
        'defaultValue' in defaultValueOrOptions &&
        typeof (defaultValueOrOptions as { defaultValue?: unknown }).defaultValue === 'string'
      ) {
        return (defaultValueOrOptions as { defaultValue: string }).defaultValue.replace(
          '{{count}}',
          String((defaultValueOrOptions as { count?: number }).count ?? ''),
        );
      }
      if (typeof defaultValueOrOptions === 'string') {
        return defaultValueOrOptions;
      }
      return key;
    },
  }),
}));

describe('RemoteSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState = createMockStoreState();
    mockWorkflowKernelState = createMockWorkflowKernelState();
    mockSettingsState = createMockSettingsState();
  });

  it('loads remote data on mount and saves allowed project roots', async () => {
    render(<RemoteSection />);

    await waitFor(() => {
      expect(mockStoreState.fetchGatewayStatus).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchConfig).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchTelegramConfig).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchSessions).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchAuditLog).toHaveBeenCalledWith(20);
    });

    fireEvent.change(screen.getByPlaceholderText('Absolute path...'), {
      target: { value: '/srv/project-b' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add workspace' }));
    fireEvent.click(screen.getAllByRole('button', { name: 'Save' })[0]);

    await waitFor(() => {
      expect(mockStoreState.saveConfig).toHaveBeenCalledWith({
        enabled: true,
        auto_start: false,
        allowed_project_roots: [
          { path: '/tmp/project-a', label: 'Project A' },
          {
            path: '/srv/project-b',
            label: null,
            default_provider: null,
            default_model: null,
          },
        ],
      });
    });
  });

  it('builds structured streaming mode payloads', async () => {
    render(<RemoteSection />);

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'LiveEdit' } });
    fireEvent.change(screen.getByDisplayValue('1200'), { target: { value: '1500' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Save' })[1]);

    await waitFor(() => {
      expect(mockStoreState.saveTelegramConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          streaming_mode: { LiveEdit: { throttle_ms: 1500 } },
        }),
      );
    });
  });

  it('shows reconnecting status and restart-required save message', async () => {
    mockStoreState.gatewayStatus = createGatewayStatus({
      reconnecting: true,
      reconnect_attempts: 2,
      error: 'Connection lost',
      last_error_at: '2026-03-12T12:00:00Z',
    });
    mockStoreState.saveTelegramConfig = vi.fn().mockResolvedValue({
      applied: true,
      restart_required: true,
    });

    render(<RemoteSection />);

    expect(screen.getByText('Reconnecting...')).toBeInTheDocument();
    expect(screen.getByText('Connection lost')).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Save' })[1]);

    await waitFor(() => {
      expect(screen.getByText('Configuration saved. Restart the gateway to apply all changes.')).toBeInTheDocument();
    });
  });

  it('imports current workspace and open session workspaces without duplicates', async () => {
    mockStoreState.remoteConfig = createRemoteConfig({
      allowed_project_roots: [{ path: '/tmp/project-a', label: 'Project A' }],
    });

    render(<RemoteSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Import Open Workspaces' }));

    await waitFor(() => {
      expect(mockWorkflowKernelState.getSessionCatalogState).toHaveBeenCalledTimes(1);
    });

    expect(screen.getByText('/tmp/project-current')).toBeInTheDocument();
    expect(screen.getByText('/tmp/project-b')).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Save' })[0]);

    await waitFor(() => {
      expect(mockStoreState.saveConfig).toHaveBeenCalledWith({
        enabled: true,
        auto_start: false,
        allowed_project_roots: [
          { path: '/tmp/project-a', label: 'Project A' },
          {
            path: '/tmp/project-current',
            label: 'project-current',
            default_provider: null,
            default_model: null,
          },
          {
            path: '/tmp/project-b',
            label: 'Session Project',
            default_provider: null,
            default_model: null,
          },
        ],
      });
    });

    fireEvent.click(screen.getByRole('button', { name: 'Import Open Workspaces' }));

    await waitFor(() => {
      expect(screen.getByText('All open workspaces are already included.')).toBeInTheDocument();
    });
  });

  it('renders remote session enum variants without crashing', async () => {
    mockStoreState.remoteSessions = [
      {
        chat_id: 1001,
        user_id: 2001,
        local_session_id: 'session-standalone',
        session_type: {
          Standalone: {
            provider: 'minimax',
            model: 'MiniMax-M2.5',
          },
        },
        created_at: '2026-03-12T12:00:00Z',
        project_path: '/tmp/project-a',
      },
      {
        chat_id: 1002,
        user_id: 2002,
        local_session_id: 'session-workflow',
        session_type: {
          WorkflowRoot: {
            kernel_session_id: 'kernel-123',
            active_mode: 'Chat',
          },
        },
        created_at: '2026-03-12T12:05:00Z',
        project_path: '/tmp/project-b',
      },
    ];

    render(<RemoteSection />);

    expect(screen.getByText(/Type: Standalone\(minimax\/MiniMax-M2\.5\)/)).toBeInTheDocument();
    expect(screen.getByText(/Type: Workflow\(Chat\/kernel-123\)/)).toBeInTheDocument();
  });
});
