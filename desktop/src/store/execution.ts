/**
 * Execution Store
 *
 * Manages task execution state including stories, progress, and results.
 */

import { create } from 'zustand';

export type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed';

export type Strategy = 'direct' | 'hybrid_auto' | 'mega_plan' | null;

export interface Story {
  id: string;
  title: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
}

export interface ExecutionResult {
  success: boolean;
  message: string;
  completedStories: number;
  totalStories: number;
  duration: number;
  error?: string;
}

interface ExecutionState {
  /** Current execution status */
  status: ExecutionStatus;

  /** Task description */
  taskDescription: string;

  /** Selected strategy */
  strategy: Strategy;

  /** List of stories */
  stories: Story[];

  /** Currently executing story ID */
  currentStoryId: string | null;

  /** Overall progress (0-100) */
  progress: number;

  /** Execution result */
  result: ExecutionResult | null;

  /** Start timestamp */
  startedAt: number | null;

  /** Execution logs */
  logs: string[];

  // Actions
  /** Start execution */
  start: (description: string, mode: 'simple' | 'expert') => Promise<void>;

  /** Pause execution */
  pause: () => void;

  /** Resume execution */
  resume: () => void;

  /** Cancel execution */
  cancel: () => void;

  /** Reset state */
  reset: () => void;

  /** Update story status */
  updateStory: (storyId: string, updates: Partial<Story>) => void;

  /** Add log entry */
  addLog: (message: string) => void;

  /** Set stories from PRD */
  setStories: (stories: Story[]) => void;

  /** Set strategy */
  setStrategy: (strategy: Strategy) => void;
}

const initialState = {
  status: 'idle' as ExecutionStatus,
  taskDescription: '',
  strategy: null as Strategy,
  stories: [] as Story[],
  currentStoryId: null,
  progress: 0,
  result: null,
  startedAt: null,
  logs: [] as string[],
};

export const useExecutionStore = create<ExecutionState>()((set, get) => ({
  ...initialState,

  start: async (description, mode) => {
    set({
      status: 'running',
      taskDescription: description,
      startedAt: Date.now(),
      result: null,
      logs: [],
    });

    get().addLog(`Starting execution in ${mode} mode...`);
    get().addLog(`Task: ${description}`);

    // TODO: Connect to actual backend via Tauri commands
    // For now, simulate execution
    try {
      // This will be replaced with actual Tauri command calls
      await simulateExecution(get, set);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({
        status: 'failed',
        result: {
          success: false,
          message: 'Execution failed',
          completedStories: get().stories.filter((s) => s.status === 'completed').length,
          totalStories: get().stories.length,
          duration: Date.now() - (get().startedAt || Date.now()),
          error: errorMessage,
        },
      });
      get().addLog(`Error: ${errorMessage}`);
    }
  },

  pause: () => {
    if (get().status === 'running') {
      set({ status: 'paused' });
      get().addLog('Execution paused');
    }
  },

  resume: () => {
    if (get().status === 'paused') {
      set({ status: 'running' });
      get().addLog('Execution resumed');
    }
  },

  cancel: () => {
    set({
      status: 'idle',
      currentStoryId: null,
      result: {
        success: false,
        message: 'Execution cancelled by user',
        completedStories: get().stories.filter((s) => s.status === 'completed').length,
        totalStories: get().stories.length,
        duration: Date.now() - (get().startedAt || Date.now()),
      },
    });
    get().addLog('Execution cancelled');
  },

  reset: () => {
    set(initialState);
  },

  updateStory: (storyId, updates) => {
    set((state) => ({
      stories: state.stories.map((s) =>
        s.id === storyId ? { ...s, ...updates } : s
      ),
    }));

    // Recalculate progress
    const stories = get().stories;
    const completed = stories.filter((s) => s.status === 'completed').length;
    set({ progress: (completed / stories.length) * 100 });
  },

  addLog: (message) => {
    const timestamp = new Date().toISOString().slice(11, 19);
    set((state) => ({
      logs: [...state.logs, `[${timestamp}] ${message}`],
    }));
  },

  setStories: (stories) => {
    set({ stories });
    get().addLog(`PRD loaded with ${stories.length} stories`);
  },

  setStrategy: (strategy) => {
    set({ strategy });
    get().addLog(`Strategy selected: ${strategy}`);
  },
}));

// Simulation helper (will be replaced with actual backend calls)
async function simulateExecution(
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  // Simulate PRD generation
  get().addLog('Generating PRD...');
  await delay(1000);

  const mockStories: Story[] = [
    { id: 'story-1', title: 'Initialize project', status: 'pending', progress: 0 },
    { id: 'story-2', title: 'Implement core logic', status: 'pending', progress: 0 },
    { id: 'story-3', title: 'Add tests', status: 'pending', progress: 0 },
  ];

  set({ stories: mockStories, strategy: 'hybrid_auto' });
  get().addLog('PRD generated with 3 stories');

  // Execute each story
  for (const story of mockStories) {
    if (get().status !== 'running') break;

    set({ currentStoryId: story.id });
    get().updateStory(story.id, { status: 'in_progress' });
    get().addLog(`Starting story: ${story.title}`);

    // Simulate progress
    for (let i = 0; i <= 100; i += 20) {
      if (get().status !== 'running') break;
      await delay(300);
      get().updateStory(story.id, { progress: i });
    }

    if (get().status === 'running') {
      get().updateStory(story.id, { status: 'completed', progress: 100 });
      get().addLog(`Completed: ${story.title}`);
    }
  }

  if (get().status === 'running') {
    set({
      status: 'completed',
      currentStoryId: null,
      result: {
        success: true,
        message: 'All stories completed successfully',
        completedStories: mockStories.length,
        totalStories: mockStories.length,
        duration: Date.now() - (get().startedAt || Date.now()),
      },
    });
    get().addLog('Execution completed successfully');
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export default useExecutionStore;
