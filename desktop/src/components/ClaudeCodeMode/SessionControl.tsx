/**
 * SessionControl Component
 *
 * Session control features for interrupting, pausing, and resuming
 * AI response streaming with visual state indicators.
 *
 * Story 011-6: Session Control (Interrupt, Pause, Resume)
 */

import { useState, useCallback, useRef, useEffect, memo, createContext, useContext, ReactNode } from 'react';
import { clsx } from 'clsx';
import {
  StopIcon,
  PauseIcon,
  PlayIcon,
  ReloadIcon,
} from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';

// ============================================================================
// Types
// ============================================================================

export type SessionState = 'idle' | 'generating' | 'paused' | 'stopped' | 'error';

export interface SessionControlState {
  state: SessionState;
  partialContent: string;
  bufferedChunks: string[];
  error: string | null;
  startTime: number | null;
  elapsedTime: number;
}

export interface SessionControlActions {
  start: () => void;
  stop: () => void;
  pause: () => void;
  resume: () => void;
  reset: () => void;
  appendContent: (chunk: string) => void;
  setError: (error: string) => void;
}

export interface SessionControlContext {
  state: SessionControlState;
  actions: SessionControlActions;
  abortController: AbortController | null;
}

// ============================================================================
// Context
// ============================================================================

const SessionControlContext = createContext<SessionControlContext | null>(null);

export function useSessionControl(): SessionControlContext {
  const context = useContext(SessionControlContext);
  if (!context) {
    throw new Error('useSessionControl must be used within SessionControlProvider');
  }
  return context;
}

// ============================================================================
// SessionControlProvider Component
// ============================================================================

interface SessionControlProviderProps {
  children: ReactNode;
  onStateChange?: (state: SessionState) => void;
  onContentUpdate?: (content: string) => void;
}

export function SessionControlProvider({
  children,
  onStateChange,
  onContentUpdate,
}: SessionControlProviderProps) {
  const [controlState, setControlState] = useState<SessionControlState>({
    state: 'idle',
    partialContent: '',
    bufferedChunks: [],
    error: null,
    startTime: null,
    elapsedTime: 0,
  });

  const abortControllerRef = useRef<AbortController | null>(null);
  const timerRef = useRef<NodeJS.Timeout | null>(null);

  // Update elapsed time while generating
  useEffect(() => {
    if (controlState.state === 'generating' && controlState.startTime) {
      timerRef.current = setInterval(() => {
        setControlState((prev) => ({
          ...prev,
          elapsedTime: Date.now() - (prev.startTime || Date.now()),
        }));
      }, 100);
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, [controlState.state, controlState.startTime]);

  // Notify state changes
  useEffect(() => {
    onStateChange?.(controlState.state);
  }, [controlState.state, onStateChange]);

  // Notify content updates
  useEffect(() => {
    onContentUpdate?.(controlState.partialContent);
  }, [controlState.partialContent, onContentUpdate]);

  const start = useCallback(() => {
    // Create new AbortController
    abortControllerRef.current = new AbortController();

    setControlState({
      state: 'generating',
      partialContent: '',
      bufferedChunks: [],
      error: null,
      startTime: Date.now(),
      elapsedTime: 0,
    });
  }, []);

  const stop = useCallback(() => {
    // Abort the current request
    abortControllerRef.current?.abort();
    abortControllerRef.current = null;

    if (timerRef.current) {
      clearInterval(timerRef.current);
    }

    setControlState((prev) => ({
      ...prev,
      state: 'stopped',
      // Keep partial content when stopped
    }));
  }, []);

  const pause = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
    }

    setControlState((prev) => ({
      ...prev,
      state: 'paused',
    }));
  }, []);

  const resume = useCallback(() => {
    setControlState((prev) => ({
      ...prev,
      state: 'generating',
      // Flush buffered chunks
      partialContent: prev.partialContent + prev.bufferedChunks.join(''),
      bufferedChunks: [],
    }));
  }, []);

  const reset = useCallback(() => {
    abortControllerRef.current?.abort();
    abortControllerRef.current = null;

    if (timerRef.current) {
      clearInterval(timerRef.current);
    }

    setControlState({
      state: 'idle',
      partialContent: '',
      bufferedChunks: [],
      error: null,
      startTime: null,
      elapsedTime: 0,
    });
  }, []);

  const appendContent = useCallback((chunk: string) => {
    setControlState((prev) => {
      if (prev.state === 'paused') {
        // Buffer chunks while paused
        return {
          ...prev,
          bufferedChunks: [...prev.bufferedChunks, chunk],
        };
      }

      if (prev.state === 'generating') {
        return {
          ...prev,
          partialContent: prev.partialContent + chunk,
        };
      }

      return prev;
    });
  }, []);

  const setError = useCallback((error: string) => {
    abortControllerRef.current?.abort();
    abortControllerRef.current = null;

    if (timerRef.current) {
      clearInterval(timerRef.current);
    }

    setControlState((prev) => ({
      ...prev,
      state: 'error',
      error,
    }));
  }, []);

  const actions: SessionControlActions = {
    start,
    stop,
    pause,
    resume,
    reset,
    appendContent,
    setError,
  };

  const contextValue: SessionControlContext = {
    state: controlState,
    actions,
    abortController: abortControllerRef.current,
  };

  return (
    <SessionControlContext.Provider value={contextValue}>
      {children}
    </SessionControlContext.Provider>
  );
}

