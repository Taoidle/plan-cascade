import type { StreamLine } from '../../store/execution';
import { isUserTurnBoundary, normalizeTurnBoundaries } from '../../lib/conversationUtils';

export interface TurnViewModel {
  turnIndex: number;
  userLineIndex: number;
  assistantStartIndex: number;
  assistantEndIndex: number;
  userLineId: number;
}

/**
 * Build turn view models from a flat line list.
 *
 * Uses one linear pass over normalized lines and stores index ranges instead of
 * cloning assistant-line arrays for each turn.
 */
export function buildTurnViewModels(lines: StreamLine[]): TurnViewModel[] {
  const normalized = normalizeTurnBoundaries(lines);
  return buildTurnViewModelsFromNormalized(normalized);
}

export function buildTurnViewModelsFromNormalized(normalizedLines: StreamLine[]): TurnViewModel[] {
  if (normalizedLines.length === 0) return [];

  const turns: TurnViewModel[] = [];
  let currentTurn: TurnViewModel | null = null;
  let turnIndex = 0;

  for (let i = 0; i < normalizedLines.length; i++) {
    if (!isUserTurnBoundary(normalizedLines[i])) {
      if (currentTurn) {
        currentTurn.assistantEndIndex = i;
      }
      continue;
    }

    if (currentTurn) {
      turns.push(currentTurn);
    }

    const userLineId = normalizedLines[i].id;
    currentTurn = {
      turnIndex,
      userLineIndex: i,
      assistantStartIndex: i + 1,
      assistantEndIndex: i,
      userLineId,
    };
    turnIndex += 1;
  }

  if (currentTurn) {
    turns.push(currentTurn);
  }

  if (turns.length === 0 && normalizedLines.some((line) => !isUserTurnBoundary(line))) {
    turns.push({
      turnIndex: 0,
      userLineIndex: -1,
      assistantStartIndex: 0,
      assistantEndIndex: normalizedLines.length - 1,
      userLineId: -1,
    });
  }

  return turns;
}
