import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { FileAttachmentData, WorkspaceFileReferenceData } from '../types/attachment';
import type { WorkflowMode, WorkflowSessionCatalogItem, WorkflowSessionCatalogState } from '../types/workflowKernel';

type DraftMap = Partial<Record<WorkflowMode, string>>;
type AttachmentMap = Partial<Record<WorkflowMode, FileAttachmentData[]>>;
type ReferenceMap = Partial<Record<WorkflowMode, WorkspaceFileReferenceData[]>>;
type UnreadMap = Partial<Record<WorkflowMode, boolean>>;

export interface SimpleSessionStore {
  activeRootSessionId: string | null;
  catalog: Record<string, WorkflowSessionCatalogItem>;
  drafts: Record<string, DraftMap>;
  attachmentsByMode: Record<string, AttachmentMap>;
  referencesByMode: Record<string, ReferenceMap>;
  unreadByMode: Record<string, UnreadMap>;

  setCatalogState: (state: WorkflowSessionCatalogState) => void;
  upsertCatalogItems: (items: WorkflowSessionCatalogItem[]) => void;
  setActiveRootSessionId: (sessionId: string | null) => void;
  setDraft: (sessionId: string, mode: WorkflowMode, value: string) => void;
  getDraft: (sessionId: string | null, mode: WorkflowMode) => string;
  setAttachments: (sessionId: string, mode: WorkflowMode, attachments: FileAttachmentData[]) => void;
  getAttachments: (sessionId: string | null, mode: WorkflowMode) => FileAttachmentData[];
  setReferences: (sessionId: string, mode: WorkflowMode, references: WorkspaceFileReferenceData[]) => void;
  getReferences: (sessionId: string | null, mode: WorkflowMode) => WorkspaceFileReferenceData[];
  markModeUnread: (sessionId: string, mode: WorkflowMode, unread: boolean) => void;
  isModeUnread: (sessionId: string | null, mode: WorkflowMode) => boolean;
  reset: () => void;
}

const DEFAULT_STATE = {
  activeRootSessionId: null as string | null,
  catalog: {} as Record<string, WorkflowSessionCatalogItem>,
  drafts: {} as Record<string, DraftMap>,
  attachmentsByMode: {} as Record<string, AttachmentMap>,
  referencesByMode: {} as Record<string, ReferenceMap>,
  unreadByMode: {} as Record<string, UnreadMap>,
};

function snapshotAttachment(item: FileAttachmentData): FileAttachmentData {
  return {
    id: item.id,
    name: item.name,
    path: item.path,
    size: item.size,
    type: item.type,
    mimeType: item.mimeType,
    isWorkspaceFile: item.isWorkspaceFile,
    isAccessible: item.isAccessible,
  };
}

function snapshotReference(item: WorkspaceFileReferenceData): WorkspaceFileReferenceData {
  return {
    id: item.id,
    name: item.name,
    relativePath: item.relativePath,
    absolutePath: item.absolutePath,
    mentionText: item.mentionText,
  };
}

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
        const cloned = attachments.map((item) => snapshotAttachment(item));
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
        return (get().attachmentsByMode[sessionId]?.[mode] ?? []).map((item) => snapshotAttachment(item));
      },

      setReferences: (sessionId, mode, references) => {
        const cloned = references.map((item) => snapshotReference(item));
        set((state) => ({
          referencesByMode: {
            ...state.referencesByMode,
            [sessionId]: {
              ...state.referencesByMode[sessionId],
              [mode]: cloned,
            },
          },
        }));
      },

      getReferences: (sessionId, mode) => {
        if (!sessionId) return [];
        return (get().referencesByMode[sessionId]?.[mode] ?? []).map((item) => snapshotReference(item));
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
      name: 'simple-session-store-v3',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        activeRootSessionId: state.activeRootSessionId,
        drafts: state.drafts,
        attachmentsByMode: state.attachmentsByMode,
        referencesByMode: state.referencesByMode,
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
          attachmentsByMode: {},
          referencesByMode: (state.referencesByMode as Record<string, ReferenceMap> | undefined) ?? {},
          unreadByMode: (state.unreadByMode as Record<string, UnreadMap> | undefined) ?? {},
          catalog: {},
        };
      },
    },
  ),
);
