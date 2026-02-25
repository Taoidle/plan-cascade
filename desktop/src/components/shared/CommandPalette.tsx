/**
 * Global Command Palette Component
 *
 * Comprehensive command palette with fuzzy search, keyboard navigation,
 * context-aware filtering, and category organization.
 *
 * Story 004: Command Palette Enhancement
 */

import { useState, useCallback, useEffect, useRef, memo, createContext, useContext, ReactNode, useMemo } from 'react';
import { clsx } from 'clsx';
import {
  MagnifyingGlassIcon,
  Cross2Icon,
  KeyboardIcon,
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
} from '@radix-ui/react-icons';
import Fuse, { type FuseResultMatch } from 'fuse.js';
import { useTranslation } from 'react-i18next';
import { useHotkeys } from 'react-hotkeys-hook';

// ============================================================================
// Types
// ============================================================================

export type CommandCategory =
  | 'projects'
  | 'agents'
  | 'analytics'
  | 'mcp'
  | 'timeline'
  | 'settings'
  | 'navigation'
  | 'chat'
  | 'file'
  | 'help';

export interface Command {
  id: string;
  title: string;
  description?: string;
  category: CommandCategory;
  icon?: React.ComponentType<{ className?: string }>;
  shortcut?: string;
  action: () => void;
  keywords?: string[];
  disabled?: boolean;
  /** Contexts where this command should be visible */
  contexts?: string[];
  /** Priority for sorting (higher = more important) */
  priority?: number;
}

export interface CommandGroup {
  category: CommandCategory;
  commands: Command[];
}

export interface GlobalCommandPaletteContext {
  isOpen: boolean;
  open: () => void;
  close: () => void;
  toggle: () => void;
  registerCommand: (command: Command) => void;
  registerCommands: (commands: Command[]) => void;
  unregisterCommand: (id: string) => void;
  unregisterCommands: (ids: string[]) => void;
  commands: Command[];
  recentCommands: string[];
  executeCommand: (id: string) => void;
  /** Current context for filtering commands */
  currentContext: string;
  setCurrentContext: (context: string) => void;
}

// ============================================================================
// Context
// ============================================================================

const GlobalCommandPaletteContext = createContext<GlobalCommandPaletteContext | null>(null);

export function useGlobalCommandPalette(): GlobalCommandPaletteContext {
  const context = useContext(GlobalCommandPaletteContext);
  if (!context) {
    throw new Error('useGlobalCommandPalette must be used within GlobalCommandPaletteProvider');
  }
  return context;
}

// ============================================================================
// Platform Detection
// ============================================================================

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform);

function getPlatformModifier(): string {
  return isMac ? 'Cmd' : 'Ctrl';
}

export function formatShortcut(keys: string): string {
  const modifier = getPlatformModifier();
  return keys
    .replace(/mod/gi, modifier)
    .replace(/ctrl/gi, isMac ? 'Ctrl' : 'Ctrl')
    .replace(/meta/gi, isMac ? 'Cmd' : 'Win')
    .replace(/alt/gi, isMac ? 'Option' : 'Alt')
    .replace(/shift/gi, 'Shift')
    .replace(/\+/g, ' + ');
}

// ============================================================================
// GlobalCommandPaletteProvider
// ============================================================================

interface GlobalCommandPaletteProviderProps {
  children: ReactNode;
  defaultCommands?: Command[];
  maxRecentCommands?: number;
  initialContext?: string;
}

const RECENT_COMMANDS_KEY = 'global-command-palette-recent';

