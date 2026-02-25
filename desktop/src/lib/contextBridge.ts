/**
 * Context Bridge
 *
 * Utilities for sharing conversation context between Chat and Task modes.
 * Handles extracting conversation history from the execution store and
 * synthesizing Task outputs back into the conversation history.
 */

import { useExecutionStore, type StandaloneTurn } from '../store/execution';
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

/**
 * Synthesize a planning-phase turn into the conversation history.
 *
 * Called after strategy analysis + PRD generation completes.
 * Creates a StandaloneTurn summarizing the Task planning output
 * and appends it to the execution store's standaloneTurns.
 */
export function synthesizePlanningTurn(description: string, analysis: StrategyAnalysis | null, prd: TaskPrd): void {
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

  appendSyntheticTurn(`[Task Mode] ${description}`, lines.join('\n'), {
    source: 'task-synthesized',
    type: 'planning',
  });
}

/**
 * Synthesize an execution-completion turn into the conversation history.
 *
 * Called when Task execution completes (success or failure).
 */
export function synthesizeExecutionTurn(completedCount: number, totalCount: number, success: boolean): void {
  const outcome = success ? 'completed successfully' : 'completed with failures';
  appendSyntheticTurn(
    '[Task Execution] Execute approved PRD',
    `Execution ${outcome}: ${completedCount}/${totalCount} stories completed.`,
    {
      source: 'task-synthesized',
      type: 'execution',
    },
  );
}

/**
 * Set pending task context for Claude Code backend injection.
 *
 * When the user switches back to Chat after a Task workflow,
 * this context will be prepended to the next sendFollowUp prompt.
 */
export function setPendingTaskContext(context: string): void {
  useExecutionStore.getState().setPendingTaskContext(context);
}

// ============================================================================
// Internal
// ============================================================================

function appendSyntheticTurn(
  userMessage: string,
  assistantMessage: string,
  metadata?: { source?: string; type?: string },
): void {
  const turn: StandaloneTurn = {
    user: userMessage,
    assistant: assistantMessage,
    createdAt: Date.now(),
    metadata,
  };
  useExecutionStore.getState().appendStandaloneTurn(turn);

  // For Claude Code backend, also set pending context for next sendFollowUp
  const execState = useExecutionStore.getState();
  if (execState.isChatSession) {
    const contextSummary = `[Context from Task Mode]\nUser: ${userMessage}\nResult: ${assistantMessage}`;
    useExecutionStore.getState().setPendingTaskContext(contextSummary);
  }
}
