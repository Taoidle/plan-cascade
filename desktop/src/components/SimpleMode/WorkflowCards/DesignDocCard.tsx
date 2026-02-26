/**
 * DesignDocCard
 *
 * Displays a non-interactive summary of the generated design document,
 * showing components, patterns, decisions, and feature mappings.
 */

import { useState, type ReactNode } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronRightIcon } from '@radix-ui/react-icons';
import type { DesignDocCardData } from '../../../types/workflowCard';
import { Collapsible } from '../Collapsible';

export function DesignDocCard({ data }: { data: DesignDocCardData }) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);
  const safeData = data as Partial<DesignDocCardData>;

  // Runtime compatibility: old persisted cards may not include newly added fields.
  const title = safeData.title ?? t('workflow.designDoc.title');
  const summary = safeData.summary ?? '';
  const systemOverview = safeData.systemOverview ?? '';
  const dataFlow = safeData.dataFlow ?? '';
  const infrastructure = {
    existingServices: safeData.infrastructure?.existingServices ?? [],
    newServices: safeData.infrastructure?.newServices ?? [],
  };
  const components = (safeData.components ?? []).map((component) => ({
    name: component?.name ?? '',
    description: component?.description ?? '',
    responsibilities: component?.responsibilities ?? [],
    dependencies: component?.dependencies ?? [],
    features: component?.features ?? [],
  }));
  const patterns = (safeData.patterns ?? []).map((pattern) => ({
    name: pattern?.name ?? '',
    description: pattern?.description ?? '',
    rationale: pattern?.rationale ?? '',
    appliesTo: pattern?.appliesTo ?? [],
  }));
  const decisions = (safeData.decisions ?? []).map((decision) => ({
    id: decision?.id ?? '',
    title: decision?.title ?? '',
    context: decision?.context ?? '',
    decision: decision?.decision ?? '',
    rationale: decision?.rationale ?? '',
    alternatives: decision?.alternatives ?? [],
    status: decision?.status ?? '',
    appliesTo: decision?.appliesTo ?? [],
  }));
  const featureMappings = (safeData.featureMappings ?? []).map((mapping) => ({
    featureId: mapping?.featureId ?? '',
    description: mapping?.description ?? '',
    components: mapping?.components ?? [],
    patterns: mapping?.patterns ?? [],
    decisions: mapping?.decisions ?? [],
  }));
  const componentsCount = safeData.componentsCount ?? components.length;
  const patternsCount = safeData.patternsCount ?? patterns.length;
  const decisionsCount = safeData.decisionsCount ?? decisions.length;
  const featureMappingsCount = safeData.featureMappingsCount ?? featureMappings.length;
  const savedPath = safeData.savedPath ?? null;

  return (
    <div className="rounded-lg border border-teal-200 dark:border-teal-800 bg-teal-50 dark:bg-teal-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-teal-100/50 dark:bg-teal-900/30 border-b border-teal-200 dark:border-teal-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-teal-700 dark:text-teal-300 uppercase tracking-wide">
            {t('workflow.designDoc.title')}
          </span>
          <button
            onClick={() => setExpanded((v) => !v)}
            className="text-2xs text-teal-600 dark:text-teal-400 hover:text-teal-800 dark:hover:text-teal-200 transition-colors"
          >
            <ChevronRightIcon
              className={clsx('w-3.5 h-3.5 transition-transform duration-200', expanded && 'rotate-90')}
            />
          </button>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Title & summary */}
        <p className="text-sm font-medium text-teal-800 dark:text-teal-200">{title}</p>
        <p className="text-xs text-teal-700/80 dark:text-teal-300/80">{summary}</p>

        {/* Stats row */}
        <div className="grid grid-cols-4 gap-2">
          <StatPill label={t('workflow.designDoc.components')} value={componentsCount} />
          <StatPill label={t('workflow.designDoc.patterns')} value={patternsCount} />
          <StatPill label={t('workflow.designDoc.decisions')} value={decisionsCount} />
          <StatPill label={t('workflow.designDoc.featureMappings')} value={featureMappingsCount} />
        </div>

        {/* Expanded details */}
        <Collapsible open={expanded}>
          <div className="space-y-2 pt-1 border-t border-teal-200 dark:border-teal-800">
            {systemOverview && (
              <Section title={t('workflow.designDoc.systemOverview', { defaultValue: 'System Overview' })}>
                <p className="text-2xs text-teal-700/90 dark:text-teal-300/90 whitespace-pre-wrap">{systemOverview}</p>
              </Section>
            )}

            {dataFlow && (
              <Section title={t('workflow.designDoc.dataFlow', { defaultValue: 'Data Flow' })}>
                <p className="text-2xs text-teal-700/90 dark:text-teal-300/90 whitespace-pre-wrap">{dataFlow}</p>
              </Section>
            )}

            {(infrastructure.existingServices.length > 0 || infrastructure.newServices.length > 0) && (
              <Section title={t('workflow.designDoc.infrastructure', { defaultValue: 'Infrastructure' })}>
                {infrastructure.existingServices.length > 0 && (
                  <div>
                    <span className="text-2xs text-teal-600 dark:text-teal-400">
                      {t('workflow.designDoc.existingServices', { defaultValue: 'Existing Services' })}
                    </span>
                    <ul className="mt-0.5 space-y-0.5">
                      {infrastructure.existingServices.map((service) => (
                        <li key={`existing-${service}`} className="text-2xs text-teal-700 dark:text-teal-300">
                          • {service}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                {infrastructure.newServices.length > 0 && (
                  <div className="mt-1">
                    <span className="text-2xs text-teal-600 dark:text-teal-400">
                      {t('workflow.designDoc.newServices', { defaultValue: 'New Services' })}
                    </span>
                    <ul className="mt-0.5 space-y-0.5">
                      {infrastructure.newServices.map((service) => (
                        <li key={`new-${service}`} className="text-2xs text-teal-700 dark:text-teal-300">
                          • {service}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </Section>
            )}

            {components.length > 0 && (
              <Section title={t('workflow.designDoc.components')}>
                <div className="space-y-1.5">
                  {components.map((component) => (
                    <div key={component.name} className="rounded border border-teal-200 dark:border-teal-800 p-1.5">
                      <div className="text-2xs font-medium text-teal-700 dark:text-teal-300">{component.name}</div>
                      {component.description && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90">
                          {component.description}
                        </div>
                      )}
                      {component.responsibilities.length > 0 && (
                        <div className="mt-0.5 text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.responsibilities', { defaultValue: 'Responsibilities' })}:{' '}
                          {component.responsibilities.join(', ')}
                        </div>
                      )}
                      {component.dependencies.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.dependencies', { defaultValue: 'Dependencies' })}:{' '}
                          {component.dependencies.join(', ')}
                        </div>
                      )}
                      {component.features.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.features', { defaultValue: 'Features' })}:{' '}
                          {component.features.join(', ')}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </Section>
            )}

            {patterns.length > 0 && (
              <Section title={t('workflow.designDoc.patterns')}>
                <div className="space-y-1.5">
                  {patterns.map((pattern) => (
                    <div key={pattern.name} className="rounded border border-teal-200 dark:border-teal-800 p-1.5">
                      <div className="text-2xs font-medium text-teal-700 dark:text-teal-300">{pattern.name}</div>
                      {pattern.description && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90">
                          {pattern.description}
                        </div>
                      )}
                      {pattern.rationale && (
                        <div className="mt-0.5 text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.rationale', { defaultValue: 'Rationale' })}: {pattern.rationale}
                        </div>
                      )}
                      {pattern.appliesTo.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.appliesTo', { defaultValue: 'Applies To' })}:{' '}
                          {pattern.appliesTo.join(', ')}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </Section>
            )}

            {decisions.length > 0 && (
              <Section title={t('workflow.designDoc.decisions')}>
                <div className="space-y-1.5">
                  {decisions.map((decision) => (
                    <div key={decision.id} className="rounded border border-teal-200 dark:border-teal-800 p-1.5">
                      <div className="text-2xs font-medium text-teal-700 dark:text-teal-300">
                        {decision.id}: {decision.title}
                      </div>
                      <div className="text-2xs text-teal-600 dark:text-teal-400">
                        {t('workflow.designDoc.status', { defaultValue: 'Status' })}: {decision.status}
                      </div>
                      {decision.context && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90 whitespace-pre-wrap">
                          <span className="font-medium">
                            {t('workflow.designDoc.context', { defaultValue: 'Context' })}:{' '}
                          </span>
                          {decision.context}
                        </div>
                      )}
                      {decision.decision && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90 whitespace-pre-wrap">
                          <span className="font-medium">
                            {t('workflow.designDoc.decision', { defaultValue: 'Decision' })}:{' '}
                          </span>
                          {decision.decision}
                        </div>
                      )}
                      {decision.rationale && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90 whitespace-pre-wrap">
                          <span className="font-medium">
                            {t('workflow.designDoc.rationale', { defaultValue: 'Rationale' })}:{' '}
                          </span>
                          {decision.rationale}
                        </div>
                      )}
                      {decision.alternatives.length > 0 && (
                        <div className="mt-0.5 text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.alternatives', { defaultValue: 'Alternatives' })}:{' '}
                          {decision.alternatives.join(', ')}
                        </div>
                      )}
                      {decision.appliesTo.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.appliesTo', { defaultValue: 'Applies To' })}:{' '}
                          {decision.appliesTo.join(', ')}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </Section>
            )}

            {featureMappings.length > 0 && (
              <Section title={t('workflow.designDoc.featureMappings')}>
                <div className="space-y-1.5">
                  {featureMappings.map((mapping) => (
                    <div key={mapping.featureId} className="rounded border border-teal-200 dark:border-teal-800 p-1.5">
                      <div className="text-2xs font-medium text-teal-700 dark:text-teal-300">{mapping.featureId}</div>
                      {mapping.description && (
                        <div className="mt-0.5 text-2xs text-teal-700/90 dark:text-teal-300/90">
                          {mapping.description}
                        </div>
                      )}
                      {mapping.components.length > 0 && (
                        <div className="mt-0.5 text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.components')}: {mapping.components.join(', ')}
                        </div>
                      )}
                      {mapping.patterns.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.patterns')}: {mapping.patterns.join(', ')}
                        </div>
                      )}
                      {mapping.decisions.length > 0 && (
                        <div className="text-2xs text-teal-600 dark:text-teal-400">
                          {t('workflow.designDoc.decisions')}: {mapping.decisions.join(', ')}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </Section>
            )}

            {savedPath && (
              <p className="text-2xs text-teal-500 dark:text-teal-400/70">
                {t('workflow.designDoc.savedTo', { path: savedPath })}
              </p>
            )}
          </div>
        </Collapsible>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <span className="text-2xs font-medium text-teal-600 dark:text-teal-400">{title}</span>
      <div className="mt-0.5">{children}</div>
    </div>
  );
}

function StatPill({ label, value }: { label: string; value: number }) {
  return (
    <div className="text-center">
      <span className="text-2xs text-gray-500 dark:text-gray-400 block">{label}</span>
      <span className={clsx('text-xs font-medium text-teal-600 dark:text-teal-400')}>{value}</span>
    </div>
  );
}
