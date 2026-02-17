/**
 * AgentPipelineList Component
 *
 * Displays saved pipelines with run/edit/delete actions.
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useAgentComposerStore } from '../../store/agentComposer';
import type { AgentPipelineInfo } from '../../types/agentComposer';

export function AgentPipelineList() {
  const { t } = useTranslation('expertMode');

  const {
    pipelines,
    currentPipeline,
    loading,
    error,
    fetchPipelines,
    selectPipeline,
    startNewPipeline,
    deletePipeline,
  } = useAgentComposerStore();

  useEffect(() => {
    fetchPipelines();
  }, [fetchPipelines]);

  return (
    <div className="space-y-3">
      {/* Header with create button */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {t('agentComposer.list.title')}
        </h3>
        <button
          onClick={startNewPipeline}
          className={clsx(
            'px-3 py-1 text-xs font-medium rounded-lg',
            'bg-primary-600 text-white hover:bg-primary-700',
            'transition-colors'
          )}
        >
          {t('agentComposer.list.newPipeline')}
        </button>
      </div>

      {/* Error display */}
      {error && (
        <div className="p-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-xs text-red-600 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Loading state */}
      {loading.list && (
        <div className="text-center py-4 text-sm text-gray-500 dark:text-gray-400">
          {t('agentComposer.list.loading')}
        </div>
      )}

      {/* Pipeline list */}
      {!loading.list && pipelines.length === 0 && (
        <div className="text-center py-6 text-sm text-gray-500 dark:text-gray-400">
          {t('agentComposer.list.empty')}
        </div>
      )}

      <div className="space-y-2">
        {pipelines.map((pipeline) => (
          <PipelineCard
            key={pipeline.pipeline_id}
            pipeline={pipeline}
            isSelected={currentPipeline?.pipeline_id === pipeline.pipeline_id}
            onSelect={() => selectPipeline(pipeline.pipeline_id)}
            onDelete={() => deletePipeline(pipeline.pipeline_id)}
          />
        ))}
      </div>
    </div>
  );
}

interface PipelineCardProps {
  pipeline: AgentPipelineInfo;
  isSelected: boolean;
  onSelect: () => void;
  onDelete: () => void;
}

function PipelineCard({ pipeline, isSelected, onSelect, onDelete }: PipelineCardProps) {
  const { t } = useTranslation('expertMode');

  return (
    <div
      onClick={onSelect}
      className={clsx(
        'p-3 rounded-lg border cursor-pointer transition-all',
        isSelected
          ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 ring-1 ring-primary-300 dark:ring-primary-700'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600'
      )}
    >
      <div className="flex items-center justify-between">
        <div className="min-w-0 flex-1">
          <p className="font-medium text-sm text-gray-900 dark:text-white truncate">
            {pipeline.name}
          </p>
          {pipeline.description && (
            <p className="text-xs text-gray-500 dark:text-gray-400 truncate mt-0.5">
              {pipeline.description}
            </p>
          )}
          <div className="flex items-center gap-2 mt-1">
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {pipeline.step_count} {t('agentComposer.list.steps')}
            </span>
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {new Date(pipeline.created_at).toLocaleDateString()}
            </span>
          </div>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            if (confirm(`Delete pipeline "${pipeline.name}"?`)) {
              onDelete();
            }
          }}
          className="p-1 text-gray-400 hover:text-red-500 transition-colors"
          title={t('agentComposer.list.deletePipeline')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z" clipRule="evenodd" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default AgentPipelineList;
