/**
 * Context Bridge
 *
 * Utilities for sharing conversation context between Chat and Task modes.
 * Uses workflow kernel transcript/context as the authoritative conversation source.
 */

import { useWorkflowKernelStore } from '../store/workflowKernel';
import { deriveConversationTurns } from './conversationUtils';
import type { CrossModeConversationTurn } from '../types/crossModeContext';
import type { StrategyAnalysis, TaskPrd } from '../store/taskMode';

/**
 * Build conversation history from the active kernel chat transcript.
 */
export function buildConversationHistory(): CrossModeConversationTurn[] {
  const kernelState = useWorkflowKernelStore.getState();
  const activeRootSessionId = kernelState.activeRootSessionId ?? kernelState.sessionId;
  if (!activeRootSessionId) {
    return [];
  }
  const transcriptLines = kernelState.getCachedModeTranscript(activeRootSessionId, 'chat').lines as Parameters<
    typeof deriveConversationTurns
  >[0];
  return deriveConversationTurns(transcriptLines)
    .filter((t) => t.assistantText.trim().length > 0)
    .map((t) => ({ user: t.userContent, assistant: t.assistantText }));
}

export function buildRootConversationHistory(): CrossModeConversationTurn[] {
  const kernelConversation = useWorkflowKernelStore.getState().session?.handoffContext.conversationContext ?? [];
  if (kernelConversation.length > 0) {
    return kernelConversation.map((turn) => ({
      user: turn.user,
      assistant: turn.assistant,
    }));
  }
  return buildConversationHistory();
}

export function buildRootConversationContextString(): string | undefined {
  const conversationHistory = buildRootConversationHistory();
  if (conversationHistory.length === 0) {
    return undefined;
  }
  return conversationHistory.map((turn) => `user: ${turn.user}\nassistant: ${turn.assistant}`).join('\n');
}

/**
 * Synthesize a planning-phase turn into the conversation history.
 *
 * Called after strategy analysis + PRD generation completes.
 * Returns a synthesized conversation turn for kernel handoff.
 */
export function synthesizePlanningTurn(
  description: string,
  analysis: StrategyAnalysis | null,
  prd: TaskPrd,
): CrossModeConversationTurn {
  const lines: string[] = [];

  if (analysis) {
    lines.push(
      `Strategy: ${analysis.strategyDecision?.strategy ?? analysis.recommendedMode} (confidence: ${analysis.strategyDecision?.confidence ?? analysis.confidence})`,
    );
    lines.push(`Risk: ${analysis.riskLevel}, Estimated stories: ${analysis.estimatedStories}`);
  }

  lines.push(`PRD: ${prd.title}`);
  lines.push(`Stories (${prd.stories.length}):`);
  for (const s of prd.stories) {
    lines.push(`- ${s.id}: ${s.title} [${s.priority}]`);
  }

  return appendSyntheticTurn(`[Task Mode] ${description}`, lines.join('\n'), {
    source: 'task-synthesized',
    type: 'planning',
  });
}

/**
 * Synthesize an execution-completion turn into the conversation history.
 *
 * Called when Task execution completes (success or failure).
 */
export function synthesizeExecutionTurn(
  completedCount: number,
  totalCount: number,
  success: boolean,
): CrossModeConversationTurn {
  const outcome = success ? 'completed successfully' : 'completed with failures';
  return appendSyntheticTurn(
    '[Task Execution] Execute approved PRD',
    `Execution ${outcome}: ${completedCount}/${totalCount} stories completed.`,
    {
      source: 'task-synthesized',
      type: 'execution',
    },
  );
}

// ============================================================================
// Internal
// ============================================================================

function appendSyntheticTurn(
  userMessage: string,
  assistantMessage: string,
  _metadata?: { source?: string; type?: string },
): CrossModeConversationTurn {
  return {
    user: userMessage,
    assistant: assistantMessage,
  };
}