export function GlobalCommandPaletteProvider({
  children,
  defaultCommands = [],
  maxRecentCommands = 8,
  initialContext = 'global',
}: GlobalCommandPaletteProviderProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [commands, setCommands] = useState<Command[]>(defaultCommands);
  const [currentContext, setCurrentContext] = useState(initialContext);
  const [recentCommands, setRecentCommands] = useState<string[]>(() => {
    try {
      const stored = localStorage.getItem(RECENT_COMMANDS_KEY);
      return stored ? JSON.parse(stored) : [];
    } catch {
      return [];
    }
  });

  // Save recent commands to localStorage
  useEffect(() => {
    localStorage.setItem(RECENT_COMMANDS_KEY, JSON.stringify(recentCommands));
  }, [recentCommands]);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);
  const toggle = useCallback(() => setIsOpen((prev) => !prev), []);

  const registerCommand = useCallback((command: Command) => {
    setCommands((prev) => {
      const existing = prev.findIndex((c) => c.id === command.id);
      if (existing >= 0) {
        const updated = [...prev];
        updated[existing] = command;
        return updated;
      }
      return [...prev, command];
    });
  }, []);

  const registerCommands = useCallback((newCommands: Command[]) => {
    setCommands((prev) => {
      const updated = [...prev];
      newCommands.forEach((command) => {
        const existing = updated.findIndex((c) => c.id === command.id);
        if (existing >= 0) {
          updated[existing] = command;
        } else {
          updated.push(command);
        }
      });
      return updated;
    });
  }, []);

  const unregisterCommand = useCallback((id: string) => {
    setCommands((prev) => prev.filter((c) => c.id !== id));
  }, []);

  const unregisterCommands = useCallback((ids: string[]) => {
    setCommands((prev) => prev.filter((c) => !ids.includes(c.id)));
  }, []);

  const executeCommand = useCallback(
    (id: string) => {
      const command = commands.find((c) => c.id === id);
      if (command && !command.disabled) {
        command.action();

        // Update recent commands
        setRecentCommands((prev) => {
          const filtered = prev.filter((cid) => cid !== id);
          return [id, ...filtered].slice(0, maxRecentCommands);
        });

        close();
      }
    },
    [commands, close, maxRecentCommands],
  );

  // Global keyboard shortcut to open command palette (Ctrl+/ or Cmd+/)
  useHotkeys(
    'mod+/',
    (e) => {
      e.preventDefault();
      toggle();
    },
    { enableOnFormTags: true },
    [toggle],
  );

  // Alternative shortcut: Ctrl+K or Cmd+K
  useHotkeys(
    'mod+k',
    (e) => {
      e.preventDefault();
      toggle();
    },
    { enableOnFormTags: true },
    [toggle],
  );

  const contextValue: GlobalCommandPaletteContext = useMemo(
    () => ({
      isOpen,
      open,
      close,
      toggle,
      registerCommand,
      registerCommands,
      unregisterCommand,
      unregisterCommands,
      commands,
      recentCommands,
      executeCommand,
      currentContext,
      setCurrentContext,
    }),
    [
      isOpen,
      open,
      close,
      toggle,
      registerCommand,
      registerCommands,
      unregisterCommand,
      unregisterCommands,
      commands,
      recentCommands,
      executeCommand,
      currentContext,
    ],
  );

  return (
    <GlobalCommandPaletteContext.Provider value={contextValue}>
      {children}
      <GlobalCommandPaletteDialog />
    </GlobalCommandPaletteContext.Provider>
  );
}

// ============================================================================
// GlobalCommandPaletteDialog Component
// ============================================================================

