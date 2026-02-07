/**
 * Mode Store
 *
 * Manages the application mode (simple/expert/claude-code) state.
 * Persists to localStorage for session continuity.
 *
 * Story 005: Navigation Flow Refinement - Added breadcrumb and transition state
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Mode = 'simple' | 'expert' | 'claude-code' | 'projects' | 'analytics';

/** All available modes in order */
export const MODES: Mode[] = ['simple', 'expert', 'claude-code', 'projects', 'analytics'];

/** Mode display names */
export const MODE_LABELS: Record<Mode, string> = {
  simple: 'Simple',
  expert: 'Expert',
  'claude-code': 'Claude Code',
  projects: 'Projects',
  analytics: 'Analytics',
};

/** Mode descriptions */
export const MODE_DESCRIPTIONS: Record<Mode, string> = {
  simple: 'One-click execution with AI-driven automation',
  expert: 'Full control over PRD editing, agents, and execution',
  'claude-code': 'Interactive chat with Claude Code CLI',
  projects: 'Browse and resume Claude Code sessions',
  analytics: 'Track usage, costs, and API analytics',
};

// ============================================================================
// Breadcrumb Types
// ============================================================================

export interface BreadcrumbItem {
  /** Unique identifier */
  id: string;
  /** Display label */
  label: string;
  /** Associated mode (if navigable) */
  mode?: Mode;
  /** Whether this item is clickable */
  navigable: boolean;
}

/** Transition direction for animations */
export type TransitionDirection = 'left' | 'right' | 'none';

// ============================================================================
// State Interface
// ============================================================================

interface ModeState {
  /** Current mode */
  mode: Mode;

  /** Previous mode (for transition direction) */
  previousMode: Mode | null;

  /** Whether a mode transition is in progress */
  isTransitioning: boolean;

  /** Direction of the current transition */
  transitionDirection: TransitionDirection;

  /** Current breadcrumb trail */
  breadcrumbs: BreadcrumbItem[];

  /** Sub-view label within the current mode (e.g., "Session Details") */
  subView: string | null;

  /** Set the mode with transition tracking */
  setMode: (mode: Mode) => void;

  /** Cycle to next mode */
  nextMode: () => void;

  /** Cycle to previous mode */
  prevMode: () => void;

  /** Mark transition as complete */
  completeTransition: () => void;

  /** Push a sub-view breadcrumb */
  pushSubView: (label: string) => void;

  /** Pop the current sub-view breadcrumb */
  popSubView: () => void;

  /** Navigate to a specific breadcrumb item */
  navigateToBreadcrumb: (id: string) => void;
}

// ============================================================================
// Helpers
// ============================================================================

function computeTransitionDirection(from: Mode, to: Mode): TransitionDirection {
  const fromIndex = MODES.indexOf(from);
  const toIndex = MODES.indexOf(to);
  if (fromIndex === toIndex) return 'none';
  return toIndex > fromIndex ? 'left' : 'right';
}

function buildBreadcrumbs(mode: Mode, subView: string | null): BreadcrumbItem[] {
  const crumbs: BreadcrumbItem[] = [
    { id: 'home', label: 'Plan Cascade', navigable: true },
    { id: `mode-${mode}`, label: MODE_LABELS[mode], mode, navigable: true },
  ];
  if (subView) {
    crumbs.push({ id: 'subview', label: subView, navigable: false });
  }
  return crumbs;
}

// ============================================================================
// Store
// ============================================================================

export const useModeStore = create<ModeState>()(
  persist(
    (set, get) => ({
      mode: 'simple',
      previousMode: null,
      isTransitioning: false,
      transitionDirection: 'none' as TransitionDirection,
      breadcrumbs: buildBreadcrumbs('simple', null),
      subView: null,

      setMode: (mode) => {
        const current = get().mode;
        if (current === mode) return;
        const direction = computeTransitionDirection(current, mode);
        set({
          previousMode: current,
          mode,
          isTransitioning: true,
          transitionDirection: direction,
          subView: null,
          breadcrumbs: buildBreadcrumbs(mode, null),
        });
        // Auto-complete transition after animation duration (200ms)
        setTimeout(() => {
          // Only clear if still transitioning to the same mode
          if (get().mode === mode) {
            set({ isTransitioning: false });
          }
        }, 200);
      },

      nextMode: () => {
        const current = get().mode;
        const currentIndex = MODES.indexOf(current);
        const nextIndex = (currentIndex + 1) % MODES.length;
        get().setMode(MODES[nextIndex]);
      },

      prevMode: () => {
        const current = get().mode;
        const currentIndex = MODES.indexOf(current);
        const prevIndex = (currentIndex - 1 + MODES.length) % MODES.length;
        get().setMode(MODES[prevIndex]);
      },

      completeTransition: () => set({ isTransitioning: false }),

      pushSubView: (label) => {
        const { mode } = get();
        set({
          subView: label,
          breadcrumbs: buildBreadcrumbs(mode, label),
        });
      },

      popSubView: () => {
        const { mode } = get();
        set({
          subView: null,
          breadcrumbs: buildBreadcrumbs(mode, null),
        });
      },

      navigateToBreadcrumb: (id) => {
        if (id === 'home') {
          get().setMode('simple');
          return;
        }
        if (id === 'subview') {
          // Already at the sub-view; no action
          return;
        }
        // Mode breadcrumb
        const prefix = 'mode-';
        if (id.startsWith(prefix)) {
          const modeStr = id.slice(prefix.length) as Mode;
          if (MODES.includes(modeStr)) {
            // If navigating to the current mode, just pop subview
            if (modeStr === get().mode) {
              get().popSubView();
            } else {
              get().setMode(modeStr);
            }
          }
        }
      },
    }),
    {
      name: 'plan-cascade-mode',
      partialize: (state) => ({
        mode: state.mode,
      }),
    }
  )
);

export default useModeStore;
