/**
 * WorkflowCardRenderer
 *
 * Dispatches card payloads to the appropriate card component based on cardType.
 */

import type { CardPayload } from '../../../types/workflowCard';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useDebugOrchestratorStore } from '../../../store/debugOrchestrator';
import { StrategyCard } from './StrategyCard';
import { ConfigCard } from './ConfigCard';
import { InterviewQuestionCard } from './InterviewQuestionCard';
import { InterviewAnswerCard } from './InterviewAnswerCard';
import { PrdCard } from './PrdCard';
import { DesignDocCard } from './DesignDocCard';
import { ExecutionUpdateCard } from './ExecutionUpdateCard';
import { GateResultCard } from './GateResultCard';
import { CompletionReportCard } from './CompletionReportCard';
import { ExplorationCard } from './ExplorationCard';
import { FileChangeCard } from './FileChangeCard';
import { TurnChangeSummaryCard } from './TurnChangeSummaryCard';
import { RequirementAnalysisCard } from './RequirementAnalysisCard';
import { ArchitectureReviewCard } from './ArchitectureReviewCard';
import { PersonaIndicatorCard } from './PersonaIndicatorCard';
import { ModeHandoffCard } from './ModeHandoffCard';
import { PlanAnalysisCard } from './PlanAnalysisCard';
import { PlanCard } from './PlanCard';
import { PlanStepUpdateCard } from './PlanStepUpdateCard';
import { PlanStepOutputCard } from './PlanStepOutputCard';
import { PlanCompletionCard } from './PlanCompletionCard';
import { PlanClarifyQuestionCard } from './PlanClarifyQuestionCard';
import { PlanClarifyAnswerCard } from './PlanClarifyAnswerCard';
import { PlanClarificationResolutionCard } from './PlanClarificationResolutionCard';
import type {
  PlanAnalysisCardData,
  PlanCardData,
  PlanStepUpdateCardData,
  PlanStepOutputCardData,
  PlanCompletionCardData,
  PlanClarifyQuestionCardData,
  PlanClarifyAnswerCardData,
  PlanClarificationResolutionCardData,
  PlanPersonaIndicatorData,
} from '../../../types/planModeCard';
import type {
  StrategyCardData,
  ConfigCardData,
  InterviewQuestionCardData,
  InterviewAnswerCardData,
  PrdCardData,
  DesignDocCardData,
  ExecutionUpdateCardData,
  GateResultCardData,
  CompletionReportCardData,
  ExplorationCardData,
  WorkflowInfoData,
  WorkflowErrorData,
  DebugIntakeCardData,
  SignalSummaryCardData,
  ReproductionStatusCardData,
  BrowserRuntimeCardData,
  EvidenceCardData,
  ConsoleErrorCardData,
  NetworkTraceCardData,
  PerformanceTraceCardData,
  SourceMappingCardData,
  HypothesisCardData,
  RootCauseCardData,
  FixCandidateCardData,
  PatchReviewCardData,
  VerificationCardData,
  IncidentSummaryCardData,
  FileChangeCardData,
  TurnChangeSummaryCardData,
  RequirementAnalysisCardData,
  ArchitectureReviewCardData,
  PersonaIndicatorData,
  ModeHandoffCardData,
} from '../../../types/workflowCard';

