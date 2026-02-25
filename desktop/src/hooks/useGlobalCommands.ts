/**
 * useGlobalCommands Hook
 *
 * Registers 50+ commands dynamically with context-aware filtering.
 * Commands are organized by category and filtered based on the current view.
 *
 * Story 004: Command Palette Enhancement
 */

import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  useGlobalCommandPalette,
  Command,
  CommandCategory,
  // Icons
  GearIcon,
  ChatBubbleIcon,
  FileTextIcon,
  DownloadIcon,
  TrashIcon,
  ViewVerticalIcon,
  QuestionMarkCircledIcon,
  RocketIcon,
  PersonIcon,
  BarChartIcon,
  MixerHorizontalIcon,
  ClockIcon,
  StackIcon,
  PlusIcon,
  ResetIcon,
  ExternalLinkIcon,
  HomeIcon,
  GitHubLogoIcon,
  LightningBoltIcon,
  ArchiveIcon,
  PlayIcon,
  StopIcon,
  CheckCircledIcon,
  KeyboardIcon,
} from '../components/shared/CommandPalette';
import { useModeStore, MODES } from '../store/mode';
import { useSettingsStore } from '../store/settings';
import { useClaudeCodeStore } from '../store/claudeCode';
import { useAgentsStore } from '../store/agents';
import { useAnalyticsStore } from '../store/analytics';
import { useProjectsStore } from '../store/projects';
import { useTimelineStore } from '../store/timeline';

// ============================================================================
// Types
// ============================================================================

