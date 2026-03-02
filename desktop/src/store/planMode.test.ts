import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

type EventCallback = (event: { payload: unknown }) => void;
const eventHandlers: Record<string, EventCallback> = {};

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((eventName: string, handler: EventCallback) => {
    eventHandlers[eventName] = handler;
    return Promise.resolve(() => {
      delete eventHandlers[eventName];
    });
  }),
}));

import { usePlanModeStore } from './planMode';
import { useContextSourcesStore } from './contextSources';
import { useProjectsStore } from './projects';

function resetStores() {
  usePlanModeStore.getState().reset();
  useContextSourcesStore.setState({
    knowledgeEnabled: false,
    selectedCollections: [],
    selectedDocuments: [],
  });
  useProjectsStore.setState({
    selectedProject: {
      id: 'default',
      name: 'Default',
      path: '/tmp/default',
      last_activity: new Date().toISOString(),
      session_count: 0,
      message_count: 0,
    },
  });
  vi.clearAllMocks();
  Object.keys(eventHandlers).forEach((key) => delete eventHandlers[key]);
}

function mockPlanSession() {
  return {
    sessionId: 'plan-session-1',
    description: 'Plan task',
    phase: 'planning',
    analysis: null,
    clarifications: [],
    currentQuestion: null,
    plan: null,
    stepOutputs: {},
    stepStates: {},
    progress: null,
    createdAt: '2026-03-02T00:00:00Z',
  };
}

describe('PlanModeStore', () => {
  beforeEach(() => {
    resetStores();
  });

  it('forwards knowledge contextSources into enter_plan_mode payload', async () => {
    useProjectsStore.setState({
      selectedProject: {
        id: 'proj-kb',
        name: 'KB Project',
        path: '/tmp/proj-kb',
        last_activity: new Date().toISOString(),
        session_count: 0,
        message_count: 0,
      },
    });
    useContextSourcesStore.setState({
      knowledgeEnabled: true,
      selectedCollections: ['col-1'],
      selectedDocuments: [{ collection_id: 'col-1', document_uid: 'doc-1' }],
    });
    const contextSources = useContextSourcesStore.getState().buildConfig();
    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: mockPlanSession(),
      error: null,
    });

    await usePlanModeStore
      .getState()
      .enterPlanMode('Plan task', undefined, undefined, undefined, '/tmp/proj-kb', contextSources, undefined, 'en');

    const call = mockInvoke.mock.calls.find(([command]) => command === 'enter_plan_mode');
    expect(call).toBeDefined();
    const args = call?.[1] as
      | {
          contextSources?: {
            project_id?: string;
            knowledge?: {
              enabled?: boolean;
              selected_collections?: string[];
              selected_documents?: Array<{ collection_id: string; document_uid: string }>;
            };
          };
        }
      | undefined;
    expect(args?.contextSources?.project_id).toBe('proj-kb');
    expect(args?.contextSources?.knowledge?.enabled).toBe(true);
    expect(args?.contextSources?.knowledge?.selected_collections).toEqual(['col-1']);
    expect(args?.contextSources?.knowledge?.selected_documents).toEqual([
      { collection_id: 'col-1', document_uid: 'doc-1' },
    ]);
  });
});