export function WorkflowCardRenderer({ payload }: { payload: CardPayload }) {
  switch (payload.cardType) {
    case 'strategy_card':
      return <StrategyCard data={payload.data as StrategyCardData} />;
    case 'config_card':
      return <ConfigCard data={payload.data as ConfigCardData} interactive={payload.interactive} />;
    case 'interview_question':
      return <InterviewQuestionCard data={payload.data as InterviewQuestionCardData} />;
    case 'interview_answer':
      return <InterviewAnswerCard data={payload.data as InterviewAnswerCardData} />;
    case 'prd_card':
      return <PrdCard data={payload.data as PrdCardData} interactive={payload.interactive} cardId={payload.cardId} />;
    case 'design_doc_card':
      return <DesignDocCard data={payload.data as DesignDocCardData} />;
    case 'execution_update':
      return <ExecutionUpdateCard data={payload.data as ExecutionUpdateCardData} />;
    case 'gate_result':
      return <GateResultCard data={payload.data as GateResultCardData} />;
    case 'completion_report':
      return <CompletionReportCard data={payload.data as CompletionReportCardData} />;
    case 'exploration_card':
      return <ExplorationCard data={payload.data as ExplorationCardData} />;
    case 'workflow_info':
      return <WorkflowInfoCard data={payload.data as WorkflowInfoData} />;
    case 'workflow_error':
      return <WorkflowErrorCard data={payload.data as WorkflowErrorData} />;
    case 'file_change':
      return <FileChangeCard data={payload.data as FileChangeCardData} />;
    case 'turn_change_summary':
      return <TurnChangeSummaryCard data={payload.data as TurnChangeSummaryCardData} />;
    case 'requirement_analysis_card':
      return <RequirementAnalysisCard data={payload.data as RequirementAnalysisCardData} />;
    case 'architecture_review_card':
      return (
        <ArchitectureReviewCard
          data={payload.data as ArchitectureReviewCardData}
          interactive={payload.interactive}
          cardId={payload.cardId}
        />
      );
    case 'persona_indicator':
      return <PersonaIndicatorCard data={payload.data as PersonaIndicatorData} />;
    case 'mode_handoff_card':
      return <ModeHandoffCard data={payload.data as ModeHandoffCardData} />;
    case 'debug_intake_card':
      return <DebugIntakeCard data={payload.data as DebugIntakeCardData} />;
    case 'signal_summary_card':
      return <SignalSummaryCard data={payload.data as SignalSummaryCardData} />;
    case 'reproduction_status_card':
      return <ReproductionStatusCard data={payload.data as ReproductionStatusCardData} />;
    case 'browser_runtime_card':
      return <BrowserRuntimeCard data={payload.data as BrowserRuntimeCardData} />;
    case 'evidence_card':
      return <EvidenceCard data={payload.data as EvidenceCardData} />;
    case 'console_error_card':
      return <ConsoleErrorCard data={payload.data as ConsoleErrorCardData} />;
    case 'network_trace_card':
      return <NetworkTraceCard data={payload.data as NetworkTraceCardData} />;
    case 'performance_trace_card':
      return <PerformanceTraceCard data={payload.data as PerformanceTraceCardData} />;
    case 'source_mapping_card':
      return <SourceMappingCard data={payload.data as SourceMappingCardData} />;
    case 'hypothesis_card':
      return <HypothesisCard data={payload.data as HypothesisCardData} />;
    case 'root_cause_card':
      return <RootCauseCard data={payload.data as RootCauseCardData} />;
    case 'fix_candidate_card':
      return <FixCandidateCard data={payload.data as FixCandidateCardData} />;
    case 'patch_review_card':
      return <PatchReviewCard data={payload.data as PatchReviewCardData} interactive={payload.interactive} />;
    case 'verification_card':
      return <VerificationCard data={payload.data as VerificationCardData} />;
    case 'incident_summary_card':
      return <IncidentSummaryCard data={payload.data as IncidentSummaryCardData} />;
    // Plan Mode cards
    case 'plan_analysis_card':
      return <PlanAnalysisCard data={payload.data as PlanAnalysisCardData} />;
    case 'plan_card':
      return <PlanCard data={payload.data as PlanCardData} interactive={payload.interactive} />;
    case 'plan_step_update':
      return <PlanStepUpdateCard data={payload.data as PlanStepUpdateCardData} />;
    case 'plan_step_output':
      return <PlanStepOutputCard data={payload.data as PlanStepOutputCardData} />;
    case 'plan_completion_card':
      return <PlanCompletionCard data={payload.data as PlanCompletionCardData} />;
    case 'plan_clarify_question':
      return <PlanClarifyQuestionCard data={payload.data as PlanClarifyQuestionCardData} />;
    case 'plan_clarify_answer':
      return <PlanClarifyAnswerCard data={payload.data as PlanClarifyAnswerCardData} />;
    case 'plan_clarification_resolution':
      return (
        <PlanClarificationResolutionCard
          data={payload.data as PlanClarificationResolutionCardData}
          interactive={payload.interactive}
        />
      );
    case 'plan_persona_indicator':
      return (
        <PersonaIndicatorCard data={payload.data as PlanPersonaIndicatorData as unknown as PersonaIndicatorData} />
      );
    default:
      return (
        <div className="px-3 py-2 rounded-lg border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-900/20">
          <p className="text-xs font-semibold text-amber-800 dark:text-amber-200">Unknown workflow card</p>
          <p className="text-2xs mt-1 text-amber-700 dark:text-amber-300 break-all">
            type: <code>{payload.cardType}</code> | id: <code>{payload.cardId}</code>
          </p>
        </div>
      );
  }
}

