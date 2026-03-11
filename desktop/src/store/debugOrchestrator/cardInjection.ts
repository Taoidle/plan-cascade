import type { CardPayload } from '../../types/workflowCard';
import { routeModeCard, routeModeStreamLine } from '../modeTranscriptRouting';

let debugCardCounter = 0;

function nextDebugCardId(cardType: string): string {
  debugCardCounter += 1;
  return `${cardType}-${Date.now()}-${debugCardCounter}`;
}

export function injectDebugCard<T extends CardPayload['cardType']>(
  cardType: T,
  data: CardPayload['data'],
  interactive = false,
): void {
  const payload: CardPayload = {
    cardType,
    cardId: nextDebugCardId(cardType),
    data,
    interactive,
  };
  void routeModeCard('debug', payload);
}

export function appendDebugUserMessage(message: string): void {
  const trimmed = message.trim();
  if (!trimmed) return;
  void routeModeStreamLine('debug', trimmed, 'info', {
    turnBoundary: 'user',
    turnId: Date.now(),
  });
}
