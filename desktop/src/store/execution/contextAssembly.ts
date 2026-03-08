import { deriveConversationTurns, type StreamLine } from '../../lib/conversationUtils';
import type { ContextSourceConfig } from '../../types/contextSources';
import type { HandoffContextBundle } from '../../types/workflowKernel';

export interface ConversationTurnInput {
  role: 'user' | 'assistant';
  content: string;
}

export interface StandaloneConversationTurn {
  user: string;
  assistant: string;
  createdAt: number;
}

export interface HandoffConfig {
  threshold: number;
  keepRecentTurns: number;
  maxPoints: number;
}

export interface HandoffManualBlock {
  id: string;
  title: string;
  content: string;
  priority: number;
}

export const DEFAULT_HANDOFF_CONFIG: HandoffConfig = {
  threshold: 120,
  keepRecentTurns: 60,
  maxPoints: 12,
};

export function trimStandaloneTurns(
  turns: StandaloneConversationTurn[],
  limit: number,
  unlimitedValue: number,
): StandaloneConversationTurn[] {
  if (limit === unlimitedValue) return turns;
  return turns.slice(-limit);
}

export function buildStandaloneConversationMessage(
  turns: StandaloneConversationTurn[],
  userInput: string,
  contextTurnsLimit: number,
  unlimitedValue: number,
): string {
  const recent = trimStandaloneTurns(turns, contextTurnsLimit, unlimitedValue);
  const history = recent.map((turn) => `User: ${turn.user}\nAssistant: ${turn.assistant}`).join('\n\n');

  return [
    'Continue the same conversation. Keep consistency with previous context.',
    '',
    'Conversation history:',
    history,
    '',
    `User: ${userInput}`,
  ].join('\n');
}

export function buildStandaloneContextConversationTurns(
  turns: StandaloneConversationTurn[],
  contextTurnsLimit: number,
  unlimitedValue: number,
): ConversationTurnInput[] {
  const history = trimStandaloneTurns(turns, contextTurnsLimit, unlimitedValue);
  const conversation: ConversationTurnInput[] = [];
  for (const turn of history) {
    if (turn.user.trim().length > 0) {
      conversation.push({ role: 'user', content: turn.user });
    }
    if (turn.assistant.trim().length > 0) {
      conversation.push({ role: 'assistant', content: turn.assistant });
    }
  }
  return conversation;
}

export function buildHandoffManualBlock(
  conversation: ConversationTurnInput[],
  config: HandoffConfig = DEFAULT_HANDOFF_CONFIG,
): HandoffManualBlock | null {
  if (conversation.length < config.threshold) return null;

  const olderTurns = conversation.slice(0, Math.max(0, conversation.length - config.keepRecentTurns));
  if (olderTurns.length === 0) return null;

  const stride = Math.max(1, Math.floor(olderTurns.length / config.maxPoints));
  const points: string[] = [];
  for (let idx = 0; idx < olderTurns.length && points.length < config.maxPoints; idx += stride) {
    const turn = olderTurns[idx];
    const role = turn.role === 'user' ? 'user' : 'assistant';
    const snippet = turn.content.replace(/\s+/g, ' ').trim().slice(0, 180);
    if (snippet.length > 0) {
      points.push(`- [${role}] ${snippet}`);
    }
  }
  if (points.length === 0) return null;

  return {
    id: 'handoff:capsule',
    title: 'Long Thread Handoff Capsule',
    content: [
      'Condensed handoff capsule for earlier conversation turns.',
      'Use this as continuity anchor before the recent detailed turns.',
      ...points,
    ].join('\n'),
    priority: 110,
  };
}

export function buildKernelEntryHandoffManualBlock(
  handoff: HandoffContextBundle | null | undefined,
): HandoffManualBlock | null {
  if (!handoff) return null;

  const sections: string[] = [];
  const conversation = handoff.conversationContext
    .map((turn) => `user: ${turn.user.trim()}\nassistant: ${turn.assistant.trim()}`)
    .join('\n\n');
  if (conversation.trim().length > 0) {
    sections.push(`[conversation]\n${conversation}`);
  }

  const summaryItems = handoff.summaryItems ?? [];
  if (summaryItems.length > 0) {
    const summaries = summaryItems
      .map((item) => `## [${item.sourceMode}:${item.kind}] ${item.title}\n${item.body}`)
      .join('\n\n');
    if (summaries.trim().length > 0) {
      sections.push(`[summary-items]\n${summaries}`);
    }
  }

  if ((handoff.artifactRefs ?? []).length > 0) {
    sections.push(`[artifact-refs]\n${handoff.artifactRefs.join('\n')}`);
  }
  if ((handoff.contextSources ?? []).length > 0) {
    sections.push(`[context-sources]\n${handoff.contextSources.join('\n')}`);
  }
  if (handoff.metadata && Object.keys(handoff.metadata).length > 0) {
    sections.push(`[handoff-metadata]\n${JSON.stringify(handoff.metadata, null, 2)}`);
  }

  const content = sections.join('\n\n').trim();
  if (!content) return null;
  return {
    id: 'handoff:kernel-entry',
    title: 'Kernel Entry Handoff',
    content,
    priority: 120,
  };
}

export function inferInjectedSourceKinds(params: {
  hasHistory: boolean;
  contextSources: ContextSourceConfig | null;
}): string[] {
  const kinds: string[] = [];
  if (params.hasHistory) kinds.push('history');
  const sources = params.contextSources;
  if (!sources) return kinds;
  if (sources.memory?.enabled) kinds.push('memory');
  if (sources.knowledge?.enabled) kinds.push('knowledge');
  if (sources.skills?.enabled) kinds.push('skills');
  return kinds;
}

export function buildChatConversationTurns(lines: StreamLine[]): ConversationTurnInput[] {
  const turns = deriveConversationTurns(lines);
  const conversation: ConversationTurnInput[] = [];
  for (const turn of turns) {
    if (turn.userContent.trim().length > 0) {
      conversation.push({ role: 'user', content: turn.userContent });
    }
    if (turn.assistantText.trim().length > 0) {
      conversation.push({ role: 'assistant', content: turn.assistantText });
    }
  }
  return conversation;
}
