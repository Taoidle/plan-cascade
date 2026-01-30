/**
 * AgentRunner Component
 *
 * Execute an agent with user input and view streaming output.
 */

import { useState, useRef, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, PlayIcon, StopIcon, ReloadIcon } from '@radix-ui/react-icons';
import type { Agent, AgentRun } from '../../types/agent';
import { getStatusColor, getStatusBgColor, formatDuration } from '../../types/agent';
import { useAgentsStore } from '../../store/agents';

interface AgentRunnerProps {
  /** Agent to run */
  agent: Agent | null;
  /** Whether the dialog is open */
  open: boolean;
  /** Callback when open state changes */
  onOpenChange: (open: boolean) => void;
}

export function AgentRunner({ agent, open, onOpenChange }: AgentRunnerProps) {
  const { t } = useTranslation();
  const { runAgent, loading } = useAgentsStore();

  const [input, setInput] = useState('');
  const [output, setOutput] = useState('');
  const [currentRun, setCurrentRun] = useState<AgentRun | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  const outputRef = useRef<HTMLDivElement>(null);

  // Reset state when agent changes
  useEffect(() => {
    if (open) {
      setInput('');
      setOutput('');
      setCurrentRun(null);
      setIsRunning(false);
    }
  }, [agent, open]);

  // Auto-scroll output
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handleRun = async () => {
    if (!agent || !input.trim() || isRunning) return;

    setIsRunning(true);
    setOutput('');
    setCurrentRun(null);

    // For now, just create a run - streaming would require event listeners
    const run = await runAgent(agent.id, input.trim());

    if (run) {
      setCurrentRun(run);
      setOutput(run.output || t('agents.runPending'));
    }

    setIsRunning(false);
  };

  const handleCancel = () => {
    // TODO: Implement cancellation via Tauri event
    setIsRunning(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleRun();
    }
  };

  if (!agent) return null;

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-3xl max-h-[85vh]',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl z-50',
            'flex flex-col'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <div>
              <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
                {t('agents.runAgent')}: {agent.name}
              </Dialog.Title>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                {agent.model}
              </p>
            </div>
            <Dialog.Close asChild>
              <button
                className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
                aria-label={t('common.close')}
              >
                <Cross2Icon className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </div>

          {/* Input Area */}
          <div className="p-4 border-b border-gray-200 dark:border-gray-700">
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {t('agents.input')}
            </label>
            <div className="relative">
              <textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder={t('agents.inputPlaceholder')}
                rows={4}
                disabled={isRunning}
                className={clsx(
                  'w-full px-3 py-2 rounded-md border',
                  'border-gray-300 dark:border-gray-600',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'placeholder-gray-500',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  'disabled:opacity-50 disabled:cursor-not-allowed'
                )}
              />
            </div>
            <div className="flex items-center justify-between mt-2">
              <p className="text-xs text-gray-500 dark:text-gray-400">
                {t('agents.runShortcut')}
              </p>
              <div className="flex items-center gap-2">
                {isRunning ? (
                  <button
                    onClick={handleCancel}
                    className={clsx(
                      'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                      'bg-red-100 dark:bg-red-900/50',
                      'text-red-700 dark:text-red-300',
                      'hover:bg-red-200 dark:hover:bg-red-800',
                      'text-sm font-medium transition-colors'
                    )}
                  >
                    <StopIcon className="w-4 h-4" />
                    {t('common.cancel')}
                  </button>
                ) : (
                  <button
                    onClick={handleRun}
                    disabled={!input.trim() || loading.running}
                    className={clsx(
                      'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                      'bg-primary-600 hover:bg-primary-700',
                      'text-white text-sm font-medium',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                      'transition-colors'
                    )}
                  >
                    <PlayIcon className="w-4 h-4" />
                    {t('agents.run')}
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Output Area */}
          <div className="flex-1 flex flex-col min-h-0 p-4">
            <div className="flex items-center justify-between mb-2">
              <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('agents.output')}
              </label>
              {currentRun && (
                <div className="flex items-center gap-2">
                  <span
                    className={clsx(
                      'px-2 py-0.5 rounded text-xs font-medium',
                      getStatusBgColor(currentRun.status),
                      getStatusColor(currentRun.status)
                    )}
                  >
                    {currentRun.status}
                  </span>
                  {currentRun.duration_ms && (
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {formatDuration(currentRun.duration_ms)}
                    </span>
                  )}
                </div>
              )}
            </div>
            <div
              ref={outputRef}
              className={clsx(
                'flex-1 overflow-y-auto rounded-md border p-3',
                'border-gray-200 dark:border-gray-700',
                'bg-gray-50 dark:bg-gray-800',
                'font-mono text-sm whitespace-pre-wrap',
                'min-h-[200px]'
              )}
            >
              {isRunning ? (
                <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
                  <ReloadIcon className="w-4 h-4 animate-spin" />
                  {t('agents.running')}
                </div>
              ) : output ? (
                <div className="text-gray-900 dark:text-white">{output}</div>
              ) : (
                <div className="text-gray-400 dark:text-gray-500 italic">
                  {t('agents.outputPlaceholder')}
                </div>
              )}
            </div>

            {/* Token Usage */}
            {currentRun && (currentRun.input_tokens || currentRun.output_tokens) && (
              <div className="flex items-center gap-4 mt-2 text-xs text-gray-500 dark:text-gray-400">
                <span>
                  {t('agents.inputTokens')}: {currentRun.input_tokens?.toLocaleString() || 0}
                </span>
                <span>
                  {t('agents.outputTokens')}: {currentRun.output_tokens?.toLocaleString() || 0}
                </span>
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AgentRunner;
