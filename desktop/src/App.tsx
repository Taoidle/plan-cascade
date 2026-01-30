/**
 * App Component
 *
 * Main application layout integrating all components.
 * Supports Simple, Expert, and Claude Code modes.
 */

import { useTranslation } from 'react-i18next';
import { ModeSwitch } from './components/ModeSwitch';
import { SettingsButton } from './components/SettingsButton';
import { SimpleMode } from './components/SimpleMode';
import { ExpertMode } from './components/ExpertMode';
import { ClaudeCodeMode } from './components/ClaudeCodeMode';
import { Projects } from './components/Projects';
import { Dashboard } from './components/Analytics';
import { SetupWizard } from './components/Settings';
import { useModeStore } from './store/mode';
import { useExecutionStore } from './store/execution';
import { clsx } from 'clsx';

export function App() {
  const { t } = useTranslation();
  const { mode, setMode } = useModeStore();
  const { status } = useExecutionStore();

  const isRunning = status === 'running';

  return (
    <div className="h-screen flex flex-col bg-gray-50 dark:bg-gray-950">
      {/* First-time Setup Wizard */}
      <SetupWizard />

      {/* Header */}
      <header
        className={clsx(
          'h-14 flex items-center justify-between px-4',
          'bg-white dark:bg-gray-900',
          'border-b border-gray-200 dark:border-gray-800',
          'shrink-0'
        )}
      >
        {/* Logo / Title */}
        <div className="flex items-center gap-3">
          <Logo />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-white">
            {t('appName')}
          </h1>

          {/* Status Badge */}
          {status !== 'idle' && (
            <StatusBadge status={status} />
          )}
        </div>

        {/* Controls */}
        <div className="flex items-center gap-4">
          <ModeSwitch
            mode={mode}
            onChange={setMode}
            disabled={isRunning}
          />
          <SettingsButton />
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 overflow-hidden">
        {mode === 'simple' && <SimpleMode />}
        {mode === 'expert' && <ExpertMode />}
        {mode === 'claude-code' && <ClaudeCodeMode />}
        {mode === 'projects' && <Projects />}
        {mode === 'analytics' && <Dashboard />}
      </main>

      {/* Footer (optional status bar) */}
      <footer
        className={clsx(
          'h-6 flex items-center justify-between px-4',
          'bg-white dark:bg-gray-900',
          'border-t border-gray-200 dark:border-gray-800',
          'text-xs text-gray-500 dark:text-gray-400',
          'shrink-0'
        )}
      >
        <span>{t('version')}</span>
        <span>{t('ready')}</span>
      </footer>
    </div>
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
