import type { DialogTurn } from '@/flow_chat/types/flow-chat';

const OPTIMISTIC_DISPATCH_JOB_ID_KEY = '__bitfunOptimisticDispatchJobId';

export function markOptimisticDispatchTurnMetadata(
  metadata: Record<string, unknown> | undefined,
  jobId: string,
): Record<string, unknown> {
  return {
    ...metadata,
    [OPTIMISTIC_DISPATCH_JOB_ID_KEY]: jobId,
  };
}

export function optimisticDispatchTurnJobId(turn: DialogTurn): string | undefined {
  const value = turn.userMessage.metadata?.[OPTIMISTIC_DISPATCH_JOB_ID_KEY];
  return typeof value === 'string' && value ? value : undefined;
}

export function stripOptimisticDispatchTurnMetadata(
  metadata: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (!metadata) {
    return undefined;
  }
  const next = { ...metadata };
  delete next[OPTIMISTIC_DISPATCH_JOB_ID_KEY];
  return Object.keys(next).length > 0 ? next : undefined;
}
