/**
 * WorkflowCardRenderer
 *
 * Dispatches card payloads to the appropriate card component based on cardType.
 */

import type { CardPayload } from '../../../types/workflowCard';
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
  FileChangeCardData,
  TurnChangeSummaryCardData,
  RequirementAnalysisCardData,
  ArchitectureReviewCardData,
  PersonaIndicatorData,
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
      return <PrdCard data={payload.data as PrdCardData} interactive={payload.interactive} />;
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
      return <ArchitectureReviewCard data={payload.data as ArchitectureReviewCardData} interactive={payload.interactive} />;
    case 'persona_indicator':
      return <PersonaIndicatorCard data={payload.data as PersonaIndicatorData} />;
    default:
      return null;
  }
}

function WorkflowInfoCard({ data }: { data: WorkflowInfoData }) {
  const colors = {
    info: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300',
    success: 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800 text-green-700 dark:text-green-300',
    warning: 'bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-300',
  };

  return (
    <div className={`text-xs px-3 py-2 rounded-lg border ${colors[data.level]}`}>
      {data.message}
    </div>
  );
}

function WorkflowErrorCard({ data }: { data: WorkflowErrorData }) {
  return (
    <div className="px-3 py-2 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-900/20">
      <p className="text-sm font-medium text-red-700 dark:text-red-300">{data.title}</p>
      <p className="text-xs text-red-600 dark:text-red-400 mt-1">{data.description}</p>
      {data.suggestedFix && (
        <p className="text-xs text-red-500 dark:text-red-400 mt-1 italic">
          Suggestion: {data.suggestedFix}
        </p>
      )}
    </div>
  );
}
