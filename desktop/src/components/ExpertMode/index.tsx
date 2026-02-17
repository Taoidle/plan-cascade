/**
 * ExpertMode Component
 *
 * Container for Expert mode interface providing full control over:
 * - PRD generation and editing
 * - Strategy and agent selection
 * - Quality gates and worktree configuration
 * - Dependency visualization
 * - Detailed execution logs with real-time streaming
 * - Quality gate badges and error feedback
 *
 * Story 008: Added StreamingOutput, GlobalProgressBar, QualityGateBadge, ErrorState
 */

import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
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
import { SpecInterviewPanel } from './SpecInterviewPanel';
import { DesignDocPanel } from './DesignDocPanel';
import {
  StreamingOutput,
  GlobalProgressBar,
  QualityGateBadge,
  ErrorState,
} from '../shared';
import { AgentComposer } from '../AgentComposer';
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
  const { t } = useTranslation('expertMode');
  const { status, start, logs } = useExecutionStore();
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

  // Auto-switch to execution tab when execution starts
  useEffect(() => {
    if (status === 'running' && activeTab !== 'execution') {
      setActiveTab('execution');
    }
  }, [status]);

  const handleStartExecution = async () => {
    if (!hasStories) return;
    // Convert PRD stories to execution format
    const executionDescription = `Execute PRD: ${prd.title}\n\nStories:\n${prd.stories
      .map((s, i) => `${i + 1}. ${s.title}`)
      .join('\n')}`;
    await start(executionDescription, 'expert');
  };

  const handleReset = () => {
    if (confirm(t('actions.resetConfirm'))) {
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
            <TabTrigger value="generate">{t('tabs.generate')}</TabTrigger>
            <TabTrigger value="interview">Spec Interview</TabTrigger>
            <TabTrigger value="design-doc">Design Doc</TabTrigger>
            <TabTrigger value="prd" disabled={!hasStories}>
              {t('tabs.prdEditor')} {hasStories && t('prdEditor.storyCount', { count: prd.stories.length })}
            </TabTrigger>
            <TabTrigger value="preview" disabled={!hasStories}>
              {t('tabs.preview')}
            </TabTrigger>
            <TabTrigger value="execution" disabled={status === 'idle'}>
              {t('tabs.execution')}
            </TabTrigger>
            <TabTrigger value="logs" disabled={logs.length === 0}>
              {t('tabs.logs')}
            </TabTrigger>
            <TabTrigger value="composer">
              Agent Composer
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
                  title={t('settings.title')}
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
                  title={t('actions.resetTitle')}
                >
                  <ResetIcon className="w-4 h-4" />
                  {t('actions.reset')}
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
                  {t('actions.execute')}
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
            <Tabs.Content value="generate" className="h-full p-6 3xl:p-8">
              <div className="max-w-2xl 3xl:max-w-3xl mx-auto">
                <div className="mb-6">
                  <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                    {t('generate.title')}
                  </h2>
                  <p className="text-gray-600 dark:text-gray-400">
                    {t('generate.description')}
                  </p>
                </div>
                <PRDGenerationForm />
              </div>
            </Tabs.Content>

            <Tabs.Content value="interview" className="h-full">
              <SpecInterviewPanel />
            </Tabs.Content>

            <Tabs.Content value="design-doc" className="h-full">
              <DesignDocPanel />
            </Tabs.Content>

            <Tabs.Content value="prd" className="h-full p-6 3xl:p-8">
              <div className="max-w-3xl 3xl:max-w-4xl mx-auto">
                <div className="mb-6 flex items-center justify-between">
                  <div>
                    <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-1">
                      {t('prdEditor.title')}
                    </h2>
                    <p className="text-sm text-gray-600 dark:text-gray-400">
                      {t('prdEditor.description')}
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

            <Tabs.Content value="composer" className="h-full">
              <AgentComposer />
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
                {t('settings.title')}
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
  const { t } = useTranslation('expertMode');
  const { stories, currentStoryId, qualityGateResults } = useExecutionStore();

  if (stories.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 dark:text-gray-400">
        {t('execution.noExecution')}
      </div>
    );
  }

  return (
    <div className="max-w-4xl 3xl:max-w-5xl mx-auto space-y-6">
      {/* Global progress bar at the top */}
      <GlobalProgressBar showStoryLabels />

      {/* Error states */}
      <ErrorState maxErrors={5} />

      {/* Two-column layout: stories + streaming output */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left column: Stories with inline quality gate badges */}
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            Stories
          </h3>
          {stories.map((story, index) => {
            const storyGates = qualityGateResults.filter((r) => r.storyId === story.id);

            return (
              <div
                key={story.id}
                className={clsx(
                  'p-4 rounded-lg border transition-all',
                  story.id === currentStoryId
                    ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 ring-1 ring-primary-300 dark:ring-primary-700'
                    : story.status === 'completed'
                    ? 'border-success-300 dark:border-success-700 bg-success-50 dark:bg-success-950'
                    : story.status === 'failed'
                    ? 'border-error-300 dark:border-error-700 bg-error-50 dark:bg-error-950'
                    : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800'
                )}
              >
                <div className="flex items-center gap-3">
                  <span className={clsx(
                    'flex items-center justify-center w-8 h-8 rounded-full text-sm font-medium shrink-0',
                    story.status === 'completed' && 'bg-success-100 dark:bg-success-900 text-success-700 dark:text-success-300',
                    story.status === 'failed' && 'bg-error-100 dark:bg-error-900 text-error-700 dark:text-error-300',
                    story.status === 'in_progress' && 'bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300',
                    story.status === 'pending' && 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400'
                  )}>
                    {index + 1}
                  </span>
                  <div className="flex-1 min-w-0">
                    <p className="font-medium text-gray-900 dark:text-white truncate">
                      {story.title}
                    </p>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                      {story.status === 'in_progress'
                        ? `${story.progress}% complete`
                        : story.status.replace('_', ' ')}
                    </p>
                  </div>
                  {story.status === 'in_progress' && (
                    <div className="w-20 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden shrink-0">
                      <div
                        className="h-full bg-primary-600 transition-all animate-progress-pulse"
                        style={{ width: `${story.progress}%` }}
                      />
                    </div>
                  )}
                </div>

                {/* Quality gate badges inline after each story */}
                {storyGates.length > 0 && (
                  <div className="mt-3 pt-3 border-t border-gray-100 dark:border-gray-700/50">
                    <QualityGateBadge storyId={story.id} />
                  </div>
                )}

                {/* Error message for failed stories */}
                {story.status === 'failed' && story.error && (
                  <div className="mt-2 p-2 rounded bg-error-50 dark:bg-error-900/30 text-xs text-error-600 dark:text-error-400 font-mono">
                    {story.error}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Right column: Streaming output */}
        <div>
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            Live Output
          </h3>
          <StreamingOutput
            maxHeight="600px"
            showClear
          />
        </div>
      </div>
    </div>
  );
}

function LogsView() {
  const { t } = useTranslation('expertMode');
  const { logs, streamingOutput } = useExecutionStore();

  return (
    <div className="h-full flex flex-col">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('logs.title')}
        </h2>
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {t('logs.entries', { count: logs.length })}
          {streamingOutput.length > 0 && (
            <span className="ml-2">
              | {streamingOutput.length} stream events
            </span>
          )}
        </span>
      </div>

      {/* Streaming output (primary display) */}
      {streamingOutput.length > 0 && (
        <div className="mb-4">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Streaming Output
          </h3>
          <StreamingOutput maxHeight="400px" showClear />
        </div>
      )}

      {/* Traditional log entries */}
      <div className="flex-1 min-h-0">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Event Log
        </h3>
        <div
          className={clsx(
            'h-full p-4 rounded-lg font-mono text-sm',
            'bg-gray-900 text-gray-100',
            'overflow-auto'
          )}
        >
          {logs.length === 0 ? (
            <span className="text-gray-500">{t('logs.empty')}</span>
          ) : (
            <pre className="whitespace-pre-wrap">
              {logs.join('\n')}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}

export default ExpertMode;
