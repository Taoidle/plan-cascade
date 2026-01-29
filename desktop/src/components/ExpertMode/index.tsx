/**
 * ExpertMode Component
 *
 * Container for Expert mode interface.
 * Provides full control over:
 * - PRD editing
 * - Strategy selection
 * - Agent selection
 * - Detailed execution logs
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import * as Tabs from '@radix-ui/react-tabs';
import { useExecutionStore } from '../../store/execution';
import { InputBox } from '../SimpleMode/InputBox';

export function ExpertMode() {
  const { status, start, stories } = useExecutionStore();
  const [description, setDescription] = useState('');
  const [activeTab, setActiveTab] = useState('input');

  const handleStart = async () => {
    if (!description.trim()) return;
    await start(description, 'expert');
  };

  const isRunning = status === 'running';

  return (
    <div className="h-full flex flex-col">
      <Tabs.Root
        value={activeTab}
        onValueChange={setActiveTab}
        className="flex-1 flex flex-col"
      >
        {/* Tab Navigation */}
        <Tabs.List
          className={clsx(
            'flex gap-1 px-4 pt-2',
            'border-b border-gray-200 dark:border-gray-700'
          )}
        >
          <TabTrigger value="input">Input</TabTrigger>
          <TabTrigger value="prd" disabled={stories.length === 0}>
            PRD ({stories.length})
          </TabTrigger>
          <TabTrigger value="execution" disabled={status === 'idle'}>
            Execution
          </TabTrigger>
          <TabTrigger value="logs" disabled={status === 'idle'}>
            Logs
          </TabTrigger>
        </Tabs.List>

        {/* Tab Content */}
        <div className="flex-1 overflow-auto">
          <Tabs.Content value="input" className="h-full p-4">
            <div className="max-w-3xl mx-auto space-y-6">
              <InputBox
                value={description}
                onChange={setDescription}
                onSubmit={handleStart}
                disabled={isRunning}
                placeholder="Describe your task in detail..."
              />

              {/* Options */}
              <div className="grid grid-cols-2 gap-4">
                {/* Strategy Selection */}
                <div className="p-4 rounded-lg bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
                  <h3 className="font-medium text-gray-900 dark:text-white mb-3">
                    Execution Strategy
                  </h3>
                  <select
                    className={clsx(
                      'w-full px-3 py-2 rounded-lg',
                      'bg-gray-50 dark:bg-gray-900',
                      'border border-gray-200 dark:border-gray-700',
                      'text-gray-900 dark:text-white',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500'
                    )}
                  >
                    <option value="auto">Auto (AI decides)</option>
                    <option value="direct">Direct (small task)</option>
                    <option value="hybrid">Hybrid (medium task)</option>
                    <option value="mega">Mega Plan (large project)</option>
                  </select>
                </div>

                {/* Worktree Option */}
                <div className="p-4 rounded-lg bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
                  <h3 className="font-medium text-gray-900 dark:text-white mb-3">
                    Git Options
                  </h3>
                  <label className="flex items-center gap-3">
                    <input
                      type="checkbox"
                      className="rounded text-primary-600"
                    />
                    <span className="text-gray-700 dark:text-gray-300">
                      Use Git Worktree (isolated development)
                    </span>
                  </label>
                </div>
              </div>
            </div>
          </Tabs.Content>

          <Tabs.Content value="prd" className="h-full p-4">
            <PRDEditor stories={stories} />
          </Tabs.Content>

          <Tabs.Content value="execution" className="h-full p-4">
            <ExecutionView />
          </Tabs.Content>

          <Tabs.Content value="logs" className="h-full p-4">
            <LogsView />
          </Tabs.Content>
        </div>
      </Tabs.Root>
    </div>
  );
}

interface TabTriggerProps {
  value: string;
  children: React.ReactNode;
  disabled?: boolean;
}

function TabTrigger({ value, children, disabled = false }: TabTriggerProps) {
  return (
    <Tabs.Trigger
      value={value}
      disabled={disabled}
      className={clsx(
        'px-4 py-2 rounded-t-lg font-medium text-sm transition-colors',
        'text-gray-600 dark:text-gray-400',
        'hover:text-gray-900 dark:hover:text-white',
        'data-[state=active]:text-primary-600 dark:data-[state=active]:text-primary-400',
        'data-[state=active]:bg-gray-100 dark:data-[state=active]:bg-gray-800',
        'disabled:opacity-50 disabled:cursor-not-allowed'
      )}
    >
      {children}
    </Tabs.Trigger>
  );
}

interface PRDEditorProps {
  stories: Array<{
    id: string;
    title: string;
    status: string;
    description?: string;
  }>;
}

function PRDEditor({ stories }: PRDEditorProps) {
  if (stories.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 dark:text-gray-400">
        No PRD generated yet. Submit a task description first.
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto space-y-4">
      <div className="flex justify-between items-center">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          PRD Editor
        </h2>
        <button
          className={clsx(
            'px-4 py-2 rounded-lg',
            'bg-primary-600 text-white',
            'hover:bg-primary-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          Approve & Execute
        </button>
      </div>

      <div className="space-y-3">
        {stories.map((story, index) => (
          <div
            key={story.id}
            className={clsx(
              'p-4 rounded-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700'
            )}
          >
            <div className="flex items-center gap-3">
              <span className="text-gray-400">#{index + 1}</span>
              <input
                type="text"
                defaultValue={story.title}
                className={clsx(
                  'flex-1 px-2 py-1 rounded',
                  'bg-transparent',
                  'text-gray-900 dark:text-white font-medium',
                  'border border-transparent',
                  'hover:border-gray-300 dark:hover:border-gray-600',
                  'focus:border-primary-500 focus:outline-none'
                )}
              />
            </div>
            {story.description && (
              <p className="mt-2 ml-8 text-sm text-gray-600 dark:text-gray-400">
                {story.description}
              </p>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function ExecutionView() {
  return (
    <div className="max-w-3xl mx-auto">
      <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
        Execution Progress
      </h2>
      {/* Detailed execution view will be implemented */}
      <p className="text-gray-500 dark:text-gray-400">
        Execution details will appear here during task processing.
      </p>
    </div>
  );
}

function LogsView() {
  return (
    <div className="h-full flex flex-col">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          Execution Logs
        </h2>
        <button
          className={clsx(
            'px-3 py-1.5 text-sm rounded-lg',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-700'
          )}
        >
          Clear
        </button>
      </div>
      <div
        className={clsx(
          'flex-1 p-4 rounded-lg font-mono text-sm',
          'bg-gray-900 text-gray-100',
          'overflow-auto'
        )}
      >
        <pre className="whitespace-pre-wrap">
          Logs will appear here during execution...
        </pre>
      </div>
    </div>
  );
}

export default ExpertMode;
