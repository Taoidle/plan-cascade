/**
 * KeyboardShortcuts Component and Registry
 *
 * Centralized keyboard shortcut management with platform-aware
 * modifier keys and customization support.
 *
 * Story 011-5: Keyboard Shortcuts Implementation
 */

import { useEffect, useCallback, useMemo, createContext, useContext, ReactNode } from 'react';
import { useHotkeys, Options as HotkeyOptions } from 'react-hotkeys-hook';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';

// ============================================================================
// Types
// ============================================================================

export interface KeyboardShortcut {
  id: string;
  keys: string;
  macKeys?: string;
  description: string;
  category: ShortcutCategory;
  action: () => void;
  enabled?: boolean;
  scope?: string;
}

export type ShortcutCategory = 'chat' | 'navigation' | 'editing' | 'settings' | 'general';

export interface ShortcutRegistry {
  shortcuts: KeyboardShortcut[];
  registerShortcut: (shortcut: KeyboardShortcut) => void;
  unregisterShortcut: (id: string) => void;
  getShortcutsByCategory: (category: ShortcutCategory) => KeyboardShortcut[];
  getShortcutKeys: (id: string) => string;
  isEnabled: (id: string) => boolean;
  setEnabled: (id: string, enabled: boolean) => void;
}

// ============================================================================
// Platform Detection
// ============================================================================

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform);

