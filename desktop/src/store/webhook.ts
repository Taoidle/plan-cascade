/**
 * Webhook Settings Store
 *
 * Zustand store for managing webhook notification channel state.
 * Follows the same pattern as the proxy store.
 */

import { create } from 'zustand';
import {
  listWebhookChannels,
  createWebhookChannel,
  updateWebhookChannel,
  deleteWebhookChannel,
  testWebhookChannel,
  getWebhookDeliveries,
  retryWebhookDelivery,
  type WebhookChannelConfig,
  type WebhookDelivery,
  type WebhookTestResult,
  type CreateWebhookRequest,
  type UpdateWebhookRequest,
} from '../lib/webhookApi';

interface WebhookState {
  // Data
  channels: WebhookChannelConfig[];
  deliveries: WebhookDelivery[];

  // UI state
  loading: boolean;
  saving: boolean;
  testing: boolean;
  testResult: WebhookTestResult | null;
  error: string | null;

  // Actions
  fetchChannels: () => Promise<void>;
  createChannel: (request: CreateWebhookRequest) => Promise<boolean>;
  updateChannel: (id: string, request: UpdateWebhookRequest) => Promise<boolean>;
  deleteChannel: (id: string) => Promise<boolean>;
  testChannel: (id: string) => Promise<void>;
  fetchDeliveries: (channelId?: string, limit?: number, offset?: number) => Promise<void>;
  retryDelivery: (deliveryId: string) => Promise<boolean>;
  clearTestResult: () => void;
  clearError: () => void;
}

export const useWebhookStore = create<WebhookState>((set, _get) => ({
  channels: [],
  deliveries: [],
  loading: false,
  saving: false,
  testing: false,
  testResult: null,
  error: null,

  fetchChannels: async () => {
    set({ loading: true, error: null });
    try {
      const response = await listWebhookChannels();
      if (response.success && response.data) {
        set({ channels: response.data, loading: false });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load webhook channels' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  createChannel: async (request) => {
    set({ saving: true, error: null });
    try {
      const response = await createWebhookChannel(request);
      if (response.success && response.data) {
        set((state) => ({
          channels: [response.data!, ...state.channels],
          saving: false,
        }));
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to create channel' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  updateChannel: async (id, request) => {
    set({ saving: true, error: null });
    try {
      const response = await updateWebhookChannel(id, request);
      if (response.success && response.data) {
        set((state) => ({
          channels: state.channels.map((ch) =>
            ch.id === id ? response.data! : ch
          ),
          saving: false,
        }));
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to update channel' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  deleteChannel: async (id) => {
    set({ saving: true, error: null });
    try {
      const response = await deleteWebhookChannel(id);
      if (response.success) {
        set((state) => ({
          channels: state.channels.filter((ch) => ch.id !== id),
          saving: false,
        }));
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to delete channel' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  testChannel: async (id) => {
    set({ testing: true, testResult: null });
    try {
      const response = await testWebhookChannel(id);
      if (response.success && response.data) {
        set({ testing: false, testResult: response.data });
      } else {
        set({
          testing: false,
          testResult: {
            success: false,
            error: response.error ?? 'Test failed',
          },
        });
      }
    } catch (e) {
      set({
        testing: false,
        testResult: {
          success: false,
          error: e instanceof Error ? e.message : String(e),
        },
      });
    }
  },

  fetchDeliveries: async (channelId, limit = 50, offset = 0) => {
    set({ loading: true, error: null });
    try {
      const response = await getWebhookDeliveries(channelId, limit, offset);
      if (response.success && response.data) {
        set({ deliveries: response.data, loading: false });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load deliveries' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  retryDelivery: async (deliveryId) => {
    set({ saving: true, error: null });
    try {
      const response = await retryWebhookDelivery(deliveryId);
      if (response.success && response.data) {
        set((state) => ({
          deliveries: state.deliveries.map((d) =>
            d.id === deliveryId ? response.data! : d
          ),
          saving: false,
        }));
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to retry delivery' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  clearTestResult: () => set({ testResult: null }),
  clearError: () => set({ error: null }),
}));
