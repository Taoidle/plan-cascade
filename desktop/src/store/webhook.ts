/**
 * Webhook Settings Store
 *
 * Zustand store for webhook channel management, delivery history,
 * retry worker health, and channel-level testing state.
 */

import { create } from 'zustand';
import {
  createWebhookChannel,
  deleteWebhookChannel,
  getWebhookDeliveries,
  getWebhookHealth,
  listWebhookChannels,
  retryWebhookDelivery,
  testWebhookChannel,
  updateWebhookChannel,
  type CreateWebhookRequest,
  type UpdateWebhookRequest,
  type WebhookChannelConfig,
  type WebhookDelivery,
  type WebhookHealth,
  type WebhookTestResult,
} from '../lib/webhookApi';

const DEFAULT_DELIVERY_LIMIT = 20;

interface WebhookState {
  channels: WebhookChannelConfig[];
  deliveries: WebhookDelivery[];
  health: WebhookHealth | null;

  loadingChannels: boolean;
  loadingDeliveries: boolean;
  loadingHealth: boolean;
  saving: boolean;
  testingByChannel: Record<string, boolean>;
  testResultsByChannel: Record<string, WebhookTestResult | undefined>;
  error: string | null;

  deliveriesFilterChannelId?: string;
  deliveriesLimit: number;
  deliveriesOffset: number;
  deliveriesHasMore: boolean;

  fetchChannels: () => Promise<void>;
  createChannel: (request: CreateWebhookRequest) => Promise<boolean>;
  updateChannel: (id: string, request: UpdateWebhookRequest) => Promise<boolean>;
  setChannelEnabled: (id: string, enabled: boolean) => Promise<boolean>;
  deleteChannel: (id: string) => Promise<boolean>;
  testChannel: (id: string) => Promise<void>;
  fetchDeliveries: (channelId?: string, limit?: number, offset?: number) => Promise<void>;
  retryDelivery: (deliveryId: string) => Promise<boolean>;
  fetchHealth: () => Promise<void>;
  clearChannelTestResult: (id: string) => void;
  clearError: () => void;
}

function resolveError(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === 'string' && error.trim()) {
    return error;
  }
  return fallback;
}

