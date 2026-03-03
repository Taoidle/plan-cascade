import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWebhookStore } from './webhook';
import type {
  WebhookChannelConfig,
  WebhookDelivery,
  WebhookHealth,
} from '../lib/webhookApi';

const {
  mockListWebhookChannels,
  mockCreateWebhookChannel,
  mockUpdateWebhookChannel,
  mockDeleteWebhookChannel,
  mockTestWebhookChannel,
  mockGetWebhookDeliveries,
  mockRetryWebhookDelivery,
  mockGetWebhookHealth,
} = vi.hoisted(() => ({
  mockListWebhookChannels: vi.fn(),
  mockCreateWebhookChannel: vi.fn(),
  mockUpdateWebhookChannel: vi.fn(),
  mockDeleteWebhookChannel: vi.fn(),
  mockTestWebhookChannel: vi.fn(),
  mockGetWebhookDeliveries: vi.fn(),
  mockRetryWebhookDelivery: vi.fn(),
  mockGetWebhookHealth: vi.fn(),
}));

vi.mock('../lib/webhookApi', () => ({
  listWebhookChannels: mockListWebhookChannels,
  createWebhookChannel: mockCreateWebhookChannel,
  updateWebhookChannel: mockUpdateWebhookChannel,
  deleteWebhookChannel: mockDeleteWebhookChannel,
  testWebhookChannel: mockTestWebhookChannel,
  getWebhookDeliveries: mockGetWebhookDeliveries,
  retryWebhookDelivery: mockRetryWebhookDelivery,
  getWebhookHealth: mockGetWebhookHealth,
}));

function resetStore() {
  useWebhookStore.setState((state) => ({
    ...state,
    channels: [],
    deliveries: [],
    health: null,
    loadingChannels: false,
    loadingDeliveries: false,
    loadingHealth: false,
    saving: false,
    testingByChannel: {},
    testResultsByChannel: {},
    error: null,
    deliveriesFilterChannelId: undefined,
    deliveriesLimit: 20,
    deliveriesOffset: 0,
    deliveriesHasMore: false,
  }));
}

function createChannel(id: string, overrides?: Partial<WebhookChannelConfig>): WebhookChannelConfig {
  return {
    id,
    name: `channel-${id}`,
    channel_type: 'Slack',
    enabled: true,
    url: 'https://hooks.slack.com/services/AAA',
    scope: 'Global',
    events: ['TaskComplete'],
    created_at: '2026-03-03T00:00:00Z',
    updated_at: '2026-03-03T00:00:00Z',
    ...overrides,
  };
}

function createDelivery(id: string, overrides?: Partial<WebhookDelivery>): WebhookDelivery {
  return {
    id,
    channel_id: 'channel-1',
    event_type: 'TaskFailed',
    payload: {
      event_type: 'TaskFailed',
      summary: 'failed summary',
      timestamp: '2026-03-03T00:00:00Z',
    },
    status: 'Failed',
    attempts: 1,
    last_attempt_at: '2026-03-03T00:00:00Z',
    created_at: '2026-03-03T00:00:00Z',
    ...overrides,
  };
}

describe('useWebhookStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('loads channels successfully', async () => {
    const channels = [createChannel('channel-1')];
    mockListWebhookChannels.mockResolvedValue({ success: true, data: channels, error: null });

    await useWebhookStore.getState().fetchChannels();

    const state = useWebhookStore.getState();
    expect(state.channels).toEqual(channels);
    expect(state.loadingChannels).toBe(false);
    expect(state.error).toBeNull();
  });

  it('supports optimistic enabled toggle and persists via update API', async () => {
    useWebhookStore.setState((state) => ({
      ...state,
      channels: [createChannel('channel-1', { enabled: false })],
    }));

    mockUpdateWebhookChannel.mockResolvedValue({
      success: true,
      data: createChannel('channel-1', { enabled: true }),
      error: null,
    });

    const ok = await useWebhookStore.getState().setChannelEnabled('channel-1', true);

    expect(ok).toBe(true);
    expect(mockUpdateWebhookChannel).toHaveBeenCalledWith('channel-1', { enabled: true });
    expect(useWebhookStore.getState().channels[0].enabled).toBe(true);
  });

  it('tracks test loading per-channel and stores result', async () => {
    mockTestWebhookChannel.mockResolvedValue({
      success: true,
      data: { success: true, latency_ms: 42 },
      error: null,
    });

    const promise = useWebhookStore.getState().testChannel('channel-1');

    expect(useWebhookStore.getState().testingByChannel['channel-1']).toBe(true);

    await promise;

    const state = useWebhookStore.getState();
    expect(state.testingByChannel['channel-1']).toBe(false);
    expect(state.testResultsByChannel['channel-1']).toEqual({ success: true, latency_ms: 42 });
  });

  it('loads deliveries with filter and pagination metadata', async () => {
    const deliveries = Array.from({ length: 20 }).map((_, index) => createDelivery(`delivery-${index}`));
    mockGetWebhookDeliveries.mockResolvedValue({ success: true, data: deliveries, error: null });

    await useWebhookStore.getState().fetchDeliveries('channel-1', 20, 20);

    const state = useWebhookStore.getState();
    expect(mockGetWebhookDeliveries).toHaveBeenCalledWith('channel-1', 20, 20);
    expect(state.deliveries).toHaveLength(20);
    expect(state.deliveriesFilterChannelId).toBe('channel-1');
    expect(state.deliveriesOffset).toBe(20);
    expect(state.deliveriesHasMore).toBe(true);
  });

  it('retries delivery with returned original payload status', async () => {
    useWebhookStore.setState((state) => ({
      ...state,
      deliveries: [createDelivery('delivery-1', { status: 'Failed', attempts: 1 })],
    }));

    mockRetryWebhookDelivery.mockResolvedValue({
      success: true,
      data: createDelivery('delivery-1', {
        status: 'Retrying',
        attempts: 2,
        next_retry_at: '2026-03-03T00:10:00Z',
      }),
      error: null,
    });

    const ok = await useWebhookStore.getState().retryDelivery('delivery-1');

    expect(ok).toBe(true);
    expect(useWebhookStore.getState().deliveries[0].attempts).toBe(2);
    expect(useWebhookStore.getState().deliveries[0].next_retry_at).toBe('2026-03-03T00:10:00Z');
  });

  it('loads worker health successfully', async () => {
    const health: WebhookHealth = {
      worker_running: true,
      failed_queue_length: 7,
      last_retry_at: '2026-03-03T00:00:30Z',
    };
    mockGetWebhookHealth.mockResolvedValue({ success: true, data: health, error: null });

    await useWebhookStore.getState().fetchHealth();

    const state = useWebhookStore.getState();
    expect(state.health).toEqual(health);
    expect(state.loadingHealth).toBe(false);
  });

  it('rolls back optimistic toggle when update fails', async () => {
    useWebhookStore.setState((state) => ({
      ...state,
      channels: [createChannel('channel-1', { enabled: true })],
    }));

    mockUpdateWebhookChannel.mockResolvedValue({ success: false, data: null, error: 'update failed' });

    const ok = await useWebhookStore.getState().setChannelEnabled('channel-1', false);

    expect(ok).toBe(false);
    expect(useWebhookStore.getState().channels[0].enabled).toBe(true);
    expect(useWebhookStore.getState().error).toBe('update failed');
  });
});
