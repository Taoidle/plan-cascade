import { clsx } from 'clsx';
import * as Switch from '@radix-ui/react-switch';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CustomGuardrailInput, GuardrailInfo, GuardrailScope } from '../../lib/guardrailsApi';
import { useGuardrailsStore } from '../../store/guardrails';

const SCOPE_OPTIONS: GuardrailScope[] = ['input', 'tool_call', 'tool_result', 'assistant_output', 'artifact'];

function getScopeLabel(t: (key: string, options?: Record<string, unknown>) => string, scope: GuardrailScope) {
  return t(`security.scope.${scope}`);
}

function getActionLabel(t: (key: string, options?: Record<string, unknown>) => string, action: string) {
  return t(`security.actions.${action}`, { defaultValue: action });
}

const PANEL_CLASS = 'rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800';
const PANEL_MUTED_CLASS = 'rounded-xl border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50';
const INPUT_CLASS =
  'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900 outline-none focus:border-primary-500 dark:border-gray-700 dark:bg-gray-900 dark:text-white';
const BUTTON_SECONDARY_CLASS =
  'rounded-lg border border-gray-200 px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-100 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-700';
const BUTTON_PRIMARY_CLASS =
  'rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-50';

function ScopePills({ scope }: { scope: GuardrailScope[] }) {
  const { t } = useTranslation('settings');

  return (
    <div className="flex flex-wrap gap-1 mt-2">
      {scope.map((item) => (
        <span
          key={item}
          className="rounded-full bg-gray-100 px-2 py-0.5 text-[11px] font-medium text-gray-700 dark:bg-gray-700 dark:text-gray-200"
        >
          {getScopeLabel(t, item)}
        </span>
      ))}
    </div>
  );
}

