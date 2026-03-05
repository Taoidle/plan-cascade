import { useQueuedChatMessages } from './useQueuedChatMessages';

export function useSimpleQueueRuntime(params: Parameters<typeof useQueuedChatMessages>[0]) {
  return useQueuedChatMessages(params);
}
