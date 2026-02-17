/**
 * GuardrailSection Component
 *
 * Settings panel for configuring guardrail security rules:
 * - Toggle built-in guardrails (SensitiveData, CodeSecurity)
 * - Add/remove custom rules
 * - View trigger log history
 */

import { clsx } from 'clsx';
import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useGuardrailsStore } from '../../store/guardrails';

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Toggle switch for a single guardrail. */
function GuardrailToggle({
  name,
  description,
  enabled,
  guardrailType,
  onToggle,
  onRemove,
}: {
  name: string;
  description: string;
  enabled: boolean;
  guardrailType: string;
  onToggle: (name: string, enabled: boolean) => void;
  onRemove?: (name: string) => void;
}) {
  const { t } = useTranslation('settings');
  return (
    <div
      className={clsx(
        'flex items-center justify-between py-3 px-4',
        'border border-gray-200 dark:border-gray-700 rounded-lg',
        'bg-white dark:bg-gray-800',
      )}
    >
      <div className="flex-1 min-w-0 mr-4">
        <div className="flex items-center gap-2">
          <span className="font-medium text-gray-900 dark:text-white text-sm">
            {name}
          </span>
          <span
            className={clsx(
              'text-xs px-1.5 py-0.5 rounded',
              guardrailType === 'builtin'
                ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                : 'bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300',
            )}
          >
            {guardrailType}
          </span>
        </div>
        {description && (
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">
            {description}
          </p>
        )}
      </div>
      <div className="flex items-center gap-2 shrink-0">
        {guardrailType === 'custom' && onRemove && (
          <button
            onClick={() => onRemove(name)}
            className={clsx(
              'p-1 rounded text-gray-400 hover:text-red-500',
              'dark:text-gray-500 dark:hover:text-red-400',
              'focus:outline-none',
            )}
            title={t('security.removeRule')}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        )}
        <button
          role="switch"
          aria-checked={enabled}
          onClick={() => onToggle(name, !enabled)}
          className={clsx(
            'relative w-10 h-5 rounded-full transition-colors',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
            enabled
              ? 'bg-primary-600'
              : 'bg-gray-300 dark:bg-gray-600',
          )}
        >
          <span
            className={clsx(
              'absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform',
              enabled && 'translate-x-5',
            )}
          />
        </button>
      </div>
    </div>
  );
}