export function getPlatformModifier(): string {
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

export function getHotkeyString(shortcut: KeyboardShortcut): string {
  if (isMac && shortcut.macKeys) {
    return shortcut.macKeys;
  }
  return shortcut.keys;
}

// ============================================================================
// Default Shortcuts
// ============================================================================

export const DEFAULT_SHORTCUTS: Omit<KeyboardShortcut, 'action'>[] = [
  // Chat shortcuts
  {
    id: 'send-message',
    keys: 'mod+enter',
    description: 'Send message',
    category: 'chat',
  },
  {
    id: 'cancel-generation',
    keys: 'mod+c',
    macKeys: 'meta+c',
    description: 'Cancel AI response generation',
    category: 'chat',
  },
  {
    id: 'clear-input',
    keys: 'escape',
    description: 'Clear input or close popups',
    category: 'chat',
  },
  {
    id: 'edit-last-message',
    keys: 'up',
    description: 'Edit last user message (when input is empty)',
    category: 'chat',
  },
  {
    id: 'clear-chat',
    keys: 'mod+l',
    description: 'Clear chat history',
    category: 'chat',
  },

  // Navigation shortcuts
  {
    id: 'command-palette',
    keys: 'mod+/',
    description: 'Open command palette',
    category: 'navigation',
  },
  {
    id: 'toggle-sidebar',
    keys: 'mod+b',
    description: 'Toggle tool history sidebar',
    category: 'navigation',
  },
  {
    id: 'focus-input',
    keys: 'mod+i',
    description: 'Focus chat input',
    category: 'navigation',
  },

  // General shortcuts
  {
    id: 'export-conversation',
    keys: 'mod+e',
    description: 'Export conversation',
    category: 'general',
  },
  {
    id: 'open-settings',
    keys: 'mod+,',
    description: 'Open settings',
    category: 'general',
  },
  {
    id: 'help',
    keys: 'mod+?',
    macKeys: 'meta+shift+/',
    description: 'Show keyboard shortcuts',
    category: 'general',
  },
];

// ============================================================================
// Shortcut Context
// ============================================================================

const ShortcutContext = createContext<ShortcutRegistry | null>(null);

export function useShortcutRegistry(): ShortcutRegistry {
  const context = useContext(ShortcutContext);
  if (!context) {
    throw new Error('useShortcutRegistry must be used within ShortcutProvider');
  }
  return context;
}

// ============================================================================
// ShortcutProvider Component
// ============================================================================

interface ShortcutProviderProps {
  children: ReactNode;
  initialShortcuts?: KeyboardShortcut[];
}

export function ShortcutProvider({ children, initialShortcuts = [] }: ShortcutProviderProps) {
  const [shortcuts, setShortcuts] = useState<KeyboardShortcut[]>(initialShortcuts);
  const [enabledState, setEnabledState] = useState<Record<string, boolean>>({});

  const registerShortcut = useCallback((shortcut: KeyboardShortcut) => {
    setShortcuts((prev) => {
      const existing = prev.findIndex((s) => s.id === shortcut.id);
      if (existing >= 0) {
        const updated = [...prev];
        updated[existing] = shortcut;
        return updated;
      }
      return [...prev, shortcut];
    });
  }, []);

  const unregisterShortcut = useCallback((id: string) => {
    setShortcuts((prev) => prev.filter((s) => s.id !== id));
  }, []);

  const getShortcutsByCategory = useCallback(
    (category: ShortcutCategory) => {
      return shortcuts.filter((s) => s.category === category);
    },
    [shortcuts],
  );

  const getShortcutKeys = useCallback(
    (id: string) => {
      const shortcut = shortcuts.find((s) => s.id === id);
      if (!shortcut) return '';
      return formatShortcut(getHotkeyString(shortcut));
    },
    [shortcuts],
  );

  const isEnabled = useCallback(
    (id: string) => {
      return enabledState[id] !== false;
    },
    [enabledState],
  );

  const setEnabled = useCallback((id: string, enabled: boolean) => {
    setEnabledState((prev) => ({ ...prev, [id]: enabled }));
  }, []);

  const registry: ShortcutRegistry = useMemo(
    () => ({
      shortcuts,
      registerShortcut,
      unregisterShortcut,
      getShortcutsByCategory,
      getShortcutKeys,
      isEnabled,
      setEnabled,
    }),
    [shortcuts, registerShortcut, unregisterShortcut, getShortcutsByCategory, getShortcutKeys, isEnabled, setEnabled],
  );

  return <ShortcutContext.Provider value={registry}>{children}</ShortcutContext.Provider>;
}

// Need to import useState
import { useState } from 'react';

// ============================================================================
// useKeyboardShortcut Hook
// ============================================================================

interface UseKeyboardShortcutOptions extends Omit<HotkeyOptions, 'enabled'> {
  enabled?: boolean;
  description?: string;
  category?: ShortcutCategory;
}

export function useKeyboardShortcut(keys: string, callback: () => void, options: UseKeyboardShortcutOptions = {}) {
  const { enabled = true, description = '', category = 'general', ...hotkeyOptions } = options;

  useHotkeys(
    keys,
    (e) => {
      e.preventDefault();
      callback();
    },
    {
      enabled,
      enableOnFormTags: false,
      ...hotkeyOptions,
    },
    [callback, enabled],
  );
}

// ============================================================================
// useChatShortcuts Hook
// ============================================================================

interface ChatShortcutsCallbacks {
  onSendMessage: () => void;
  onCancelGeneration: () => void;
  onClearInput: () => void;
  onEditLastMessage: () => void;
  onClearChat: () => void;
  onOpenCommandPalette: () => void;
  onToggleSidebar: () => void;
  onExportConversation: () => void;
  onFocusInput: () => void;
}

export function useChatShortcuts(
  callbacks: Partial<ChatShortcutsCallbacks>,
  options: { enabled?: boolean; inputEmpty?: boolean; isStreaming?: boolean } = {},
) {
  const { enabled = true, inputEmpty = true, isStreaming = false } = options;

  // Send message - Ctrl/Cmd + Enter
  useHotkeys(
    'mod+enter',
    (e) => {
      e.preventDefault();
      callbacks.onSendMessage?.();
    },
    { enabled: enabled && !!callbacks.onSendMessage },
    [callbacks.onSendMessage, enabled],
  );

  // Cancel generation - Escape or Ctrl/Cmd + C (when streaming)
  useHotkeys(
    'escape',
    (e) => {
      e.preventDefault();
      if (isStreaming) {
        callbacks.onCancelGeneration?.();
      } else {
        callbacks.onClearInput?.();
      }
    },
    { enabled: enabled && (!!callbacks.onCancelGeneration || !!callbacks.onClearInput) },
    [callbacks.onCancelGeneration, callbacks.onClearInput, enabled, isStreaming],
  );

  // Edit last message - Up arrow when input is empty
  useHotkeys(
    'up',
    (e) => {
      if (inputEmpty) {
        e.preventDefault();
        callbacks.onEditLastMessage?.();
      }
    },
    { enabled: enabled && inputEmpty && !!callbacks.onEditLastMessage },
    [callbacks.onEditLastMessage, enabled, inputEmpty],
  );

  // Clear chat - Ctrl/Cmd + L
  useHotkeys(
    'mod+l',
    (e) => {
      e.preventDefault();
      callbacks.onClearChat?.();
    },
    { enabled: enabled && !!callbacks.onClearChat },
    [callbacks.onClearChat, enabled],
  );

  // Command palette - Ctrl/Cmd + /
  useHotkeys(
    'mod+/',
    (e) => {
      e.preventDefault();
      callbacks.onOpenCommandPalette?.();
    },
    { enabled: enabled && !!callbacks.onOpenCommandPalette },
    [callbacks.onOpenCommandPalette, enabled],
  );

  // Toggle sidebar - Ctrl/Cmd + B
  useHotkeys(
    'mod+b',
    (e) => {
      e.preventDefault();
      callbacks.onToggleSidebar?.();
    },
    { enabled: enabled && !!callbacks.onToggleSidebar },
    [callbacks.onToggleSidebar, enabled],
  );

  // Export conversation - Ctrl/Cmd + E
  useHotkeys(
    'mod+e',
    (e) => {
      e.preventDefault();
      callbacks.onExportConversation?.();
    },
    { enabled: enabled && !!callbacks.onExportConversation },
    [callbacks.onExportConversation, enabled],
  );

  // Focus input - Ctrl/Cmd + I
  useHotkeys(
    'mod+i',
    (e) => {
      e.preventDefault();
      callbacks.onFocusInput?.();
    },
    { enabled: enabled && !!callbacks.onFocusInput },
    [callbacks.onFocusInput, enabled],
  );
}

// ============================================================================
// ShortcutsHelpDialog Component
// ============================================================================

interface ShortcutsHelpDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ShortcutsHelpDialog({ isOpen, onClose }: ShortcutsHelpDialogProps) {
  const { t } = useTranslation('claudeCode');

  const categories: { id: ShortcutCategory; label: string }[] = [
    { id: 'chat', label: 'Chat' },
    { id: 'navigation', label: 'Navigation' },
    { id: 'general', label: 'General' },
  ];

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className={clsx('fixed inset-0 z-50', 'flex items-center justify-center', 'bg-black/50')} onClick={onClose}>
      <div
        className={clsx(
          'w-full max-w-lg max-h-[80vh] overflow-auto',
          'bg-white dark:bg-gray-900',
          'rounded-lg shadow-xl',
          'animate-in fade-in-0 zoom-in-95',
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="sticky top-0 px-6 py-4 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('shortcuts.title')}</h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            Press <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-xs">Esc</kbd> to close
          </p>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6">
          {categories.map((category) => (
            <div key={category.id}>
              <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
                {category.label}
              </h3>
              <div className="space-y-2">
                {DEFAULT_SHORTCUTS.filter((s) => s.category === category.id).map((shortcut) => (
                  <div key={shortcut.id} className="flex items-center justify-between py-2">
                    <span className="text-sm text-gray-700 dark:text-gray-300">{shortcut.description}</span>
                    <kbd
                      className={clsx(
                        'px-2 py-1 rounded',
                        'bg-gray-100 dark:bg-gray-800',
                        'text-xs font-mono',
                        'text-gray-700 dark:text-gray-300',
                        'border border-gray-200 dark:border-gray-700',
                      )}
                    >
                      {formatShortcut(getHotkeyString(shortcut as KeyboardShortcut))}
                    </kbd>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// KeyboardShortcutHint Component
// ============================================================================

interface KeyboardShortcutHintProps {
  shortcut: string;
  className?: string;
}

export function KeyboardShortcutHint({ shortcut, className }: KeyboardShortcutHintProps) {
  const formatted = formatShortcut(shortcut);
  const keys = formatted.split(' + ');

  return (
    <span className={clsx('inline-flex items-center gap-0.5', className)}>
      {keys.map((key, i) => (
        <span key={i} className="flex items-center">
          {i > 0 && <span className="mx-0.5 text-gray-400">+</span>}
          <kbd
            className={clsx(
              'px-1.5 py-0.5 rounded',
              'bg-gray-100 dark:bg-gray-800',
              'text-xs font-mono',
              'text-gray-500 dark:text-gray-400',
              'border border-gray-200 dark:border-gray-700',
            )}
          >
            {key}
          </kbd>
        </span>
      ))}
    </span>
  );
}

export default ShortcutsHelpDialog;