const GlobalCommandPaletteDialog = memo(function GlobalCommandPaletteDialog() {
  const { t } = useTranslation('common');
  const { isOpen, close, commands, recentCommands, executeCommand, currentContext } = useGlobalCommandPalette();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Filter commands based on context
  const contextFilteredCommands = useMemo(() => {
    return commands.filter((cmd) => {
      if (!cmd.contexts || cmd.contexts.length === 0) return true;
      return cmd.contexts.includes(currentContext) || cmd.contexts.includes('global');
    });
  }, [commands, currentContext]);

  // Fuzzy search setup
  const fuse = useMemo(
    () =>
      new Fuse(contextFilteredCommands, {
        keys: [
          { name: 'title', weight: 0.4 },
          { name: 'description', weight: 0.2 },
          { name: 'keywords', weight: 0.3 },
          { name: 'category', weight: 0.1 },
        ],
        threshold: 0.4,
        includeScore: true,
        includeMatches: true,
      }),
    [contextFilteredCommands],
  );

  // Filter and sort commands
  const filteredCommands = useMemo(() => {
    if (!query) {
      // Show recent commands first, then all by category
      const recent = recentCommands
        .map((id) => contextFilteredCommands.find((c) => c.id === id))
        .filter(Boolean) as Command[];

      const others = contextFilteredCommands
        .filter((c) => !recentCommands.includes(c.id))
        .sort((a, b) => (b.priority || 0) - (a.priority || 0));

      return { recent, all: others, matches: null };
    }

    const results = fuse.search(query);
    return {
      recent: [],
      all: results.map((r) => r.item),
      matches: results.reduce(
        (acc, r) => {
          if (r.matches) {
            acc[r.item.id] = r.matches;
          }
          return acc;
        },
        {} as Record<string, readonly FuseResultMatch[]>,
      ),
    };
  }, [query, contextFilteredCommands, recentCommands, fuse]);

  const allVisibleCommands = [...filteredCommands.recent, ...filteredCommands.all];

  // Reset state when opening
  useEffect(() => {
    if (isOpen) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [isOpen]);

  // Reset selection when query changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex((prev) => (prev < allVisibleCommands.length - 1 ? prev + 1 : prev));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
          break;
        case 'Enter':
          e.preventDefault();
          if (allVisibleCommands[selectedIndex]) {
            executeCommand(allVisibleCommands[selectedIndex].id);
          }
          break;
        case 'Escape':
          e.preventDefault();
          close();
          break;
        case 'Tab':
          e.preventDefault();
          if (e.shiftKey) {
            setSelectedIndex((prev) => (prev > 0 ? prev - 1 : allVisibleCommands.length - 1));
          } else {
            setSelectedIndex((prev) => (prev < allVisibleCommands.length - 1 ? prev + 1 : 0));
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, selectedIndex, allVisibleCommands, executeCommand, close]);

  // Scroll selected item into view
  useEffect(() => {
    if (listRef.current) {
      const selectedElement = listRef.current.querySelector(`[data-index="${selectedIndex}"]`) as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [selectedIndex]);

  if (!isOpen) return null;

  return (
    <div
      className={clsx(
        'fixed inset-0 z-50',
        'flex items-start justify-center pt-[12vh]',
        'bg-black/50 backdrop-blur-sm',
      )}
      onClick={close}
    >
      <div
        className={clsx(
          'w-full max-w-2xl',
          'bg-white dark:bg-gray-900',
          'rounded-xl shadow-2xl',
          'overflow-hidden',
          'border border-gray-200 dark:border-gray-700',
          'animate-in fade-in-0 zoom-in-95 slide-in-from-top-2',
          'duration-200',
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <MagnifyingGlassIcon className="w-5 h-5 text-gray-400 flex-shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t('commandPalette.placeholder')}
            className={clsx(
              'flex-1 bg-transparent',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'outline-none',
              'text-base',
            )}
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            >
              <Cross2Icon className="w-4 h-4 text-gray-400" />
            </button>
          )}
          <kbd className="hidden sm:inline-flex px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded text-xs text-gray-500 dark:text-gray-400 font-mono">
            {isMac ? 'Cmd' : 'Ctrl'}+/
          </kbd>
        </div>

        {/* Command list */}
        <div ref={listRef} className="max-h-[60vh] overflow-auto" role="listbox">
          {allVisibleCommands.length === 0 ? (
            <div className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
              <MagnifyingGlassIcon className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <p className="font-medium">{t('commandPalette.noResults')}</p>
              <p className="text-sm mt-1">{t('commandPalette.noResultsHint')}</p>
            </div>
          ) : (
            <>
              {/* Recent commands section */}
              {filteredCommands.recent.length > 0 && (
                <div className="py-2">
                  <div className="px-4 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
                    {t('commandPalette.recent')}
                  </div>
                  {filteredCommands.recent.map((command, index) => (
                    <CommandItem
                      key={command.id}
                      command={command}
                      isSelected={index === selectedIndex}
                      onClick={() => executeCommand(command.id)}
                      dataIndex={index}
                      matches={filteredCommands.matches?.[command.id]}
                    />
                  ))}
                </div>
              )}

              {/* All commands grouped by category */}
              {filteredCommands.all.length > 0 && (
                <div className="py-2">
                  {!query && filteredCommands.recent.length > 0 && (
                    <div className="px-4 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
                      {t('commandPalette.allCommands')}
                    </div>
                  )}
                  {query
                    ? // When searching, show flat list
                      filteredCommands.all.map((command) => {
                        const globalIndex = filteredCommands.recent.length + filteredCommands.all.indexOf(command);
                        return (
                          <CommandItem
                            key={command.id}
                            command={command}
                            isSelected={globalIndex === selectedIndex}
                            onClick={() => executeCommand(command.id)}
                            dataIndex={globalIndex}
                            matches={filteredCommands.matches?.[command.id]}
                          />
                        );
                      })
                    : // When not searching, group by category
                      Object.entries(groupByCategory(filteredCommands.all)).map(([category, categoryCommands]) => (
                        <div key={category} className="mt-2 first:mt-0">
                          <div className="px-4 py-1 text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide">
                            {getCategoryLabel(category as CommandCategory, t)}
                          </div>
                          {categoryCommands.map((command) => {
                            const globalIndex = filteredCommands.recent.length + filteredCommands.all.indexOf(command);
                            return (
                              <CommandItem
                                key={command.id}
                                command={command}
                                isSelected={globalIndex === selectedIndex}
                                onClick={() => executeCommand(command.id)}
                                dataIndex={globalIndex}
                              />
                            );
                          })}
                        </div>
                      ))}
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-[10px] font-mono">
                  {'\u2191\u2193'}
                </kbd>
                {t('commandPalette.navigate')}
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-[10px] font-mono">Enter</kbd>
                {t('commandPalette.select')}
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-[10px] font-mono">Esc</kbd>
                {t('commandPalette.close')}
              </span>
            </div>
            <span className="text-gray-400 dark:text-gray-500">
              {allVisibleCommands.length} {t('commandPalette.commandsAvailable')}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
});

// ============================================================================
// CommandItem Component
// ============================================================================

interface CommandItemProps {
  command: Command;
  isSelected: boolean;
  onClick: () => void;
  dataIndex: number;
  matches?: readonly FuseResultMatch[];
}

const CommandItem = memo(function CommandItem({ command, isSelected, onClick, dataIndex, matches }: CommandItemProps) {
  const Icon = command.icon || getDefaultIcon(command.category);

  // Highlight matched text
  const highlightText = (text: string, key: string) => {
    if (!matches) return text;

    const match = matches.find((m) => m.key === key);
    if (!match || !match.indices || match.indices.length === 0) return text;

    const indices = match.indices;
    let result: React.ReactNode[] = [];
    let lastIndex = 0;

    indices.forEach((range: readonly [number, number], i: number) => {
      const [start, end] = range;
      if (start > lastIndex) {
        result.push(text.slice(lastIndex, start));
      }
      result.push(
        <mark key={i} className="bg-yellow-200 dark:bg-yellow-500/30 text-inherit rounded-sm px-0.5">
          {text.slice(start, end + 1)}
        </mark>,
      );
      lastIndex = end + 1;
    });

    if (lastIndex < text.length) {
      result.push(text.slice(lastIndex));
    }

    return result;
  };

  return (
    <button
      data-index={dataIndex}
      onClick={onClick}
      disabled={command.disabled}
      className={clsx(
        'w-full flex items-center gap-3 px-4 py-2.5',
        'text-left transition-colors',
        isSelected ? 'bg-primary-100 dark:bg-primary-900/50' : 'hover:bg-gray-100 dark:hover:bg-gray-800',
        command.disabled && 'opacity-50 cursor-not-allowed',
      )}
      role="option"
      aria-selected={isSelected}
    >
      <div
        className={clsx(
          'w-8 h-8 flex items-center justify-center rounded-lg flex-shrink-0',
          isSelected ? 'bg-primary-200 dark:bg-primary-800' : 'bg-gray-100 dark:bg-gray-800',
        )}
      >
        <Icon
          className={clsx(
            'w-4 h-4',
            isSelected ? 'text-primary-700 dark:text-primary-300' : 'text-gray-500 dark:text-gray-400',
          )}
        />
      </div>

      <div className="flex-1 min-w-0">
        <div
          className={clsx(
            'text-sm font-medium truncate',
            isSelected ? 'text-primary-900 dark:text-primary-100' : 'text-gray-900 dark:text-gray-100',
          )}
        >
          {highlightText(command.title, 'title')}
        </div>
        {command.description && (
          <div className="text-xs text-gray-500 dark:text-gray-400 truncate mt-0.5">
            {highlightText(command.description, 'description')}
          </div>
        )}
      </div>

      {command.shortcut && (
        <kbd
          className={clsx(
            'px-2 py-1 rounded text-xs font-mono flex-shrink-0',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-500 dark:text-gray-400',
            'border border-gray-200 dark:border-gray-700',
          )}
        >
          {formatShortcut(command.shortcut)}
        </kbd>
      )}
    </button>
  );
});

// ============================================================================
// Helper Functions
// ============================================================================

function groupByCategory(commands: Command[]): Record<CommandCategory, Command[]> {
  const categoryOrder: CommandCategory[] = [
    'navigation',
    'projects',
    'agents',
    'analytics',
    'mcp',
    'timeline',
    'chat',
    'file',
    'settings',
    'help',
  ];

  const grouped = commands.reduce(
    (acc, command) => {
      if (!acc[command.category]) {
        acc[command.category] = [];
      }
      acc[command.category].push(command);
      return acc;
    },
    {} as Record<CommandCategory, Command[]>,
  );

  // Sort categories by predefined order
  const sortedGrouped: Record<CommandCategory, Command[]> = {} as Record<CommandCategory, Command[]>;
  categoryOrder.forEach((cat) => {
    if (grouped[cat]) {
      sortedGrouped[cat] = grouped[cat];
    }
  });

  return sortedGrouped;
}

function getCategoryLabel(
  category: CommandCategory,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  t: any,
): string {
  const labels: Record<CommandCategory, string> = {
    projects: t('commandPalette.categories.projects'),
    agents: t('commandPalette.categories.agents'),
    analytics: t('commandPalette.categories.analytics'),
    mcp: t('commandPalette.categories.mcp'),
    timeline: t('commandPalette.categories.timeline'),
    settings: t('commandPalette.categories.settings'),
    navigation: t('commandPalette.categories.navigation'),
    chat: t('commandPalette.categories.chat'),
    file: t('commandPalette.categories.file'),
    help: t('commandPalette.categories.help'),
  };
  return labels[category] || category;
}

function getDefaultIcon(category: CommandCategory): React.ComponentType<{ className?: string }> {
  const icons: Record<CommandCategory, React.ComponentType<{ className?: string }>> = {
    projects: StackIcon,
    agents: RocketIcon,
    analytics: BarChartIcon,
    mcp: MixerHorizontalIcon,
    timeline: ClockIcon,
    settings: GearIcon,
    navigation: ViewVerticalIcon,
    chat: ChatBubbleIcon,
    file: FileTextIcon,
    help: QuestionMarkCircledIcon,
  };
  return icons[category] || FileTextIcon;
}

// ============================================================================
// Exports for use in other components
// ============================================================================

export {
  MagnifyingGlassIcon,
  Cross2Icon,
  KeyboardIcon,
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
};

export default GlobalCommandPaletteProvider;
