/**
 * App Component
 *
 * Main application layout integrating all components.
 * Supports Simple, Expert, and Claude Code modes.
 *
 * Story 004: Command Palette Enhancement - Global command palette integration
 * Story 005: Navigation Flow Refinement - Breadcrumb, contextual actions, shortcut overlay
 * Story 004 (Recovery): Resume & Recovery System - Detect and resume interrupted executions
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { ModeSwitch, AnimatedModeContent } from './components/ModeSwitch';
import { SettingsButton } from './components/SettingsButton';
import { SimpleMode } from './components/SimpleMode';
import { ExpertMode } from './components/ExpertMode';
import { ClaudeCodeMode } from './components/ClaudeCodeMode';
import { Projects } from './components/Projects';
import { Dashboard } from './components/Analytics';
import { KnowledgeBasePanel } from './components/KnowledgeBase';
import { ArtifactBrowserPanel } from './components/ArtifactBrowser';
import { SetupWizard } from './components/Settings';
import { FeatureTour } from './components/shared/FeatureTour';
import { GlobalCommandPaletteProvider, useGlobalCommandPalette } from './components/shared/CommandPalette';
import { Breadcrumb } from './components/shared/Breadcrumb';
import { ContextualActions } from './components/shared/ContextualActions';
import { ShortcutOverlay } from './components/shared/ShortcutOverlay';
import { RecoveryPrompt } from './components/shared/RecoveryPrompt';
import { useGlobalCommands } from './hooks/useGlobalCommands';
import { ShortcutsHelpDialog } from './components/ClaudeCodeMode/KeyboardShortcuts';
import { useModeStore } from './store/mode';
import { useExecutionStore } from './store/execution';
import { useOnboardingStore } from './store/onboarding';
import { useRecoveryStore } from './store/recovery';
import { clsx } from 'clsx';

// ============================================================================
// AppContent Component (uses Command Palette context)
// ============================================================================

function AppContent() {
  const { t } = useTranslation();
  const { mode, setMode } = useModeStore();
  const { status, pause, resume, cancel, reset } = useExecutionStore();
  const { open: openCommandPalette } = useGlobalCommandPalette();

  const { detectIncompleteTasks, initializeListener, cleanupListener } = useRecoveryStore();

  const [showShortcuts, setShowShortcuts] = useState(false);
  const {
    forceShowWizard,
    clearWizardTrigger,
    tourActive,
    startTour,
    endTour,
  } = useOnboardingStore();
  // ShortcutOverlay manages its own open/close state via mod+shift+/ hotkey

  const initCalled = useRef(false);

  // Initialize Tauri backend on mount
  useEffect(() => {
    if (initCalled.current) return;
    initCalled.current = true;

    invoke('init_app')
      .then(() => {
        // After backend is ready, detect incomplete tasks
        detectIncompleteTasks();
      })
      .catch((err) => {
        console.warn('Backend initialization failed:', err);
      });

    // Initialize recovery event listener
    initializeListener();

    return () => {
      cleanupListener();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const isRunning = status === 'running';

  // Callback for showing shortcuts (settings is handled by SettingsButton)
  const handleOpenSettings = useCallback(() => {
    // Settings is handled by SettingsButton component's internal state
    // This is a placeholder for command palette integration
  }, []);

  const handleShowShortcuts = useCallback(() => {
    setShowShortcuts(true);
  }, []);

  // Register global commands
  useGlobalCommands({
    onOpenSettings: handleOpenSettings,
    onShowShortcuts: handleShowShortcuts,
  });

  return (
    <div className="h-screen flex flex-col bg-gray-50 dark:bg-gray-950">
      {/* Recovery Prompt - shows when interrupted executions are detected (Story-004) */}
      <RecoveryPrompt />

      {/* First-time Setup Wizard */}
      <SetupWizard
        forceShow={forceShowWizard}
        onComplete={(launchTour) => {
          clearWizardTrigger();
          if (launchTour) {
            // Delay tour start to let wizard close animation finish
            setTimeout(() => startTour(), 400);
          }
        }}
      />

      {/* Feature Tour (triggered after wizard or from Settings) */}
      <FeatureTour
        active={tourActive}
        onFinish={endTour}
      />

      {/* Header */}
      <header
        className={clsx(
          'h-14 3xl:h-16 flex items-center justify-between px-4 3xl:px-6 5xl:px-8',
          'bg-white dark:bg-gray-900',
          'border-b border-gray-200 dark:border-gray-800',
          'shrink-0'
        )}
      >
        {/* Logo / Title + Breadcrumb */}
        <div className="flex items-center gap-3 min-w-0">
          <Logo />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-white shrink-0">
            {t('appName')}
          </h1>

          {/* Breadcrumb Navigation */}
          <Breadcrumb className="hidden md:flex ml-2" />

          {/* Status Badge */}
          {status !== 'idle' && (
            <StatusBadge status={status} />
          )}
        </div>

        {/* Controls */}
        <div className="flex items-center gap-3">
          {/* Contextual Actions */}
          <ContextualActions
            className="hidden lg:flex"
            onPauseExecution={pause}
            onResumeExecution={resume}
            onCancelExecution={cancel}
            onResetExecution={reset}
          />

          {/* Command Palette Trigger */}
          <button
            onClick={openCommandPalette}
            className={clsx(
              'hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-lg',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'text-sm transition-colors',
              'border border-gray-200 dark:border-gray-700'
            )}
            title={t('commandPalette.placeholder')}
          >
            <span className="text-gray-400">Search commands...</span>
            <kbd className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs font-mono">
              Ctrl+/
            </kbd>
          </button>

          <ModeSwitch
            mode={mode}
            onChange={setMode}
            disabled={isRunning}
          />
          <SettingsButton />
        </div>
      </header>

      {/* Main Content with animated transitions */}
      <main className="flex-1 overflow-hidden flex flex-col">
        <AnimatedModeContent mode={mode}>
          {mode === 'simple' && <div data-tour="mode-simple" className="h-full"><SimpleMode /></div>}
          {mode === 'expert' && <div data-tour="mode-expert" className="h-full"><ExpertMode /></div>}
          {mode === 'claude-code' && <div data-tour="mode-claude-code" className="h-full"><ClaudeCodeMode /></div>}
          {mode === 'projects' && <div data-tour="mode-projects" className="h-full"><Projects /></div>}
          {mode === 'analytics' && <div data-tour="mode-analytics" className="h-full"><Dashboard /></div>}
          {mode === 'knowledge' && <div data-tour="mode-knowledge" className="h-full"><KnowledgeBasePanel /></div>}
          {mode === 'artifacts' && <div data-tour="mode-artifacts" className="h-full"><ArtifactBrowserPanel /></div>}
        </AnimatedModeContent>
      </main>

      {/* Footer (optional status bar) */}
      <footer
        className={clsx(
          'h-6 3xl:h-7 flex items-center justify-between px-4 3xl:px-6 5xl:px-8',
          'bg-white dark:bg-gray-900',
          'border-t border-gray-200 dark:border-gray-800',
          'text-xs text-gray-500 dark:text-gray-400',
          'shrink-0'
        )}
      >
        <span>{t('version')}</span>
        <span>{t('ready')}</span>
      </footer>

      {/* Keyboard Shortcuts Help Dialog (legacy, from Story 004) */}
      <ShortcutsHelpDialog
        isOpen={showShortcuts}
        onClose={() => setShowShortcuts(false)}
      />

      {/* Shortcut Overlay (Story 005) - toggled via Ctrl+Shift+/ */}
      <ShortcutOverlay />
    </div>
  );
}