function DebugCardFrame({
  title,
  children,
  tone = 'slate',
}: {
  title: string;
  children: ReactNode;
  tone?: 'slate' | 'amber' | 'emerald' | 'rose' | 'sky';
}) {
  const tones = {
    slate: 'border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/30',
    amber: 'border-amber-200 dark:border-amber-700 bg-amber-50 dark:bg-amber-900/20',
    emerald: 'border-emerald-200 dark:border-emerald-700 bg-emerald-50 dark:bg-emerald-900/20',
    rose: 'border-rose-200 dark:border-rose-700 bg-rose-50 dark:bg-rose-900/20',
    sky: 'border-sky-200 dark:border-sky-700 bg-sky-50 dark:bg-sky-900/20',
  };
  return (
    <div className={`rounded-lg border px-3 py-3 space-y-2 ${tones[tone]}`}>
      <p className="text-xs font-semibold uppercase tracking-wide text-slate-600 dark:text-slate-300">{title}</p>
      {children}
    </div>
  );
}

function DebugList({ items }: { items: string[] }) {
  if (items.length === 0) return null;
  return (
    <ul className="space-y-1">
      {items.map((item, index) => (
        <li key={`${item}-${index}`} className="text-xs text-slate-700 dark:text-slate-200">
          {item}
        </li>
      ))}
    </ul>
  );
}