function GuardrailCard({
  guardrail,
  onToggle,
  onEdit,
  onDelete,
}: {
  guardrail: GuardrailInfo;
  onToggle: (id: string, enabled: boolean) => void;
  onEdit: (guardrail: GuardrailInfo) => void;
  onDelete: (id: string) => void;
}) {
  const { t } = useTranslation('settings');
  const isCustom = guardrail.guardrail_type === 'custom';

  return (
    <div className={clsx(PANEL_CLASS, 'p-4')}>
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h5 className="truncate text-sm font-semibold text-gray-900 dark:text-white">{guardrail.name}</h5>
            <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[11px] font-medium text-gray-700 dark:bg-gray-700 dark:text-gray-200">
              {t(`security.types.${guardrail.guardrail_type}`)}
            </span>
            <span
              className={clsx(
                'rounded-full px-2 py-0.5 text-[11px] font-medium',
                guardrail.action === 'block'
                  ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300'
                  : guardrail.action === 'redact'
                    ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300'
                    : 'bg-primary-50 text-primary-700 dark:bg-primary-950/20 dark:text-primary-300',
              )}
            >
              {getActionLabel(t, guardrail.action)}
            </span>
          </div>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{guardrail.description}</p>
          <ScopePills scope={guardrail.scope} />
          {guardrail.pattern && (
            <pre className="mt-3 overflow-x-auto rounded-lg bg-gray-950 p-3 text-xs text-gray-100">
              {guardrail.pattern}
            </pre>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {isCustom && (
            <button type="button" onClick={() => onEdit(guardrail)} className={BUTTON_SECONDARY_CLASS}>
              {t('security.custom.edit')}
            </button>
          )}
          {isCustom && (
            <button type="button" onClick={() => onDelete(guardrail.id)} className={BUTTON_SECONDARY_CLASS}>
              {t('security.custom.delete')}
            </button>
          )}
          <Switch.Root
            checked={guardrail.enabled}
            onCheckedChange={(checked) => onToggle(guardrail.id, checked)}
            className={clsx(
              'relative h-6 w-10 shrink-0 rounded-full bg-gray-200 transition-colors dark:bg-gray-700',
              'data-[state=checked]:bg-primary-600',
            )}
          >
            <Switch.Thumb
              className={clsx(
                'block h-5 w-5 rounded-full bg-white shadow transition-transform',
                'data-[state=checked]:translate-x-[18px]',
                'data-[state=unchecked]:translate-x-[2px]',
              )}
            />
          </Switch.Root>
        </div>
      </div>
    </div>
  );
}

function RuleEditor({
  editing,
  isMutating,
  onSubmit,
  onCancel,
}: {
  editing: GuardrailInfo | null;
  isMutating: boolean;
  onSubmit: (rule: CustomGuardrailInput) => Promise<boolean>;
  onCancel: () => void;
}) {
  const { t } = useTranslation('settings');
  const initialScope = editing?.scope?.length ? editing.scope : ['input', 'assistant_output', 'tool_result'];
  const [name, setName] = useState(editing?.name ?? '');
  const [pattern, setPattern] = useState(editing?.pattern ?? '');
  const [action, setAction] = useState(editing?.action ?? 'warn');
  const [scope, setScope] = useState<GuardrailScope[]>(initialScope as GuardrailScope[]);
  const [description, setDescription] = useState(editing?.description ?? '');

  useEffect(() => {
    setName(editing?.name ?? '');
    setPattern(editing?.pattern ?? '');
    setAction(editing?.action ?? 'warn');
    setScope(
      (editing?.scope?.length ? editing.scope : ['input', 'assistant_output', 'tool_result']) as GuardrailScope[],
    );
    setDescription(editing?.description ?? '');
  }, [editing]);

  const toggleScope = (value: GuardrailScope) => {
    setScope((current) => (current.includes(value) ? current.filter((item) => item !== value) : [...current, value]));
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const success = await onSubmit({
      id: editing?.id,
      name: name.trim(),
      pattern: pattern.trim(),
      action,
      enabled: editing?.enabled ?? true,
      scope,
      description: description.trim(),
    });
    if (success && !editing) {
      setName('');
      setPattern('');
      setAction('warn');
      setScope(['input', 'assistant_output', 'tool_result']);
      setDescription('');
    }
  };

  return (
    <form onSubmit={submit} className={clsx(PANEL_CLASS, 'border-dashed')}>
      <div className="mb-4 flex items-center justify-between">
        <h5 className="text-sm font-semibold text-gray-900 dark:text-white">
          {editing ? t('security.custom.editTitle') : t('security.custom.addTitle')}
        </h5>
        {editing && (
          <button type="button" onClick={onCancel} className={BUTTON_SECONDARY_CLASS}>
            {t('security.custom.cancel')}
          </button>
        )}
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <label className="block">
          <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {t('security.custom.ruleName')}
          </span>
          <input value={name} onChange={(event) => setName(event.target.value)} className={INPUT_CLASS} />
        </label>

        <label className="block">
          <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {t('security.custom.action')}
          </span>
          <select value={action} onChange={(event) => setAction(event.target.value)} className={INPUT_CLASS}>
            <option value="warn">{t('security.actions.warn')}</option>
            <option value="block">{t('security.actions.block')}</option>
            <option value="redact">{t('security.actions.redact')}</option>
          </select>
        </label>
      </div>

      <label className="mt-3 block">
        <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          {t('security.custom.regexPattern')}
        </span>
        <input
          value={pattern}
          onChange={(event) => setPattern(event.target.value)}
          className={clsx(INPUT_CLASS, 'font-mono')}
        />
      </label>

      <label className="mt-3 block">
        <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          {t('security.custom.description')}
        </span>
        <input value={description} onChange={(event) => setDescription(event.target.value)} className={INPUT_CLASS} />
      </label>

      <div className="mt-3">
        <span className="mb-2 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          {t('security.custom.scope')}
        </span>
        <div className="flex flex-wrap gap-2">
          {SCOPE_OPTIONS.map((option) => {
            const selected = scope.includes(option);
            return (
              <button
                key={option}
                type="button"
                onClick={() => toggleScope(option)}
                className={clsx(
                  'rounded-lg border px-3 py-2 text-sm transition-colors',
                  selected
                    ? 'border-primary-500 bg-primary-50 text-primary-700 dark:border-primary-500 dark:bg-primary-950/20 dark:text-primary-300'
                    : 'border-gray-200 bg-gray-50 text-gray-700 dark:border-gray-700 dark:bg-gray-900/40 dark:text-gray-200',
                )}
              >
                {getScopeLabel(t, option)}
              </button>
            );
          })}
        </div>
      </div>

      <button
        type="submit"
        disabled={isMutating || !name.trim() || !pattern.trim() || scope.length === 0}
        className={clsx('mt-4', BUTTON_PRIMARY_CLASS)}
      >
        {isMutating ? t('security.custom.saving') : editing ? t('security.custom.save') : t('security.custom.create')}
      </button>
    </form>
  );
}

function EventsTable() {
  const { t } = useTranslation('settings');
  const { events, isLoadingEvents, fetchEvents, clearEvents } = useGuardrailsStore();

  useEffect(() => {
    fetchEvents(50, 0);
  }, [fetchEvents]);

  if (isLoadingEvents) {
    return <p className="text-sm text-slate-500">{t('security.events.loading')}</p>;
  }

  if (!events.length) {
    return <p className="text-sm text-slate-500 dark:text-slate-400">{t('security.events.empty')}</p>;
  }

  return (
    <div className={PANEL_CLASS}>
      <div className="flex items-center justify-between border-b border-gray-200 px-0 pb-3 dark:border-gray-700">
        <span className="text-xs text-gray-500 dark:text-gray-400">
          {t('security.events.recentCount', { count: events.length })}
        </span>
        <button type="button" onClick={() => clearEvents()} className={BUTTON_SECONDARY_CLASS}>
          {t('security.events.clear')}
        </button>
      </div>
      <div className="overflow-x-auto pt-3">
        <table className="min-w-full text-sm">
          <thead className="bg-gray-50 text-left text-xs text-gray-500 dark:bg-gray-900 dark:text-gray-400">
            <tr>
              <th className="px-4 py-2">{t('security.events.columns.time')}</th>
              <th className="px-4 py-2">{t('security.events.columns.rule')}</th>
              <th className="px-4 py-2">{t('security.events.columns.surface')}</th>
              <th className="px-4 py-2">{t('security.events.columns.decision')}</th>
              <th className="px-4 py-2">{t('security.events.columns.preview')}</th>
            </tr>
          </thead>
          <tbody>
            {events.map((event) => (
              <tr key={event.id} className="border-t border-gray-100 dark:border-gray-700">
                <td className="px-4 py-3 text-xs text-gray-500 dark:text-gray-400">
                  {new Date(event.timestamp).toLocaleString()}
                </td>
                <td className="px-4 py-3 text-sm text-gray-900 dark:text-white">{event.rule_name}</td>
                <td className="px-4 py-3 text-xs text-gray-600 dark:text-gray-300">
                  {t(`security.scope.${event.surface}`, { defaultValue: event.surface })}
                </td>
                <td className="px-4 py-3 text-xs font-medium uppercase tracking-wide text-gray-700 dark:text-gray-200">
                  {getActionLabel(t, event.decision)}
                </td>
                <td className="px-4 py-3 text-xs text-gray-600 dark:text-gray-300">
                  <div>{event.safe_preview || t('security.events.noPreview')}</div>
                  <div className="mt-1 font-mono text-[10px] text-gray-400">{event.content_hash.slice(0, 16)}...</div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function GuardrailSection() {
  const { t } = useTranslation('settings');
  const {
    guardrails,
    runtime,
    isLoading,
    isMutating,
    error,
    fetchGuardrails,
    toggleGuardrail,
    createRule,
    updateRule,
    deleteRule,
    clearError,
  } = useGuardrailsStore();
  const [editing, setEditing] = useState<GuardrailInfo | null>(null);

  useEffect(() => {
    fetchGuardrails();
  }, [fetchGuardrails]);

  const builtinGuardrails = useMemo(
    () => guardrails.filter((guardrail) => guardrail.guardrail_type === 'builtin'),
    [guardrails],
  );
  const customGuardrails = useMemo(
    () => guardrails.filter((guardrail) => guardrail.guardrail_type === 'custom'),
    [guardrails],
  );

  const submitRule = async (rule: CustomGuardrailInput) => {
    const success = rule.id ? await updateRule(rule) : await createRule(rule);
    if (success) {
      setEditing(null);
    }
    return success;
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="mb-1 text-lg font-semibold text-gray-900 dark:text-white">{t('security.title')}</h3>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('security.description')}</p>
      </div>

      {runtime && (
        <div className="grid gap-3 md:grid-cols-2">
          <div className={PANEL_CLASS}>
            <div className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              {t('security.runtime.native.title')}
            </div>
            <p className="mt-1 text-sm text-gray-900 dark:text-white">
              {runtime.native_runtime_managed
                ? t('security.runtime.native.managed')
                : t('security.runtime.native.unmanaged')}
            </p>
            <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
              {t('security.runtime.strictMode', {
                status: runtime.strict_mode ? t('security.runtime.enabled') : t('security.runtime.disabled'),
              })}
            </p>
            {runtime.init_error && <p className="mt-2 text-xs text-red-600">{runtime.init_error}</p>}
          </div>
          <div className={PANEL_CLASS}>
            <div className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              {t('security.runtime.claudeCode.title')}
            </div>
            <p className="mt-1 text-sm text-gray-900 dark:text-white">
              {runtime.claude_code_managed
                ? t('security.runtime.claudeCode.managed')
                : t('security.runtime.claudeCode.unmanaged')}
            </p>
          </div>
        </div>
      )}

      <div className={clsx(PANEL_MUTED_CLASS, 'text-sm text-gray-600 dark:text-gray-300')}>
        {t('security.runtime.networkHint')}
      </div>

      {error && (
        <div className="flex items-center justify-between rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300">
          <span>{error}</span>
          <button type="button" onClick={clearError} className="text-xs">
            {t('security.dismiss')}
          </button>
        </div>
      )}

      <RuleEditor editing={editing} isMutating={isMutating} onSubmit={submitRule} onCancel={() => setEditing(null)} />

      {isLoading ? (
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('security.loading')}</p>
      ) : (
        <>
          <section className="space-y-3">
            <h4 className="text-sm font-medium text-gray-900 dark:text-white">{t('security.builtin.title')}</h4>
            <div className="space-y-3">
              {builtinGuardrails.map((guardrail) => (
                <GuardrailCard
                  key={guardrail.id}
                  guardrail={guardrail}
                  onToggle={toggleGuardrail}
                  onEdit={setEditing}
                  onDelete={deleteRule}
                />
              ))}
            </div>
          </section>

          <section className="space-y-3">
            <h4 className="text-sm font-medium text-gray-900 dark:text-white">{t('security.custom.title')}</h4>
            <div className="space-y-3">
              {customGuardrails.length ? (
                customGuardrails.map((guardrail) => (
                  <GuardrailCard
                    key={guardrail.id}
                    guardrail={guardrail}
                    onToggle={toggleGuardrail}
                    onEdit={setEditing}
                    onDelete={deleteRule}
                  />
                ))
              ) : (
                <div className="rounded-lg border border-dashed border-gray-300 px-4 py-3 text-sm text-gray-500 dark:border-gray-700 dark:text-gray-400">
                  {t('security.custom.empty')}
                </div>
              )}
            </div>
          </section>

          <section className="space-y-3">
            <h4 className="text-sm font-medium text-gray-900 dark:text-white">{t('security.events.title')}</h4>
            <EventsTable />
          </section>
        </>
      )}
    </div>
  );
}

export default GuardrailSection;
