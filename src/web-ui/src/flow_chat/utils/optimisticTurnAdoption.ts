/**
 * Optimistic-turn adoption.
 *
 * A driver that projects a user turn before the executing side confirms it
 * marks that turn with an adoption key. When the executor's own
 * DialogTurnStarted arrives with a different turn id, the event handler
 * adopts the marked turn in place instead of duplicating the message.
 *
 * The metadata key is wire-compatible with the original dispatch-only
 * implementation and must not change: cached transcripts persist it.
 */

import type { DialogTurn, Session } from '../types/flow-chat';

export const OPTIMISTIC_TURN_ADOPTION_KEY = '__openbitfunOptimisticDispatchJobId';

export function markOptimisticTurnAdoption(
  metadata: Record<string, unknown> | undefined,
  adoptionKey: string,
): Record<string, unknown> {
  return {
    ...metadata,
    [OPTIMISTIC_TURN_ADOPTION_KEY]: adoptionKey,
  };
}

export function optimisticTurnAdoptionKey(turn: DialogTurn): string | undefined {
  const value = turn.userMessage.metadata?.[OPTIMISTIC_TURN_ADOPTION_KEY];
  return typeof value === 'string' && value ? value : undefined;
}

export function stripOptimisticTurnAdoption(
  metadata: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (!metadata) {
    return undefined;
  }
  const next = { ...metadata };
  delete next[OPTIMISTIC_TURN_ADOPTION_KEY];
  return Object.keys(next).length > 0 ? next : undefined;
}

/**
 * The adoption key a not-yet-confirmed turn of this session would carry.
 * Currently only dispatch projections mark turns (their durable job id); any
 * future driver that sets an adoption key gets in-place adoption for free.
 */
export function sessionPendingTurnAdoptionKey(
  session: Pick<Session, 'config'>,
): string | undefined {
  return session.config.dispatchJobId?.trim() || undefined;
}
