/**
 * App Component
 *
 * Main application layout integrating all components.
 * Uses a left icon rail for navigation with full vertical space for content.
 *
 * Story 004: Command Palette Enhancement - Global command palette integration
 * Story 005: Navigation Flow Refinement - Contextual actions, shortcut overlay
 * Story 004 (Recovery): Resume & Recovery System - Detect and resume interrupted executions
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AnimatedModeContent } from './components/ModeSwitch';
import { IconRail } from './components/IconRail';
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
import { ShortcutOverlay } from './components/shared/ShortcutOverlay';
import { RecoveryPrompt } from './components/shared/RecoveryPrompt';
import { ToastProvider } from './components/shared/Toast';
import { useGlobalCommands } from './hooks/useGlobalCommands';
import { ShortcutsHelpDialog } from './components/ClaudeCodeMode/KeyboardShortcuts';
import { useModeStore } from './store/mode';
import { useOnboardingStore } from './store/onboarding';
import { useRecoveryStore } from './store/recovery';

// ============================================================================
// AppContent Component (uses Command Palette context)
// ============================================================================

function AppContent() {
  const { mode } = useModeStore();
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

  // Callback for showing shortcuts (settings is handled by IconRail)
  const handleOpenSettings = useCallback(() => {
    // Settings is handled by IconRail component's internal state
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
    <div className="h-screen flex flex-row bg-gray-50 dark:bg-gray-950">
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

      {/* Left Icon Rail Navigation */}
      <IconRail onOpenCommandPalette={openCommandPalette} />

      {/* Main Content with animated transitions */}
      <main className="flex-1 overflow-hidden flex flex-col min-w-0">
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
    <ToastProvider>
      <GlobalCommandPaletteProvider>
        <AppContent />
      </GlobalCommandPaletteProvider>
    </ToastProvider>
  );
}

export default App;
