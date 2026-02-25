/**
 * ModeSwitch Component
 *
 * Toggle between Simple, Expert, and Claude Code modes.
 * Simple mode: One-click execution with AI-driven automation
 * Expert mode: Full control over PRD editing, agent selection, and execution
 * Claude Code mode: Interactive chat with Claude Code CLI
 *
 * Story 005: Navigation Flow Refinement - Added animated mode transitions
 */

import { useRef, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
  ChevronDownIcon,
  CheckIcon,
  LightningBoltIcon,
  MixerHorizontalIcon,
  ChatBubbleIcon,
  FileIcon,
  BarChartIcon,
  ReaderIcon,
  ArchiveIcon,
} from '@radix-ui/react-icons';
import { Mode, MODES, useModeStore } from '../store/mode';

// Re-export Mode type for backwards compatibility
export type { Mode };

interface ModeSwitchProps {
  mode: Mode;
  onChange: (mode: Mode) => void;
  disabled?: boolean;
}

const MODE_ICONS: Record<Mode, typeof LightningBoltIcon> = {
  simple: LightningBoltIcon,
  expert: MixerHorizontalIcon,
  'claude-code': ChatBubbleIcon,
  projects: FileIcon,
  analytics: BarChartIcon,
  knowledge: ReaderIcon,
  artifacts: ArchiveIcon,
};

export function ModeSwitch({ mode, onChange, disabled = false }: ModeSwitchProps) {
  const { t } = useTranslation();
  const { isTransitioning } = useModeStore();
  const CurrentIcon = MODE_ICONS[mode];
  const triggerRef = useRef<HTMLButtonElement>(null);

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

  // Trigger a subtle scale pulse on mode change for visual feedback
  useEffect(() => {
    if (isTransitioning && triggerRef.current) {
      triggerRef.current.animate([{ transform: 'scale(1)' }, { transform: 'scale(0.95)' }, { transform: 'scale(1)' }], {
        duration: 200,
        easing: 'ease-out',
      });
    }
  }, [isTransitioning, mode]);

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild disabled={disabled}>
        <button
          ref={triggerRef}
          className={clsx(
            'flex items-center gap-2 px-3 py-1.5 rounded-lg',
            'bg-gray-100 dark:bg-gray-800',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'border border-gray-200 dark:border-gray-700',
            'text-sm font-medium text-gray-700 dark:text-gray-300',
            'transition-all duration-200 ease-out',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
        >
          <span
            className={clsx('inline-flex transition-transform duration-200 ease-out', isTransitioning && 'rotate-12')}
          >
            <CurrentIcon className="w-4 h-4" />
          </span>
          <span
            className={clsx('transition-opacity duration-200 ease-out', isTransitioning ? 'opacity-0' : 'opacity-100')}
          >
            {getModeLabel(mode)}
          </span>
          <ChevronDownIcon className="w-4 h-4 text-gray-500" />
        </button>
      </DropdownMenu.Trigger>

      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className={clsx(
            'min-w-[220px] rounded-lg p-1',
            'bg-white dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'shadow-lg',
            'animate-in fade-in-0 zoom-in-95 duration-200',
            'data-[side=bottom]:slide-in-from-top-2',
            'data-[side=top]:slide-in-from-bottom-2',
          )}
          sideOffset={5}
          align="end"
        >
          <DropdownMenu.Label className="px-3 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            {t('modeSwitch.label')}
          </DropdownMenu.Label>

          {MODES.map((modeOption) => {
            const Icon = MODE_ICONS[modeOption];
            const isSelected = mode === modeOption;

            return (
              <DropdownMenu.Item
                key={modeOption}
                onClick={() => onChange(modeOption)}
                className={clsx(
                  'flex items-start gap-3 px-3 py-2.5 rounded-md',
                  'cursor-pointer outline-none',
                  'transition-colors duration-150',
                  isSelected ? 'bg-primary-50 dark:bg-primary-900/30' : 'hover:bg-gray-100 dark:hover:bg-gray-700',
                )}
              >
                <div
                  className={clsx(
                    'p-1.5 rounded-md transition-colors duration-150',
                    isSelected ? 'bg-primary-100 dark:bg-primary-900/50' : 'bg-gray-100 dark:bg-gray-700',
                  )}
                >
                  <Icon
                    className={clsx(
                      'w-4 h-4 transition-colors duration-150',
                      isSelected ? 'text-primary-600 dark:text-primary-400' : 'text-gray-500 dark:text-gray-400',
                    )}
                  />
                </div>

                <div className="flex-1 min-w-0">
                  <div
                    className={clsx(
                      'font-medium text-sm transition-colors duration-150',
                      isSelected ? 'text-primary-700 dark:text-primary-300' : 'text-gray-900 dark:text-white',
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
  );
}

// ============================================================================
// AnimatedModeContent Component
// ============================================================================

/**
 * Wraps mode content with fade/slide transition animations.
 * Completes within 200ms without layout shifts by using fixed positioning
 * during transitions.
 */
interface AnimatedModeContentProps {
  children: React.ReactNode;
  mode: Mode;
}

export function AnimatedModeContent({ children, mode }: AnimatedModeContentProps) {
  const { isTransitioning, transitionDirection } = useModeStore();

  const enterClass = (() => {
    if (!isTransitioning) return 'opacity-100 translate-x-0';
    switch (transitionDirection) {
      case 'left':
        return 'animate-in fade-in-0 slide-in-from-bottom-2 duration-200 ease-out';
      case 'right':
        return 'animate-in fade-in-0 slide-in-from-top-2 duration-200 ease-out';
      default:
        return 'animate-in fade-in-0 duration-200 ease-out';
    }
  })();

  return (
    <div key={mode} className={clsx('flex-1 overflow-hidden', 'will-change-transform', enterClass)}>
      {children}
    </div>
  );
}

// ============================================================================
// ModeTabs Component (Alternative UI for tab-style switching)
// ============================================================================

interface ModeTabsProps {
  mode: Mode;
  onChange: (mode: Mode) => void;
  disabled?: boolean;
}

export function ModeTabs({ mode, onChange, disabled = false }: ModeTabsProps) {
  const { t } = useTranslation();

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
    <div
      className={clsx(
        'inline-flex items-center rounded-lg p-1',
        'bg-gray-100 dark:bg-gray-800',
        disabled && 'opacity-50 pointer-events-none',
      )}
    >
      {MODES.map((modeOption) => {
        const Icon = MODE_ICONS[modeOption];
        const isSelected = mode === modeOption;

        return (
          <button
            key={modeOption}
            onClick={() => onChange(modeOption)}
            disabled={disabled}
            className={clsx(
              'relative flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium',
              'transition-all duration-200 ease-out',
              isSelected
                ? 'bg-white dark:bg-gray-700 text-primary-600 dark:text-primary-400 shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white',
            )}
            title={getModeDescription(modeOption)}
          >
            <Icon className="w-4 h-4" />
            <span className="hidden sm:inline">{getModeLabel(modeOption)}</span>
          </button>
        );
      })}
    </div>
  );
}

export default ModeSwitch;
