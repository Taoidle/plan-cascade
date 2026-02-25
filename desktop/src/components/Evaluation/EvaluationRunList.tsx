/**
 * EvaluationRunList Component
 *
 * Displays evaluation runs with ability to create new runs,
 * configure models and test cases, and view run status.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useEvaluationStore } from '../../store/evaluation';
import type { ModelConfig, EvaluationCase } from '../../types/evaluation';
import { createDefaultCase } from '../../types/evaluation';

export function EvaluationRunList() {
  const { t } = useTranslation('expertMode');
  const {
    evaluators,
    runs,
    selectedModels,
    testCases,
    isRunning,
    loading,
    addModel,
    removeModel,
    addTestCase,
    updateTestCase,
    removeTestCase,
    startRun,
    removeRun,
    selectRun,
    setActiveTab,
  } = useEvaluationStore();

  const [showNewRun, setShowNewRun] = useState(false);
  const [selectedEvaluatorId, setSelectedEvaluatorId] = useState('');

  const handleCreateRun = async () => {
    if (!selectedEvaluatorId) return;
    await startRun(selectedEvaluatorId);
    setShowNewRun(false);
  };

  const handleViewReports = (runId: string) => {
    selectRun(runId);
    setActiveTab('reports');
  };

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('evaluation.runs.title')}</h2>
        <button
          onClick={() => setShowNewRun(!showNewRun)}
          disabled={evaluators.length === 0}
          className={clsx(
            'px-3 py-1.5 text-sm font-medium rounded-lg transition-colors',
            'bg-primary-600 text-white hover:bg-primary-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
        >
          {showNewRun ? t('evaluation.runs.cancel') : t('evaluation.runs.newRun')}
        </button>
      </div>

      {evaluators.length === 0 && (
        <div className="text-sm text-gray-500 dark:text-gray-400 italic">{t('evaluation.runs.noEvaluator')}</div>
      )}

      {/* New Run Configuration */}
      {showNewRun && (
        <div
          className={clsx(
            'p-4 rounded-lg border',
            'border-primary-200 dark:border-primary-800',
            'bg-primary-50/50 dark:bg-primary-900/10',
          )}
        >
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white mb-4">
            {t('evaluation.runs.configTitle')}
          </h3>

          {/* Evaluator selection */}
          <div className="mb-4">
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('evaluation.runs.evaluator')}
            </label>
            <select
              value={selectedEvaluatorId}
              onChange={(e) => setSelectedEvaluatorId(e.target.value)}
              className="w-full text-sm px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
            >
              <option value="">{t('evaluation.runs.evaluatorPlaceholder')}</option>
              {evaluators.map((ev) => (
                <option key={ev.id} value={ev.id}>
                  {ev.name}
                </option>
              ))}
            </select>
          </div>

          {/* Models */}
          <div className="mb-4">
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('evaluation.runs.modelsToCompare')}
            </label>
            <ModelSelector models={selectedModels} onAdd={addModel} onRemove={removeModel} />
          </div>

          {/* Test Cases */}
          <div className="mb-4">
            <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
              {t('evaluation.runs.testCases')}
            </label>
            <TestCaseEditor cases={testCases} onAdd={addTestCase} onUpdate={updateTestCase} onRemove={removeTestCase} />
          </div>

          {/* Start button */}
          <button
            onClick={handleCreateRun}
            disabled={isRunning || !selectedEvaluatorId || selectedModels.length === 0 || testCases.length === 0}
            className={clsx(
              'w-full py-2 text-sm font-medium rounded-lg transition-colors',
              'bg-primary-600 text-white hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isRunning ? t('evaluation.runs.creating') : t('evaluation.runs.createButton')}
          </button>
        </div>
      )}

      {/* Run History */}
      {loading.runs ? (
        <div className="text-sm text-gray-500 dark:text-gray-400">{t('evaluation.runs.loading')}</div>
      ) : runs.length === 0 ? (
        <div className="text-center py-12 text-gray-500 dark:text-gray-400">
          <p className="text-sm">{t('evaluation.runs.empty')}</p>
          <p className="text-xs mt-1">{t('evaluation.runs.emptyHint')}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {runs.map((run) => (
            <div
              key={run.id}
              className={clsx(
                'flex items-center justify-between p-4 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
              )}
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-gray-900 dark:text-white truncate">
                    Run {run.id.slice(0, 8)}
                  </span>
                  <StatusBadge status={run.status} />
                </div>
                <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                  {t('evaluation.runs.modelCount', { count: run.model_count })} |{' '}
                  {t('evaluation.runs.caseCount', { count: run.case_count })} |{' '}
                  {new Date(run.created_at).toLocaleDateString()}
                </div>
              </div>
              <div className="flex items-center gap-2">
                {run.status === 'completed' && (
                  <button
                    onClick={() => handleViewReports(run.id)}
                    className="px-3 py-1 text-xs font-medium rounded bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 hover:bg-green-200 dark:hover:bg-green-900/50 transition-colors"
                  >
                    {t('evaluation.runs.viewReports')}
                  </button>
                )}
                <button
                  onClick={() => removeRun(run.id)}
                  className="px-2 py-1 text-xs text-red-500 hover:text-red-700 dark:hover:text-red-400 transition-colors"
                >
                  {t('evaluation.runs.delete')}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Sub-Components
// ============================================================================

function StatusBadge({ status }: { status: string }) {
  const colorMap: Record<string, string> = {
    pending: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300',
    running: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
    completed: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
    failed: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
  };

  return (
    <span
      className={clsx(
        'text-[10px] px-1.5 py-0.5 rounded font-medium',
        colorMap[status] ?? 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400',
      )}
    >
      {status}
    </span>
  );
}

function ModelSelector({
  models,
  onAdd,
  onRemove,
}: {
  models: ModelConfig[];
  onAdd: (model: ModelConfig) => void;
  onRemove: (index: number) => void;
}) {
  const { t } = useTranslation('expertMode');
  const [provider, setProvider] = useState('anthropic');
  const [model, setModel] = useState('');

  const handleAdd = () => {
    if (model.trim()) {
      onAdd({ provider, model: model.trim(), display_name: null });
      setModel('');
    }
  };

  return (
    <div className="space-y-2">
      {models.map((m, i) => (
        <div key={i} className="flex items-center gap-2 text-xs">
          <span className="font-mono text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
            {m.provider}/{m.model}
          </span>
          <button onClick={() => onRemove(i)} className="text-red-400 hover:text-red-600">
            x
          </button>
        </div>
      ))}
      <div className="flex gap-1">
        <select
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
          className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800"
        >
          <option value="anthropic">{t('evaluation.runs.providerAnthropic')}</option>
          <option value="openai">{t('evaluation.runs.providerOpenAI')}</option>
          <option value="deepseek">{t('evaluation.runs.providerDeepSeek')}</option>
          <option value="ollama">{t('evaluation.runs.providerOllama')}</option>
        </select>
        <input
          type="text"
          value={model}
          onChange={(e) => setModel(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
          className="flex-1 text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800"
          placeholder={t('evaluation.runs.modelNamePlaceholder')}
        />
        <button
          onClick={handleAdd}
          disabled={!model.trim()}
          className="px-2 py-1 text-xs rounded bg-primary-600 text-white disabled:opacity-50"
        >
          +
        </button>
      </div>
    </div>
  );
}

function TestCaseEditor({
  cases,
  onAdd,
  onUpdate,
  onRemove,
}: {
  cases: EvaluationCase[];
  onAdd: (c: EvaluationCase) => void;
  onUpdate: (id: string, updates: Partial<EvaluationCase>) => void;
  onRemove: (id: string) => void;
}) {
  const { t } = useTranslation('expertMode');

  const handleAddCase = () => {
    const id = `case-${Date.now()}`;
    onAdd(createDefaultCase(id));
  };

  return (
    <div className="space-y-2">
      {cases.map((tc) => (
        <div
          key={tc.id}
          className="p-3 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 space-y-2"
        >
          <div className="flex items-center justify-between">
            <input
              type="text"
              value={tc.name}
              onChange={(e) => onUpdate(tc.id, { name: e.target.value })}
              className="text-xs font-medium bg-transparent border-none outline-none text-gray-900 dark:text-white flex-1"
              placeholder={t('evaluation.runs.caseName')}
            />
            <button onClick={() => onRemove(tc.id)} className="text-xs text-red-400 hover:text-red-600">
              {t('evaluation.runs.remove')}
            </button>
          </div>
          <textarea
            value={typeof tc.input === 'object' ? (((tc.input as Record<string, unknown>).prompt as string) ?? '') : ''}
            onChange={(e) => onUpdate(tc.id, { input: { prompt: e.target.value } })}
            className="w-full text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-gray-900 h-12 resize-y"
            placeholder={t('evaluation.runs.inputPlaceholder')}
          />
          <input
            type="text"
            value={tc.expected_output ?? ''}
            onChange={(e) => onUpdate(tc.id, { expected_output: e.target.value || null })}
            className="w-full text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-gray-900"
            placeholder={t('evaluation.runs.expectedOutput')}
          />
        </div>
      ))}
      <button
        onClick={handleAddCase}
        className="w-full py-1.5 text-xs font-medium rounded border border-dashed border-gray-300 dark:border-gray-600 text-gray-500 dark:text-gray-400 hover:border-primary-400 hover:text-primary-500 transition-colors"
      >
        {t('evaluation.runs.addTestCase')}
      </button>
    </div>
  );
}
