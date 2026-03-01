import {
  Command,
  CommandCategory,
  // Icons
  GearIcon,
  ChatBubbleIcon,
  PlusIcon,
  FileTextIcon,
  DownloadIcon,
  TrashIcon,
  ViewVerticalIcon,
  QuestionMarkCircledIcon,
  BarChartIcon,
  PersonIcon,
  ResetIcon,
  ExternalLinkIcon,
  HomeIcon,
  GitHubLogoIcon,
  StackIcon,
  LightningBoltIcon,
  KeyboardIcon,
} from '../../components/shared/CommandPalette';
import { MODES } from '../../store/mode';

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

export interface GlobalCommandOptions {
  onOpenSettings?: () => void;
  onShowShortcuts?: () => void;
  onToggleTheme?: () => void;
  onExportData?: () => void;
  onClaudeCodeClear?: () => void;
  onClaudeCodeExport?: () => void;
  onClaudeCodeToggleSidebar?: () => void;
  sidebarVisible?: boolean;
}

export function createNavigationCommands({
  t,
  mode,
  setMode,
}: {
  t: TranslateFn;
  mode: string;
  setMode: (mode: (typeof MODES)[number]) => void;
}): Command[] {
  return [
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
      id: 'nav-knowledge',
      title: t('commands.nav.knowledge'),
      description: t('commands.nav.knowledgeDesc'),
      category: 'navigation' as CommandCategory,
      icon: FileTextIcon,
      shortcut: 'mod+4',
      action: () => setMode('knowledge'),
      keywords: ['knowledge', 'rag', 'docs', 'search', 'mode'],
      priority: 100,
      contexts: ['global'],
    },
    {
      id: 'nav-codebase',
      title: t('commands.nav.codebase'),
      description: t('commands.nav.codebaseDesc'),
      category: 'navigation' as CommandCategory,
      icon: GitHubLogoIcon,
      shortcut: 'mod+5',
      action: () => setMode('codebase'),
      keywords: ['codebase', 'index', 'symbols', 'files', 'mode'],
      priority: 100,
      contexts: ['global'],
    },
    {
      id: 'nav-artifacts',
      title: t('commands.nav.artifacts'),
      description: t('commands.nav.artifactsDesc'),
      category: 'navigation' as CommandCategory,
      icon: DownloadIcon,
      shortcut: 'mod+6',
      action: () => setMode('artifacts'),
      keywords: ['artifacts', 'versions', 'outputs', 'history', 'mode'],
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
        const currentIndex = MODES.indexOf(mode as (typeof MODES)[number]);
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
        const currentIndex = MODES.indexOf(mode as (typeof MODES)[number]);
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
  ];
}

export function createChatCommands({
  t,
  claudeCodeStore,
  options,
}: {
  t: TranslateFn;
  claudeCodeStore: {
    clearConversation: () => void;
    cleanup: () => void;
    initialize: () => void;
    connectionStatus: string;
  };
  options: GlobalCommandOptions;
}): Command[] {
  return [
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
  ];
}

export function createSettingsCommands({
  t,
  options,
  theme,
  setTheme,
  language,
  setLanguage,
}: {
  t: TranslateFn;
  options: GlobalCommandOptions;
  theme: 'system' | 'light' | 'dark';
  setTheme: (theme: 'system' | 'light' | 'dark') => void;
  language: 'en' | 'zh' | 'ja';
  setLanguage: (language: 'en' | 'zh' | 'ja') => void;
}): Command[] {
  return [
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
  ];
}

export function createHelpCommands({ t, options }: { t: TranslateFn; options: GlobalCommandOptions }): Command[] {
  return [
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
  ];
}