export const useWebhookStore = create<WebhookState>((set, get) => ({
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
  deliveriesLimit: DEFAULT_DELIVERY_LIMIT,
  deliveriesOffset: 0,
  deliveriesHasMore: false,

  fetchChannels: async () => {
    set({ loadingChannels: true, error: null });
    try {
      const response = await listWebhookChannels();
      if (!response.success || !response.data) {
        set({
          loadingChannels: false,
          error: response.error ?? 'Failed to load webhook channels',
        });
        return;
      }

      set({
        channels: response.data,
        loadingChannels: false,
      });
    } catch (error) {
      set({
        loadingChannels: false,
        error: resolveError(error, 'Failed to load webhook channels'),
      });
    }
  },

  createChannel: async (request) => {
    set({ saving: true, error: null });
    try {
      const response = await createWebhookChannel(request);
      if (!response.success || !response.data) {
        set({
          saving: false,
          error: response.error ?? 'Failed to create webhook channel',
        });
        return false;
      }

      set((state) => ({
        channels: [response.data!, ...state.channels],
        saving: false,
      }));
      return true;
    } catch (error) {
      set({ saving: false, error: resolveError(error, 'Failed to create webhook channel') });
      return false;
    }
  },

  updateChannel: async (id, request) => {
    set({ saving: true, error: null });
    try {
      const response = await updateWebhookChannel(id, request);
      if (!response.success || !response.data) {
        set({
          saving: false,
          error: response.error ?? 'Failed to update webhook channel',
        });
        return false;
      }

      set((state) => ({
        channels: state.channels.map((channel) => (channel.id === id ? response.data! : channel)),
        saving: false,
      }));
      return true;
    } catch (error) {
      set({ saving: false, error: resolveError(error, 'Failed to update webhook channel') });
      return false;
    }
  },

  setChannelEnabled: async (id, enabled) => {
    const current = get().channels.find((channel) => channel.id === id);
    if (!current) {
      return false;
    }

    const previous = current.enabled;
    set((state) => ({
      channels: state.channels.map((channel) => (channel.id === id ? { ...channel, enabled } : channel)),
    }));

    const ok = await get().updateChannel(id, { enabled });
    if (!ok) {
      set((state) => ({
        channels: state.channels.map((channel) => (channel.id === id ? { ...channel, enabled: previous } : channel)),
      }));
    }

    return ok;
  },

  deleteChannel: async (id) => {
    set({ saving: true, error: null });
    try {
      const response = await deleteWebhookChannel(id);
      if (!response.success) {
        set({
          saving: false,
          error: response.error ?? 'Failed to delete webhook channel',
        });
        return false;
      }

      set((state) => {
        const nextTesting = { ...state.testingByChannel };
        const nextResults = { ...state.testResultsByChannel };
        delete nextTesting[id];
        delete nextResults[id];
        return {
          channels: state.channels.filter((channel) => channel.id !== id),
          deliveries: state.deliveries.filter((delivery) => delivery.channel_id !== id),
          testingByChannel: nextTesting,
          testResultsByChannel: nextResults,
          saving: false,
        };
      });
      return true;
    } catch (error) {
      set({ saving: false, error: resolveError(error, 'Failed to delete webhook channel') });
      return false;
    }
  },

  testChannel: async (id) => {
    set((state) => ({
      error: null,
      testingByChannel: { ...state.testingByChannel, [id]: true },
      testResultsByChannel: { ...state.testResultsByChannel, [id]: undefined },
    }));

    try {
      const response = await testWebhookChannel(id);
      set((state) => ({
        testingByChannel: { ...state.testingByChannel, [id]: false },
        testResultsByChannel: {
          ...state.testResultsByChannel,
          [id]:
            response.success && response.data
              ? response.data
              : { success: false, error: response.error ?? 'Test failed' },
        },
      }));
    } catch (error) {
      set((state) => ({
        testingByChannel: { ...state.testingByChannel, [id]: false },
        testResultsByChannel: {
          ...state.testResultsByChannel,
          [id]: { success: false, error: resolveError(error, 'Test failed') },
        },
      }));
    }
  },

  fetchDeliveries: async (channelId, limit = DEFAULT_DELIVERY_LIMIT, offset = 0) => {
    set({ loadingDeliveries: true, error: null });
    try {
      const response = await getWebhookDeliveries(channelId, limit, offset);
      if (!response.success || !response.data) {
        set({
          loadingDeliveries: false,
          error: response.error ?? 'Failed to load webhook deliveries',
        });
        return;
      }

      set({
        deliveries: response.data,
        deliveriesFilterChannelId: channelId,
        deliveriesLimit: limit,
        deliveriesOffset: offset,
        deliveriesHasMore: response.data.length >= limit,
        loadingDeliveries: false,
      });
    } catch (error) {
      set({
        loadingDeliveries: false,
        error: resolveError(error, 'Failed to load webhook deliveries'),
      });
    }
  },

  retryDelivery: async (deliveryId) => {
    set({ saving: true, error: null });
    try {
      const response = await retryWebhookDelivery(deliveryId);
      if (!response.success || !response.data) {
        set({
          saving: false,
          error: response.error ?? 'Failed to retry webhook delivery',
        });
        return false;
      }

      set((state) => ({
        deliveries: state.deliveries.map((delivery) => (delivery.id === deliveryId ? response.data! : delivery)),
        saving: false,
      }));
      return true;
    } catch (error) {
      set({
        saving: false,
        error: resolveError(error, 'Failed to retry webhook delivery'),
      });
      return false;
    }
  },

  fetchHealth: async () => {
    set({ loadingHealth: true, error: null });
    try {
      const response = await getWebhookHealth();
      if (!response.success || !response.data) {
        set({
          loadingHealth: false,
          error: response.error ?? 'Failed to load webhook health',
        });
        return;
      }

      set({
        health: response.data,
        loadingHealth: false,
      });
    } catch (error) {
      set({
        loadingHealth: false,
        error: resolveError(error, 'Failed to load webhook health'),
      });
    }
  },

  clearChannelTestResult: (id) => {
    set((state) => {
      const next = { ...state.testResultsByChannel };
      delete next[id];
      return { testResultsByChannel: next };
    });
  },

  clearError: () => set({ error: null }),
}));
