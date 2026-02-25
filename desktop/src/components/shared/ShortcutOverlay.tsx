/**
 * ShortcutOverlay Component
 *
 * A keyboard shortcut overlay that toggles with Ctrl+/ and displays
 * shortcuts grouped by context. Uses react-hotkeys-hook for keyboard
 * binding and includes smooth enter/exit animations.
 *
 * Story 005: Navigation Flow Refinement
 */

import { useState, useCallback, useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useHotkeys } from 'react-hotkeys-hook';
import { Cross2Icon, KeyboardIcon } from '@radix-ui/react-icons';

// ============================================================================
// Types
// ============================================================================

interface ShortcutDefinition {
  keys: string;
  description: string;
}

interface ShortcutGroup {
  label: string;
  shortcuts: ShortcutDefinition[];
}

interface ShortcutOverlayProps {
  /** External open state control */
  isOpen?: boolean;
  /** Callback when the overlay is closed */
  onClose?: () => void;
  /** Additional shortcut groups to display */
  additionalGroups?: ShortcutGroup[];
  className?: string;
}

// ============================================================================
// Platform Detection
// ============================================================================

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform);

function formatKey(key: string): string {
  return key
    .replace(/mod/gi, isMac ? '\u2318' : 'Ctrl')
    .replace(/ctrl/gi, isMac ? '\u2303' : 'Ctrl')
    .replace(/meta/gi, isMac ? '\u2318' : 'Win')
    .replace(/alt/gi, isMac ? '\u2325' : 'Alt')
    .replace(/shift/gi, isMac ? '\u21E7' : 'Shift')
    .replace(/enter/gi, isMac ? '\u23CE' : 'Enter')
    .replace(/escape/gi, 'Esc')
    .replace(/backspace/gi, isMac ? '\u232B' : 'Backspace');
}

// ============================================================================
// Default Shortcut Groups
// ============================================================================

const DEFAULT_GROUPS: ShortcutGroup[] = [
  {
    label: 'Navigation',
    shortcuts: [
      { keys: 'mod+/', description: 'Open command palette / Toggle shortcut overlay' },
      { keys: 'mod+k', description: 'Open command palette (alternative)' },
      { keys: 'mod+]', description: 'Switch to next mode' },
      { keys: 'mod+[', description: 'Switch to previous mode' },
      { keys: 'mod+1', description: 'Switch to Claude Code mode' },
      { keys: 'mod+2', description: 'Switch to Projects mode' },
      { keys: 'mod+3', description: 'Switch to Analytics mode' },
      { keys: 'F11', description: 'Toggle fullscreen' },
    ],
  },
  {
    label: 'Chat',
    shortcuts: [
      { keys: 'mod+Enter', description: 'Send message' },
      { keys: 'Escape', description: 'Cancel generation / Clear input' },
      { keys: 'mod+l', description: 'Clear chat history' },
      { keys: 'mod+n', description: 'New conversation' },
      { keys: 'mod+e', description: 'Export conversation' },
      { keys: 'mod+i', description: 'Focus chat input' },
      { keys: 'mod+b', description: 'Toggle sidebar' },
    ],
  },
  {
    label: 'General',
    shortcuts: [
      { keys: 'mod+,', description: 'Open settings' },
      { keys: 'mod+shift+t', description: 'Toggle theme (dark/light)' },
      { keys: 'mod+shift+a', description: 'Create new agent' },
      { keys: 'mod+shift+s', description: 'Create checkpoint' },
      { keys: 'mod+shift+p', description: 'Search projects' },
      { keys: 'mod+?', description: 'Show keyboard shortcuts help' },
    ],
  },
];

// ============================================================================
// ShortcutOverlay Component
// ============================================================================

