/**
 * Mode Store
 *
 * Manages the application mode (simple/expert/claude-code) state.
 * Persists to localStorage for session continuity.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Mode = 'simple' | 'expert' | 'claude-code';

/** All available modes in order */
export const MODES: Mode[] = ['simple', 'expert', 'claude-code'];

/** Mode display names */
export const MODE_LABELS: Record<Mode, string> = {
  simple: 'Simple',
  expert: 'Expert',
  'claude-code': 'Claude Code',
};

/** Mode descriptions */
export const MODE_DESCRIPTIONS: Record<Mode, string> = {
  simple: 'One-click execution with AI-driven automation',
  expert: 'Full control over PRD editing, agents, and execution',
  'claude-code': 'Interactive chat with Claude Code CLI',
};

interface ModeState {
  /** Current mode */
  mode: Mode;

  /** Set the mode */
  setMode: (mode: Mode) => void;

  /** Cycle to next mode */
  nextMode: () => void;

  /** Cycle to previous mode */
  prevMode: () => void;
}

export const useModeStore = create<ModeState>()(
  persist(
    (set) => ({
      mode: 'simple',

      setMode: (mode) => set({ mode }),

      nextMode: () =>
        set((state) => {
          const currentIndex = MODES.indexOf(state.mode);
          const nextIndex = (currentIndex + 1) % MODES.length;
          return { mode: MODES[nextIndex] };
        }),

      prevMode: () =>
        set((state) => {
          const currentIndex = MODES.indexOf(state.mode);
          const prevIndex = (currentIndex - 1 + MODES.length) % MODES.length;
          return { mode: MODES[prevIndex] };
        }),
    }),
    {
      name: 'plan-cascade-mode',
    }
  )
);

export default useModeStore;
