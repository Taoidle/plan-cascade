/**
 * Context Bridge
 *
 * Utilities for sharing conversation context between Chat and Task modes.
 * Handles extracting conversation history from the execution store and
 * synthesizing Task outputs back into the conversation history.
 */

import { useExecutionStore, type StandaloneTurn } from '../store/execution';
import { useWorkflowKernelStore } from '../store/workflowKernel';
import { deriveConversationTurns } from './conversationUtils';
import type { CrossModeConversationTurn } from '../types/crossModeContext';
import type { StrategyAnalysis, TaskPrd } from '../store/taskMode';

/**
 * Build conversation history from the current execution state.
 *
 * For Claude Code backend (isChatSession): extracts turns from streamingOutput
 * For Standalone backend: directly uses standaloneTurns
 *
 * Returns CrossModeConversationTurn[] for IPC serialization to Rust.
 */
export function buildConversationHistory(): CrossModeConversationTurn[] {
  const kernelState = useWorkflowKernelStore.getState();
  const activeRootSessionId = kernelState.activeRootSessionId ?? kernelState.sessionId;
  if (activeRootSessionId && kernelState.activeMode === 'chat') {
    const transcriptLines = kernelState.getCachedModeTranscript(activeRootSessionId, 'chat').lines as Parameters<
      typeof deriveConversationTurns
    >[0];
    if (transcriptLines.length > 0) {
      return deriveConversationTurns(transcriptLines)
        .filter((t) => t.assistantText.trim().length > 0)
        .map((t) => ({ user: t.userContent, assistant: t.assistantText }));
    }
  }

  const execState = useExecutionStore.getState();

  if (execState.isChatSession) {
    // Claude Code backend: derive from streamingOutput
    return deriveConversationTurns(execState.streamingOutput)
      .filter((t) => t.assistantText.trim().length > 0)
      .map((t) => ({ user: t.userContent, assistant: t.assistantText }));
  }

  // Standalone backend: directly use standaloneTurns
  return execState.standaloneTurns.map((t) => ({
    user: t.user,
    assistant: t.assistant,
  }));
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
 * Creates a StandaloneTurn summarizing the Task planning output
 * and appends it to the execution store's standaloneTurns.
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
  metadata?: { source?: string; type?: string },
): CrossModeConversationTurn {
  const turn: StandaloneTurn = {
    user: userMessage,
    assistant: assistantMessage,
    createdAt: Date.now(),
    metadata,
  };
  useExecutionStore.getState().appendStandaloneTurn(turn);

  return {
    user: userMessage,
    assistant: assistantMessage,
  };
}
