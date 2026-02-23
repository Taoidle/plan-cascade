/**
 * Tool Permission Store
 *
 * Zustand store managing the tool permission approval queue
 * and session-level permission settings.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  PermissionLevel,
  ToolPermissionRequest,
  PermissionResponseType,
} from '../types/permission';

interface ToolPermissionState {
  /** Current session's permission level (default: standard) */
  sessionLevel: PermissionLevel;
  /** Currently displayed permission request (head of queue) */
  pendingRequest: ToolPermissionRequest | null;
  /** Queued permission requests waiting to be displayed */
  requestQueue: ToolPermissionRequest[];
  /** Whether a response is being sent */
  isResponding: boolean;

  /** Set the permission level for a session */
  setSessionLevel: (sessionId: string, level: PermissionLevel) => Promise<void>;
  /** Enqueue a new permission request from a stream event */
  enqueueRequest: (request: ToolPermissionRequest) => void;
  /** Respond to the current permission request */
  respond: (requestId: string, response: PermissionResponseType) => Promise<void>;
  /** Clear all pending requests (on cancel/reset) */
  clearAll: () => void;
  /** Reset to default state */
  reset: () => void;
}

export const useToolPermissionStore = create<ToolPermissionState>((set, get) => ({
  sessionLevel: 'standard',
  pendingRequest: null,
  requestQueue: [],
  isResponding: false,

  setSessionLevel: async (sessionId: string, level: PermissionLevel) => {
    try {
      await invoke('set_session_permission_level', {
        request: { session_id: sessionId, level },
      });
      set({ sessionLevel: level });
    } catch (e) {
      console.error('[toolPermission] Failed to set session level:', e);
    }
  },

  enqueueRequest: (request: ToolPermissionRequest) => {
    const { pendingRequest, requestQueue } = get();
    if (!pendingRequest) {
      // No current request — show immediately
      set({ pendingRequest: request });
    } else {
      // Already showing one — queue it
      set({ requestQueue: [...requestQueue, request] });
    }
  },

  respond: async (requestId: string, response: PermissionResponseType) => {
    set({ isResponding: true });
    try {
      await invoke('respond_tool_permission', {
        request: {
          request_id: requestId,
          allowed: response === 'allow' || response === 'allow_always',
          always_allow: response === 'allow_always',
        },
      });
    } catch (e) {
      console.error('[toolPermission] Failed to respond:', e);
    }

    // Dequeue next request
    const { requestQueue } = get();
    if (requestQueue.length > 0) {
      const [next, ...rest] = requestQueue;
      set({
        pendingRequest: next,
        requestQueue: rest,
        isResponding: false,
      });
    } else {
      set({
        pendingRequest: null,
        isResponding: false,
      });
    }
  },

  clearAll: () => {
    set({
      pendingRequest: null,
      requestQueue: [],
      isResponding: false,
    });
  },

  reset: () => {
    set({
      sessionLevel: 'standard',
      pendingRequest: null,
      requestQueue: [],
      isResponding: false,
    });
  },
}));
