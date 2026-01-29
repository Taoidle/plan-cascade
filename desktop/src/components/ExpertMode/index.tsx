/**
 * ExpertMode Component
 *
 * Container for Expert mode interface providing full control over:
 * - PRD generation and editing
 * - Strategy and agent selection
 * - Quality gates and worktree configuration
 * - Dependency visualization
 * - Detailed execution logs
 */

import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import * as Tabs from '@radix-ui/react-tabs';
import { useExecutionStore } from '../../store/execution';
import { usePRDStore } from '../../store/prd';

// Components
import { PRDGenerationForm } from './PRDGenerationForm';
import { SortableStoryList } from './SortableStoryList';
import { StoryList } from './StoryList';
import { StrategySelector } from './StrategySelector';
import { QualityGates } from './QualityGates';
import { WorktreeToggle } from './WorktreeToggle';
import { BulkAgentSelector } from './AgentSelector';
import { DraftManager } from './DraftManager';
import { PRDPreviewPanel } from './DependencyGraph';
import {
  PlayIcon,
  ResetIcon,
  GearIcon,
} from '@radix-ui/react-icons';

// Check if dnd-kit is available (for graceful degradation)
let hasDndKit = true;
try {
  require('@dnd-kit/core');
} catch {
  hasDndKit = false;
}