// ============================================================================
// App Component (wrapper with providers)
// ============================================================================

export function App() {
  return (
    <GlobalCommandPaletteProvider>
      <AppContent />
    </GlobalCommandPaletteProvider>
  );
}

function Logo() {
  return (
    <svg
      className="w-8 h-8"
      viewBox="0 0 32 32"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* Cascade layers */}
      <rect
        x="4"
        y="4"
        width="24"
        height="6"
        rx="2"
        className="fill-primary-600"
      />
      <rect
        x="6"
        y="12"
        width="20"
        height="6"
        rx="2"
        className="fill-primary-500"
      />
      <rect
        x="8"
        y="20"
        width="16"
        height="6"
        rx="2"
        className="fill-primary-400"
      />
    </svg>
  );
}

interface StatusBadgeProps {
  status: string;
}

function StatusBadge({ status }: StatusBadgeProps) {
  const { t } = useTranslation();

  const config = {
    running: {
      bg: 'bg-blue-100 dark:bg-blue-900',
      text: 'text-blue-700 dark:text-blue-300',
      label: t('status.running'),
      dot: 'bg-blue-500 animate-pulse',
    },
    paused: {
      bg: 'bg-yellow-100 dark:bg-yellow-900',
      text: 'text-yellow-700 dark:text-yellow-300',
      label: t('status.paused'),
      dot: 'bg-yellow-500',
    },
    completed: {
      bg: 'bg-green-100 dark:bg-green-900',
      text: 'text-green-700 dark:text-green-300',
      label: t('status.completed'),
      dot: 'bg-green-500',
    },
    failed: {
      bg: 'bg-red-100 dark:bg-red-900',
      text: 'text-red-700 dark:text-red-300',
      label: t('status.failed'),
      dot: 'bg-red-500',
    },
  }[status] || {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-700 dark:text-gray-300',
    label: status,
    dot: 'bg-gray-500',
  };

  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium',
        config.bg,
        config.text
      )}
    >
      <span className={clsx('w-1.5 h-1.5 rounded-full', config.dot)} />
      {config.label}
    </span>
  );
}

export default App;
