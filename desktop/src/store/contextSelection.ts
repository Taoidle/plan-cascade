import { create } from 'zustand';
import type { MemoryScope } from '../types/skillMemory';
import type {
  ContextSelectionSnapshot,
  LegacyContextSelectionInput,
  MemorySelectionMode,
} from '../types/contextSources';

function normalizeMemoryScopes(scopes: MemoryScope[], sessionId: string | null): MemoryScope[] {
  const unique: MemoryScope[] = [];
  for (const scope of scopes) {
    if (!unique.includes(scope)) unique.push(scope);
  }
  const filtered = unique.filter((scope) => scope !== 'session' || !!sessionId?.trim());
  return filtered.length > 0 ? filtered : ['project', 'global'];
}

function toMemorySelectionMode(mode: 'auto' | 'only_selected'): MemorySelectionMode {
  return mode === 'only_selected' ? 'only_selected' : 'auto_exclude';
}

function toIsoNow(): string {
  return new Date().toISOString();
}

const DAILY_STATS_STORAGE_KEY = 'simple-context-selection-daily-stats-v1';
const DAILY_STATS_RETENTION_DAYS = 30;

function toDateKey(date = new Date()): string {
  return date.toISOString().slice(0, 10);
}

function loadDailyStats(): ContextSelectionSnapshot['uiMeta']['dailyStats'] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(DAILY_STATS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (item): item is { date: string; buildCount: number; mismatchCount: number } =>
          !!item &&
          typeof item.date === 'string' &&
          typeof item.buildCount === 'number' &&
          typeof item.mismatchCount === 'number',
      )
      .sort((a, b) => a.date.localeCompare(b.date))
      .slice(-DAILY_STATS_RETENTION_DAYS);
  } catch {
    return [];
  }
}

function persistDailyStats(stats: ContextSelectionSnapshot['uiMeta']['dailyStats']): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(DAILY_STATS_STORAGE_KEY, JSON.stringify(stats));
  } catch {
    // ignore storage errors for non-critical telemetry
  }
}

function bumpDailyStats(
  stats: ContextSelectionSnapshot['uiMeta']['dailyStats'],
  deltas: { build?: number; mismatch?: number },
): ContextSelectionSnapshot['uiMeta']['dailyStats'] {
  const date = toDateKey();
  const next = [...stats];
  const idx = next.findIndex((item) => item.date === date);
  const buildDelta = deltas.build ?? 0;
  const mismatchDelta = deltas.mismatch ?? 0;
  if (idx >= 0) {
    next[idx] = {
      ...next[idx],
      buildCount: next[idx].buildCount + buildDelta,
      mismatchCount: next[idx].mismatchCount + mismatchDelta,
    };
  } else {
    next.push({
      date,
      buildCount: buildDelta,
      mismatchCount: mismatchDelta,
    });
  }
  return next.sort((a, b) => a.date.localeCompare(b.date)).slice(-DAILY_STATS_RETENTION_DAYS);
}

export interface ContextSelectionState extends ContextSelectionSnapshot {
  hydrateFromLegacy: (legacy: LegacyContextSelectionInput, source?: 'legacy' | 'unified') => void;
  patchSessionBinding: (
    activeSessionId: string | null,
    source: ContextSelectionSnapshot['sessionBinding']['source'],
  ) => void;
  recordBuild: () => void;
  recordMismatch: () => void;
  getEffectiveSelection: () => ContextSelectionSnapshot;
}

const defaultSelection: ContextSelectionSnapshot = {
  knowledge: {
    enabled: false,
    selectedCollections: [],
    selectedDocuments: [],
  },
  memory: {
    enabled: true,
    selectionMode: 'auto_exclude',
    selectedScopes: ['global', 'project', 'session'],
    sessionId: null,
    selectedCategories: [],
    selectedMemoryIds: [],
    includedMemoryIds: [],
    excludedMemoryIds: [],
    statuses: [],
    reviewMode: 'active_only',
  },
  skills: {
    enabled: false,
    selectedSkillIds: [],
    selectionMode: 'auto',
  },
  sessionBinding: {
    activeSessionId: null,
    source: 'none',
    updatedAt: null,
  },
  uiMeta: {
    stateSource: 'legacy',
    lastSyncedAt: null,
    mismatchCount: 0,
    buildCount: 0,
    dailyStats: loadDailyStats(),
  },
};

