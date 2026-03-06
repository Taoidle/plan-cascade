import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { FileAttachmentData } from '../types/attachment';
import type {
  ModeViewSnapshot,
  WorkflowMode,
  WorkflowSessionCatalogItem,
  WorkflowSessionCatalogState,
} from '../types/workflowKernel';

type ModeViewMap = Partial<Record<WorkflowMode, ModeViewSnapshot>>;
type DraftMap = Partial<Record<WorkflowMode, string>>;
type AttachmentMap = Partial<Record<WorkflowMode, FileAttachmentData[]>>;
type TranscriptRevisionMap = Partial<Record<WorkflowMode, number>>;

export interface SimpleSessionStore {
  activeRootSessionId: string | null;
  catalog: Record<string, WorkflowSessionCatalogItem>;
  modeViews: Record<string, ModeViewMap>;
  drafts: Record<string, DraftMap>;
  attachmentsByMode: Record<string, AttachmentMap>;
  transcriptRevisions: Record<string, TranscriptRevisionMap>;

  setCatalogState: (state: WorkflowSessionCatalogState) => void;
  upsertCatalogItems: (items: WorkflowSessionCatalogItem[]) => void;
  setActiveRootSessionId: (sessionId: string | null) => void;
  setModeLines: (sessionId: string, mode: WorkflowMode, lines: unknown[]) => void;
  setModeTranscriptSnapshot: (sessionId: string, mode: WorkflowMode, lines: unknown[], revision: number) => void;
  getModeLines: (sessionId: string | null, mode: WorkflowMode) => unknown[];
  setDraft: (sessionId: string, mode: WorkflowMode, value: string) => void;
  getDraft: (sessionId: string | null, mode: WorkflowMode) => string;
  setAttachments: (sessionId: string, mode: WorkflowMode, attachments: FileAttachmentData[]) => void;
  getAttachments: (sessionId: string | null, mode: WorkflowMode) => FileAttachmentData[];
  markModeUnread: (sessionId: string, mode: WorkflowMode, unread: boolean) => void;
  reset: () => void;
}

const DEFAULT_MODE_VIEW = (mode: WorkflowMode): ModeViewSnapshot => ({
  mode,
  lines: [],
  draftInput: '',
  queuedMessages: [],
  attachments: [],
  scrollAnchor: null,
  lastLoadedAt: null,
  hasUnreadBackgroundUpdates: false,
});

function getModeView(state: SimpleSessionStore, sessionId: string, mode: WorkflowMode): ModeViewSnapshot {
  return state.modeViews[sessionId]?.[mode] ?? DEFAULT_MODE_VIEW(mode);
}

const DEFAULT_STATE = {
  activeRootSessionId: null as string | null,
  catalog: {} as Record<string, WorkflowSessionCatalogItem>,
  modeViews: {} as Record<string, ModeViewMap>,
  drafts: {} as Record<string, DraftMap>,
  attachmentsByMode: {} as Record<string, AttachmentMap>,
  transcriptRevisions: {} as Record<string, TranscriptRevisionMap>,
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

      setModeLines: (sessionId, mode, lines) => {
        const cloned = lines.map((line) =>
          line && typeof line === 'object' ? ({ ...(line as Record<string, unknown>) } as unknown) : line,
        );
        set((state) => {
          const previousRevision = state.transcriptRevisions[sessionId]?.[mode] ?? 0;
          return {
            modeViews: {
              ...state.modeViews,
              [sessionId]: {
                ...state.modeViews[sessionId],
                [mode]: {
                  ...getModeView(state, sessionId, mode),
                  lines: cloned,
                  lastLoadedAt: Date.now(),
                },
              },
            },
            transcriptRevisions: {
              ...state.transcriptRevisions,
              [sessionId]: {
                ...state.transcriptRevisions[sessionId],
                [mode]: previousRevision + 1,
              },
            },
          };
        });
      },

      setModeTranscriptSnapshot: (sessionId, mode, lines, revision) => {
        const cloned = lines.map((line) =>
          line && typeof line === 'object' ? ({ ...(line as Record<string, unknown>) } as unknown) : line,
        );
        set((state) => ({
          modeViews: {
            ...state.modeViews,
            [sessionId]: {
              ...state.modeViews[sessionId],
              [mode]: {
                ...getModeView(state, sessionId, mode),
                lines: cloned,
                lastLoadedAt: Date.now(),
              },
            },
          },
          transcriptRevisions: {
            ...state.transcriptRevisions,
            [sessionId]: {
              ...state.transcriptRevisions[sessionId],
              [mode]: revision,
            },
          },
        }));
      },

      getModeLines: (sessionId, mode) => {
        if (!sessionId) return [];
        return (get().modeViews[sessionId]?.[mode]?.lines ?? []).map((line) =>
          line && typeof line === 'object' ? ({ ...(line as Record<string, unknown>) } as unknown) : line,
        );
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
          modeViews: {
            ...state.modeViews,
            [sessionId]: {
              ...state.modeViews[sessionId],
              [mode]: {
                ...getModeView(state, sessionId, mode),
                draftInput: value,
              },
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
          modeViews: {
            ...state.modeViews,
            [sessionId]: {
              ...state.modeViews[sessionId],
              [mode]: {
                ...getModeView(state, sessionId, mode),
                attachments: cloned,
              },
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
          modeViews: {
            ...state.modeViews,
            [sessionId]: {
              ...state.modeViews[sessionId],
              [mode]: {
                ...getModeView(state, sessionId, mode),
                hasUnreadBackgroundUpdates: unread,
              },
            },
          },
        }));
      },

      reset: () => {
        set(DEFAULT_STATE);
      },
    }),
    {
      name: 'simple-session-store-v1',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        activeRootSessionId: state.activeRootSessionId,
        modeViews: state.modeViews,
        drafts: state.drafts,
        attachmentsByMode: state.attachmentsByMode,
        transcriptRevisions: state.transcriptRevisions,
      }),
    },
  ),
);
