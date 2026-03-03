/**
 * useGlobalCommands Hook
 *
 * Registers 50+ commands dynamically with context-aware filtering.
 * Commands are organized by category and filtered based on the current view.
 *
 * Story 004: Command Palette Enhancement
 */

import { useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  useGlobalCommandPalette,
  Command,
  CommandCategory,
  // Icons
  DownloadIcon,
  TrashIcon,
  ViewVerticalIcon,
  RocketIcon,
  BarChartIcon,
  MixerHorizontalIcon,
  ClockIcon,
  StackIcon,
  PlusIcon,
  ResetIcon,
  GitHubLogoIcon,
  ArchiveIcon,
  PlayIcon,
  StopIcon,
  CheckCircledIcon,
} from '../components/shared/CommandPalette';
import { useModeStore } from '../store/mode';
import { useSettingsStore } from '../store/settings';
import { useClaudeCodeStore } from '../store/claudeCode';
import { useAgentsStore } from '../store/agents';
import { useAnalyticsStore } from '../store/analytics';
import { useProjectsStore } from '../store/projects';
import { useTimelineStore } from '../store/timeline';
import { dispatchMcpUiIntent } from '../store/mcpUi';
import {
  createChatCommands,
  createHelpCommands,
  createNavigationCommands,
  createSettingsCommands,
  type GlobalCommandOptions,
} from './useGlobalCommands/commandGroups';

// ============================================================================
// Types
// ============================================================================

type UseGlobalCommandsOptions = GlobalCommandOptions;

// ============================================================================
// Hook Implementation
// ============================================================================