/** Form for adding a new custom rule. */
function AddCustomRuleForm({ onAdd }: { onAdd: (name: string, pattern: string, action: string) => Promise<boolean> }) {
  const { t } = useTranslation('settings');
  const [name, setName] = useState('');
  const [pattern, setPattern] = useState('');
  const [action, setAction] = useState('warn');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !pattern.trim()) {
      setFormError(t('security.custom.nameRequired'));
      return;
    }
    setIsSubmitting(true);
    setFormError(null);
    const success = await onAdd(name.trim(), pattern.trim(), action);
    setIsSubmitting(false);
    if (success) {
      setName('');
      setPattern('');
      setAction('warn');
    } else {
      setFormError(t('security.custom.addFailed'));
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div className="grid grid-cols-3 gap-3">
        <div>
          <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('security.custom.ruleName')}
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('security.custom.ruleNamePlaceholder')}
            className={clsx(
              'w-full px-3 py-1.5 rounded-lg text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('security.custom.regexPattern')}
          </label>
          <input
            type="text"
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            placeholder={t('security.custom.regexPatternPlaceholder')}
            className={clsx(
              'w-full px-3 py-1.5 rounded-lg text-sm font-mono',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('security.custom.action')}
          </label>
          <select
            value={action}
            onChange={(e) => setAction(e.target.value)}
            className={clsx(
              'w-full px-3 py-1.5 rounded-lg text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          >
            <option value="warn">{t('security.custom.actionWarn')}</option>
            <option value="block">{t('security.custom.actionBlock')}</option>
            <option value="redact">{t('security.custom.actionRedact')}</option>
          </select>
        </div>
      </div>
      {formError && (
        <p className="text-xs text-red-500">{formError}</p>
      )}
      <button
        type="submit"
        disabled={isSubmitting}
        className={clsx(
          'px-4 py-1.5 rounded-lg text-sm font-medium',
          'bg-primary-600 text-white',
          'hover:bg-primary-700',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'focus:outline-none focus:ring-2 focus:ring-primary-500',
        )}
      >
        {isSubmitting ? t('security.custom.adding') : t('security.custom.addButton')}
      </button>
    </form>
  );
}

/** Trigger log viewer table. */
function TriggerLogViewer() {
  const { t } = useTranslation('settings');
  const { triggerLog, isLoadingLog, fetchTriggerLog, clearTriggerLog } = useGuardrailsStore();

  useEffect(() => {
    fetchTriggerLog(50, 0);
  }, [fetchTriggerLog]);

  const resultTypeColor = (type: string) => {
    switch (type) {
      case 'block': return 'text-red-600 dark:text-red-400';
      case 'redact': return 'text-yellow-600 dark:text-yellow-400';
      case 'warn': return 'text-orange-600 dark:text-orange-400';
      default: return 'text-gray-600 dark:text-gray-400';
    }
  };

  if (isLoadingLog) {
    return <p className="text-sm text-gray-500">{t('security.triggerLog.loading')}</p>;
  }

  if (triggerLog.length === 0) {
    return (
      <p className="text-sm text-gray-500 dark:text-gray-400">
        {t('security.triggerLog.empty')}
      </p>
    );
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs text-gray-500 dark:text-gray-400">
          {t('security.triggerLog.eventCount', { count: triggerLog.length })}
        </span>
        <button
          onClick={() => clearTriggerLog()}
          className={clsx(
            'text-xs text-red-500 hover:text-red-700',
            'dark:text-red-400 dark:hover:text-red-300',
          )}
        >
          {t('security.triggerLog.clearLog')}
        </button>
      </div>
      <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 dark:bg-gray-800">
            <tr>
              <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400">{t('security.triggerLog.colTime')}</th>
              <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400">{t('security.triggerLog.colGuardrail')}</th>
              <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400">{t('security.triggerLog.colDirection')}</th>
              <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400">{t('security.triggerLog.colResult')}</th>
              <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400">{t('security.triggerLog.colContent')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
            {triggerLog.map((entry) => (
              <tr key={entry.id} className="bg-white dark:bg-gray-900">
                <td className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                  {new Date(entry.timestamp).toLocaleString()}
                </td>
                <td className="px-3 py-2 text-xs font-medium text-gray-900 dark:text-white">
                  {entry.guardrail_name}
                </td>
                <td className="px-3 py-2 text-xs text-gray-600 dark:text-gray-300">
                  {entry.direction}
                </td>
                <td className={clsx('px-3 py-2 text-xs font-medium', resultTypeColor(entry.result_type))}>
                  {entry.result_type}
                </td>
                <td className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 max-w-[200px] truncate">
                  {entry.content_snippet}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function GuardrailSection() {
  const { t } = useTranslation('settings');
  const {
    guardrails,
    isLoading,
    error,
    fetchGuardrails,
    toggleGuardrail,
    addCustomRule,
    removeCustomRule,
    clearError,
  } = useGuardrailsStore();

  useEffect(() => {
    fetchGuardrails();
  }, [fetchGuardrails]);

  const handleToggle = useCallback(
    (name: string, enabled: boolean) => {
      toggleGuardrail(name, enabled);
    },
    [toggleGuardrail],
  );

  const handleRemove = useCallback(
    (name: string) => {
      removeCustomRule(name);
    },
    [removeCustomRule],
  );

  const handleAddRule = useCallback(
    async (name: string, pattern: string, action: string) => {
      return await addCustomRule(name, pattern, action);
    },
    [addCustomRule],
  );

  const builtinGuardrails = guardrails.filter((g) => g.guardrail_type === 'builtin');
  const customGuardrails = guardrails.filter((g) => g.guardrail_type === 'custom');

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('security.title')}
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          {t('security.description')}
        </p>
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center justify-between p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
          <button
            onClick={clearError}
            className="text-xs text-red-500 hover:text-red-700"
          >
            {t('security.dismiss')}
          </button>
        </div>
      )}

      {isLoading ? (
        <p className="text-sm text-gray-500">{t('security.loading')}</p>
      ) : (
        <>
          {/* Built-in Guardrails */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {t('security.builtin.title')}
            </h4>
            <div className="space-y-2">
              {builtinGuardrails.map((g) => (
                <GuardrailToggle
                  key={g.name}
                  name={g.name}
                  description={g.description}
                  enabled={g.enabled}
                  guardrailType={g.guardrail_type}
                  onToggle={handleToggle}
                />
              ))}
              {builtinGuardrails.length === 0 && (
                <p className="text-sm text-gray-400">{t('security.builtin.empty')}</p>
              )}
            </div>
          </div>

          {/* Custom Rules */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {t('security.custom.title')}
            </h4>
            <div className="space-y-2">
              {customGuardrails.map((g) => (
                <GuardrailToggle
                  key={g.name}
                  name={g.name}
                  description={g.description}
                  enabled={g.enabled}
                  guardrailType={g.guardrail_type}
                  onToggle={handleToggle}
                  onRemove={handleRemove}
                />
              ))}
              {customGuardrails.length === 0 && (
                <p className="text-sm text-gray-400">{t('security.custom.empty')}</p>
              )}
            </div>

            {/* Add Custom Rule Form */}
            <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-800/50 rounded-lg border border-gray-200 dark:border-gray-700">
              <h5 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                {t('security.custom.addTitle')}
              </h5>
              <AddCustomRuleForm onAdd={handleAddRule} />
            </div>
          </div>

          {/* Trigger Log */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {t('security.triggerLog.title')}
            </h4>
            <TriggerLogViewer />
          </div>
        </>
      )}
    </div>
  );
}

export default GuardrailSection;
