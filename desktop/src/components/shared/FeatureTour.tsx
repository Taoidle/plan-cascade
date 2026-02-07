/**
 * FeatureTour Component
 *
 * Tooltip overlay system that highlights UI regions to guide users
 * through the application's key capabilities.
 *
 * Tour stops: Simple Mode, Expert Mode, Claude Code, Projects, Analytics
 * Each stop features a positioned tooltip with title, description, and
 * navigation controls over a backdrop with a cutout for the highlighted element.
 *
 * Story 007: Onboarding & Setup Wizard
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  Cross2Icon,
  CheckCircledIcon,
} from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';
import { useModeStore, type Mode } from '../../store/mode';

// ============================================================================
// Types
// ============================================================================

interface FeatureTourProps {
  /** Whether the tour is currently active */
  active: boolean;
  /** Callback when tour is completed or dismissed */
  onFinish: () => void;
}

interface TourStop {
  /** Unique identifier */
  id: string;
  /** Mode this stop corresponds to */
  mode: Mode;
  /** i18n key prefix for title/description */
  i18nKey: string;
  /** CSS selector for the target element to highlight */
  targetSelector: string;
  /** Preferred tooltip position */
  position: 'top' | 'bottom' | 'left' | 'right';
}

interface TooltipPosition {
  top: number;
  left: number;
  arrowSide: 'top' | 'bottom' | 'left' | 'right';
}

interface CutoutRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

// ============================================================================
// Tour Stop Definitions
// ============================================================================

const tourStops: TourStop[] = [
  {
    id: 'simple',
    mode: 'simple',
    i18nKey: 'simple',
    targetSelector: '[data-tour="mode-simple"]',
    position: 'bottom',
  },
  {
    id: 'expert',
    mode: 'expert',
    i18nKey: 'expert',
    targetSelector: '[data-tour="mode-expert"]',
    position: 'bottom',
  },
  {
    id: 'claude-code',
    mode: 'claude-code',
    i18nKey: 'claudeCode',
    targetSelector: '[data-tour="mode-claude-code"]',
    position: 'bottom',
  },
  {
    id: 'projects',
    mode: 'projects',
    i18nKey: 'projects',
    targetSelector: '[data-tour="mode-projects"]',
    position: 'bottom',
  },
  {
    id: 'analytics',
    mode: 'analytics',
    i18nKey: 'analytics',
    targetSelector: '[data-tour="mode-analytics"]',
    position: 'bottom',
  },
];

// ============================================================================
// Constants
// ============================================================================

const TOOLTIP_WIDTH = 320;
const TOOLTIP_OFFSET = 16;
const CUTOUT_PADDING = 8;

// ============================================================================
// FeatureTour Component
// ============================================================================