export function useGlobalCommands(options: UseGlobalCommandsOptions = {}) {
  const { t } = useTranslation('common');
  const { registerCommands, unregisterCommands, setCurrentContext } = useGlobalCommandPalette();
  const { mode, setMode } = useModeStore();
  const { theme, setTheme, language, setLanguage } = useSettingsStore();
  const claudeCodeStore = useClaudeCodeStore();
  const agentsStore = useAgentsStore();
  const analyticsStore = useAnalyticsStore();
  const projectsStore = useProjectsStore();
  const timelineStore = useTimelineStore();

  const dispatchMcpAction = useCallback(
    (
      action:
        | 'open-add'
        | 'open-import'
        | 'open-discover'
        | 'install-recommended'
        | 'refresh'
        | 'test-enabled'
        | 'export',
    ) => {
      dispatchMcpUiIntent(action);
    },
    [],
  );

  // Update current context based on mode
  useEffect(() => {
    setCurrentContext(mode);
  }, [mode, setCurrentContext]);

  // ============================================================================
  // Navigation Commands (8 commands)
  // ============================================================================

  const navigationCommands: Command[] = useMemo(
    () =>
      createNavigationCommands({
        t,
        mode,
        setMode,
      }),
    [t, setMode, mode],
  );

  // ============================================================================
  // Projects Commands (8 commands)
  // ============================================================================

  const projectsCommands: Command[] = useMemo(
    () => [
      {
        id: 'projects-refresh',
        title: t('commands.projects.refresh'),
        description: t('commands.projects.refreshDesc'),
        category: 'projects' as CommandCategory,
        icon: ResetIcon,
        action: () => projectsStore.fetchProjects(),
        keywords: ['refresh', 'reload', 'projects', 'update'],
        priority: 80,
        contexts: ['projects', 'global'],
      },
      {
        id: 'projects-add',
        title: t('commands.projects.add'),
        description: t('commands.projects.addDesc'),
        category: 'projects' as CommandCategory,
        icon: PlusIcon,
        action: () => {
          // Open folder dialog to add project
          setMode('projects');
        },
        keywords: ['add', 'new', 'project', 'folder', 'open'],
        priority: 90,
        contexts: ['projects', 'global'],
      },
      {
        id: 'projects-search',
        title: t('commands.projects.search'),
        description: t('commands.projects.searchDesc'),
        category: 'projects' as CommandCategory,
        icon: StackIcon,
        shortcut: 'mod+shift+p',
        action: () => {
          setMode('projects');
          // Focus search in projects view
        },
        keywords: ['search', 'find', 'project', 'filter'],
        priority: 85,
        contexts: ['projects', 'global'],
      },
      {
        id: 'projects-clear-search',
        title: t('commands.projects.clearSearch'),
        description: t('commands.projects.clearSearchDesc'),
        category: 'projects' as CommandCategory,
        icon: TrashIcon,
        action: () => {
          projectsStore.setSearchQuery('');
          projectsStore.fetchProjects();
        },
        keywords: ['clear', 'search', 'projects', 'filter'],
        priority: 40,
        contexts: ['projects'],
      },
      {
        id: 'projects-view-sessions',
        title: t('commands.projects.viewSessions'),
        description: t('commands.projects.viewSessionsDesc'),
        category: 'projects' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          setMode('projects');
        },
        keywords: ['sessions', 'history', 'view', 'browse'],
        priority: 70,
        contexts: ['projects', 'global'],
      },
      {
        id: 'projects-export-all',
        title: t('commands.projects.exportAll'),
        description: t('commands.projects.exportAllDesc'),
        category: 'projects' as CommandCategory,
        icon: DownloadIcon,
        action: () => {
          // Export all projects data
          options.onExportData?.();
        },
        keywords: ['export', 'backup', 'projects', 'all'],
        priority: 50,
        contexts: ['projects'],
      },
      {
        id: 'projects-import',
        title: t('commands.projects.import'),
        description: t('commands.projects.importDesc'),
        category: 'projects' as CommandCategory,
        icon: ArchiveIcon,
        action: () => {
          // Import projects from backup
        },
        keywords: ['import', 'restore', 'projects', 'backup'],
        priority: 45,
        contexts: ['projects'],
      },
      {
        id: 'projects-sort-recent',
        title: t('commands.projects.sortRecent'),
        description: t('commands.projects.sortRecentDesc'),
        category: 'projects' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          projectsStore.setSortBy('recent_activity');
        },
        keywords: ['sort', 'recent', 'order', 'projects'],
        priority: 35,
        contexts: ['projects'],
      },
    ],
    [t, projectsStore, setMode, options],
  );

  // ============================================================================
  // Agents Commands (8 commands)
  // ============================================================================

  const agentsCommands: Command[] = useMemo(
    () => [
      {
        id: 'agents-library',
        title: t('commands.agents.library'),
        description: t('commands.agents.libraryDesc'),
        category: 'agents' as CommandCategory,
        icon: RocketIcon,
        action: () => {
          // Open agents library panel
          agentsStore.fetchAgents();
        },
        keywords: ['agents', 'library', 'browse', 'list'],
        priority: 90,
        contexts: ['expert', 'global'],
      },
      {
        id: 'agents-create',
        title: t('commands.agents.create'),
        description: t('commands.agents.createDesc'),
        category: 'agents' as CommandCategory,
        icon: PlusIcon,
        shortcut: 'mod+shift+a',
        action: () => {
          // Open agent creation dialog
        },
        keywords: ['create', 'new', 'agent', 'add'],
        priority: 85,
        contexts: ['expert', 'global'],
      },
      {
        id: 'agents-refresh',
        title: t('commands.agents.refresh'),
        description: t('commands.agents.refreshDesc'),
        category: 'agents' as CommandCategory,
        icon: ResetIcon,
        action: () => agentsStore.fetchAgents(),
        keywords: ['refresh', 'reload', 'agents', 'update'],
        priority: 70,
        contexts: ['expert'],
      },
      {
        id: 'agents-run-first',
        title: t('commands.agents.runFirst'),
        description: t('commands.agents.runFirstDesc'),
        category: 'agents' as CommandCategory,
        icon: PlayIcon,
        action: () => {
          const firstAgent = agentsStore.agents[0];
          if (firstAgent) {
            agentsStore.selectAgent(firstAgent);
          }
        },
        keywords: ['run', 'execute', 'agent', 'first'],
        priority: 80,
        contexts: ['expert', 'simple'],
        disabled: agentsStore.agents.length === 0,
      },
      {
        id: 'agents-stop-all',
        title: t('commands.agents.stopAll'),
        description: t('commands.agents.stopAllDesc'),
        category: 'agents' as CommandCategory,
        icon: StopIcon,
        action: () => {
          // Stop all running agents
        },
        keywords: ['stop', 'cancel', 'agents', 'all'],
        priority: 75,
        contexts: ['expert', 'simple'],
      },
      {
        id: 'agents-export',
        title: t('commands.agents.export'),
        description: t('commands.agents.exportDesc'),
        category: 'agents' as CommandCategory,
        icon: DownloadIcon,
        action: async () => {
          const data = await agentsStore.exportAgents();
          if (data) {
            // Download as JSON file
            const blob = new Blob([data], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = 'agents-export.json';
            a.click();
            URL.revokeObjectURL(url);
          }
        },
        keywords: ['export', 'backup', 'agents', 'download'],
        priority: 50,
        contexts: ['expert'],
      },
      {
        id: 'agents-import',
        title: t('commands.agents.import'),
        description: t('commands.agents.importDesc'),
        category: 'agents' as CommandCategory,
        icon: ArchiveIcon,
        action: () => {
          // Open file picker for import
          const input = document.createElement('input');
          input.type = 'file';
          input.accept = '.json';
          input.onchange = async (e) => {
            const file = (e.target as HTMLInputElement).files?.[0];
            if (file) {
              const text = await file.text();
              await agentsStore.importAgents(text);
            }
          };
          input.click();
        },
        keywords: ['import', 'restore', 'agents', 'upload'],
        priority: 45,
        contexts: ['expert'],
      },
      {
        id: 'agents-view-history',
        title: t('commands.agents.viewHistory'),
        description: t('commands.agents.viewHistoryDesc'),
        category: 'agents' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          // Open agent run history panel
        },
        keywords: ['history', 'runs', 'agents', 'logs'],
        priority: 60,
        contexts: ['expert'],
      },
    ],
    [t, agentsStore],
  );

  // ============================================================================
  // Analytics Commands (6 commands)
  // ============================================================================

  const analyticsCommands: Command[] = useMemo(
    () => [
      {
        id: 'analytics-dashboard',
        title: t('commands.analytics.dashboard'),
        description: t('commands.analytics.dashboardDesc'),
        category: 'analytics' as CommandCategory,
        icon: BarChartIcon,
        action: () => {
          setMode('analytics');
          analyticsStore.fetchDashboardSummary();
        },
        keywords: ['analytics', 'dashboard', 'overview', 'stats'],
        priority: 90,
        contexts: ['analytics', 'global'],
      },
      {
        id: 'analytics-refresh',
        title: t('commands.analytics.refresh'),
        description: t('commands.analytics.refreshDesc'),
        category: 'analytics' as CommandCategory,
        icon: ResetIcon,
        action: () => analyticsStore.fetchDashboardSummary(),
        keywords: ['refresh', 'reload', 'analytics', 'update'],
        priority: 80,
        contexts: ['analytics'],
      },
      {
        id: 'analytics-export-csv',
        title: t('commands.analytics.exportCsv'),
        description: t('commands.analytics.exportCsvDesc'),
        category: 'analytics' as CommandCategory,
        icon: DownloadIcon,
        action: async () => {
          const result = await analyticsStore.exportData('csv', true);
          if (result) {
            const blob = new Blob([result.data], { type: 'text/csv' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = result.suggested_filename;
            a.click();
            URL.revokeObjectURL(url);
          }
        },
        keywords: ['export', 'csv', 'analytics', 'download'],
        priority: 70,
        contexts: ['analytics'],
      },
      {
        id: 'analytics-export-json',
        title: t('commands.analytics.exportJson'),
        description: t('commands.analytics.exportJsonDesc'),
        category: 'analytics' as CommandCategory,
        icon: DownloadIcon,
        action: async () => {
          const result = await analyticsStore.exportData('json', true);
          if (result) {
            const blob = new Blob([result.data], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = result.suggested_filename;
            a.click();
            URL.revokeObjectURL(url);
          }
        },
        keywords: ['export', 'json', 'analytics', 'download'],
        priority: 65,
        contexts: ['analytics'],
      },
      {
        id: 'analytics-period-7days',
        title: t('commands.analytics.period7days'),
        description: t('commands.analytics.period7daysDesc'),
        category: 'analytics' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          analyticsStore.setPeriodPreset('last7days');
          analyticsStore.fetchDashboardSummary();
        },
        keywords: ['period', '7', 'days', 'week', 'analytics'],
        priority: 50,
        contexts: ['analytics'],
      },
      {
        id: 'analytics-period-30days',
        title: t('commands.analytics.period30days'),
        description: t('commands.analytics.period30daysDesc'),
        category: 'analytics' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          analyticsStore.setPeriodPreset('last30days');
          analyticsStore.fetchDashboardSummary();
        },
        keywords: ['period', '30', 'days', 'month', 'analytics'],
        priority: 50,
        contexts: ['analytics'],
      },
    ],
    [t, analyticsStore, setMode],
  );

  // ============================================================================
  // MCP Commands (8 commands)
  // ============================================================================

  const mcpCommands: Command[] = useMemo(
    () => [
      {
        id: 'mcp-server-registry',
        title: t('commands.mcp.serverRegistry'),
        description: t('commands.mcp.serverRegistryDesc'),
        category: 'mcp' as CommandCategory,
        icon: MixerHorizontalIcon,
        action: () => {
          setMode('mcp');
        },
        keywords: ['mcp', 'server', 'registry', 'browse'],
        priority: 90,
        contexts: ['expert', 'mcp', 'global'],
      },
      {
        id: 'mcp-add-server',
        title: t('commands.mcp.addServer'),
        description: t('commands.mcp.addServerDesc'),
        category: 'mcp' as CommandCategory,
        icon: PlusIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('open-add');
        },
        keywords: ['mcp', 'add', 'server', 'new'],
        priority: 85,
        contexts: ['expert', 'mcp'],
      },
      {
        id: 'mcp-open-discover',
        title: t('commands.mcp.openDiscover'),
        description: t('commands.mcp.openDiscoverDesc'),
        category: 'mcp' as CommandCategory,
        icon: ViewVerticalIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('open-discover');
        },
        keywords: ['mcp', 'discover', 'recommended', 'catalog'],
        priority: 80,
        contexts: ['expert', 'mcp', 'global'],
      },
      {
        id: 'mcp-install-recommended',
        title: t('commands.mcp.installRecommended'),
        description: t('commands.mcp.installRecommendedDesc'),
        category: 'mcp' as CommandCategory,
        icon: RocketIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('install-recommended');
        },
        keywords: ['mcp', 'install', 'recommended', 'catalog'],
        priority: 78,
        contexts: ['expert', 'mcp', 'global'],
      },
      {
        id: 'mcp-refresh-servers',
        title: t('commands.mcp.refreshServers'),
        description: t('commands.mcp.refreshServersDesc'),
        category: 'mcp' as CommandCategory,
        icon: ResetIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('refresh');
        },
        keywords: ['mcp', 'refresh', 'servers', 'reload'],
        priority: 70,
        contexts: ['expert', 'mcp'],
      },
      {
        id: 'mcp-import-config',
        title: t('commands.mcp.importConfig'),
        description: t('commands.mcp.importConfigDesc'),
        category: 'mcp' as CommandCategory,
        icon: ArchiveIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('open-import');
        },
        keywords: ['mcp', 'import', 'config', 'configuration'],
        priority: 60,
        contexts: ['expert', 'mcp'],
      },
      {
        id: 'mcp-export-config',
        title: t('commands.mcp.exportConfig'),
        description: t('commands.mcp.exportConfigDesc'),
        category: 'mcp' as CommandCategory,
        icon: DownloadIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('export');
        },
        keywords: ['mcp', 'export', 'config', 'backup'],
        priority: 55,
        contexts: ['expert', 'mcp'],
      },
      {
        id: 'mcp-test-connection',
        title: t('commands.mcp.testConnection'),
        description: t('commands.mcp.testConnectionDesc'),
        category: 'mcp' as CommandCategory,
        icon: CheckCircledIcon,
        action: () => {
          setMode('mcp');
          dispatchMcpAction('test-enabled');
        },
        keywords: ['mcp', 'test', 'connection', 'verify'],
        priority: 65,
        contexts: ['expert', 'mcp'],
      },
    ],
    [dispatchMcpAction, t, setMode],
  );

  // ============================================================================
  // Timeline Commands (5 commands)
  // ============================================================================

  const timelineCommands: Command[] = useMemo(
    () => [
      {
        id: 'timeline-view',
        title: t('commands.timeline.view'),
        description: t('commands.timeline.viewDesc'),
        category: 'timeline' as CommandCategory,
        icon: ClockIcon,
        action: () => {
          // Open timeline view
          timelineStore.fetchTimeline();
        },
        keywords: ['timeline', 'checkpoints', 'history', 'view'],
        priority: 90,
        contexts: ['projects', 'global'],
      },
      {
        id: 'timeline-create-checkpoint',
        title: t('commands.timeline.createCheckpoint'),
        description: t('commands.timeline.createCheckpointDesc'),
        category: 'timeline' as CommandCategory,
        icon: PlusIcon,
        shortcut: 'mod+shift+s',
        action: () => {
          // Create new checkpoint
        },
        keywords: ['checkpoint', 'create', 'save', 'snapshot'],
        priority: 85,
        contexts: ['claude-code', 'expert'],
      },
      {
        id: 'timeline-restore-latest',
        title: t('commands.timeline.restoreLatest'),
        description: t('commands.timeline.restoreLatestDesc'),
        category: 'timeline' as CommandCategory,
        icon: ResetIcon,
        action: () => {
          // Restore to latest checkpoint
        },
        keywords: ['restore', 'checkpoint', 'latest', 'revert'],
        priority: 75,
        contexts: ['claude-code', 'expert'],
      },
      {
        id: 'timeline-compare',
        title: t('commands.timeline.compare'),
        description: t('commands.timeline.compareDesc'),
        category: 'timeline' as CommandCategory,
        icon: ViewVerticalIcon,
        action: () => {
          // Open diff comparison view
        },
        keywords: ['compare', 'diff', 'checkpoint', 'changes'],
        priority: 70,
        contexts: ['projects'],
      },
      {
        id: 'timeline-branch',
        title: t('commands.timeline.branch'),
        description: t('commands.timeline.branchDesc'),
        category: 'timeline' as CommandCategory,
        icon: GitHubLogoIcon,
        action: () => {
          // Create timeline branch
        },
        keywords: ['branch', 'fork', 'checkpoint', 'timeline'],
        priority: 65,
        contexts: ['projects'],
      },
    ],
    [t, timelineStore],
  );

  // ============================================================================
  // Chat Commands (6 commands)
  // ============================================================================

  const chatCommands: Command[] = useMemo(
    () =>
      createChatCommands({
        t,
        claudeCodeStore,
        options,
      }),
    [t, claudeCodeStore, options],
  );

  // ============================================================================
  // Settings Commands (8 commands)
  // ============================================================================

  const settingsCommands: Command[] = useMemo(
    () =>
      createSettingsCommands({
        t,
        options,
        theme,
        setTheme,
        language,
        setLanguage,
      }),
    [t, options, theme, setTheme, language, setLanguage],
  );

  // ============================================================================
  // Help Commands (5 commands)
  // ============================================================================

  const helpCommands: Command[] = useMemo(
    () =>
      createHelpCommands({
        t,
        options,
      }),
    [t, options],
  );

  // ============================================================================
  // Register All Commands
  // ============================================================================

  const allCommands = useMemo(
    () => [
      ...navigationCommands,
      ...projectsCommands,
      ...agentsCommands,
      ...analyticsCommands,
      ...mcpCommands,
      ...timelineCommands,
      ...chatCommands,
      ...settingsCommands,
      ...helpCommands,
    ],
    [
      navigationCommands,
      projectsCommands,
      agentsCommands,
      analyticsCommands,
      mcpCommands,
      timelineCommands,
      chatCommands,
      settingsCommands,
      helpCommands,
    ],
  );

  useEffect(() => {
    registerCommands(allCommands);

    return () => {
      unregisterCommands(allCommands.map((c) => c.id));
    };
  }, [allCommands, registerCommands, unregisterCommands]);

  return {
    commandCount: allCommands.length,
  };
}

export default useGlobalCommands;