// ============================================================================
// SessionControlBar Component
// ============================================================================

interface SessionControlBarProps {
  className?: string;
}

export const SessionControlBar = memo(function SessionControlBar({
  className,
}: SessionControlBarProps) {
  const { t } = useTranslation('claudeCode');
  const { state, actions } = useSessionControl();

  const isGenerating = state.state === 'generating';
  const isPaused = state.state === 'paused';
  const isStopped = state.state === 'stopped';
  const hasError = state.state === 'error';
  const isActive = isGenerating || isPaused;

  if (state.state === 'idle') {
    return null;
  }

  return (
    <div
      className={clsx(
        'flex items-center justify-between px-4 py-2',
        'bg-gray-50 dark:bg-gray-800',
        'border-t border-gray-200 dark:border-gray-700',
        className
      )}
    >
      {/* Status indicator */}
      <div className="flex items-center gap-3">
        <SessionStateIndicator state={state.state} />
        <div className="text-sm">
          <span className="text-gray-700 dark:text-gray-300">
            {getStateLabel(state.state, t)}
          </span>
          {isActive && state.elapsedTime > 0 && (
            <span className="ml-2 text-gray-500 dark:text-gray-400">
              {formatElapsedTime(state.elapsedTime)}
            </span>
          )}
        </div>
      </div>

      {/* Control buttons */}
      <div className="flex items-center gap-2">
        {isGenerating && (
          <>
            <ControlButton
              icon={PauseIcon}
              label={t('sessionControl.pause')}
              onClick={actions.pause}
              variant="secondary"
            />
            <ControlButton
              icon={StopIcon}
              label={t('sessionControl.stop')}
              onClick={actions.stop}
              variant="danger"
            />
          </>
        )}

        {isPaused && (
          <>
            <ControlButton
              icon={PlayIcon}
              label={t('sessionControl.resume')}
              onClick={actions.resume}
              variant="primary"
            />
            <ControlButton
              icon={StopIcon}
              label={t('sessionControl.stop')}
              onClick={actions.stop}
              variant="danger"
            />
          </>
        )}

        {(isStopped || hasError) && (
          <ControlButton
            icon={ReloadIcon}
            label={t('sessionControl.retry')}
            onClick={actions.reset}
            variant="secondary"
          />
        )}
      </div>
    </div>
  );
});

// ============================================================================
// SessionStateIndicator Component
// ============================================================================

interface SessionStateIndicatorProps {
  state: SessionState;
  size?: 'sm' | 'md' | 'lg';
}

export const SessionStateIndicator = memo(function SessionStateIndicator({
  state,
  size = 'md',
}: SessionStateIndicatorProps) {
  const sizeClasses = {
    sm: 'w-2 h-2',
    md: 'w-3 h-3',
    lg: 'w-4 h-4',
  };

  const stateClasses = {
    idle: 'bg-gray-400',
    generating: 'bg-blue-500 animate-pulse',
    paused: 'bg-yellow-500',
    stopped: 'bg-orange-500',
    error: 'bg-red-500',
  };

  return (
    <span
      className={clsx(
        'rounded-full',
        sizeClasses[size],
        stateClasses[state]
      )}
      aria-label={state}
    />
  );
});

// ============================================================================
// ControlButton Component
// ============================================================================

interface ControlButtonProps {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  onClick: () => void;
  variant?: 'primary' | 'secondary' | 'danger';
  disabled?: boolean;
}

const ControlButton = memo(function ControlButton({
  icon: Icon,
  label,
  onClick,
  variant = 'secondary',
  disabled = false,
}: ControlButtonProps) {
  const variantClasses = {
    primary: 'bg-primary-600 text-white hover:bg-primary-700',
    secondary: 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600',
    danger: 'bg-red-600 text-white hover:bg-red-700',
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={clsx(
        'flex items-center gap-1.5 px-3 py-1.5 rounded',
        'text-sm font-medium',
        'transition-colors',
        'focus:outline-none focus:ring-2 focus:ring-offset-1',
        variantClasses[variant],
        disabled && 'opacity-50 cursor-not-allowed'
      )}
      title={label}
    >
      <Icon className="w-4 h-4" />
      <span>{label}</span>
    </button>
  );
});