function DebugIntakeCard({ data }: { data: DebugIntakeCardData }) {
  return (
    <DebugCardFrame title={data.title || 'Debug intake'} tone="slate">
      <p className="text-sm font-medium text-slate-800 dark:text-slate-100">{data.symptomSummary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">
        {data.environment} · {data.severity}
      </p>
      <DebugList items={data.reproSteps} />
    </DebugCardFrame>
  );
}

function SignalSummaryCard({ data }: { data: SignalSummaryCardData }) {
  return (
    <DebugCardFrame title="Signal summary" tone="amber">
      <p className="text-sm text-slate-800 dark:text-slate-100">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">
        {data.environment} · {data.severity} · evidence {data.evidenceCount}
      </p>
      <DebugList items={data.highlights} />
    </DebugCardFrame>
  );
}

function ReproductionStatusCard({ data }: { data: ReproductionStatusCardData }) {
  return (
    <DebugCardFrame title="Reproduction" tone="slate">
      <p className="text-sm text-slate-800 dark:text-slate-100">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">{data.status}</p>
      <DebugList items={data.reproductionSteps} />
    </DebugCardFrame>
  );
}

function BrowserRuntimeCard({ data }: { data: BrowserRuntimeCardData }) {
  const { t } = useTranslation('debugMode');
  return (
    <DebugCardFrame title={t('cards.browserRuntime.title', { defaultValue: 'Browser bridge' })} tone="sky">
      <p className="text-sm text-slate-800 dark:text-slate-100">
        {t(`bridgeKinds.${data.bridgeKind}`, { defaultValue: data.bridgeKind })}
      </p>
      {data.serverName && <p className="text-xs text-slate-600 dark:text-slate-300">{data.serverName}</p>}
      {data.targetUrl && <p className="text-xs text-slate-600 dark:text-slate-300">{data.targetUrl}</p>}
      <DebugList items={data.capabilities} />
      <DebugList items={data.notes.map((note) => t(note, { defaultValue: note }))} />
      {data.recommendedCatalogItemId && (
        <p className="text-2xs text-slate-500 dark:text-slate-400">
          {t('cards.browserRuntime.installHint', {
            defaultValue: 'Recommended MCP catalog item: {{itemId}}',
            itemId: data.recommendedCatalogItemId,
          })}
        </p>
      )}
    </DebugCardFrame>
  );
}

function EvidenceCard({ data }: { data: EvidenceCardData }) {
  return (
    <DebugCardFrame title={data.title} tone="slate">
      <p className="text-sm text-slate-800 dark:text-slate-100">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">
        {data.source} · {data.collectedAt}
      </p>
    </DebugCardFrame>
  );
}

function ConsoleErrorCard({ data }: { data: ConsoleErrorCardData }) {
  const { t } = useTranslation('debugMode');
  return (
    <DebugCardFrame title={t('cards.console.title', { defaultValue: 'Console diagnostics' })} tone="amber">
      {data.currentUrl && <p className="text-xs text-slate-600 dark:text-slate-300">{data.currentUrl}</p>}
      <p className="text-xs text-slate-600 dark:text-slate-300">{data.collectedAt}</p>
      <div className="space-y-1">
        {data.entries.map((entry, index) => (
          <div
            key={`${entry.timestamp ?? 'ts'}-${index}`}
            className="rounded border border-amber-200 dark:border-amber-700 px-2 py-2"
          >
            <p className="text-xs font-medium text-slate-700 dark:text-slate-200">{entry.level}</p>
            <p className="text-xs text-slate-800 dark:text-slate-100 whitespace-pre-wrap break-words">
              {entry.message}
            </p>
          </div>
        ))}
      </div>
    </DebugCardFrame>
  );
}

function NetworkTraceCard({ data }: { data: NetworkTraceCardData }) {
  const { t } = useTranslation('debugMode');
  return (
    <DebugCardFrame title={t('cards.network.title', { defaultValue: 'Network diagnostics' })} tone="sky">
      {data.currentUrl && <p className="text-xs text-slate-600 dark:text-slate-300">{data.currentUrl}</p>}
      <p className="text-xs text-slate-600 dark:text-slate-300">
        {data.collectedAt} · {data.totalEvents} events · {data.failedEvents} failed
      </p>
      <DebugList items={data.highlights} />
      {data.slowestRequests && data.slowestRequests.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.network.slowest', { defaultValue: 'Slowest requests' })}
          </p>
          <DebugList items={data.slowestRequests} />
        </>
      )}
      {data.harCaptured ? (
        <p className="text-2xs text-slate-500 dark:text-slate-400">
          {t('cards.network.harCaptured', { defaultValue: 'HAR-style summary captured from browser traffic.' })}
        </p>
      ) : null}
    </DebugCardFrame>
  );
}

function SourceMappingCard({ data }: { data: SourceMappingCardData }) {
  const { t } = useTranslation('debugMode');
  return (
    <DebugCardFrame title={t('cards.sourceMapping.title', { defaultValue: 'Source mapping hints' })} tone="slate">
      <p className="text-sm text-slate-800 dark:text-slate-100">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">
        {t(`bridgeKinds.${data.source}`, { defaultValue: data.source })}
      </p>
      <DebugList items={data.candidateFiles} />
      {data.bundleScripts && data.bundleScripts.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.bundleScripts', { defaultValue: 'Bundle scripts' })}
          </p>
          <DebugList items={data.bundleScripts} />
        </>
      )}
      {data.sourceMapUrls && data.sourceMapUrls.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.sourceMaps', { defaultValue: 'Source maps' })}
          </p>
          <DebugList items={data.sourceMapUrls} />
        </>
      )}
      {data.resolvedSources && data.resolvedSources.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.resolvedSources', { defaultValue: 'Resolved sources' })}
          </p>
          <DebugList items={data.resolvedSources} />
        </>
      )}
      {data.stackFrames && data.stackFrames.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.stackFrames', { defaultValue: 'Stack frames' })}
          </p>
          <DebugList items={data.stackFrames} />
        </>
      )}
      {data.matchedSourceMaps && data.matchedSourceMaps.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.matchedMaps', { defaultValue: 'Matched source maps' })}
          </p>
          <DebugList items={data.matchedSourceMaps} />
        </>
      )}
      {data.originalPositionHints && data.originalPositionHints.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.sourceMapping.originalPositions', { defaultValue: 'Original positions' })}
          </p>
          <DebugList items={data.originalPositionHints} />
        </>
      )}
    </DebugCardFrame>
  );
}

