import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { SkillMemoryDialog } from './SkillMemoryDialog';
import { useContextOpsStore } from '../../store/contextOps';
import { useExecutionStore } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { useSkillMemoryStore } from '../../store/skillMemory';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

function seedEnvelope() {
  useContextOpsStore.setState({
    latestEnvelope: {
      request_meta: {
        turn_id: 'turn-1',
        mode: 'chat',
        query: 'q',
      },
      budget: {
        input_token_budget: 1000,
        reserved_output_tokens: 200,
        hard_limit: 1200,
        used_input_tokens: 200,
        over_budget: false,
      },
      sources: [
        {
          id: 'skills:selected',
          kind: 'skills',
          label: 'Skills',
          token_cost: 30,
          included: true,
          reason: 'skills_included',
        },
        {
          id: 'memory:retrieved',
          kind: 'memory',
          label: 'Memory',
          token_cost: 40,
          included: true,
          reason: 'memory_included',
        },
      ],
      blocks: [],
      compaction: {
        triggered: false,
        trigger_reason: 'none',
        strategy: 'none',
        before_tokens: 0,
        after_tokens: 0,
        compaction_tokens: 0,
        net_saving: 0,
        quality_score: 1,
      },
      trace_id: 'trace-1',
      assembled_prompt: 'prompt',
      diagnostics: {
        blocked_tools: ['Write'],
        effective_statuses: ['active'],
        selected_skills: ['Skill A'],
        effective_skill_ids: ['skill-a'],
        effective_memory_ids: ['mem-a'],
        memory_candidates_count: 3,
        degraded_reason: 'memory_query_failed',
        selection_reason: 'skills_user_selected',
        selection_origin: 'mixed',
      },
    },
  });
}

describe('SkillMemoryDialog integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSkillMemoryStore.getState().reset();
    useSkillMemoryStore.setState({
      dialogOpen: true,
      activeTab: 'skills',
      skills: [],
      memories: [],
      pendingMemoryCandidates: [],
      memoryStats: null,
      memoryScope: 'project',
      memoryCategoryFilter: 'all',
      memorySearchQuery: '',
      memoryHasMore: false,
    });
    useExecutionStore.setState({
      taskId: null,
      standaloneSessionId: null,
      foregroundOriginSessionId: null,
    });
    useSettingsStore.setState({ workspacePath: '' });
    seedEnvelope();
  });

  it('maps diagnostics.selection_origin in Why Skills panel', () => {
    render(<SkillMemoryDialog />);

    fireEvent.click(screen.getByRole('button', { name: 'Why Skills?' }));
    expect(screen.getByText(/Selection origin/i)).toBeInTheDocument();
    expect(screen.getByText(/mixed/i)).toBeInTheDocument();
    expect(screen.getByText(/Blocked tools/i)).toBeInTheDocument();
  });

  it('maps diagnostics.selection_origin in Why Memory panel', () => {
    useSkillMemoryStore.setState({ activeTab: 'memory' });
    render(<SkillMemoryDialog />);

    fireEvent.click(screen.getByRole('button', { name: 'Why Memory?' }));
    expect(screen.getByText(/Selection origin/i)).toBeInTheDocument();
    expect(screen.getByText(/mixed/i)).toBeInTheDocument();
    expect(screen.getByText(/Degraded reason/i)).toBeInTheDocument();
  });

  it('shows rejected memories with restore actions', () => {
    useSkillMemoryStore.setState({
      activeTab: 'memory',
      memories: [
        {
          id: 'mem-rejected-1',
          project_path: '/tmp/project',
          category: 'fact',
          content: 'Do not use the legacy API client',
          keywords: ['legacy', 'api'],
          importance: 0.7,
          access_count: 0,
          source_session_id: 'standalone:abc',
          source_context: 'test',
          status: 'rejected',
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          last_accessed_at: new Date().toISOString(),
        },
      ],
      memoryHasMore: false,
    });
    render(<SkillMemoryDialog />);

    fireEvent.click(screen.getByRole('button', { name: 'Rejected' }));
    expect(screen.getByText(/Do not use the legacy API client/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Restore Selected' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Restore to Review' })).toBeInTheDocument();
  });
});
