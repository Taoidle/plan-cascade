/**
 * GraphEdgeConfig Component
 *
 * Sidebar panel for editing a selected edge's configuration.
 * Supports direct and conditional edge types.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { Edge } from '../../types/graphWorkflow';

interface GraphEdgeConfigProps {
  edge: Edge;
  onUpdate: (edge: Edge) => void;
  onDelete: () => void;
}

export function GraphEdgeConfig({ edge, onUpdate, onDelete }: GraphEdgeConfigProps) {
  const { t } = useTranslation('expertMode');
  const [newBranchKey, setNewBranchKey] = useState('');
  const [newBranchTarget, setNewBranchTarget] = useState('');

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{t('graphWorkflow.edgeConfig.title')}</h3>

      {/* Edge type */}
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          {t('graphWorkflow.edgeConfig.type')}
        </label>
        <div className="text-xs text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
          {edge.edge_type}
        </div>
      </div>

      {/* From node */}
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          {t('graphWorkflow.edgeConfig.from')}
        </label>
        <div className="text-xs text-gray-700 dark:text-gray-300 font-mono bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
          {edge.from}
        </div>
      </div>

      {/* Direct edge: To node */}
      {edge.edge_type === 'direct' && (
        <div>
          <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
            {t('graphWorkflow.edgeConfig.to')}
          </label>
          <div className="text-xs text-gray-700 dark:text-gray-300 font-mono bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
            {edge.to}
          </div>
        </div>
      )}

      {/* Conditional edge: Condition + Branches */}
      {edge.edge_type === 'conditional' && (
        <>
          {/* Condition key */}
          <div>
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('graphWorkflow.edgeConfig.conditionKey')}
            </label>
            <input
              type="text"
              value={edge.condition.condition_key}
              onChange={(e) => {
                onUpdate({
                  ...edge,
                  condition: { condition_key: e.target.value },
                });
              }}
              className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
              placeholder={t('graphWorkflow.edgeConfig.conditionKeyPlaceholder')}
            />
          </div>

          {/* Default branch */}
          <div>
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('graphWorkflow.edgeConfig.defaultBranch')}
            </label>
            <input
              type="text"
              value={edge.default_branch ?? ''}
              onChange={(e) => {
                onUpdate({
                  ...edge,
                  default_branch: e.target.value || null,
                });
              }}
              className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
              placeholder={t('graphWorkflow.edgeConfig.defaultBranchPlaceholder')}
            />
          </div>

          {/* Branches */}
          <div>
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('graphWorkflow.edgeConfig.branches')}
            </label>
            {Object.entries(edge.branches).length === 0 ? (
              <div className="text-xs text-gray-400 italic">{t('graphWorkflow.edgeConfig.noBranches')}</div>
            ) : (
              <div className="space-y-1">
                {Object.entries(edge.branches).map(([key, target]) => (
                  <div key={key} className="flex items-center gap-2 text-xs">
                    <span className="font-mono text-gray-700 dark:text-gray-300">{key}</span>
                    <span className="text-gray-400">-&gt;</span>
                    <span className="font-mono text-gray-700 dark:text-gray-300">{target}</span>
                    <button
                      onClick={() => {
                        const branches = { ...edge.branches };
                        delete branches[key];
                        onUpdate({ ...edge, branches });
                      }}
                      className="text-red-400 hover:text-red-600"
                    >
                      x
                    </button>
                  </div>
                ))}
              </div>
            )}

            {/* Add branch */}
            <div className="flex gap-1 mt-2">
              <input
                type="text"
                value={newBranchKey}
                onChange={(e) => setNewBranchKey(e.target.value)}
                className="flex-1 text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800"
                placeholder={t('graphWorkflow.edgeConfig.value')}
              />
              <input
                type="text"
                value={newBranchTarget}
                onChange={(e) => setNewBranchTarget(e.target.value)}
                className="flex-1 text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800"
                placeholder={t('graphWorkflow.edgeConfig.nodeIdLabel')}
              />
              <button
                onClick={() => {
                  if (newBranchKey && newBranchTarget) {
                    const branches = { ...edge.branches, [newBranchKey]: newBranchTarget };
                    onUpdate({ ...edge, branches });
                    setNewBranchKey('');
                    setNewBranchTarget('');
                  }
                }}
                disabled={!newBranchKey || !newBranchTarget}
                className="text-xs px-2 py-1 rounded bg-primary-600 text-white disabled:opacity-50"
              >
                +
              </button>
            </div>
          </div>
        </>
      )}

      {/* Delete */}
      <button
        onClick={onDelete}
        className={clsx(
          'w-full py-1.5 text-xs font-medium rounded transition-colors',
          'border border-red-300 dark:border-red-700',
          'text-red-600 dark:text-red-400',
          'hover:bg-red-50 dark:hover:bg-red-900/20',
        )}
      >
        {t('graphWorkflow.edgeConfig.deleteEdge')}
      </button>
    </div>
  );
}
