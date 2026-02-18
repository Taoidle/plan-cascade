/**
 * Remote Control Store
 *
 * Zustand store for managing remote gateway state.
 */

import { create } from 'zustand';
import {
  getRemoteGatewayStatus,
  startRemoteGateway,
  stopRemoteGateway,
  getRemoteConfig,
  updateRemoteConfig,
  getTelegramConfig,
  updateTelegramConfig,
  listRemoteSessions,
  disconnectRemoteSession,
  getRemoteAuditLog,
  type GatewayStatus,
  type RemoteGatewayConfig,
  type TelegramAdapterConfig,
  type RemoteSessionMapping,
  type AuditLogResponse,
  type UpdateRemoteConfigRequest,
  type UpdateTelegramConfigRequest,
} from '../lib/remoteApi';

interface RemoteState {
  // Gateway status
  gatewayStatus: GatewayStatus | null;

  // Configuration
  remoteConfig: RemoteGatewayConfig | null;
  telegramConfig: TelegramAdapterConfig | null;

  // Sessions and audit
  remoteSessions: RemoteSessionMapping[];
  auditLog: AuditLogResponse | null;

  // UI state
  loading: boolean;
  saving: boolean;
  error: string | null;

  // Actions
  fetchGatewayStatus: () => Promise<void>;
  startGateway: () => Promise<boolean>;
  stopGateway: () => Promise<boolean>;
  fetchConfig: () => Promise<void>;
  saveConfig: (config: UpdateRemoteConfigRequest) => Promise<boolean>;
  fetchTelegramConfig: () => Promise<void>;
  saveTelegramConfig: (config: UpdateTelegramConfigRequest) => Promise<boolean>;
  fetchSessions: () => Promise<void>;
  disconnectSession: (chatId: number) => Promise<boolean>;
  fetchAuditLog: (limit?: number, offset?: number) => Promise<void>;
  clearError: () => void;
}

export const useRemoteStore = create<RemoteState>((set) => ({
  gatewayStatus: null,
  remoteConfig: null,
  telegramConfig: null,
  remoteSessions: [],
  auditLog: null,
  loading: false,
  saving: false,
  error: null,

  fetchGatewayStatus: async () => {
    try {
      const response = await getRemoteGatewayStatus();
      if (response.success && response.data) {
        set({ gatewayStatus: response.data });
      }
    } catch (e) {
      // Silently fail for status polling
    }
  },

  startGateway: async () => {
    set({ saving: true, error: null });
    try {
      const response = await startRemoteGateway();
      if (response.success) {
        // Refresh status
        const statusResp = await getRemoteGatewayStatus();
        if (statusResp.success && statusResp.data) {
          set({ gatewayStatus: statusResp.data, saving: false });
        } else {
          set({ saving: false });
        }
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to start gateway' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  stopGateway: async () => {
    set({ saving: true, error: null });
    try {
      const response = await stopRemoteGateway();
      if (response.success) {
        const statusResp = await getRemoteGatewayStatus();
        if (statusResp.success && statusResp.data) {
          set({ gatewayStatus: statusResp.data, saving: false });
        } else {
          set({ saving: false });
        }
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to stop gateway' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  fetchConfig: async () => {
    set({ loading: true, error: null });
    try {
      const response = await getRemoteConfig();
      if (response.success && response.data) {
        set({ remoteConfig: response.data, loading: false });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load config' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  saveConfig: async (config: UpdateRemoteConfigRequest) => {
    set({ saving: true, error: null });
    try {
      const response = await updateRemoteConfig(config);
      if (response.success) {
        // Refresh config
        const configResp = await getRemoteConfig();
        if (configResp.success && configResp.data) {
          set({ remoteConfig: configResp.data, saving: false });
        } else {
          set({ saving: false });
        }
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to save config' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  fetchTelegramConfig: async () => {
    set({ loading: true, error: null });
    try {
      const response = await getTelegramConfig();
      if (response.success && response.data) {
        set({ telegramConfig: response.data, loading: false });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load Telegram config' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  saveTelegramConfig: async (config: UpdateTelegramConfigRequest) => {
    set({ saving: true, error: null });
    try {
      const response = await updateTelegramConfig(config);
      if (response.success) {
        const tgResp = await getTelegramConfig();
        if (tgResp.success && tgResp.data) {
          set({ telegramConfig: tgResp.data, saving: false });
        } else {
          set({ saving: false });
        }
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to save Telegram config' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  fetchSessions: async () => {
    try {
      const response = await listRemoteSessions();
      if (response.success && response.data) {
        set({ remoteSessions: response.data });
      }
    } catch (e) {
      // Silently fail for sessions polling
    }
  },

  disconnectSession: async (chatId: number) => {
    set({ saving: true, error: null });
    try {
      const response = await disconnectRemoteSession(chatId);
      if (response.success) {
        // Refresh sessions
        const sessResp = await listRemoteSessions();
        if (sessResp.success && sessResp.data) {
          set({ remoteSessions: sessResp.data, saving: false });
        } else {
          set({ saving: false });
        }
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to disconnect session' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  fetchAuditLog: async (limit?: number, offset?: number) => {
    set({ loading: true, error: null });
    try {
      const response = await getRemoteAuditLog(limit, offset);
      if (response.success && response.data) {
        set({ auditLog: response.data, loading: false });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load audit log' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  clearError: () => set({ error: null }),
}));
