/**
 * SimpleMode cross-component navigation events.
 *
 * Chat cards use this to request opening the right panel and focusing AI changes.
 * SimpleMode listens and performs the actual panel/tab transition.
 */

export const SIMPLE_MODE_OPEN_AI_CHANGES_EVENT = 'simple-mode:open-ai-changes';

export interface OpenAIChangesDetail {
  turnIndex?: number;
}

export function requestOpenAIChanges(detail: OpenAIChangesDetail = {}): void {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent<OpenAIChangesDetail>(SIMPLE_MODE_OPEN_AI_CHANGES_EVENT, { detail }));
}

export function listenOpenAIChanges(handler: (detail: OpenAIChangesDetail) => void): () => void {
  if (typeof window === 'undefined') return () => {};
  const listener = (event: Event) => {
    const custom = event as CustomEvent<OpenAIChangesDetail>;
    handler(custom.detail || {});
  };
  window.addEventListener(SIMPLE_MODE_OPEN_AI_CHANGES_EVENT, listener);
  return () => window.removeEventListener(SIMPLE_MODE_OPEN_AI_CHANGES_EVENT, listener);
}
