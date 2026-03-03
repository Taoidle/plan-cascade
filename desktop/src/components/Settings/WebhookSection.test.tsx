import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { WebhookChannelConfig, WebhookDelivery } from '../../lib/webhookApi';
import { WebhookSection } from './WebhookSection';

let mockStoreState: ReturnType<typeof createMockStoreState>;

function createChannel(overrides?: Partial<WebhookChannelConfig>): WebhookChannelConfig {
  return {
    id: 'channel-1',
    name: 'Primary Slack',
    channel_type: 'Slack',
    enabled: true,
    url: 'https://hooks.slack.com/services/AAA/BBB/CCC',
    scope: 'Global',
    events: ['TaskComplete', 'TaskFailed'],
    created_at: '2026-03-03T00:00:00Z',
    updated_at: '2026-03-03T00:00:00Z',
    ...overrides,
  };
}

function createDelivery(overrides?: Partial<WebhookDelivery>): WebhookDelivery {
  return {
    id: 'delivery-1',
    channel_id: 'channel-1',
    event_type: 'TaskFailed',
    payload: {
      event_type: 'TaskFailed',
      summary: 'Delivery failed while executing task',
      timestamp: '2026-03-03T00:00:00Z',
    },
    status: 'Failed',
    attempts: 1,
    status_code: 500,
    last_error: 'HTTP 500',
    next_retry_at: '2026-03-03T00:10:00Z',
    last_attempt_at: '2026-03-03T00:00:00Z',
    created_at: '2026-03-03T00:00:00Z',
    ...overrides,
  };
}

function createMockStoreState() {
  return {
    channels: [createChannel()],
    deliveries: [createDelivery()],
    health: {
      worker_running: true,
      failed_queue_length: 1,
      last_retry_at: '2026-03-03T00:00:30Z',
    },
    loadingChannels: false,
    loadingDeliveries: false,
    loadingHealth: false,
    saving: false,
    testingByChannel: {},
    testResultsByChannel: {},
    error: null,
    deliveriesHasMore: true,
    fetchChannels: vi.fn().mockResolvedValue(undefined),
    createChannel: vi.fn().mockResolvedValue(true),
    updateChannel: vi.fn().mockResolvedValue(true),
    setChannelEnabled: vi.fn().mockResolvedValue(true),
    deleteChannel: vi.fn().mockResolvedValue(true),
    testChannel: vi.fn().mockResolvedValue(undefined),
    fetchDeliveries: vi.fn().mockResolvedValue(undefined),
    retryDelivery: vi.fn().mockResolvedValue(true),
    fetchHealth: vi.fn().mockResolvedValue(undefined),
    clearChannelTestResult: vi.fn(),
    clearError: vi.fn(),
  };
}

vi.mock('../../store/webhook', () => ({
  useWebhookStore: () => mockStoreState,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: unknown) => {
      if (typeof options === 'string') {
        return options;
      }
      if (options && typeof options === 'object' && 'defaultValue' in options) {
        const value = String((options as { defaultValue?: string }).defaultValue ?? key);
        return value
          .replace('{{max}}', String((options as { max?: number }).max ?? ''))
          .replace('{{page}}', String((options as { page?: number }).page ?? ''))
          .replace('{{count}}', String((options as { count?: number }).count ?? ''));
      }
      return key;
    },
  }),
}));

describe('WebhookSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState = createMockStoreState();
  });

  it('loads channels, health and deliveries on mount', async () => {
    render(<WebhookSection />);

    await waitFor(() => {
      expect(mockStoreState.fetchChannels).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchHealth).toHaveBeenCalledTimes(1);
      expect(mockStoreState.fetchDeliveries).toHaveBeenCalledWith(undefined, 20, 0);
    });
  });

  it('creates a channel from form input', async () => {
    render(<WebhookSection />);

    fireEvent.click(screen.getByRole('button', { name: /\+ Add/i }));

    fireEvent.change(screen.getByPlaceholderText('e.g., Team Slack'), {
      target: { value: 'Team Alerts' },
    });
    fireEvent.change(screen.getByPlaceholderText('https://hooks.example.com/...'), {
      target: { value: 'https://hooks.slack.com/services/A/B/C' },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() => {
      expect(mockStoreState.createChannel).toHaveBeenCalledWith(
        expect.objectContaining({
          name: 'Team Alerts',
          channel_type: 'Slack',
          url: 'https://hooks.slack.com/services/A/B/C',
        }),
      );
    });
  });

  it('edits, enables, tests and deletes a channel', async () => {
    render(<WebhookSection />);

    fireEvent.click(screen.getByRole('checkbox', { name: 'Primary Slack-enabled' }));
    await waitFor(() => {
      expect(mockStoreState.setChannelEnabled).toHaveBeenCalledWith('channel-1', false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Test' }));
    expect(mockStoreState.testChannel).toHaveBeenCalledWith('channel-1');

    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    fireEvent.change(screen.getByDisplayValue('https://hooks.slack.com/services/AAA/BBB/CCC'), {
      target: { value: 'https://hooks.slack.com/services/NEW/URL/123' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Update' }));

    await waitFor(() => {
      expect(mockStoreState.updateChannel).toHaveBeenCalledWith(
        'channel-1',
        expect.objectContaining({
          url: 'https://hooks.slack.com/services/NEW/URL/123',
        }),
      );
    });

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(mockStoreState.deleteChannel).toHaveBeenCalledWith('channel-1');
  });

  it('retries failed delivery and supports filter/pagination', async () => {
    render(<WebhookSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => {
      expect(mockStoreState.retryDelivery).toHaveBeenCalledWith('delivery-1');
    });

    fireEvent.change(screen.getByRole('combobox'), {
      target: { value: 'channel-1' },
    });

    await waitFor(() => {
      expect(mockStoreState.fetchDeliveries).toHaveBeenCalledWith('channel-1', 20, 0);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => {
      expect(mockStoreState.fetchDeliveries).toHaveBeenCalledWith('channel-1', 20, 20);
    });
  });
});
