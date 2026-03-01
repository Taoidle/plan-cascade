/**
 * App Component
 *
 * Main application layout integrating all components.
 * Uses a top navigation bar with hamburger menu for mode switching.
 *
 * Story 004: Command Palette Enhancement - Global command palette integration
 * Story 005: Navigation Flow Refinement - Contextual actions, shortcut overlay
 * Story 004 (Recovery): Resume & Recovery System - Detect and resume interrupted executions
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { AnimatedModeContent } from './components/ModeSwitch';
import { TopNavBar } from './components/TopNavBar';
import { SimpleMode } from './components/SimpleMode';
import { ExpertMode } from './components/ExpertMode';
import { ClaudeCodeMode } from './components/ClaudeCodeMode';
import { Projects } from './components/Projects';
import { Dashboard } from './components/Analytics';
import { KnowledgeBasePanel } from './components/KnowledgeBase';
import { CodebasePanel } from './components/Codebase';
import { ArtifactBrowserPanel } from './components/ArtifactBrowser';
import { SetupWizard } from './components/Settings';
import { FeatureTour } from './components/shared/FeatureTour';
import { GlobalCommandPaletteProvider, useGlobalCommandPalette } from './components/shared/CommandPalette';
import { ShortcutOverlay } from './components/shared/ShortcutOverlay';
import { RecoveryPrompt } from './components/shared/RecoveryPrompt';
import { ToastProvider } from './components/shared/Toast';
import { useGlobalCommands } from './hooks/useGlobalCommands';
import { ShortcutsHelpDialog } from './components/ClaudeCodeMode/KeyboardShortcuts';
import { useModeStore } from './store/mode';
import { useOnboardingStore } from './store/onboarding';
import { useRecoveryStore } from './store/recovery';
import { usePermissionPolicyStore } from './store/permissionPolicy';

const STARTUP_MIN_DURATION_MS = 3000;
const STARTUP_MAX_DURATION_MS = 5000;
const STARTUP_FADE_OUT_MS = 450;
const INIT_PROGRESS_EVENT = 'app-init-progress';
const DEFAULT_STARTUP_TOTAL_STEPS = 6;

type BackendInitStage =
  | 'core_state'
  | 'plugins'
  | 'index_manager'
  | 'spec_interview'
  | 'recovery_scan'
  | 'remote_gateway';

type StartupStage = 'booting' | BackendInitStage | 'ready';

interface InitProgressPayload {
  stage: BackendInitStage;
  step_index: number;
  total_steps: number;
}

function StartupSplash({
  title,
  stageLabel,
  progressLabel,
  progressPercent,
  exiting,
}: {
  title: string;
  stageLabel: string;
  progressLabel: string;
  progressPercent: number;
  exiting: boolean;
}) {
  return (
    <div
      aria-live="polite"
      aria-busy="true"
      className={`fixed inset-0 z-[120] flex items-center justify-center overflow-hidden bg-slate-50 dark:bg-slate-950 transition-opacity duration-500 ${
        exiting ? 'pointer-events-none opacity-0' : 'opacity-100'
      }`}
    >
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,rgba(14,165,233,0.15),transparent_52%),radial-gradient(circle_at_80%_70%,rgba(37,99,235,0.14),transparent_48%)] dark:bg-[radial-gradient(circle_at_20%_20%,rgba(56,189,248,0.18),transparent_50%),radial-gradient(circle_at_80%_70%,rgba(59,130,246,0.2),transparent_45%)] animate-startup-glow motion-reduce:animate-none" />

      <div className="relative w-[min(420px,90vw)] rounded-3xl border border-slate-200/80 bg-white/85 px-8 py-10 text-center shadow-[0_20px_60px_rgba(15,23,42,0.18)] backdrop-blur dark:border-white/10 dark:bg-white/5 dark:shadow-2xl">
        <div className="mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-2xl border border-slate-200 bg-white/90 dark:border-white/20 dark:bg-white/10">
          <div className="h-6 w-6 rounded-full bg-cyan-500/90 shadow-[0_0_18px_rgba(14,116,144,0.4)] animate-pulse motion-reduce:animate-none dark:bg-cyan-200/90 dark:shadow-[0_0_22px_rgba(56,189,248,0.65)]" />
        </div>

        <h1 className="text-2xl font-semibold tracking-tight text-slate-900 dark:text-white">{title}</h1>
        <p className="mt-2 text-sm text-slate-600 dark:text-slate-300">{stageLabel}</p>
        <p className="mt-1 text-xs font-medium text-slate-500 dark:text-slate-400">{progressLabel}</p>

        <div className="mt-6 h-1.5 w-full overflow-hidden rounded-full bg-slate-200/80 dark:bg-white/15">
          <div
            className="h-full rounded-full bg-cyan-500/90 transition-[width] duration-300 ease-out dark:bg-cyan-200/90"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// AppContent Component (uses Command Palette context)
// ============================================================================

function AppContent() {
  const { t } = useTranslation('common');
  const { mode } = useModeStore();
  const { open: openCommandPalette } = useGlobalCommandPalette();

  const { detectIncompleteTasks, initializeListener, cleanupListener } = useRecoveryStore();

  const [showShortcuts, setShowShortcuts] = useState(false);
  const [showStartupSplash, setShowStartupSplash] = useState(true);
  const [startupSplashExiting, setStartupSplashExiting] = useState(false);
  const [startupStage, setStartupStage] = useState<StartupStage>('booting');
  const [startupProgress, setStartupProgress] = useState({ current: 0, total: DEFAULT_STARTUP_TOTAL_STEPS });
  const { forceShowWizard, clearWizardTrigger, tourActive, startTour, endTour } = useOnboardingStore();
  // ShortcutOverlay manages its own open/close state via mod+shift+/ hotkey

  const backendInitPromiseRef = useRef<Promise<boolean> | null>(null);
  const recoveryDetectionCalledRef = useRef(false);

  // Initialize Tauri backend on mount
  useEffect(() => {
    let minDurationReached = false;
    let maxDurationReached = false;
    let backendReady = false;
    let splashExitScheduled = false;
    let unmounted = false;
    let progressUnlisten: UnlistenFn | null = null;
    let progressListenerDisposed = false;

    let fadeOutTimer: number | null = null;

    listen<InitProgressPayload>(INIT_PROGRESS_EVENT, (event) => {
      if (unmounted) return;
      const payload = event.payload;
      if (!payload) return;
      setStartupStage(payload.stage);
      setStartupProgress({
        current: payload.step_index,
        total: payload.total_steps || DEFAULT_STARTUP_TOTAL_STEPS,
      });
    })
      .then((unlisten) => {
        if (progressListenerDisposed) {
          unlisten();
          return;
        }
        progressUnlisten = unlisten;
      })
      .catch((err) => {
        console.warn('Failed to listen init progress event:', err);
      });

    const scheduleSplashExit = () => {
      if (splashExitScheduled || unmounted) return;
      splashExitScheduled = true;
      setStartupStage('ready');
      setStartupProgress((prev) => {
        const total = prev.total || DEFAULT_STARTUP_TOTAL_STEPS;
        return { current: total, total };
      });
      setStartupSplashExiting(true);
      fadeOutTimer = window.setTimeout(() => {
        if (!unmounted) {
          setShowStartupSplash(false);
        }
      }, STARTUP_FADE_OUT_MS);
    };

    const tryFinishStartupTransition = () => {
      if (minDurationReached && (backendReady || maxDurationReached)) {
        scheduleSplashExit();
      }
    };

    const minDurationTimer = window.setTimeout(() => {
      minDurationReached = true;
      tryFinishStartupTransition();
    }, STARTUP_MIN_DURATION_MS);

    const maxDurationTimer = window.setTimeout(() => {
      maxDurationReached = true;
      tryFinishStartupTransition();
    }, STARTUP_MAX_DURATION_MS);

    if (!backendInitPromiseRef.current) {
      backendInitPromiseRef.current = invoke('init_app')
        .then(() => true)
        .catch((err) => {
          console.warn('Backend initialization failed:', err);
          return false;
        });
    }

    backendInitPromiseRef.current.then((initSucceeded) => {
      if (initSucceeded && !unmounted && !recoveryDetectionCalledRef.current) {
        recoveryDetectionCalledRef.current = true;
        // After backend is ready, detect incomplete tasks
        detectIncompleteTasks();

        // Apply persisted permission policy config to backend runtime.
        void usePermissionPolicyStore.getState().initializePolicy();
      }
      backendReady = true;
      tryFinishStartupTransition();
    });

    // Initialize recovery event listener
    initializeListener();

    return () => {
      unmounted = true;
      progressListenerDisposed = true;
      if (progressUnlisten) {
        progressUnlisten();
      }
      window.clearTimeout(minDurationTimer);
      window.clearTimeout(maxDurationTimer);
      if (fadeOutTimer) {
        window.clearTimeout(fadeOutTimer);
      }
      cleanupListener();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Callback for showing shortcuts (settings is handled by TopNavBar)
  const handleOpenSettings = useCallback(() => {
    // Settings is handled by TopNavBar component's internal state
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

  const startupStageLabel: Record<StartupStage, string> = {
    booting: t('startup.stage.booting'),
    core_state: t('startup.stage.coreState'),
    plugins: t('startup.stage.plugins'),
    index_manager: t('startup.stage.indexManager'),
    spec_interview: t('startup.stage.specInterview'),
    recovery_scan: t('startup.stage.recoveryScan'),
    remote_gateway: t('startup.stage.remoteGateway'),
    ready: t('startup.stage.ready'),
  };

  const progressTotal = startupProgress.total > 0 ? startupProgress.total : DEFAULT_STARTUP_TOTAL_STEPS;
  const progressCurrent = Math.min(startupProgress.current, progressTotal);
  const startupProgressLabel = t('startup.progress', {
    current: progressCurrent,
    total: progressTotal,
  });
  const startupProgressPercent =
    startupStage === 'ready' ? 100 : Math.max(10, Math.round((progressCurrent / progressTotal) * 100));

  return (
    <div className="relative h-screen flex flex-col bg-gray-100 dark:bg-gray-950">
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
      <FeatureTour active={tourActive} onFinish={endTour} />

      {/* Top Navigation Bar */}
      <TopNavBar onOpenCommandPalette={openCommandPalette} />

      {/* Main Content with animated transitions */}
      <main className="flex-1 overflow-hidden flex flex-col min-w-0">
        <AnimatedModeContent mode={mode}>
          {mode === 'simple' && (
            <div data-tour="mode-simple" className="h-full">
              <SimpleMode />
            </div>
          )}
          {mode === 'expert' && (
            <div data-tour="mode-expert" className="h-full">
              <ExpertMode />
            </div>
          )}
          {mode === 'claude-code' && (
            <div data-tour="mode-claude-code" className="h-full">
              <ClaudeCodeMode />
            </div>
          )}
          {mode === 'projects' && (
            <div data-tour="mode-projects" className="h-full">
              <Projects />
            </div>
          )}
          {mode === 'analytics' && (
            <div data-tour="mode-analytics" className="h-full">
              <Dashboard />
            </div>
          )}
          {mode === 'knowledge' && (
            <div data-tour="mode-knowledge" className="h-full">
              <KnowledgeBasePanel />
            </div>
          )}
          {mode === 'codebase' && (
            <div data-tour="mode-codebase" className="h-full">
              <CodebasePanel />
            </div>
          )}
          {mode === 'artifacts' && (
            <div data-tour="mode-artifacts" className="h-full">
              <ArtifactBrowserPanel />
            </div>
          )}
        </AnimatedModeContent>
      </main>

      {/* Keyboard Shortcuts Help Dialog (legacy, from Story 004) */}
      <ShortcutsHelpDialog isOpen={showShortcuts} onClose={() => setShowShortcuts(false)} />

      {/* Shortcut Overlay (Story 005) - toggled via Ctrl+Shift+/ */}
      <ShortcutOverlay />

      {showStartupSplash && (
        <StartupSplash
          title={t('appName')}
          stageLabel={startupStageLabel[startupStage]}
          progressLabel={startupProgressLabel}
          progressPercent={startupProgressPercent}
          exiting={startupSplashExiting}
        />
      )}
    </div>
  );
}

// ============================================================================
// App Component (wrapper with providers)
// ============================================================================

export function App() {
  return (
    <ToastProvider>
      <GlobalCommandPaletteProvider>
        <AppContent />
      </GlobalCommandPaletteProvider>
    </ToastProvider>
  );
}

export default App;
