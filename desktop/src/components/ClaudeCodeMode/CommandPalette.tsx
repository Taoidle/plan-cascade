/**
 * CommandPalette Component
 *
 * Searchable command palette with fuzzy search, keyboard navigation,
 * and extensible command registry.
 *
 * Story 011-7: Command Palette with Fuzzy Search
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
} from '@radix-ui/react-icons';
import Fuse from 'fuse.js';
import { useTranslation } from 'react-i18next';
import { useHotkeys } from 'react-hotkeys-hook';
import { formatShortcut } from './KeyboardShortcuts';

// ============================================================================
// Types
// ============================================================================

export type CommandCategory = 'chat' | 'navigation' | 'settings' | 'file' | 'help';

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
}

export interface CommandPaletteContext {
  isOpen: boolean;
  open: () => void;
  close: () => void;
  toggle: () => void;
  registerCommand: (command: Command) => void;
  unregisterCommand: (id: string) => void;
  commands: Command[];
  recentCommands: string[];
  executeCommand: (id: string) => void;
}

// ============================================================================
// Context
// ============================================================================

const CommandPaletteContext = createContext<CommandPaletteContext | null>(null);

export function useCommandPalette(): CommandPaletteContext {
  const context = useContext(CommandPaletteContext);
  if (!context) {
    throw new Error('useCommandPalette must be used within CommandPaletteProvider');
  }
  return context;
}

// ============================================================================
// CommandPaletteProvider
// ============================================================================

interface CommandPaletteProviderProps {
  children: ReactNode;
  defaultCommands?: Command[];
  maxRecentCommands?: number;
}

const RECENT_COMMANDS_KEY = 'command-palette-recent';

export function CommandPaletteProvider({
  children,
  defaultCommands = [],
  maxRecentCommands = 5,
}: CommandPaletteProviderProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [commands, setCommands] = useState<Command[]>(defaultCommands);
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

  const unregisterCommand = useCallback((id: string) => {
    setCommands((prev) => prev.filter((c) => c.id !== id));
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

  // Global keyboard shortcut to open command palette
  useHotkeys(
    'mod+/',
    (e) => {
      e.preventDefault();
      toggle();
    },
    { enableOnFormTags: true },
    [toggle],
  );

  const contextValue: CommandPaletteContext = useMemo(
    () => ({
      isOpen,
      open,
      close,
      toggle,
      registerCommand,
      unregisterCommand,
      commands,
      recentCommands,
      executeCommand,
    }),
    [isOpen, open, close, toggle, registerCommand, unregisterCommand, commands, recentCommands, executeCommand],
  );

  return (
    <CommandPaletteContext.Provider value={contextValue}>
      {children}
      <CommandPaletteDialog />
    </CommandPaletteContext.Provider>
  );
}

// ============================================================================
// CommandPaletteDialog Component
// ============================================================================

const CommandPaletteDialog = memo(function CommandPaletteDialog() {
  const { t } = useTranslation('claudeCode');
  const { isOpen, close, commands, recentCommands, executeCommand } = useCommandPalette();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Fuzzy search setup
  const fuse = useMemo(
    () =>
      new Fuse(commands, {
        keys: [
          { name: 'title', weight: 0.4 },
          { name: 'description', weight: 0.2 },
          { name: 'keywords', weight: 0.3 },
          { name: 'category', weight: 0.1 },
        ],
        threshold: 0.4,
        includeScore: true,
      }),
    [commands],
  );

  // Filter and sort commands
  const filteredCommands = useMemo(() => {
    if (!query) {
      // Show recent commands first, then all by category
      const recent = recentCommands.map((id) => commands.find((c) => c.id === id)).filter(Boolean) as Command[];

      const others = commands.filter((c) => !recentCommands.includes(c.id));

      return { recent, all: others };
    }

    const results = fuse.search(query).map((r) => r.item);
    return { recent: [], all: results };
  }, [query, commands, recentCommands, fuse]);

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
      className={clsx('fixed inset-0 z-50', 'flex items-start justify-center pt-[15vh]', 'bg-black/50')}
      onClick={close}
    >
      <div
        className={clsx(
          'w-full max-w-xl',
          'bg-white dark:bg-gray-900',
          'rounded-lg shadow-2xl',
          'overflow-hidden',
          'animate-in fade-in-0 zoom-in-95 slide-in-from-top-2',
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <MagnifyingGlassIcon className="w-5 h-5 text-gray-400" />
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
              'text-sm',
            )}
          />
          {query && (
            <button onClick={() => setQuery('')} className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800">
              <Cross2Icon className="w-4 h-4 text-gray-400" />
            </button>
          )}
        </div>

        {/* Command list */}
        <div ref={listRef} className="max-h-[50vh] overflow-auto" role="listbox">
          {allVisibleCommands.length === 0 ? (
            <div className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
              <p>{t('commandPalette.noResults')}</p>
            </div>
          ) : (
            <>
              {/* Recent commands section */}
              {filteredCommands.recent.length > 0 && (
                <div className="py-2">
                  <div className="px-4 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    {t('commandPalette.recent')}
                  </div>
                  {filteredCommands.recent.map((command, index) => (
                    <CommandItem
                      key={command.id}
                      command={command}
                      isSelected={index === selectedIndex}
                      onClick={() => executeCommand(command.id)}
                      dataIndex={index}
                    />
                  ))}
                </div>
              )}

              {/* All commands section */}
              {filteredCommands.all.length > 0 && (
                <div className="py-2">
                  {!query && filteredCommands.recent.length > 0 && (
                    <div className="px-4 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                      {t('commandPalette.allCommands')}
                    </div>
                  )}
                  {Object.entries(groupByCategory(filteredCommands.all)).map(([category, categoryCommands]) => (
                    <div key={category}>
                      {query && (
                        <div className="px-4 py-1 text-xs font-medium text-gray-400 dark:text-gray-500 uppercase">
                          {getCategoryLabel(category as CommandCategory, t)}
                        </div>
                      )}
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
        <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1">
                <kbd className="px-1 py-0.5 bg-gray-200 dark:bg-gray-700 rounded">{'\u2191\u2193'}</kbd>
                {t('commandPalette.navigate')}
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1 py-0.5 bg-gray-200 dark:bg-gray-700 rounded">Enter</kbd>
                {t('commandPalette.select')}
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1 py-0.5 bg-gray-200 dark:bg-gray-700 rounded">Esc</kbd>
                {t('commandPalette.close')}
              </span>
            </div>
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
}

const CommandItem = memo(function CommandItem({ command, isSelected, onClick, dataIndex }: CommandItemProps) {
  const Icon = command.icon || getDefaultIcon(command.category);

  return (
    <button
      data-index={dataIndex}
      onClick={onClick}
      disabled={command.disabled}
      className={clsx(
        'w-full flex items-center gap-3 px-4 py-2',
        'text-left transition-colors',
        isSelected ? 'bg-primary-100 dark:bg-primary-900/50' : 'hover:bg-gray-100 dark:hover:bg-gray-800',
        command.disabled && 'opacity-50 cursor-not-allowed',
      )}
      role="option"
      aria-selected={isSelected}
    >
      <Icon
        className={clsx(
          'w-4 h-4 flex-shrink-0',
          isSelected ? 'text-primary-600 dark:text-primary-400' : 'text-gray-400',
        )}
      />

      <div className="flex-1 min-w-0">
        <div
          className={clsx(
            'text-sm font-medium truncate',
            isSelected ? 'text-primary-900 dark:text-primary-100' : 'text-gray-900 dark:text-gray-100',
          )}
        >
          {command.title}
        </div>
        {command.description && (
          <div className="text-xs text-gray-500 dark:text-gray-400 truncate">{command.description}</div>
        )}
      </div>

      {command.shortcut && (
        <kbd
          className={clsx(
            'px-2 py-0.5 rounded text-xs font-mono',
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
  return commands.reduce(
    (acc, command) => {
      if (!acc[command.category]) {
        acc[command.category] = [];
      }
      acc[command.category].push(command);
      return acc;
    },
    {} as Record<CommandCategory, Command[]>,
  );
}

function getCategoryLabel(
  category: CommandCategory,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  t: any,
): string {
  const labels: Record<CommandCategory, string> = {
    chat: t('commandPalette.categories.chat'),
    navigation: t('commandPalette.categories.navigation'),
    settings: t('commandPalette.categories.settings'),
    file: t('commandPalette.categories.file'),
    help: t('commandPalette.categories.help'),
  };
  return labels[category] || category;
}

function getDefaultIcon(category: CommandCategory): React.ComponentType<{ className?: string }> {
  const icons: Record<CommandCategory, React.ComponentType<{ className?: string }>> = {
    chat: ChatBubbleIcon,
    navigation: ViewVerticalIcon,
    settings: GearIcon,
    file: FileTextIcon,
    help: QuestionMarkCircledIcon,
  };
  return icons[category] || FileTextIcon;
}

// ============================================================================
// Default Commands Factory
// ============================================================================

export function createDefaultCommands(
  callbacks: {
    onClearChat?: () => void;
    onExportConversation?: () => void;
    onToggleSidebar?: () => void;
    onOpenSettings?: () => void;
    onShowShortcuts?: () => void;
    onFocusInput?: () => void;
    onNewConversation?: () => void;
  },
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  t: any,
): Command[] {
  const commands: Command[] = [];

  if (callbacks.onClearChat) {
    commands.push({
      id: 'clear-chat',
      title: t('commandPalette.commands.clearChat'),
      description: t('commandPalette.commands.clearChatDesc'),
      category: 'chat',
      icon: TrashIcon,
      shortcut: 'mod+l',
      action: callbacks.onClearChat,
      keywords: ['clear', 'delete', 'reset', 'conversation'],
    });
  }

  if (callbacks.onNewConversation) {
    commands.push({
      id: 'new-conversation',
      title: t('commandPalette.commands.newConversation'),
      description: t('commandPalette.commands.newConversationDesc'),
      category: 'chat',
      icon: ChatBubbleIcon,
      shortcut: 'mod+n',
      action: callbacks.onNewConversation,
      keywords: ['new', 'start', 'fresh'],
    });
  }

  if (callbacks.onExportConversation) {
    commands.push({
      id: 'export-conversation',
      title: t('commandPalette.commands.export'),
      description: t('commandPalette.commands.exportDesc'),
      category: 'file',
      icon: DownloadIcon,
      shortcut: 'mod+e',
      action: callbacks.onExportConversation,
      keywords: ['export', 'download', 'save', 'backup'],
    });
  }

  if (callbacks.onToggleSidebar) {
    commands.push({
      id: 'toggle-sidebar',
      title: t('commandPalette.commands.toggleSidebar'),
      description: t('commandPalette.commands.toggleSidebarDesc'),
      category: 'navigation',
      icon: ViewVerticalIcon,
      shortcut: 'mod+b',
      action: callbacks.onToggleSidebar,
      keywords: ['sidebar', 'panel', 'tools', 'hide', 'show'],
    });
  }

  if (callbacks.onOpenSettings) {
    commands.push({
      id: 'open-settings',
      title: t('commandPalette.commands.openSettings'),
      description: t('commandPalette.commands.openSettingsDesc'),
      category: 'settings',
      icon: GearIcon,
      shortcut: 'mod+,',
      action: callbacks.onOpenSettings,
      keywords: ['settings', 'preferences', 'config', 'options'],
    });
  }

  if (callbacks.onShowShortcuts) {
    commands.push({
      id: 'show-shortcuts',
      title: t('commandPalette.commands.showShortcuts'),
      description: t('commandPalette.commands.showShortcutsDesc'),
      category: 'help',
      icon: KeyboardIcon,
      shortcut: 'mod+?',
      action: callbacks.onShowShortcuts,
      keywords: ['shortcuts', 'keyboard', 'hotkeys', 'help'],
    });
  }

  if (callbacks.onFocusInput) {
    commands.push({
      id: 'focus-input',
      title: t('commandPalette.commands.focusInput'),
      description: t('commandPalette.commands.focusInputDesc'),
      category: 'navigation',
      icon: ChatBubbleIcon,
      shortcut: 'mod+i',
      action: callbacks.onFocusInput,
      keywords: ['focus', 'input', 'type', 'chat'],
    });
  }

  return commands;
}

// ============================================================================
// useCommandPaletteCommands Hook
// ============================================================================

export function useRegisterCommands(commands: Command[]) {
  const { registerCommand, unregisterCommand } = useCommandPalette();

  useEffect(() => {
    commands.forEach((command) => registerCommand(command));

    return () => {
      commands.forEach((command) => unregisterCommand(command.id));
    };
  }, [commands, registerCommand, unregisterCommand]);
}

export default CommandPaletteProvider;
