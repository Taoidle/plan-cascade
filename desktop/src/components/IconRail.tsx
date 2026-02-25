/**
 * IconRail Component
 *
 * Thin left navigation rail (~48px) with mode icons, search, and settings.
 * Replaces the top header for a more vertical-space-efficient layout.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Tooltip from '@radix-ui/react-tooltip';
import {
  LightningBoltIcon,
  MixerHorizontalIcon,
  ChatBubbleIcon,
  FileIcon,
  BarChartIcon,
  ReaderIcon,
  ArchiveIcon,
  MagnifyingGlassIcon,
  GearIcon,
} from '@radix-ui/react-icons';
import { Mode, MODES, useModeStore } from '../store/mode';
import { useExecutionStore } from '../store/execution';
import { SettingsDialog } from './Settings';

// ============================================================================
// Icon Map
// ============================================================================

const MODE_ICONS: Record<Mode, typeof LightningBoltIcon> = {
  simple: LightningBoltIcon,
  expert: MixerHorizontalIcon,
  'claude-code': ChatBubbleIcon,
  projects: FileIcon,
  analytics: BarChartIcon,
  knowledge: ReaderIcon,
  artifacts: ArchiveIcon,
};

// ============================================================================
// MiniLogo
// ============================================================================

function MiniLogo() {
  return (
    <svg className="w-6 h-6" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect x="4" y="4" width="24" height="6" rx="2" className="fill-primary-600" />
      <rect x="6" y="12" width="20" height="6" rx="2" className="fill-primary-500" />
      <rect x="8" y="20" width="16" height="6" rx="2" className="fill-primary-400" />
    </svg>
  );
}

// ============================================================================
// RailIcon — mode button with indicator + tooltip
// ============================================================================

interface RailIconProps {
  mode: Mode;
  icon: typeof LightningBoltIcon;
  label: string;
  description: string;
  selected: boolean;
  running: boolean;
  disabled: boolean;
  onClick: () => void;
}

function RailIcon({ mode, icon: Icon, label, description, selected, running, disabled, onClick }: RailIconProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <button
          onClick={onClick}
          disabled={disabled}
          aria-current={selected ? 'page' : undefined}
          aria-label={label}
          data-testid={`rail-icon-${mode}`}
          className={clsx(
            'relative flex items-center justify-center w-10 h-10 3xl:w-11 3xl:h-11 rounded-lg',
            'transition-colors duration-150',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            selected
              ? 'bg-primary-50 dark:bg-primary-900/30 text-primary-600 dark:text-primary-400'
              : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-900 dark:hover:text-white',
          )}
        >
          {/* Left indicator bar */}
          {selected && (
            <span className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-5 rounded-r bg-primary-600 dark:bg-primary-400" />
          )}

          <Icon className="w-5 h-5" />

          {/* Running pulse dot */}
          {running && selected && (
            <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
          )}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="right"
          sideOffset={8}
          className={clsx(
            'px-3 py-2 rounded-lg text-sm max-w-[200px]',
            'bg-gray-900 dark:bg-gray-100',
            'text-white dark:text-gray-900',
            'shadow-lg z-50',
          )}
        >
          <div className="font-medium">{label}</div>
          <div className="text-xs text-gray-300 dark:text-gray-600 mt-0.5">{description}</div>
          <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

// ============================================================================
// RailActionIcon — utility button (search, settings)
// ============================================================================

interface RailActionIconProps {
  icon: typeof MagnifyingGlassIcon;
  label: string;
  onClick: () => void;
  'data-testid'?: string;
}

function RailActionIcon({ icon: Icon, label, onClick, 'data-testid': testId }: RailActionIconProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <button
          onClick={onClick}
          aria-label={label}
          data-testid={testId}
          className={clsx(
            'flex items-center justify-center w-10 h-10 3xl:w-11 3xl:h-11 rounded-lg',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            'hover:text-gray-900 dark:hover:text-white',
            'transition-colors duration-150',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
          )}
        >
          <Icon className="w-5 h-5" />
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="right"
          sideOffset={8}
          className={clsx(
            'px-3 py-1.5 rounded-md text-sm',
            'bg-gray-900 dark:bg-gray-100',
            'text-white dark:text-gray-900',
            'shadow-lg z-50',
          )}
        >
          {label}
          <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

// ============================================================================
// IconRail
// ============================================================================

interface IconRailProps {
  onOpenCommandPalette: () => void;
}

export function IconRail({ onOpenCommandPalette }: IconRailProps) {
  const { t } = useTranslation();
  const { mode, setMode } = useModeStore();
  const { status } = useExecutionStore();
  const [settingsOpen, setSettingsOpen] = useState(false);

  const isRunning = status === 'running';

  const getModeLabel = (m: Mode) => {
    const labels: Record<Mode, string> = {
      simple: t('modeSwitch.simple.name'),
      expert: t('modeSwitch.expert.name'),
      'claude-code': t('modeSwitch.claudeCode.name'),
      projects: t('modeSwitch.projects.name'),
      analytics: t('modeSwitch.analytics.name'),
      knowledge: t('modeSwitch.knowledge.name'),
      artifacts: t('modeSwitch.artifacts.name'),
    };
    return labels[m];
  };

  const getModeDescription = (m: Mode) => {
    const descriptions: Record<Mode, string> = {
      simple: t('modeSwitch.simple.description'),
      expert: t('modeSwitch.expert.description'),
      'claude-code': t('modeSwitch.claudeCode.description'),
      projects: t('modeSwitch.projects.description'),
      analytics: t('modeSwitch.analytics.description'),
      knowledge: t('modeSwitch.knowledge.description'),
      artifacts: t('modeSwitch.artifacts.description'),
    };
    return descriptions[m];
  };

  return (
    <Tooltip.Provider delayDuration={300}>
      <nav
        className={clsx(
          'w-12 3xl:w-14 h-full flex-col items-center',
          'border-r border-gray-200 dark:border-gray-800',
          'bg-white dark:bg-gray-900',
          'py-2 shrink-0',
          'hidden sm:flex',
        )}
        aria-label="Main navigation"
        data-testid="icon-rail"
      >
        {/* Mini Logo */}
        <div className="flex items-center justify-center w-10 h-10 mb-1">
          <MiniLogo />
        </div>

        {/* Divider */}
        <div className="w-6 h-px bg-gray-200 dark:bg-gray-700 my-1" />

        {/* Mode icons */}
        <div className="flex-1 flex flex-col items-center gap-1 py-1">
          {MODES.map((m) => (
            <RailIcon
              key={m}
              mode={m}
              icon={MODE_ICONS[m]}
              label={getModeLabel(m)}
              description={getModeDescription(m)}
              selected={mode === m}
              running={isRunning}
              disabled={isRunning && mode !== m}
              onClick={() => setMode(m)}
            />
          ))}
        </div>

        {/* Spacer */}
        <div className="mt-auto" />

        {/* Divider */}
        <div className="w-6 h-px bg-gray-200 dark:bg-gray-700 my-1" />

        {/* Action icons */}
        <div className="flex flex-col items-center gap-1 py-1">
          <RailActionIcon
            icon={MagnifyingGlassIcon}
            label={t('iconRail.searchTooltip')}
            onClick={onOpenCommandPalette}
            data-testid="rail-search"
          />
          <RailActionIcon
            icon={GearIcon}
            label={t('iconRail.settingsTooltip')}
            onClick={() => setSettingsOpen(true)}
            data-testid="rail-settings"
          />
        </div>
      </nav>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </Tooltip.Provider>
  );
}

export default IconRail;
