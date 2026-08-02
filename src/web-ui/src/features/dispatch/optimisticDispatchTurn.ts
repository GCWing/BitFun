/**
 * Dispatch-named wrappers over the generic optimistic-turn adoption helpers.
 * The metadata key and behavior are unchanged; see
 * `flow_chat/utils/optimisticTurnAdoption.ts`.
 */

import type { DialogTurn } from '@/flow_chat/types/flow-chat';
import {
  markOptimisticTurnAdoption,
  optimisticTurnAdoptionKey,
  stripOptimisticTurnAdoption,
} from '@/flow_chat/utils/optimisticTurnAdoption';

export function markOptimisticDispatchTurnMetadata(
  metadata: Record<string, unknown> | undefined,
  jobId: string,
): Record<string, unknown> {
  return markOptimisticTurnAdoption(metadata, jobId);
}

export function optimisticDispatchTurnJobId(turn: DialogTurn): string | undefined {
  return optimisticTurnAdoptionKey(turn);
}

export function stripOptimisticDispatchTurnMetadata(
  metadata: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  return stripOptimisticTurnAdoption(metadata);
}
