/**
 * Git Store
 *
 * Zustand store for git-related state used by the commit history graph
 * and other git UI components.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { create } from 'zustand';
import type {
  CommitNode,
  GraphLayout,
  BranchInfo,
  DiffOutput,
  CompareSelection,
} from '../types/git';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type GitTab = 'changes' | 'history' | 'branches' | 'stash';

interface GitState {
  /** Currently active tab in the git panel */
  activeTab: GitTab;

  /** Selected commit SHA in history view */
  selectedCommitSha: string | null;

  /** Second selected commit for compare mode (shift+click) */
  compareSelection: CompareSelection | null;

  /** Whether the commit detail panel is expanded */
  commitDetailExpanded: boolean;

  /** Cached commit list from last fetch */
  commits: CommitNode[];

  /** Cached graph layout from last fetch */
  graphLayout: GraphLayout | null;

  /** Cached branch list */
  branches: BranchInfo[];

  /** Current branch filter (null = all branches) */
  branchFilter: string | null;

  /** Search query for filtering commits */
  searchQuery: string;

  /** Diff output for selected commit */
  selectedCommitDiff: DiffOutput | null;

  // Actions
  setActiveTab: (tab: GitTab) => void;
  setSelectedCommitSha: (sha: string | null) => void;
  setCompareSelection: (selection: CompareSelection | null) => void;
  setCommitDetailExpanded: (expanded: boolean) => void;
  setCommits: (commits: CommitNode[]) => void;
  setGraphLayout: (layout: GraphLayout | null) => void;
  setBranches: (branches: BranchInfo[]) => void;
  setBranchFilter: (branch: string | null) => void;
  setSearchQuery: (query: string) => void;
  setSelectedCommitDiff: (diff: DiffOutput | null) => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Initial State
// ---------------------------------------------------------------------------

const initialState = {
  activeTab: 'changes' as GitTab,
  selectedCommitSha: null as string | null,
  compareSelection: null as CompareSelection | null,
  commitDetailExpanded: false,
  commits: [] as CommitNode[],
  graphLayout: null as GraphLayout | null,
  branches: [] as BranchInfo[],
  branchFilter: null as string | null,
  searchQuery: '',
  selectedCommitDiff: null as DiffOutput | null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useGitStore = create<GitState>()((set) => ({
  ...initialState,

  setActiveTab: (tab) => set({ activeTab: tab }),

  setSelectedCommitSha: (sha) =>
    set({
      selectedCommitSha: sha,
      commitDetailExpanded: sha !== null,
      // Clear compare selection when selecting a new single commit
      compareSelection: null,
    }),

  setCompareSelection: (selection) => set({ compareSelection: selection }),

  setCommitDetailExpanded: (expanded) => set({ commitDetailExpanded: expanded }),

  setCommits: (commits) => set({ commits }),

  setGraphLayout: (layout) => set({ graphLayout: layout }),

  setBranches: (branches) => set({ branches }),

  setBranchFilter: (branch) => set({ branchFilter: branch }),

  setSearchQuery: (query) => set({ searchQuery: query }),

  setSelectedCommitDiff: (diff) => set({ selectedCommitDiff: diff }),

  reset: () => set(initialState),
}));
