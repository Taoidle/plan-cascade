/**
 * PRD Store
 *
 * Manages PRD (Product Requirements Document) state including stories,
 * dependencies, agent assignments, and draft persistence.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type AgentType = 'claude-code' | 'aider' | 'codex';
export type StoryStatus = 'pending' | 'in_progress' | 'completed' | 'failed';
export type ExecutionStrategy = 'direct' | 'hybrid_auto' | 'mega_plan';

export interface PRDStory {
  id: string;
  title: string;
  description: string;
  acceptance_criteria: string[];
  status: StoryStatus;
  dependencies: string[]; // IDs of stories this story is blocked by
  agent: AgentType;
  order: number;
}

export interface QualityGate {
  id: string;
  name: string;
  enabled: boolean;
  command?: string; // For custom gates
}

export interface WorktreeConfig {
  enabled: boolean;
  branchName: string;
  baseBranch: string;
}

export interface PRDDraft {
  id: string;
  name: string;
  timestamp: number;
  prd: PRDState['prd'];
}

export interface PRD {
  feature_id: string;
  title: string;
  description: string;
  stories: PRDStory[];
  strategy: ExecutionStrategy;
  qualityGates: QualityGate[];
  worktree: WorktreeConfig;
}

interface PRDState {
  /** Current PRD being edited */
  prd: PRD;

  /** Saved drafts */
  drafts: PRDDraft[];

  /** Last auto-save timestamp */
  lastAutoSave: number | null;

  /** Is PRD generation in progress */
  isGenerating: boolean;

  /** Generation error */
  generationError: string | null;

  // Story CRUD operations
  addStory: (story: Omit<PRDStory, 'id' | 'order'>) => void;
  updateStory: (id: string, updates: Partial<PRDStory>) => void;
  deleteStory: (id: string) => void;
  reorderStories: (startIndex: number, endIndex: number) => void;

  // Dependency management
  addDependency: (storyId: string, dependsOnId: string) => void;
  removeDependency: (storyId: string, dependsOnId: string) => void;
  hasCircularDependency: (storyId: string, dependsOnId: string) => boolean;

  // Agent assignment
  setStoryAgent: (storyId: string, agent: AgentType) => void;
  setBulkAgent: (agent: AgentType) => void;

  // PRD configuration
  setStrategy: (strategy: ExecutionStrategy) => void;
  setQualityGate: (gateId: string, enabled: boolean) => void;
  addCustomQualityGate: (name: string, command: string) => void;
  removeQualityGate: (gateId: string) => void;
  setWorktreeConfig: (config: Partial<WorktreeConfig>) => void;

  // PRD metadata
  setPRDTitle: (title: string) => void;
  setPRDDescription: (description: string) => void;
  setFeatureId: (featureId: string) => void;

  // Draft management
  saveDraft: (name?: string) => void;
  loadDraft: (draftId: string) => void;
  deleteDraft: (draftId: string) => void;
  autoSave: () => void;

  // PRD generation
  generatePRD: (requirements: string) => Promise<void>;

  // Reset
  reset: () => void;
  loadFromJSON: (json: string) => void;
  exportToJSON: () => string;
}

const defaultQualityGates: QualityGate[] = [
  { id: 'typecheck', name: 'TypeCheck', enabled: true },
  { id: 'test', name: 'Test', enabled: true },
  { id: 'lint', name: 'Lint', enabled: true },
];

const initialPRD: PRD = {
  feature_id: '',
  title: '',
  description: '',
  stories: [],
  strategy: 'hybrid_auto',
  qualityGates: defaultQualityGates,
  worktree: {
    enabled: false,
    branchName: '',
    baseBranch: 'main',
  },
};

const initialState = {
  prd: initialPRD,
  drafts: [] as PRDDraft[],
  lastAutoSave: null as number | null,
  isGenerating: false,
  generationError: null as string | null,
};