export function ShortcutOverlay({
  isOpen: externalIsOpen,
  onClose,
  additionalGroups = [],
  className,
}: ShortcutOverlayProps) {
  const [internalIsOpen, setInternalIsOpen] = useState(false);
  const [isAnimatingOut, setIsAnimatingOut] = useState(false);

  // Use external state if provided, otherwise use internal
  const isOpen = externalIsOpen !== undefined ? externalIsOpen : internalIsOpen;

  const close = useCallback(() => {
    setIsAnimatingOut(true);
    setTimeout(() => {
      setIsAnimatingOut(false);
      setInternalIsOpen(false);
      onClose?.();
    }, 150);
  }, [onClose]);

  const toggle = useCallback(() => {
    if (isOpen) {
      close();
    } else {
      setInternalIsOpen(true);
    }
  }, [isOpen, close]);

  // Toggle with Ctrl+/ (same as command palette -- if command palette is not capturing it)
  // Use a dedicated shortcut: Ctrl+Shift+/ for the overlay specifically
  useHotkeys(
    'mod+shift+/',
    (e) => {
      e.preventDefault();
      toggle();
    },
    { enableOnFormTags: true },
    [toggle],
  );

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        close();
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [isOpen, close]);

  // Merge default and additional groups
  const allGroups = useMemo(() => [...DEFAULT_GROUPS, ...additionalGroups], [additionalGroups]);

  if (!isOpen && !isAnimatingOut) return null;

  return (
    <div
      className={clsx(
        'fixed inset-0 z-50',
        'flex items-center justify-center',
        'bg-black/50 backdrop-blur-sm',
        isAnimatingOut ? 'animate-out fade-out-0 duration-150' : 'animate-in fade-in-0 duration-200',
        className,
      )}
      onClick={close}
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
    >
      <div
        className={clsx(
          'w-full max-w-2xl max-h-[80vh] overflow-auto',
          'bg-white dark:bg-gray-900',
          'rounded-xl shadow-2xl',
          'border border-gray-200 dark:border-gray-700',
          isAnimatingOut
            ? 'animate-out zoom-out-95 fade-out-0 duration-150'
            : 'animate-in zoom-in-95 fade-in-0 slide-in-from-bottom-2 duration-200',
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="sticky top-0 z-10 flex items-center justify-between px-6 py-4 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-primary-100 dark:bg-primary-900/50 rounded-lg">
              <KeyboardIcon className="w-5 h-5 text-primary-600 dark:text-primary-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-gray-900 dark:text-white">Keyboard Shortcuts</h2>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                Press{' '}
                <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-[10px] font-mono border border-gray-200 dark:border-gray-700">
                  Esc
                </kbd>{' '}
                to close
              </p>
            </div>
          </div>
          <button
            onClick={close}
            className={clsx(
              'p-2 rounded-lg',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'transition-colors duration-150',
            )}
            aria-label="Close"
          >
            <Cross2Icon className="w-5 h-5" />
          </button>
        </div>

        {/* Shortcut Groups */}
        <div className="p-6 space-y-6">
          {allGroups.map((group) => (
            <div key={group.label}>
              <h3 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
                {group.label}
              </h3>
              <div className="grid gap-1">
                {group.shortcuts.map((shortcut) => (
                  <div
                    key={shortcut.description}
                    className={clsx(
                      'flex items-center justify-between py-2 px-3 rounded-lg',
                      'hover:bg-gray-50 dark:hover:bg-gray-800/50',
                      'transition-colors duration-100',
                    )}
                  >
                    <span className="text-sm text-gray-700 dark:text-gray-300">{shortcut.description}</span>
                    <ShortcutKeys keys={shortcut.keys} />
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="sticky bottom-0 px-6 py-3 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 rounded-b-xl">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>
              Toggle this overlay: <ShortcutKeys keys="mod+shift+/" />
            </span>
            <span>
              {isMac ? '\u2318' : 'Ctrl'} = {isMac ? 'Command' : 'Control'} key
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// ShortcutKeys Component
// ============================================================================

function ShortcutKeys({ keys }: { keys: string }) {
  const parts = keys.split('+').map((k) => formatKey(k.trim()));

  return (
    <span className="inline-flex items-center gap-1 flex-shrink-0">
      {parts.map((part, i) => (
        <span key={i} className="flex items-center">
          {i > 0 && <span className="mx-0.5 text-gray-300 dark:text-gray-600">+</span>}
          <kbd
            className={clsx(
              'inline-flex items-center justify-center',
              'min-w-[24px] h-6 px-1.5',
              'bg-gray-100 dark:bg-gray-800',
              'text-xs font-mono',
              'text-gray-600 dark:text-gray-400',
              'border border-gray-200 dark:border-gray-700',
              'rounded shadow-sm',
            )}
          >
            {part}
          </kbd>
        </span>
      ))}
    </span>
  );
}

export default ShortcutOverlay;