// ============================================================================
// InlineSessionControl Component (for chat input)
// ============================================================================

interface InlineSessionControlProps {
  onStop: () => void;
  onPause: () => void;
  onResume: () => void;
  state: SessionState;
  className?: string;
}

export const InlineSessionControl = memo(function InlineSessionControl({
  onStop,
  onPause,
  onResume,
  state,
  className,
}: InlineSessionControlProps) {
  const { t } = useTranslation('claudeCode');
  const isGenerating = state === 'generating';
  const isPaused = state === 'paused';

  if (state === 'idle' || state === 'stopped' || state === 'error') {
    return null;
  }

  return (
    <div className={clsx('flex items-center gap-1', className)}>
      {isGenerating && (
        <>
          <button
            onClick={onPause}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              'bg-yellow-100 dark:bg-yellow-900/50',
              'text-yellow-700 dark:text-yellow-400',
              'hover:bg-yellow-200 dark:hover:bg-yellow-900'
            )}
            title={t('sessionControl.pause')}
          >
            <PauseIcon className="w-5 h-5" />
          </button>
          <button
            onClick={onStop}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              'bg-red-100 dark:bg-red-900/50',
              'text-red-700 dark:text-red-400',
              'hover:bg-red-200 dark:hover:bg-red-900'
            )}
            title={t('sessionControl.stop')}
          >
            <StopIcon className="w-5 h-5" />
          </button>
        </>
      )}

      {isPaused && (
        <>
          <button
            onClick={onResume}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              'bg-green-100 dark:bg-green-900/50',
              'text-green-700 dark:text-green-400',
              'hover:bg-green-200 dark:hover:bg-green-900'
            )}
            title={t('sessionControl.resume')}
          >
            <PlayIcon className="w-5 h-5" />
          </button>
          <button
            onClick={onStop}
            className={clsx(
              'p-2 rounded-lg transition-colors',
              'bg-red-100 dark:bg-red-900/50',
              'text-red-700 dark:text-red-400',
              'hover:bg-red-200 dark:hover:bg-red-900'
            )}
            title={t('sessionControl.stop')}
          >
            <StopIcon className="w-5 h-5" />
          </button>
        </>
      )}
    </div>
  );
});

// ============================================================================
// Helper Functions
// ============================================================================

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getStateLabel(state: SessionState, t: any): string {
  const labels: Record<SessionState, string> = {
    idle: t('sessionControl.states.idle'),
    generating: t('sessionControl.states.generating'),
    paused: t('sessionControl.states.paused'),
    stopped: t('sessionControl.states.stopped'),
    error: t('sessionControl.states.error'),
  };
  return labels[state] || state;
}

function formatElapsedTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;

  if (minutes > 0) {
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  }
  return `${seconds}s`;
}

// ============================================================================
// useStreamingWithControl Hook
// ============================================================================

export interface UseStreamingWithControlOptions {
  onChunk?: (chunk: string) => void;
  onComplete?: (content: string) => void;
  onError?: (error: Error) => void;
}

export function useStreamingWithControl(options: UseStreamingWithControlOptions = {}) {
  const { state, actions, abortController } = useSessionControl();

  const handleStream = useCallback(
    async (
      streamFn: (signal: AbortSignal) => AsyncIterable<string>
    ): Promise<string> => {
      if (!abortController) {
        throw new Error('No active session');
      }

      actions.start();

      try {
        let fullContent = '';

        for await (const chunk of streamFn(abortController.signal)) {
          actions.appendContent(chunk);
          fullContent += chunk;
          options.onChunk?.(chunk);
        }

        options.onComplete?.(fullContent);
        return fullContent;
      } catch (error) {
        if ((error as Error).name === 'AbortError') {
          // User cancelled - not an error
          return state.partialContent;
        }
        actions.setError((error as Error).message);
        options.onError?.(error as Error);
        throw error;
      }
    },
    [abortController, actions, options, state.partialContent]
  );

  return {
    state: state.state,
    partialContent: state.partialContent,
    elapsedTime: state.elapsedTime,
    error: state.error,
    start: actions.start,
    stop: actions.stop,
    pause: actions.pause,
    resume: actions.resume,
    reset: actions.reset,
    handleStream,
  };
}

export default SessionControlBar;