function PerformanceTraceCard({ data }: { data: PerformanceTraceCardData }) {
  const { t } = useTranslation('debugMode');
  return (
    <DebugCardFrame title={t('cards.performance.title', { defaultValue: 'Performance diagnostics' })} tone="amber">
      {data.currentUrl && <p className="text-xs text-slate-600 dark:text-slate-300">{data.currentUrl}</p>}
      <p className="text-sm text-slate-800 dark:text-slate-100">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">{data.collectedAt}</p>
      <DebugList items={data.metrics} />
      {data.longTasks && data.longTasks.length > 0 && (
        <>
          <p className="pt-1 text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
            {t('cards.performance.longTasks', { defaultValue: 'Longest tasks' })}
          </p>
          <DebugList items={data.longTasks} />
        </>
      )}
    </DebugCardFrame>
  );
}

function HypothesisCard({ data }: { data: HypothesisCardData }) {
  return (
    <DebugCardFrame title="Hypotheses" tone="amber">
      <div className="space-y-2">
        {data.hypotheses.map((hypothesis) => (
          <div key={hypothesis.id} className="rounded border border-amber-200 dark:border-amber-700 px-2 py-2">
            <p className="text-sm text-slate-800 dark:text-slate-100">{hypothesis.statement}</p>
            <p className="text-xs text-slate-600 dark:text-slate-300">
              {hypothesis.status} · {Math.round(hypothesis.confidence * 100)}%
            </p>
          </div>
        ))}
      </div>
      <DebugList items={data.recommendedNextChecks} />
    </DebugCardFrame>
  );
}

function RootCauseCard({ data }: { data: RootCauseCardData }) {
  return (
    <DebugCardFrame title="Root cause" tone="rose">
      <p className="text-sm font-medium text-slate-900 dark:text-slate-50">{data.conclusion}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">confidence {Math.round(data.confidence * 100)}%</p>
      <DebugList items={data.impactScope} />
      <p className="text-xs text-slate-700 dark:text-slate-200">{data.recommendedDirection}</p>
    </DebugCardFrame>
  );
}

