/**
 * ModeSwitch Component
 *
 * Toggle between Simple, Expert, and Claude Code modes.
 * Simple mode: One-click execution with AI-driven automation
 * Expert mode: Full control over PRD editing, agent selection, and execution
 * Claude Code mode: Interactive chat with Claude Code CLI
 */

import { clsx } from 'clsx';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
  ChevronDownIcon,
  CheckIcon,
  LightningBoltIcon,
  MixerHorizontalIcon,
  ChatBubbleIcon,
} from '@radix-ui/react-icons';
import { Mode, MODES, MODE_LABELS, MODE_DESCRIPTIONS } from '../store/mode';

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
};

export function ModeSwitch({ mode, onChange, disabled = false }: ModeSwitchProps) {
  const CurrentIcon = MODE_ICONS[mode];

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild disabled={disabled}>
        <button
          className={clsx(
            'flex items-center gap-2 px-3 py-1.5 rounded-lg',
            'bg-gray-100 dark:bg-gray-800',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'border border-gray-200 dark:border-gray-700',
            'text-sm font-medium text-gray-700 dark:text-gray-300',
            'transition-colors',
            'disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          <CurrentIcon className="w-4 h-4" />
          <span>{MODE_LABELS[mode]}</span>
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
            'animate-in fade-in-0 zoom-in-95',
            'data-[side=bottom]:slide-in-from-top-2',
            'data-[side=top]:slide-in-from-bottom-2'
          )}
          sideOffset={5}
          align="end"
        >
          <DropdownMenu.Label className="px-3 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            Select Mode
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
                  isSelected
                    ? 'bg-primary-50 dark:bg-primary-900/30'
                    : 'hover:bg-gray-100 dark:hover:bg-gray-700'
                )}
              >
                <div
                  className={clsx(
                    'p-1.5 rounded-md',
                    isSelected
                      ? 'bg-primary-100 dark:bg-primary-900/50'
                      : 'bg-gray-100 dark:bg-gray-700'
                  )}
                >
                  <Icon
                    className={clsx(
                      'w-4 h-4',
                      isSelected
                        ? 'text-primary-600 dark:text-primary-400'
                        : 'text-gray-500 dark:text-gray-400'
                    )}
                  />
                </div>

                <div className="flex-1 min-w-0">
                  <div
                    className={clsx(
                      'font-medium text-sm',
                      isSelected
                        ? 'text-primary-700 dark:text-primary-300'
                        : 'text-gray-900 dark:text-white'
                    )}
                  >
                    {MODE_LABELS[modeOption]}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                    {MODE_DESCRIPTIONS[modeOption]}
                  </div>
                </div>

                {isSelected && (
                  <CheckIcon className="w-5 h-5 text-primary-600 dark:text-primary-400 mt-0.5" />
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
// ModeTabs Component (Alternative UI for tab-style switching)
// ============================================================================

interface ModeTabsProps {
  mode: Mode;
  onChange: (mode: Mode) => void;
  disabled?: boolean;
}

export function ModeTabs({ mode, onChange, disabled = false }: ModeTabsProps) {
  return (
    <div
      className={clsx(
        'inline-flex items-center rounded-lg p-1',
        'bg-gray-100 dark:bg-gray-800',
        disabled && 'opacity-50 pointer-events-none'
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
              'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
              isSelected
                ? 'bg-white dark:bg-gray-700 text-primary-600 dark:text-primary-400 shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            )}
            title={MODE_DESCRIPTIONS[modeOption]}
          >
            <Icon className="w-4 h-4" />
            <span className="hidden sm:inline">{MODE_LABELS[modeOption]}</span>
          </button>
        );
      })}
    </div>
  );
}

export default ModeSwitch;
