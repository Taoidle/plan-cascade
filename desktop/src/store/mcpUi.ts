import { create } from 'zustand';

export type McpUiIntentAction =
  | 'open-add'
  | 'open-import'
  | 'open-discover'
  | 'install-recommended'
  | 'refresh'
  | 'test-enabled'
  | 'export';

interface McpUiIntent {
  id: number;
  action: McpUiIntentAction;
}

interface McpUiState {
  lastIntent: McpUiIntent | null;
  dispatchIntent: (action: McpUiIntentAction) => void;
  clearIntent: (id: number) => void;
}

let nextIntentId = 1;

export const useMcpUiStore = create<McpUiState>((set, get) => ({
  lastIntent: null,
  dispatchIntent: (action) => {
    const intent: McpUiIntent = {
      id: nextIntentId++,
      action,
    };
    set({ lastIntent: intent });
  },
  clearIntent: (id) => {
    const current = get().lastIntent;
    if (current?.id === id) {
      set({ lastIntent: null });
    }
  },
}));

export function dispatchMcpUiIntent(action: McpUiIntentAction): void {
  useMcpUiStore.getState().dispatchIntent(action);
}
