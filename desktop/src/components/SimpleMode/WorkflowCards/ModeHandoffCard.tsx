import { useTranslation } from 'react-i18next';
import type { ModeHandoffCardData } from '../../../types/workflowCard';

function humanizeToken(value: string): string {
  return value
    .split('_')
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ');
}

export function ModeHandoffCard({ data }: { data: ModeHandoffCardData }) {
  const { t } = useTranslation('simpleMode');
  const conversationTurns =
    data.conversationTurns ??
    (data as ModeHandoffCardData & { conversationTurnCount?: number }).conversationTurnCount ??
    0;

  const formatModeLabel = (mode: string): string =>
    t(`workflow.progress.kernel.mode.${mode}` as const, { defaultValue: humanizeToken(mode) });

  const formatSummaryKindLabel = (kind: string): string =>
    t(`workflow.modeHandoffCard.summaryKinds.${kind}` as const, { defaultValue: humanizeToken(kind) });

  const formatContextSourceLabel = (source: string): string =>
    t(`workflow.modeHandoffCard.contextSources.${source}` as const, { defaultValue: humanizeToken(source) });

  return (
    <div className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-3 text-sm text-slate-800 shadow-sm dark:border-slate-700 dark:bg-slate-900/40 dark:text-slate-100">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-500 dark:text-slate-400">
            {t('workflow.modeHandoffCard.title')}
          </p>
          <p className="mt-1 font-medium">
            {t('workflow.modeHandoffCard.route', {
              source: formatModeLabel(data.sourceMode),
              target: formatModeLabel(data.targetMode),
            })}
          </p>
        </div>
        <div className="rounded-full bg-white px-2 py-1 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
          {t('workflow.modeHandoffCard.conversationTurns', { count: conversationTurns })}
        </div>
      </div>

      {data.summaryItems.length > 0 && (
        <div className="mt-3 space-y-2">
          {data.summaryItems.map((item) => (
            <div
              key={item.id}
              className="rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-slate-700 dark:bg-slate-950/50"
            >
              <div className="mb-2 flex flex-wrap items-center gap-2">
                <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                  {formatModeLabel(item.sourceMode)}
                </span>
                <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                  {formatSummaryKindLabel(item.kind)}
                </span>
              </div>
              <p className="text-xs font-semibold text-slate-700 dark:text-slate-200">{item.title}</p>
              <p className="mt-1 whitespace-pre-wrap text-xs text-slate-600 dark:text-slate-300">{item.body}</p>
            </div>
          ))}
        </div>
      )}

      {(data.artifactRefs.length > 0 || data.contextSources.length > 0) && (
        <div className="mt-3 space-y-2 text-xs text-slate-600 dark:text-slate-300">
          {data.artifactRefs.length > 0 && (
            <p className="whitespace-pre-wrap">
              {t('workflow.modeHandoffCard.artifacts')}: {data.artifactRefs.join(', ')}
            </p>
          )}
          {data.contextSources.length > 0 && (
            <p className="whitespace-pre-wrap">
              {t('workflow.modeHandoffCard.sources')}: {data.contextSources.map(formatContextSourceLabel).join(', ')}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
