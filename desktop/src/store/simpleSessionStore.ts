import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { FileAttachmentData } from '../types/attachment';
import type { WorkflowMode, WorkflowSessionCatalogItem, WorkflowSessionCatalogState } from '../types/workflowKernel';

type DraftMap = Partial<Record<WorkflowMode, string>>;
type AttachmentMap = Partial<Record<WorkflowMode, FileAttachmentData[]>>;
type UnreadMap = Partial<Record<WorkflowMode, boolean>>;

export interface SimpleSessionStore {
  activeRootSessionId: string | null;
  catalog: Record<string, WorkflowSessionCatalogItem>;
  drafts: Record<string, DraftMap>;
  attachmentsByMode: Record<string, AttachmentMap>;
  unreadByMode: Record<string, UnreadMap>;

  setCatalogState: (state: WorkflowSessionCatalogState) => void;
  upsertCatalogItems: (items: WorkflowSessionCatalogItem[]) => void;
  setActiveRootSessionId: (sessionId: string | null) => void;
  setDraft: (sessionId: string, mode: WorkflowMode, value: string) => void;
  getDraft: (sessionId: string | null, mode: WorkflowMode) => string;
  setAttachments: (sessionId: string, mode: WorkflowMode, attachments: FileAttachmentData[]) => void;
  getAttachments: (sessionId: string | null, mode: WorkflowMode) => FileAttachmentData[];
  markModeUnread: (sessionId: string, mode: WorkflowMode, unread: boolean) => void;
  isModeUnread: (sessionId: string | null, mode: WorkflowMode) => boolean;
  reset: () => void;
}

const DEFAULT_STATE = {
  activeRootSessionId: null as string | null,
  catalog: {} as Record<string, WorkflowSessionCatalogItem>,
  drafts: {} as Record<string, DraftMap>,
  attachmentsByMode: {} as Record<string, AttachmentMap>,
  unreadByMode: {} as Record<string, UnreadMap>,
};

export const useSimpleSessionStore = create<SimpleSessionStore>()(
  persist(
    (set, get) => ({
      ...DEFAULT_STATE,

      setCatalogState: (state) => {
        set({
          activeRootSessionId: state.activeSessionId,
          catalog: Object.fromEntries(state.sessions.map((item) => [item.sessionId, item])),
        });
      },

      upsertCatalogItems: (items) => {
        set((state) => ({
          catalog: {
            ...state.catalog,
            ...Object.fromEntries(items.map((item) => [item.sessionId, item])),
          },
        }));
      },

      setActiveRootSessionId: (sessionId) => {
        set({ activeRootSessionId: sessionId });
      },

      setDraft: (sessionId, mode, value) => {
        set((state) => ({
          drafts: {
            ...state.drafts,
            [sessionId]: {
              ...state.drafts[sessionId],
              [mode]: value,
            },
          },
        }));
      },

      getDraft: (sessionId, mode) => {
        if (!sessionId) return '';
        return get().drafts[sessionId]?.[mode] ?? '';
      },

      setAttachments: (sessionId, mode, attachments) => {
        const cloned = attachments.map((item) => ({ ...item }));
        set((state) => ({
          attachmentsByMode: {
            ...state.attachmentsByMode,
            [sessionId]: {
              ...state.attachmentsByMode[sessionId],
              [mode]: cloned,
            },
          },
        }));
      },

      getAttachments: (sessionId, mode) => {
        if (!sessionId) return [];
        return (get().attachmentsByMode[sessionId]?.[mode] ?? []).map((item) => ({ ...item }));
      },

      markModeUnread: (sessionId, mode, unread) => {
        set((state) => ({
          unreadByMode: {
            ...state.unreadByMode,
            [sessionId]: {
              ...state.unreadByMode[sessionId],
              [mode]: unread,
            },
          },
        }));
      },

      isModeUnread: (sessionId, mode) => {
        if (!sessionId) return false;
        return get().unreadByMode[sessionId]?.[mode] ?? false;
      },

      reset: () => {
        set(DEFAULT_STATE);
      },
    }),
    {
      name: 'simple-session-store-v2',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        activeRootSessionId: state.activeRootSessionId,
        drafts: state.drafts,
        attachmentsByMode: state.attachmentsByMode,
        unreadByMode: state.unreadByMode,
      }),
      migrate: (persistedState) => {
        if (!persistedState || typeof persistedState !== 'object') {
          return DEFAULT_STATE;
        }
        const state = persistedState as Record<string, unknown>;
        return {
          activeRootSessionId:
            typeof state.activeRootSessionId === 'string' || state.activeRootSessionId === null
              ? (state.activeRootSessionId as string | null)
              : null,
          drafts: (state.drafts as Record<string, DraftMap> | undefined) ?? {},
          attachmentsByMode: (state.attachmentsByMode as Record<string, AttachmentMap> | undefined) ?? {},
          unreadByMode: (state.unreadByMode as Record<string, UnreadMap> | undefined) ?? {},
          catalog: {},
        };
      },
    },
  ),
);
