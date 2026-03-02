import type { KnowledgeState } from '../knowledge';

export type SetState = (
  partial: Partial<KnowledgeState> | ((state: KnowledgeState) => Partial<KnowledgeState>),
) => void;
export type GetState = () => KnowledgeState;