export const useContextSelectionStore = create<ContextSelectionState>()((set, get) => ({
  ...defaultSelection,

  hydrateFromLegacy: (legacy, source = 'legacy') => {
    const trimmedSessionId = legacy.memorySessionId?.trim() || null;
    const normalizedScopes = normalizeMemoryScopes(legacy.selectedMemoryScopes, trimmedSessionId);
    const compatExcluded = legacy.excludedMemoryIds.length > 0 ? legacy.excludedMemoryIds : legacy.selectedMemoryIds;

    set((state) => ({
      knowledge: {
        enabled: legacy.knowledgeEnabled,
        selectedCollections: legacy.selectedCollections,
        selectedDocuments: legacy.selectedDocuments,
      },
      memory: {
        enabled: legacy.memoryEnabled,
        selectionMode: toMemorySelectionMode(legacy.memorySelectionMode),
        selectedScopes: normalizedScopes,
        sessionId: trimmedSessionId,
        selectedCategories: legacy.selectedMemoryCategories,
        selectedMemoryIds: legacy.memorySelectionMode === 'only_selected' ? legacy.includedMemoryIds : compatExcluded,
        includedMemoryIds: legacy.includedMemoryIds,
        excludedMemoryIds: compatExcluded,
        statuses: state.memory.statuses,
        reviewMode: state.memory.reviewMode,
      },
      skills: {
        enabled: legacy.skillsEnabled,
        selectedSkillIds: legacy.selectedSkillIds,
        selectionMode:
          legacy.selectedSkillIds.length > 0
            ? 'explicit'
            : legacy.skillSelectionMode === 'explicit'
              ? 'explicit'
              : 'auto',
      },
      uiMeta: {
        ...state.uiMeta,
        stateSource: source,
        lastSyncedAt: toIsoNow(),
      },
    }));
  },

  patchSessionBinding: (activeSessionId, source) => {
    const normalized = activeSessionId?.trim() || null;
    set((state) => ({
      sessionBinding: {
        activeSessionId: normalized,
        source,
        updatedAt: toIsoNow(),
      },
      memory: {
        ...state.memory,
        sessionId: normalized,
        selectedScopes: normalizeMemoryScopes(state.memory.selectedScopes, normalized),
      },
      uiMeta: {
        ...state.uiMeta,
        stateSource: 'unified',
        lastSyncedAt: toIsoNow(),
      },
    }));
  },

  recordBuild: () => {
    set((state) => {
      const dailyStats = bumpDailyStats(state.uiMeta.dailyStats, { build: 1 });
      persistDailyStats(dailyStats);
      return {
        uiMeta: {
          ...state.uiMeta,
          buildCount: state.uiMeta.buildCount + 1,
          dailyStats,
          lastSyncedAt: toIsoNow(),
        },
      };
    });
  },

  recordMismatch: () => {
    set((state) => {
      const dailyStats = bumpDailyStats(state.uiMeta.dailyStats, { mismatch: 1 });
      persistDailyStats(dailyStats);
      return {
        uiMeta: {
          ...state.uiMeta,
          mismatchCount: state.uiMeta.mismatchCount + 1,
          dailyStats,
          lastSyncedAt: toIsoNow(),
        },
      };
    });
  },

  getEffectiveSelection: () => {
    const state = get();
    const memorySessionId = state.memory.sessionId?.trim() || null;
    const normalizedScopes = normalizeMemoryScopes(state.memory.selectedScopes, memorySessionId);
    const effectiveSelectionMode = state.memory.selectionMode;
    const selectedMemoryIds =
      effectiveSelectionMode === 'only_selected' ? state.memory.includedMemoryIds : state.memory.excludedMemoryIds;

    return {
      ...state,
      memory: {
        ...state.memory,
        sessionId: memorySessionId,
        selectedScopes: normalizedScopes,
        selectedMemoryIds,
      },
    };
  },
}));

export function getEffectiveContextSelection(): ContextSelectionSnapshot {
  return useContextSelectionStore.getState().getEffectiveSelection();
}

export default useContextSelectionStore;
