import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useMcpRegistryController } from './useMcpRegistryController';
import type { McpServer } from '../../types/mcp';

const mockListServers = vi.fn();
const mockListConnectedServers = vi.fn();
const mockListRuntimeInventory = vi.fn();
const mockListen = vi.fn();
const mockShowToast = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

vi.mock('../shared/Toast', () => ({
  useToast: () => ({
    showToast: (...args: unknown[]) => mockShowToast(...args),
  }),
}));

vi.mock('../../lib/mcpApi', () => ({
  mcpApi: {
    listServers: (...args: unknown[]) => mockListServers(...args),
    listConnectedServers: (...args: unknown[]) => mockListConnectedServers(...args),
    listRuntimeInventory: (...args: unknown[]) => mockListRuntimeInventory(...args),
    getConnectedServerTools: vi.fn().mockResolvedValue({ success: true, data: [], error: null }),
    repairRuntime: vi.fn(),
    testServer: vi.fn(),
    toggleServer: vi.fn(),
    connectServer: vi.fn(),
    disconnectServer: vi.fn(),
    removeServer: vi.fn(),
    getServerDetail: vi.fn(),
    exportServers: vi.fn(),
  },
}));

function HookHarness() {
  const controller = useMcpRegistryController();
  const addedServer: McpServer = {
    id: 'new-1',
    name: 'New Server',
    server_type: 'stdio',
    command: 'echo',
    args: [],
    env: {},
    url: null,
    headers: {},
    enabled: true,
    status: 'unknown',
    last_checked: null,
    created_at: null,
    updated_at: null,
  };

  return (
    <button type="button" onClick={() => controller.handleServerAdded(addedServer)}>
      add-server-locally
    </button>
  );
}

describe('useMcpRegistryController fetching behavior', () => {
  it('fetches server list once on initial mount and does not refetch on local server count change', async () => {
    mockListen.mockResolvedValue(() => {});
    mockListServers.mockResolvedValue({ success: true, data: [], error: null });
    mockListConnectedServers.mockResolvedValue({ success: true, data: [], error: null });
    mockListRuntimeInventory.mockResolvedValue({ success: true, data: [], error: null });

    render(<HookHarness />);

    await waitFor(() => {
      expect(mockListServers).toHaveBeenCalledTimes(1);
      expect(mockListConnectedServers).toHaveBeenCalledTimes(1);
      expect(mockListRuntimeInventory).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByRole('button', { name: 'add-server-locally' }));
    expect(mockListServers).toHaveBeenCalledTimes(1);
  });
});
