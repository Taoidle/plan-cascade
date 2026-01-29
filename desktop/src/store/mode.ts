/**
 * Mode Store
 *
 * Manages the application mode (simple/expert) state.
 * Persists to localStorage for session continuity.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Mode = 'simple' | 'expert';

interface ModeState {
  /** Current mode */
  mode: Mode;

  /** Set the mode */
  setMode: (mode: Mode) => void;

  /** Toggle between modes */
  toggleMode: () => void;
}

export const useModeStore = create<ModeState>()(
  persist(
    (set) => ({
      mode: 'simple',

      setMode: (mode) => set({ mode }),

      toggleMode: () =>
        set((state) => ({
          mode: state.mode === 'simple' ? 'expert' : 'simple',
        })),
    }),
    {
      name: 'plan-cascade-mode',
    }
  )
);

export default useModeStore;