function FixCandidateCard({ data }: { data: FixCandidateCardData }) {
  return (
    <DebugCardFrame title="Fix candidate" tone="emerald">
      <p className="text-sm font-medium text-slate-900 dark:text-slate-50">{data.summary}</p>
      <DebugList items={data.filesOrSystemsTouched} />
      <DebugList items={data.verificationPlan} />
      {data.patchOperations && data.patchOperations.length > 0 ? (
        <div className="space-y-1">
          {data.patchOperations.map((operation) => (
            <div
              key={operation.id}
              className="rounded border border-slate-200/70 bg-slate-50/80 p-2 dark:border-slate-700 dark:bg-slate-900/50"
            >
              <p className="text-xs text-slate-700 dark:text-slate-200">
                [{operation.kind}] {operation.filePath} - {operation.description}
              </p>
              {operation.findText ? (
                <pre className="mt-1 overflow-x-auto rounded bg-white px-2 py-1 text-2xs text-amber-700 dark:bg-slate-950 dark:text-amber-300">
                  find: {operation.findText}
                </pre>
              ) : null}
              {operation.replaceText ? (
                <pre className="mt-1 overflow-x-auto rounded bg-white px-2 py-1 text-2xs text-emerald-700 dark:bg-slate-950 dark:text-emerald-300">
                  replace: {operation.replaceText}
                </pre>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
      {data.patchPreviewRef ? (
        <p className="text-2xs text-slate-500 dark:text-slate-400">
          patch preview: <code>{data.patchPreviewRef}</code>
        </p>
      ) : null}
    </DebugCardFrame>
  );
}

function PatchReviewCard({ data, interactive }: { data: PatchReviewCardData; interactive: boolean }) {
  const { t } = useTranslation('debugMode');
  const approvePatch = useDebugOrchestratorStore((s) => s.approvePatch);
  const rejectPatch = useDebugOrchestratorStore((s) => s.rejectPatch);
  return (
    <DebugCardFrame title={data.title} tone="emerald">
      <p className="text-sm font-medium text-slate-900 dark:text-slate-50">{data.summary}</p>
      <p className="text-xs text-slate-600 dark:text-slate-300">{data.approvalDescription}</p>
      <DebugList items={data.filesOrSystemsTouched} />
      <DebugList items={data.verificationPlan} />
      {data.patchOperations && data.patchOperations.length > 0 ? (
        <div className="space-y-1">
          {data.patchOperations.map((operation) => (
            <div
              key={operation.id}
              className="rounded border border-slate-200/70 bg-slate-50/80 p-2 dark:border-slate-700 dark:bg-slate-900/50"
            >
              <p className="text-xs text-slate-700 dark:text-slate-200">
                [{operation.kind}] {operation.filePath} - {operation.description}
              </p>
              {operation.findText ? (
                <pre className="mt-1 overflow-x-auto rounded bg-white px-2 py-1 text-2xs text-amber-700 dark:bg-slate-950 dark:text-amber-300">
                  find: {operation.findText}
                </pre>
              ) : null}
              {operation.replaceText ? (
                <pre className="mt-1 overflow-x-auto rounded bg-white px-2 py-1 text-2xs text-emerald-700 dark:bg-slate-950 dark:text-emerald-300">
                  replace: {operation.replaceText}
                </pre>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
      {data.patchPreviewRef ? (
        <p className="text-2xs text-slate-500 dark:text-slate-400">
          patch preview: <code>{data.patchPreviewRef}</code>
        </p>
      ) : null}
      {interactive && (
        <div className="flex items-center gap-2 pt-1">
          <button
            onClick={() => void approvePatch()}
            className="rounded bg-emerald-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-emerald-700"
          >
            {t('cards.patchReview.approve', { defaultValue: 'Approve patch' })}
          </button>
          <button
            onClick={() => void rejectPatch()}
            className="rounded border border-slate-300 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-600 dark:text-slate-200 dark:hover:bg-slate-800"
          >
            {t('cards.patchReview.reject', { defaultValue: 'Ask for another fix' })}
          </button>
        </div>
      )}
    </DebugCardFrame>
  );
}

function VerificationCard({ data }: { data: VerificationCardData }) {
  return (
    <DebugCardFrame title="Verification" tone="emerald">
      <p className="text-sm text-slate-900 dark:text-slate-50">{data.summary}</p>
      <div className="space-y-1">
        {data.checks.map((check) => (
          <p key={check.id} className="text-xs text-slate-700 dark:text-slate-200">
            {check.label}: {check.status}
          </p>
        ))}
      </div>
      <DebugList items={data.residualRisks} />
      <DebugList items={data.artifacts} />
    </DebugCardFrame>
  );
}

function IncidentSummaryCard({ data }: { data: IncidentSummaryCardData }) {
  return (
    <DebugCardFrame title={data.title} tone="emerald">
      <p className="text-sm font-medium text-slate-900 dark:text-slate-50">{data.summary}</p>
      {data.rootCauseConclusion && (
        <p className="text-xs text-slate-700 dark:text-slate-200">{data.rootCauseConclusion}</p>
      )}
      {data.verificationSummary && (
        <p className="text-xs text-slate-600 dark:text-slate-300">{data.verificationSummary}</p>
      )}
      <DebugList items={data.residualRisks} />
    </DebugCardFrame>
  );
}

function WorkflowInfoCard({ data }: { data: WorkflowInfoData }) {
  const colors = {
    info: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300',
    success:
      'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800 text-green-700 dark:text-green-300',
    warning:
      'bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-300',
  };

  return <div className={`text-xs px-3 py-2 rounded-lg border ${colors[data.level]}`}>{data.message}</div>;
}

function WorkflowErrorCard({ data }: { data: WorkflowErrorData }) {
  return (
    <div className="px-3 py-2 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-900/20">
      <p className="text-sm font-medium text-red-700 dark:text-red-300">{data.title}</p>
      <p className="text-xs text-red-600 dark:text-red-400 mt-1">{data.description}</p>
      {data.suggestedFix && (
        <p className="text-xs text-red-500 dark:text-red-400 mt-1 italic">Suggestion: {data.suggestedFix}</p>
      )}
    </div>
  );
}