export function ExpertMode() {
  const { status, start, stories: executionStories, logs } = useExecutionStore();
  const { prd, reset: resetPRD } = usePRDStore();
  const [activeTab, setActiveTab] = useState('generate');
  const [showSettings, setShowSettings] = useState(false);

  const isRunning = status === 'running';
  const hasStories = prd.stories.length > 0;

  // Switch to PRD tab when stories are generated
  useEffect(() => {
    if (hasStories && activeTab === 'generate') {
      setActiveTab('prd');
    }
  }, [hasStories]);

  const handleStartExecution = async () => {
    if (!hasStories) return;
    // Convert PRD stories to execution format
    const executionDescription = `Execute PRD: ${prd.title}\n\nStories:\n${prd.stories
      .map((s, i) => `${i + 1}. ${s.title}`)
      .join('\n')}`;
    await start(executionDescription, 'expert');
  };

  const handleReset = () => {
    if (confirm('Are you sure you want to reset? This will clear all stories.')) {
      resetPRD();
      setActiveTab('generate');
    }
  };

  return (
    <div className="h-full flex flex-col">
      <Tabs.Root
        value={activeTab}
        onValueChange={setActiveTab}
        className="flex-1 flex flex-col"
      >
        {/* Tab Navigation */}
        <div className={clsx(
          'flex items-center justify-between px-4 pt-2',
          'border-b border-gray-200 dark:border-gray-700'
        )}>
          <Tabs.List className="flex gap-1">
            <TabTrigger value="generate">Generate</TabTrigger>
            <TabTrigger value="prd" disabled={!hasStories}>
              PRD Editor {hasStories && `(${prd.stories.length})`}
            </TabTrigger>
            <TabTrigger value="preview" disabled={!hasStories}>
              Preview
            </TabTrigger>
            <TabTrigger value="execution" disabled={status === 'idle'}>
              Execution
            </TabTrigger>
            <TabTrigger value="logs" disabled={logs.length === 0}>
              Logs
            </TabTrigger>
          </Tabs.List>

          {/* Actions */}
          <div className="flex items-center gap-2 pb-2">
            {hasStories && (
              <>
                <DraftManager />
                <button
                  onClick={() => setShowSettings(!showSettings)}
                  className={clsx(
                    'p-2 rounded-lg',
                    showSettings
                      ? 'bg-primary-100 dark:bg-primary-900 text-primary-600 dark:text-primary-400'
                      : 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400',
                    'hover:bg-gray-200 dark:hover:bg-gray-600',
                    'transition-colors'
                  )}
                  title="Settings"
                >
                  <GearIcon className="w-4 h-4" />
                </button>
                <button
                  onClick={handleReset}
                  disabled={isRunning}
                  className={clsx(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm',
                    'bg-gray-100 dark:bg-gray-700',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-600',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                    'transition-colors'
                  )}
                  title="Reset PRD"
                >
                  <ResetIcon className="w-4 h-4" />
                  Reset
                </button>
                <button
                  onClick={handleStartExecution}
                  disabled={isRunning || !hasStories}
                  className={clsx(
                    'flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-sm font-medium',
                    'bg-primary-600 text-white',
                    'hover:bg-primary-700',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                    'transition-colors'
                  )}
                >
                  <PlayIcon className="w-4 h-4" />
                  Execute
                </button>
              </>
            )}
          </div>
        </div>

        {/* Tab Content */}
        <div className="flex-1 overflow-hidden flex">
          {/* Main content area */}
          <div className={clsx(
            'flex-1 overflow-auto',
            showSettings && hasStories ? 'w-2/3' : 'w-full'
          )}>
            <Tabs.Content value="generate" className="h-full p-6">
              <div className="max-w-2xl mx-auto">
                <div className="mb-6">
                  <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                    Generate PRD
                  </h2>
                  <p className="text-gray-600 dark:text-gray-400">
                    Describe your requirements and we'll generate a structured PRD with stories.
                  </p>
                </div>
                <PRDGenerationForm />
              </div>
            </Tabs.Content>

            <Tabs.Content value="prd" className="h-full p-6">
              <div className="max-w-3xl mx-auto">
                <div className="mb-6 flex items-center justify-between">
                  <div>
                    <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-1">
                      PRD Editor
                    </h2>
                    <p className="text-sm text-gray-600 dark:text-gray-400">
                      Drag to reorder, click to expand and edit
                    </p>
                  </div>
                </div>

                {/* Story List - use SortableStoryList if dnd-kit available */}
                {hasDndKit ? <SortableStoryList /> : <StoryList />}
              </div>
            </Tabs.Content>

            <Tabs.Content value="preview" className="h-full">
              <PRDPreviewPanel />
            </Tabs.Content>

            <Tabs.Content value="execution" className="h-full p-6">
              <ExecutionView />
            </Tabs.Content>

            <Tabs.Content value="logs" className="h-full p-6">
              <LogsView />
            </Tabs.Content>
          </div>

          {/* Settings sidebar */}
          {showSettings && hasStories && (
            <div className={clsx(
              'w-1/3 min-w-[300px] max-w-[400px] p-6 overflow-auto',
              'border-l border-gray-200 dark:border-gray-700',
              'bg-gray-50 dark:bg-gray-900'
            )}>
              <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-6">
                Execution Settings
              </h3>

              <div className="space-y-6">
                <StrategySelector />
                <BulkAgentSelector />
                <QualityGates />
                <WorktreeToggle />
              </div>
            </div>
          )}
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

function ExecutionView() {
  const { stories, status, progress, currentStoryId } = useExecutionStore();

  if (stories.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 dark:text-gray-400">
        No execution in progress
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
          Execution Progress
        </h2>
        <span className="text-2xl font-bold text-primary-600">
          {Math.round(progress)}%
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          className="h-full bg-primary-600 transition-all duration-300"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Stories list */}
      <div className="space-y-2">
        {stories.map((story, index) => (
          <div
            key={story.id}
            className={clsx(
              'p-4 rounded-lg border transition-all',
              story.id === currentStoryId
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : story.status === 'completed'
                ? 'border-green-500 bg-green-50 dark:bg-green-900/20'
                : story.status === 'failed'
                ? 'border-red-500 bg-red-50 dark:bg-red-900/20'
                : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800'
            )}
          >
            <div className="flex items-center gap-3">
              <span className="flex items-center justify-center w-8 h-8 rounded-full bg-gray-100 dark:bg-gray-700 text-sm font-medium">
                {index + 1}
              </span>
              <div className="flex-1">
                <p className="font-medium text-gray-900 dark:text-white">
                  {story.title}
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  {story.status === 'in_progress'
                    ? `${story.progress}% complete`
                    : story.status.replace('_', ' ')}
                </p>
              </div>
              {story.status === 'in_progress' && (
                <div className="w-20 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-primary-600 transition-all"
                    style={{ width: `${story.progress}%` }}
                  />
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function LogsView() {
  const { logs } = useExecutionStore();

  return (
    <div className="h-full flex flex-col">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          Execution Logs
        </h2>
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {logs.length} entries
        </span>
      </div>
      <div
        className={clsx(
          'flex-1 p-4 rounded-lg font-mono text-sm',
          'bg-gray-900 text-gray-100',
          'overflow-auto'
        )}
      >
        {logs.length === 0 ? (
          <span className="text-gray-500">No logs yet...</span>
        ) : (
          <pre className="whitespace-pre-wrap">
            {logs.join('\n')}
          </pre>
        )}
      </div>
    </div>
  );
}

export default ExpertMode;
