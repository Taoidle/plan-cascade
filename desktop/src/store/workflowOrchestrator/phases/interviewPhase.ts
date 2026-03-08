import i18n from '../../../i18n';
import { resolvePersonaDisplayName } from '../../../lib/personaI18n';
import { failResult, okResult, type ActionResult } from '../../../types/actionResult';
import type { InterviewQuestionCardData } from '../../../types/workflowCard';
import { useSettingsStore } from '../../settings';
import { useSpecInterviewStore, type InterviewQuestion, type InterviewSession } from '../../specInterview';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface InterviewPhaseDeps {
  mapInterviewQuestion: (question: InterviewQuestion, index: number, total: number) => InterviewQuestionCardData;
  runRequirementPhase: (runtime: WorkflowPhaseRuntime) => Promise<ActionResult>;
  runPrdPhase: (runtime: WorkflowPhaseRuntime) => Promise<ActionResult>;
}

export async function runInterviewPhase(
  runtime: WorkflowPhaseRuntime,
  config: { flowLevel: 'quick' | 'standard' | 'full' },
  deps: InterviewPhaseDeps,
): Promise<ActionResult> {
  const { set, get, runToken, isRunActive } = runtime;
  if (!isRunActive(get, runToken)) {
    return failResult('stale_run_token', 'Configuration request was superseded');
  }

  set({ phase: 'interviewing' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../../../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) {
    return failResult('stale_run_token', 'Configuration request was superseded');
  }
  const interviewResolved = resolvePhaseAgent('plan_interview');

  injectCard('persona_indicator', {
    role: 'BusinessAnalyst',
    displayName: resolvePersonaDisplayName(i18n.t.bind(i18n), 'BusinessAnalyst'),
    phase: 'interviewing',
    model: formatModelDisplay(interviewResolved),
  });
  injectInfo(i18n.t('workflow.orchestrator.startingInterview', { ns: 'simpleMode' }), 'info');

  const settings = useSettingsStore.getState();
  const workspacePath = settings.workspacePath;
  const { explorationResult, taskDescription, sessionId } = get() as {
    explorationResult: unknown;
    taskDescription: string;
    sessionId: string | null;
  };
  const interviewConfig = {
    description: taskDescription,
    flow_level: config.flowLevel,
    max_questions: config.flowLevel === 'quick' ? 10 : config.flowLevel === 'full' ? 25 : 18,
    first_principles: false,
    project_path: workspacePath,
    exploration_context: explorationResult ? JSON.stringify(explorationResult) : null,
    task_session_id: sessionId,
    locale: i18n.language,
  };

  if (interviewResolved.provider) {
    useSpecInterviewStore.getState().setProviderSettings({
      provider: interviewResolved.provider,
      model: interviewResolved.model || undefined,
      baseUrl: interviewResolved.baseUrl || undefined,
    });
  }

  const maxRetries = 5;
  const baseDelay = 500;
  let session: InterviewSession | null = null;
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    session = await useSpecInterviewStore.getState().startInterview(interviewConfig);
    if (!isRunActive(get, runToken)) {
      return failResult('stale_run_token', 'Configuration request was superseded');
    }
    if (session) break;

    const interviewError = useSpecInterviewStore.getState().error || '';
    if (interviewError.includes('not initialized') && attempt < maxRetries - 1) {
      await new Promise((r) => setTimeout(r, baseDelay * Math.pow(2, attempt)));
      if (!isRunActive(get, runToken)) {
        return failResult('stale_run_token', 'Configuration request was superseded');
      }
      useSpecInterviewStore.getState().clearError();
      continue;
    }
    break;
  }

  if (!session) {
    const interviewError = useSpecInterviewStore.getState().error;
    set({ phase: 'failed', error: interviewError || 'Failed to start interview' });
    injectError(
      i18n.t('workflow.orchestrator.interviewFailed', { ns: 'simpleMode' }),
      interviewError || i18n.t('workflow.orchestrator.interviewStartFailed', { ns: 'simpleMode' }),
    );
    return failResult('interview_start_failed', interviewError || 'Failed to start interview');
  }

  if (!isRunActive(get, runToken)) {
    return failResult('stale_run_token', 'Configuration request was superseded');
  }
  set({ interviewId: session.id });

  let interviewSession = session;
  if (!interviewSession.current_question && interviewSession.status !== 'finalized') {
    const recovered = await useSpecInterviewStore.getState().fetchState(interviewSession.id);
    if (!isRunActive(get, runToken)) {
      return failResult('stale_run_token', 'Configuration request was superseded');
    }
    if (recovered) interviewSession = recovered;
  }

  if (interviewSession.current_question) {
    const questionData = deps.mapInterviewQuestion(
      interviewSession.current_question,
      interviewSession.question_cursor + 1,
      interviewSession.max_questions,
    );
    set({ pendingInterviewQuestion: questionData });
    injectCard('interview_question', questionData, true);
    return okResult();
  }

  injectInfo(
    i18n.t('workflow.orchestrator.interviewQuestionUnavailable', {
      ns: 'simpleMode',
      defaultValue: 'Interview question unavailable, continuing with requirement analysis.',
    }),
    'warning',
  );
  const requirementResult = await deps.runRequirementPhase(runtime);
  if (!isRunActive(get, runToken)) {
    return failResult('stale_run_token', 'Configuration request was superseded');
  }
  if (!requirementResult.ok) {
    return requirementResult;
  }

  return deps.runPrdPhase(runtime);
}
