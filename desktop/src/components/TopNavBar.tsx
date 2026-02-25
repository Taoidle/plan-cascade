/**
 * TopNavBar Component
 *
 * Horizontal top navigation bar with hamburger menu for mode switching,
 * current mode display, and action buttons (search, settings).
 * Replaces the vertical IconRail for a more space-efficient layout.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import * as Tooltip from '@radix-ui/react-tooltip';
import {
  HamburgerMenuIcon,
  CheckIcon,
  MagnifyingGlassIcon,
  GearIcon,
  LightningBoltIcon,
  MixerHorizontalIcon,
  ChatBubbleIcon,
  FileIcon,
  BarChartIcon,
  ReaderIcon,
  ArchiveIcon,
  SunIcon,
  MoonIcon,
} from '@radix-ui/react-icons';
import { Mode, MODES, useModeStore } from '../store/mode';
import { useExecutionStore } from '../store/execution';
import { useSettingsStore } from '../store/settings';
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
// TopNavBar
// ============================================================================

interface TopNavBarProps {
  onOpenCommandPalette: () => void;
}

export function TopNavBar({ onOpenCommandPalette }: TopNavBarProps) {
  const { t } = useTranslation();
  const { mode, setMode } = useModeStore();
  const { status } = useExecutionStore();
  const { theme, setTheme } = useSettingsStore();
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Detect effective dark state for the toggle icon
  const isDark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  const handleToggleTheme = () => {
    setTheme(isDark ? 'light' : 'dark');
  };

  const isRunning = status === 'running';
  const CurrentIcon = MODE_ICONS[mode];

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
      <header
        className={clsx(
          'shrink-0 flex items-center justify-between h-10 px-3',
          'border-b border-gray-200 dark:border-gray-800',
          'bg-white dark:bg-gray-900'
        )}
        aria-label="Main navigation"
        data-testid="top-nav-bar"
      >
        {/* Left: Hamburger menu + current mode name */}
        <div className="flex items-center gap-2">
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <button
                aria-label={t('topNavBar.menuTooltip')}
                data-testid="nav-menu-trigger"
                className={clsx(
                  'flex items-center justify-center w-8 h-8 rounded-lg',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'hover:text-gray-900 dark:hover:text-white',
                  'transition-colors duration-150',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
                )}
              >
                <HamburgerMenuIcon className="w-5 h-5" />
              </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
              <DropdownMenu.Content
                className={clsx(
                  'min-w-[220px] rounded-lg p-1',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-200 dark:border-gray-700',
                  'shadow-lg z-50',
                  'animate-in fade-in-0 zoom-in-95 duration-200',
                  'data-[side=bottom]:slide-in-from-top-2'
                )}
                sideOffset={5}
                align="start"
              >
                <DropdownMenu.Label className="px-3 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                  {t('modeSwitch.label')}
                </DropdownMenu.Label>

                {MODES.map((modeOption) => {
                  const Icon = MODE_ICONS[modeOption];
                  const isSelected = mode === modeOption;
                  const isDisabled = isRunning && mode !== modeOption;

                  return (
                    <DropdownMenu.Item
                      key={modeOption}
                      disabled={isDisabled}
                      onSelect={() => setMode(modeOption)}
                      className={clsx(
                        'flex items-start gap-3 px-3 py-2.5 rounded-md',
                        'outline-none transition-colors duration-150',
                        isDisabled
                          ? 'opacity-50 cursor-not-allowed'
                          : 'cursor-pointer',
                        isSelected
                          ? 'bg-primary-50 dark:bg-primary-900/30'
                          : !isDisabled && 'hover:bg-gray-100 dark:hover:bg-gray-700'
                      )}
                    >
                      <div
                        className={clsx(
                          'p-1.5 rounded-md transition-colors duration-150',
                          isSelected
                            ? 'bg-primary-100 dark:bg-primary-900/50'
                            : 'bg-gray-100 dark:bg-gray-700'
                        )}
                      >
                        <Icon
                          className={clsx(
                            'w-4 h-4 transition-colors duration-150',
                            isSelected
                              ? 'text-primary-600 dark:text-primary-400'
                              : 'text-gray-500 dark:text-gray-400'
                          )}
                        />
                      </div>

                      <div className="flex-1 min-w-0">
                        <div
                          className={clsx(
                            'font-medium text-sm transition-colors duration-150',
                            isSelected
                              ? 'text-primary-700 dark:text-primary-300'
                              : 'text-gray-900 dark:text-white'
                          )}
                        >
                          {getModeLabel(modeOption)}
                        </div>
                        <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                          {getModeDescription(modeOption)}
                        </div>
                      </div>

                      {isSelected && (
                        <CheckIcon className="w-5 h-5 text-primary-600 dark:text-primary-400 mt-0.5 animate-in fade-in-0 zoom-in-50 duration-200" />
                      )}
                    </DropdownMenu.Item>
                  );
                })}
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>

          {/* Current mode icon + name */}
          <CurrentIcon className="w-4 h-4 text-primary-600 dark:text-primary-400" />
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {getModeLabel(mode)}
          </span>

          {/* Running pulse indicator */}
          {isRunning && (
            <span className="w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
          )}
        </div>

        {/* Right: Search + Settings */}
        <div className="flex items-center gap-1">
          <Tooltip.Root>
            <Tooltip.Trigger asChild>
              <button
                onClick={onOpenCommandPalette}
                aria-label={t('topNavBar.searchTooltip')}
                data-testid="nav-search"
                className={clsx(
                  'flex items-center justify-center w-8 h-8 rounded-lg',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'hover:text-gray-900 dark:hover:text-white',
                  'transition-colors duration-150',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
                )}
              >
                <MagnifyingGlassIcon className="w-5 h-5" />
              </button>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content
                side="bottom"
                sideOffset={8}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-sm',
                  'bg-gray-900 dark:bg-gray-100',
                  'text-white dark:text-gray-900',
                  'shadow-lg z-50'
                )}
              >
                {t('topNavBar.searchTooltip')}
                <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger asChild>
              <button
                onClick={handleToggleTheme}
                aria-label={t('topNavBar.themeTooltip')}
                data-testid="nav-theme-toggle"
                className={clsx(
                  'flex items-center justify-center w-8 h-8 rounded-lg',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'hover:text-gray-900 dark:hover:text-white',
                  'transition-colors duration-150',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
                )}
              >
                {isDark ? <SunIcon className="w-5 h-5" /> : <MoonIcon className="w-5 h-5" />}
              </button>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content
                side="bottom"
                sideOffset={8}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-sm',
                  'bg-gray-900 dark:bg-gray-100',
                  'text-white dark:text-gray-900',
                  'shadow-lg z-50'
                )}
              >
                {t('topNavBar.themeTooltip')}
                <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger asChild>
              <button
                onClick={() => setSettingsOpen(true)}
                aria-label={t('topNavBar.settingsTooltip')}
                data-testid="nav-settings"
                className={clsx(
                  'flex items-center justify-center w-8 h-8 rounded-lg',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'hover:text-gray-900 dark:hover:text-white',
                  'transition-colors duration-150',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
                )}
              >
                <GearIcon className="w-5 h-5" />
              </button>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content
                side="bottom"
                sideOffset={8}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-sm',
                  'bg-gray-900 dark:bg-gray-100',
                  'text-white dark:text-gray-900',
                  'shadow-lg z-50'
                )}
              >
                {t('topNavBar.settingsTooltip')}
                <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        </div>
      </header>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </Tooltip.Provider>
  );
}

export default TopNavBar;