export function FeatureTour({ active, onFinish }: FeatureTourProps) {
  const { t } = useTranslation('wizard');
  const [currentIndex, setCurrentIndex] = useState(0);
  const [cutout, setCutout] = useState<CutoutRect | null>(null);
  const [tooltipPos, setTooltipPos] = useState<TooltipPosition>({ top: 0, left: 0, arrowSide: 'top' });
  const [isVisible, setIsVisible] = useState(false);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const { setMode } = useModeStore();
  const { setTourCompleted } = useSettingsStore();

  const currentStop = tourStops[currentIndex];
  const totalStops = tourStops.length;

  // Position tooltip relative to the target element
  const positionTooltip = useCallback(() => {
    if (!currentStop) return;

    const target = document.querySelector(currentStop.targetSelector);
    if (!target) {
      // Fallback: center the tooltip if target not found
      setCutout(null);
      setTooltipPos({
        top: window.innerHeight / 2 - 80,
        left: window.innerWidth / 2 - TOOLTIP_WIDTH / 2,
        arrowSide: 'top',
      });
      setIsVisible(true);
      return;
    }

    const rect = target.getBoundingClientRect();

    // Set cutout area with padding
    const newCutout: CutoutRect = {
      top: rect.top - CUTOUT_PADDING,
      left: rect.left - CUTOUT_PADDING,
      width: rect.width + CUTOUT_PADDING * 2,
      height: rect.height + CUTOUT_PADDING * 2,
    };
    setCutout(newCutout);

    // Calculate tooltip position
    const tooltipHeight = tooltipRef.current?.offsetHeight || 180;
    let top = 0;
    let left = 0;
    let arrowSide: 'top' | 'bottom' | 'left' | 'right' = 'top';

    switch (currentStop.position) {
      case 'bottom':
        top = rect.bottom + TOOLTIP_OFFSET;
        left = rect.left + rect.width / 2 - TOOLTIP_WIDTH / 2;
        arrowSide = 'top';
        break;
      case 'top':
        top = rect.top - tooltipHeight - TOOLTIP_OFFSET;
        left = rect.left + rect.width / 2 - TOOLTIP_WIDTH / 2;
        arrowSide = 'bottom';
        break;
      case 'right':
        top = rect.top + rect.height / 2 - tooltipHeight / 2;
        left = rect.right + TOOLTIP_OFFSET;
        arrowSide = 'left';
        break;
      case 'left':
        top = rect.top + rect.height / 2 - tooltipHeight / 2;
        left = rect.left - TOOLTIP_WIDTH - TOOLTIP_OFFSET;
        arrowSide = 'right';
        break;
    }

    // Clamp to viewport
    left = Math.max(16, Math.min(left, window.innerWidth - TOOLTIP_WIDTH - 16));
    top = Math.max(16, Math.min(top, window.innerHeight - tooltipHeight - 16));

    // If tooltip would overlap cutout, flip position
    if (arrowSide === 'top' && top < newCutout.top + newCutout.height) {
      top = newCutout.top - tooltipHeight - TOOLTIP_OFFSET;
      arrowSide = 'bottom';
    }

    setTooltipPos({ top, left, arrowSide });
    setIsVisible(true);

    // Scroll target into view if needed
    target.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, [currentStop]);

  // Navigate to the mode for current stop and reposition
  useEffect(() => {
    if (!active || !currentStop) return;

    // Switch to the mode for this tour stop
    setMode(currentStop.mode);

    // Small delay to let mode switch animation complete before positioning
    const timer = setTimeout(() => {
      positionTooltip();
    }, 350);

    return () => clearTimeout(timer);
  }, [active, currentIndex, currentStop, setMode, positionTooltip]);

  // Reposition on window resize
  useEffect(() => {
    if (!active) return;

    const handleResize = () => positionTooltip();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [active, positionTooltip]);

  // Keyboard navigation
  useEffect(() => {
    if (!active) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight' || e.key === 'Enter') {
        e.preventDefault();
        handleNext();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        handlePrev();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        handleFinish();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [active, currentIndex]);

  const handleNext = useCallback(() => {
    if (currentIndex < totalStops - 1) {
      setIsVisible(false);
      setTimeout(() => setCurrentIndex(currentIndex + 1), 200);
    } else {
      handleFinish();
    }
  }, [currentIndex, totalStops]);

  const handlePrev = useCallback(() => {
    if (currentIndex > 0) {
      setIsVisible(false);
      setTimeout(() => setCurrentIndex(currentIndex - 1), 200);
    }
  }, [currentIndex]);

  const handleFinish = useCallback(() => {
    setTourCompleted(true);
    setIsVisible(false);
    setCurrentIndex(0);
    onFinish();
  }, [setTourCompleted, onFinish]);

  if (!active || !currentStop) return null;

  const isLastStop = currentIndex === totalStops - 1;

  return (
    <div className="fixed inset-0 z-[9999]" role="dialog" aria-label={t('tour.title')}>
      {/* Backdrop with cutout */}
      <svg
        className="absolute inset-0 w-full h-full"
        xmlns="http://www.w3.org/2000/svg"
        style={{ pointerEvents: 'none' }}
      >
        <defs>
          <mask id="tour-mask">
            <rect x="0" y="0" width="100%" height="100%" fill="white" />
            {cutout && (
              <rect
                x={cutout.left}
                y={cutout.top}
                width={cutout.width}
                height={cutout.height}
                rx="8"
                fill="black"
              />
            )}
          </mask>
        </defs>
        <rect
          x="0"
          y="0"
          width="100%"
          height="100%"
          fill="rgba(0, 0, 0, 0.6)"
          mask="url(#tour-mask)"
          style={{ pointerEvents: 'auto' }}
          onClick={handleFinish}
        />
      </svg>

      {/* Highlight ring around cutout */}
      {cutout && (
        <div
          className="absolute rounded-lg ring-2 ring-[var(--color-primary)] ring-offset-2 transition-all duration-300"
          style={{
            top: cutout.top,
            left: cutout.left,
            width: cutout.width,
            height: cutout.height,
            pointerEvents: 'none',
          }}
        />
      )}

      {/* Tooltip */}
      <div
        ref={tooltipRef}
        className={clsx(
          'absolute z-10 transition-all duration-300',
          isVisible ? 'opacity-100 scale-100' : 'opacity-0 scale-95'
        )}
        style={{
          top: tooltipPos.top,
          left: tooltipPos.left,
          width: TOOLTIP_WIDTH,
          pointerEvents: 'auto',
        }}
      >
        {/* Arrow */}
        <div
          className={clsx(
            'absolute w-3 h-3 bg-[var(--surface)] rotate-45',
            'border border-[var(--border-default)]',
            tooltipPos.arrowSide === 'top' && 'top-[-7px] left-1/2 -translate-x-1/2 border-b-0 border-r-0',
            tooltipPos.arrowSide === 'bottom' && 'bottom-[-7px] left-1/2 -translate-x-1/2 border-t-0 border-l-0',
            tooltipPos.arrowSide === 'left' && 'left-[-7px] top-1/2 -translate-y-1/2 border-t-0 border-r-0',
            tooltipPos.arrowSide === 'right' && 'right-[-7px] top-1/2 -translate-y-1/2 border-b-0 border-l-0'
          )}
        />

        {/* Tooltip content */}
        <div
          className={clsx(
            'rounded-xl shadow-xl border border-[var(--border-default)]',
            'bg-[var(--surface)] overflow-hidden'
          )}
        >
          {/* Header with step counter */}
          <div className="flex items-center justify-between px-4 pt-4 pb-2">
            <span className="text-2xs font-medium text-[var(--color-primary)] uppercase tracking-wider">
              {t('tour.stepOf', { current: currentIndex + 1, total: totalStops })}
            </span>
            <button
              onClick={handleFinish}
              className={clsx(
                'p-1 rounded-md',
                'hover:bg-[var(--bg-subtle)]',
                'text-[var(--text-muted)]',
                'transition-colors'
              )}
              aria-label="Close tour"
            >
              <Cross2Icon className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Body */}
          <div className="px-4 pb-3">
            <h3 className="text-base font-semibold text-[var(--text-primary)] mb-1">
              {t(`tour.stops.${currentStop.i18nKey}.title`)}
            </h3>
            <p className="text-sm text-[var(--text-secondary)] leading-relaxed">
              {t(`tour.stops.${currentStop.i18nKey}.description`)}
            </p>
          </div>

          {/* Progress dots */}
          <div className="flex justify-center gap-1.5 pb-3">
            {tourStops.map((_, i) => (
              <div
                key={i}
                className={clsx(
                  'w-2 h-2 rounded-full transition-all duration-200',
                  i === currentIndex
                    ? 'bg-[var(--color-primary)] w-4'
                    : i < currentIndex
                    ? 'bg-[var(--color-primary)] opacity-40'
                    : 'bg-[var(--bg-muted)]'
                )}
              />
            ))}
          </div>

          {/* Navigation */}
          <div className="flex items-center justify-between px-4 py-3 border-t border-[var(--border-default)] bg-[var(--surface-sunken)]">
            <button
              onClick={handleFinish}
              className="text-sm text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
            >
              {t('tour.skipTour')}
            </button>

            <div className="flex gap-2">
              {currentIndex > 0 && (
                <button
                  onClick={handlePrev}
                  className={clsx(
                    'inline-flex items-center gap-1 px-3 py-1.5 rounded-lg text-sm',
                    'bg-[var(--bg-subtle)] hover:bg-[var(--bg-muted)]',
                    'text-[var(--text-secondary)]',
                    'transition-colors'
                  )}
                >
                  <ChevronLeftIcon className="w-3.5 h-3.5" />
                  {t('tour.prevStop')}
                </button>
              )}

              <button
                onClick={handleNext}
                className={clsx(
                  'inline-flex items-center gap-1 px-4 py-1.5 rounded-lg text-sm font-medium',
                  'bg-[var(--color-primary)] text-[var(--text-inverted)]',
                  'hover:bg-[var(--color-primary-hover)]',
                  'transition-colors'
                )}
              >
                {isLastStop ? (
                  <>
                    <CheckCircledIcon className="w-3.5 h-3.5" />
                    {t('tour.finish')}
                  </>
                ) : (
                  <>
                    {t('tour.nextStop')}
                    <ChevronRightIcon className="w-3.5 h-3.5" />
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default FeatureTour;