interface UseGlobalCommandsOptions {
  /** Callbacks for UI actions */
  onOpenSettings?: () => void;
  onShowShortcuts?: () => void;
  onToggleTheme?: () => void;
  onExportData?: () => void;
  /** Mode-specific callbacks */
  onClaudeCodeClear?: () => void;
  onClaudeCodeExport?: () => void;
  onClaudeCodeToggleSidebar?: () => void;
  /** Current sidebar visibility for toggle */
  sidebarVisible?: boolean;
}

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

  // Update current context based on mode
  useEffect(() => {
    setCurrentContext(mode);
  }, [mode, setCurrentContext]);

  // ============================================================================
  // Navigation Commands (8 commands)
  // ============================================================================

  const navigationCommands: Command[] = useMemo(
    () => [
      {
        id: 'nav-simple-mode',
        title: t('commands.nav.simpleMode'),
        description: t('commands.nav.simpleModeDesc'),
        category: 'navigation' as CommandCategory,
        icon: HomeIcon,
        action: () => setMode('simple'),
        keywords: ['simple', 'easy', 'mode', 'switch', 'view'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'nav-expert-mode',
        title: t('commands.nav.expertMode'),
        description: t('commands.nav.expertModeDesc'),
        category: 'navigation' as CommandCategory,
        icon: LightningBoltIcon,
        action: () => setMode('expert'),
        keywords: ['expert', 'advanced', 'prd', 'mode', 'switch'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'nav-claude-code',
        title: t('commands.nav.claudeCode'),
        description: t('commands.nav.claudeCodeDesc'),
        category: 'navigation' as CommandCategory,
        icon: ChatBubbleIcon,
        shortcut: 'mod+1',
        action: () => setMode('claude-code'),
        keywords: ['claude', 'code', 'chat', 'ai', 'mode'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'nav-projects',
        title: t('commands.nav.projects'),
        description: t('commands.nav.projectsDesc'),
        category: 'navigation' as CommandCategory,
        icon: StackIcon,
        shortcut: 'mod+2',
        action: () => setMode('projects'),
        keywords: ['projects', 'sessions', 'browse', 'history'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'nav-analytics',
        title: t('commands.nav.analytics'),
        description: t('commands.nav.analyticsDesc'),
        category: 'navigation' as CommandCategory,
        icon: BarChartIcon,
        shortcut: 'mod+3',
        action: () => setMode('analytics'),
        keywords: ['analytics', 'usage', 'costs', 'stats', 'dashboard'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'nav-next-mode',
        title: t('commands.nav.nextMode'),
        description: t('commands.nav.nextModeDesc'),
        category: 'navigation' as CommandCategory,
        icon: ViewVerticalIcon,
        shortcut: 'mod+]',
        action: () => {
          const currentIndex = MODES.indexOf(mode);
          const nextIndex = (currentIndex + 1) % MODES.length;
          setMode(MODES[nextIndex]);
        },
        keywords: ['next', 'mode', 'switch', 'cycle'],
        priority: 50,
        contexts: ['global'],
      },
      {
        id: 'nav-prev-mode',
        title: t('commands.nav.prevMode'),
        description: t('commands.nav.prevModeDesc'),
        category: 'navigation' as CommandCategory,
        icon: ViewVerticalIcon,
        shortcut: 'mod+[',
        action: () => {
          const currentIndex = MODES.indexOf(mode);
          const prevIndex = (currentIndex - 1 + MODES.length) % MODES.length;
          setMode(MODES[prevIndex]);
        },
        keywords: ['previous', 'mode', 'switch', 'cycle'],
        priority: 50,
        contexts: ['global'],
      },
      {
        id: 'nav-toggle-fullscreen',
        title: t('commands.nav.toggleFullscreen'),
        description: t('commands.nav.toggleFullscreenDesc'),
        category: 'navigation' as CommandCategory,
        icon: ExternalLinkIcon,
        shortcut: 'F11',
        action: () => {
          if (document.fullscreenElement) {
            document.exitFullscreen();
          } else {
            document.documentElement.requestFullscreen();
          }
        },
        keywords: ['fullscreen', 'maximize', 'screen'],
        priority: 30,
        contexts: ['global'],
      },
    ],
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
  // MCP Commands (6 commands)
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
          // Open MCP server registry
        },
        keywords: ['mcp', 'server', 'registry', 'browse'],
        priority: 90,
        contexts: ['expert', 'global'],
      },
      {
        id: 'mcp-add-server',
        title: t('commands.mcp.addServer'),
        description: t('commands.mcp.addServerDesc'),
        category: 'mcp' as CommandCategory,
        icon: PlusIcon,
        action: () => {
          // Open add MCP server dialog
        },
        keywords: ['mcp', 'add', 'server', 'new'],
        priority: 85,
        contexts: ['expert'],
      },
      {
        id: 'mcp-refresh-servers',
        title: t('commands.mcp.refreshServers'),
        description: t('commands.mcp.refreshServersDesc'),
        category: 'mcp' as CommandCategory,
        icon: ResetIcon,
        action: () => {
          // Refresh MCP servers
        },
        keywords: ['mcp', 'refresh', 'servers', 'reload'],
        priority: 70,
        contexts: ['expert'],
      },
      {
        id: 'mcp-import-config',
        title: t('commands.mcp.importConfig'),
        description: t('commands.mcp.importConfigDesc'),
        category: 'mcp' as CommandCategory,
        icon: ArchiveIcon,
        action: () => {
          // Import MCP config
        },
        keywords: ['mcp', 'import', 'config', 'configuration'],
        priority: 60,
        contexts: ['expert'],
      },
      {
        id: 'mcp-export-config',
        title: t('commands.mcp.exportConfig'),
        description: t('commands.mcp.exportConfigDesc'),
        category: 'mcp' as CommandCategory,
        icon: DownloadIcon,
        action: () => {
          // Export MCP config
        },
        keywords: ['mcp', 'export', 'config', 'backup'],
        priority: 55,
        contexts: ['expert'],
      },
      {
        id: 'mcp-test-connection',
        title: t('commands.mcp.testConnection'),
        description: t('commands.mcp.testConnectionDesc'),
        category: 'mcp' as CommandCategory,
        icon: CheckCircledIcon,
        action: () => {
          // Test MCP server connections
        },
        keywords: ['mcp', 'test', 'connection', 'verify'],
        priority: 65,
        contexts: ['expert'],
      },
    ],
    [t],
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
    () => [
      {
        id: 'chat-clear',
        title: t('commands.chat.clear'),
        description: t('commands.chat.clearDesc'),
        category: 'chat' as CommandCategory,
        icon: TrashIcon,
        shortcut: 'mod+l',
        action: () => {
          if (confirm(t('commands.chat.clearConfirm'))) {
            claudeCodeStore.clearConversation();
            options.onClaudeCodeClear?.();
          }
        },
        keywords: ['clear', 'chat', 'conversation', 'reset'],
        priority: 70,
        contexts: ['claude-code'],
      },
      {
        id: 'chat-new',
        title: t('commands.chat.new'),
        description: t('commands.chat.newDesc'),
        category: 'chat' as CommandCategory,
        icon: PlusIcon,
        shortcut: 'mod+n',
        action: () => {
          claudeCodeStore.clearConversation();
        },
        keywords: ['new', 'chat', 'conversation', 'fresh'],
        priority: 80,
        contexts: ['claude-code'],
      },
      {
        id: 'chat-export',
        title: t('commands.chat.export'),
        description: t('commands.chat.exportDesc'),
        category: 'chat' as CommandCategory,
        icon: DownloadIcon,
        shortcut: 'mod+e',
        action: () => {
          options.onClaudeCodeExport?.();
        },
        keywords: ['export', 'chat', 'conversation', 'download', 'save'],
        priority: 75,
        contexts: ['claude-code'],
      },
      {
        id: 'chat-toggle-sidebar',
        title: options.sidebarVisible ? t('commands.chat.hideSidebar') : t('commands.chat.showSidebar'),
        description: t('commands.chat.toggleSidebarDesc'),
        category: 'chat' as CommandCategory,
        icon: ViewVerticalIcon,
        shortcut: 'mod+b',
        action: () => {
          options.onClaudeCodeToggleSidebar?.();
        },
        keywords: ['sidebar', 'toggle', 'panel', 'tools'],
        priority: 60,
        contexts: ['claude-code'],
      },
      {
        id: 'chat-focus-input',
        title: t('commands.chat.focusInput'),
        description: t('commands.chat.focusInputDesc'),
        category: 'chat' as CommandCategory,
        icon: ChatBubbleIcon,
        shortcut: 'mod+i',
        action: () => {
          const input = document.querySelector('[data-chat-input]') as HTMLTextAreaElement;
          input?.focus();
        },
        keywords: ['focus', 'input', 'chat', 'type'],
        priority: 55,
        contexts: ['claude-code'],
      },
      {
        id: 'chat-reconnect',
        title: t('commands.chat.reconnect'),
        description: t('commands.chat.reconnectDesc'),
        category: 'chat' as CommandCategory,
        icon: ResetIcon,
        action: () => {
          claudeCodeStore.cleanup();
          claudeCodeStore.initialize();
        },
        keywords: ['reconnect', 'connection', 'websocket', 'refresh'],
        priority: 50,
        disabled: claudeCodeStore.connectionStatus === 'connected',
        contexts: ['claude-code'],
      },
    ],
    [t, claudeCodeStore, options],
  );

  // ============================================================================
  // Settings Commands (8 commands)
  // ============================================================================

  const settingsCommands: Command[] = useMemo(
    () => [
      {
        id: 'settings-open',
        title: t('commands.settings.open'),
        description: t('commands.settings.openDesc'),
        category: 'settings' as CommandCategory,
        icon: GearIcon,
        shortcut: 'mod+,',
        action: () => {
          options.onOpenSettings?.();
        },
        keywords: ['settings', 'preferences', 'config', 'options'],
        priority: 100,
        contexts: ['global'],
      },
      {
        id: 'settings-theme-light',
        title: t('commands.settings.themeLight'),
        description: t('commands.settings.themeLightDesc'),
        category: 'settings' as CommandCategory,
        icon: GearIcon,
        action: () => setTheme('light'),
        keywords: ['theme', 'light', 'appearance', 'mode'],
        priority: 60,
        disabled: theme === 'light',
        contexts: ['global'],
      },
      {
        id: 'settings-theme-dark',
        title: t('commands.settings.themeDark'),
        description: t('commands.settings.themeDarkDesc'),
        category: 'settings' as CommandCategory,
        icon: GearIcon,
        action: () => setTheme('dark'),
        keywords: ['theme', 'dark', 'appearance', 'mode'],
        priority: 60,
        disabled: theme === 'dark',
        contexts: ['global'],
      },
      {
        id: 'settings-theme-system',
        title: t('commands.settings.themeSystem'),
        description: t('commands.settings.themeSystemDesc'),
        category: 'settings' as CommandCategory,
        icon: GearIcon,
        action: () => setTheme('system'),
        keywords: ['theme', 'system', 'auto', 'appearance'],
        priority: 55,
        disabled: theme === 'system',
        contexts: ['global'],
      },
      {
        id: 'settings-theme-toggle',
        title: t('commands.settings.themeToggle'),
        description: t('commands.settings.themeToggleDesc'),
        category: 'settings' as CommandCategory,
        icon: GearIcon,
        shortcut: 'mod+shift+t',
        action: () => {
          const newTheme = theme === 'dark' ? 'light' : 'dark';
          setTheme(newTheme);
        },
        keywords: ['toggle', 'theme', 'dark', 'light'],
        priority: 70,
        contexts: ['global'],
      },
      {
        id: 'settings-language-en',
        title: t('commands.settings.languageEn'),
        description: t('commands.settings.languageEnDesc'),
        category: 'settings' as CommandCategory,
        icon: PersonIcon,
        action: () => setLanguage('en'),
        keywords: ['language', 'english', 'locale'],
        priority: 45,
        disabled: language === 'en',
        contexts: ['global'],
      },
      {
        id: 'settings-language-zh',
        title: t('commands.settings.languageZh'),
        description: t('commands.settings.languageZhDesc'),
        category: 'settings' as CommandCategory,
        icon: PersonIcon,
        action: () => setLanguage('zh'),
        keywords: ['language', 'chinese', 'locale', 'zhongwen'],
        priority: 45,
        disabled: language === 'zh',
        contexts: ['global'],
      },
      {
        id: 'settings-language-ja',
        title: t('commands.settings.languageJa'),
        description: t('commands.settings.languageJaDesc'),
        category: 'settings' as CommandCategory,
        icon: PersonIcon,
        action: () => setLanguage('ja'),
        keywords: ['language', 'japanese', 'locale', 'nihongo'],
        priority: 45,
        disabled: language === 'ja',
        contexts: ['global'],
      },
    ],
    [t, options, theme, setTheme, language, setLanguage],
  );

  // ============================================================================
  // Help Commands (5 commands)
  // ============================================================================

  const helpCommands: Command[] = useMemo(
    () => [
      {
        id: 'help-shortcuts',
        title: t('commands.help.shortcuts'),
        description: t('commands.help.shortcutsDesc'),
        category: 'help' as CommandCategory,
        icon: KeyboardIcon,
        shortcut: 'mod+?',
        action: () => {
          options.onShowShortcuts?.();
        },
        keywords: ['help', 'shortcuts', 'keyboard', 'hotkeys'],
        priority: 90,
        contexts: ['global'],
      },
      {
        id: 'help-documentation',
        title: t('commands.help.documentation'),
        description: t('commands.help.documentationDesc'),
        category: 'help' as CommandCategory,
        icon: FileTextIcon,
        action: () => {
          window.open('https://github.com/anthropics/claude-code', '_blank');
        },
        keywords: ['help', 'docs', 'documentation', 'guide'],
        priority: 85,
        contexts: ['global'],
      },
      {
        id: 'help-github',
        title: t('commands.help.github'),
        description: t('commands.help.githubDesc'),
        category: 'help' as CommandCategory,
        icon: GitHubLogoIcon,
        action: () => {
          window.open('https://github.com/anthropics/claude-code', '_blank');
        },
        keywords: ['github', 'source', 'code', 'repository'],
        priority: 80,
        contexts: ['global'],
      },
      {
        id: 'help-report-issue',
        title: t('commands.help.reportIssue'),
        description: t('commands.help.reportIssueDesc'),
        category: 'help' as CommandCategory,
        icon: QuestionMarkCircledIcon,
        action: () => {
          window.open('https://github.com/anthropics/claude-code/issues/new', '_blank');
        },
        keywords: ['report', 'issue', 'bug', 'feedback'],
        priority: 70,
        contexts: ['global'],
      },
      {
        id: 'help-about',
        title: t('commands.help.about'),
        description: t('commands.help.aboutDesc'),
        category: 'help' as CommandCategory,
        icon: QuestionMarkCircledIcon,
        action: () => {
          // Show about dialog
          alert(t('commands.help.aboutContent'));
        },
        keywords: ['about', 'version', 'info', 'app'],
        priority: 60,
        contexts: ['global'],
      },
    ],
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
