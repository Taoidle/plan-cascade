import type { CardPayload, WorkflowErrorData, WorkflowInfoData } from '../../types/workflowCard';
import { routeModeCard, routeModeStreamLine } from '../modeTranscriptRouting';

let cardCounter = 0;

function nextCardId(): string {
  cardCounter += 1;
  return `card-${cardCounter}-${Date.now()}`;
}

export function injectWorkflowCard<T extends CardPayload['cardType']>(
  cardType: T,
  data: CardPayload['data'],
  interactive = false,
): void {
  const payload: CardPayload = {
    cardType,
    cardId: nextCardId(),
    data,
    interactive,
  };
  void routeModeCard('task', payload);
}

export function injectWorkflowInfo(message: string, level: WorkflowInfoData['level'] = 'info'): void {
  injectWorkflowCard('workflow_info', { message, level } as WorkflowInfoData);
}

export function injectWorkflowError(title: string, description: string, suggestedFix: string | null = null): void {
  injectWorkflowCard('workflow_error', { title, description, suggestedFix } as WorkflowErrorData);
}

export function appendWorkflowUserMessage(message: string): void {
  const trimmed = message.trim();
  if (!trimmed) return;
  void routeModeStreamLine('task', trimmed, 'info', {
    turnBoundary: 'user',
    turnId: Date.now(),
  });
}