function generateId(): string {
  return `story-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

export const usePRDStore = create<PRDState>()(
  persist(
    (set, get) => ({
      ...initialState,

      // Story CRUD operations
      addStory: (story) => {
        const newStory: PRDStory = {
          ...story,
          id: generateId(),
          order: get().prd.stories.length,
        };
        set((state) => ({
          prd: {
            ...state.prd,
            stories: [...state.prd.stories, newStory],
          },
        }));
      },

      updateStory: (id, updates) => {
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories.map((s) =>
              s.id === id ? { ...s, ...updates } : s
            ),
          },
        }));
      },

      deleteStory: (id) => {
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories
              .filter((s) => s.id !== id)
              .map((s, index) => ({
                ...s,
                order: index,
                dependencies: s.dependencies.filter((d) => d !== id),
              })),
          },
        }));
      },

      reorderStories: (startIndex, endIndex) => {
        set((state) => {
          const stories = [...state.prd.stories];
          const [removed] = stories.splice(startIndex, 1);
          stories.splice(endIndex, 0, removed);
          return {
            prd: {
              ...state.prd,
              stories: stories.map((s, index) => ({ ...s, order: index })),
            },
          };
        });
      },

      // Dependency management
      addDependency: (storyId, dependsOnId) => {
        if (get().hasCircularDependency(storyId, dependsOnId)) {
          console.warn('Circular dependency detected');
          return;
        }
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories.map((s) =>
              s.id === storyId && !s.dependencies.includes(dependsOnId)
                ? { ...s, dependencies: [...s.dependencies, dependsOnId] }
                : s
            ),
          },
        }));
      },

      removeDependency: (storyId, dependsOnId) => {
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories.map((s) =>
              s.id === storyId
                ? { ...s, dependencies: s.dependencies.filter((d) => d !== dependsOnId) }
                : s
            ),
          },
        }));
      },

      hasCircularDependency: (storyId, dependsOnId) => {
        const stories = get().prd.stories;
        const visited = new Set<string>();

        function checkCycle(currentId: string): boolean {
          if (currentId === storyId) return true;
          if (visited.has(currentId)) return false;
          visited.add(currentId);

          const story = stories.find((s) => s.id === currentId);
          if (!story) return false;

          return story.dependencies.some((depId) => checkCycle(depId));
        }

        return checkCycle(dependsOnId);
      },

      // Agent assignment
      setStoryAgent: (storyId, agent) => {
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories.map((s) =>
              s.id === storyId ? { ...s, agent } : s
            ),
          },
        }));
      },

      setBulkAgent: (agent) => {
        set((state) => ({
          prd: {
            ...state.prd,
            stories: state.prd.stories.map((s) => ({ ...s, agent })),
          },
        }));
      },

      // PRD configuration
      setStrategy: (strategy) => {
        set((state) => ({
          prd: { ...state.prd, strategy },
        }));
      },

      setQualityGate: (gateId, enabled) => {
        set((state) => ({
          prd: {
            ...state.prd,
            qualityGates: state.prd.qualityGates.map((g) =>
              g.id === gateId ? { ...g, enabled } : g
            ),
          },
        }));
      },

      addCustomQualityGate: (name, command) => {
        const newGate: QualityGate = {
          id: `custom-${Date.now()}`,
          name,
          enabled: true,
          command,
        };
        set((state) => ({
          prd: {
            ...state.prd,
            qualityGates: [...state.prd.qualityGates, newGate],
          },
        }));
      },

      removeQualityGate: (gateId) => {
        set((state) => ({
          prd: {
            ...state.prd,
            qualityGates: state.prd.qualityGates.filter((g) => g.id !== gateId),
          },
        }));
      },

      setWorktreeConfig: (config) => {
        set((state) => ({
          prd: {
            ...state.prd,
            worktree: { ...state.prd.worktree, ...config },
          },
        }));
      },

      // PRD metadata
      setPRDTitle: (title) => {
        set((state) => ({
          prd: { ...state.prd, title },
        }));
      },

      setPRDDescription: (description) => {
        set((state) => ({
          prd: { ...state.prd, description },
        }));
      },

      setFeatureId: (featureId) => {
        set((state) => ({
          prd: { ...state.prd, feature_id: featureId },
        }));
      },

      // Draft management
      saveDraft: (name) => {
        const draft: PRDDraft = {
          id: `draft-${Date.now()}`,
          name: name || `Draft ${new Date().toLocaleString()}`,
          timestamp: Date.now(),
          prd: JSON.parse(JSON.stringify(get().prd)),
        };
        set((state) => ({
          drafts: [...state.drafts, draft],
        }));
      },

      loadDraft: (draftId) => {
        const draft = get().drafts.find((d) => d.id === draftId);
        if (draft) {
          set({ prd: JSON.parse(JSON.stringify(draft.prd)) });
        }
      },

      deleteDraft: (draftId) => {
        set((state) => ({
          drafts: state.drafts.filter((d) => d.id !== draftId),
        }));
      },

      autoSave: () => {
        const now = Date.now();
        const lastSave = get().lastAutoSave;

        // Auto-save every 30 seconds
        if (!lastSave || now - lastSave > 30000) {
          const drafts = get().drafts;
          const autoSaveDraft = drafts.find((d) => d.name === 'Auto-save');

          if (autoSaveDraft) {
            // Update existing auto-save
            set((state) => ({
              drafts: state.drafts.map((d) =>
                d.id === autoSaveDraft.id
                  ? { ...d, timestamp: now, prd: JSON.parse(JSON.stringify(get().prd)) }
                  : d
              ),
              lastAutoSave: now,
            }));
          } else {
            // Create new auto-save
            const draft: PRDDraft = {
              id: `draft-autosave`,
              name: 'Auto-save',
              timestamp: now,
              prd: JSON.parse(JSON.stringify(get().prd)),
            };
            set((state) => ({
              drafts: [...state.drafts, draft],
              lastAutoSave: now,
            }));
          }
        }
      },

      // PRD generation
      generatePRD: async (requirements) => {
        set({ isGenerating: true, generationError: null });

        try {
          // TODO: Replace with actual API call to backend
          // For now, simulate PRD generation
          await new Promise((resolve) => setTimeout(resolve, 1500));

          // Parse requirements and create stories
          const lines = requirements.split('\n').filter((l) => l.trim());
          const stories: PRDStory[] = lines.slice(0, 5).map((line, index) => ({
            id: generateId(),
            title: line.trim().substring(0, 100),
            description: `Implement: ${line.trim()}`,
            acceptance_criteria: ['Functionality works as expected', 'Tests pass', 'Code is documented'],
            status: 'pending' as StoryStatus,
            dependencies: index > 0 ? [] : [],
            agent: 'claude-code' as AgentType,
            order: index,
          }));

          set((state) => ({
            prd: {
              ...state.prd,
              stories,
              title: `Generated PRD - ${new Date().toLocaleDateString()}`,
              description: requirements.substring(0, 200),
            },
            isGenerating: false,
          }));
        } catch (error) {
          set({
            isGenerating: false,
            generationError: error instanceof Error ? error.message : 'Failed to generate PRD',
          });
        }
      },

      // Reset
      reset: () => {
        set({ prd: initialPRD });
      },

      loadFromJSON: (json) => {
        try {
          const parsed = JSON.parse(json);
          set({ prd: { ...initialPRD, ...parsed } });
        } catch (error) {
          console.error('Failed to parse PRD JSON:', error);
        }
      },

      exportToJSON: () => {
        return JSON.stringify(get().prd, null, 2);
      },
    }),
    {
      name: 'plan-cascade-prd',
      partialize: (state) => ({
        prd: state.prd,
        drafts: state.drafts,
      }),
    }
  )
);

export default usePRDStore;
