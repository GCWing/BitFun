/**
 * Session-scoped runtime wait status.
 *
 * Runtime status is transient UI state. It never enters model round items, so
 * showing or hiding it cannot change the virtualized conversation structure.
 */

import {
  clearRuntimeStatusState,
  showRuntimeStatus,
} from '../../store/runtimeStatusStore';
import type { DialogTurn, FlowChatContext } from './types';

export const MODEL_RESPONSE_STATUS_DELAY_MS = 1000;

interface RuntimeStatusOptions {
  delayMs?: number;
}

interface ClearRuntimeStatusOptions {
  roundId?: string;
}

function runtimeStatusKey(sessionId: string, turnId: string, roundId: string): string {
  return `${sessionId}:${turnId}:${roundId}`;
}

function getDialogTurn(context: FlowChatContext, sessionId: string, turnId: string): DialogTurn | undefined {
  return context.flowChatStore
    .getState()
    .sessions
    .get(sessionId)
    ?.dialogTurns
    .find(turn => turn.id === turnId);
}

function hasVisibleOutputForRound(turn: DialogTurn, roundId: string): boolean {
  return turn.modelRounds.find(candidate => candidate.id === roundId)?.items.length !== 0;
}

export function scheduleModelResponseStatus(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  options: RuntimeStatusOptions = {},
): void {
  const key = runtimeStatusKey(sessionId, turnId, roundId);
  const sessionTimerPrefix = `${sessionId}:`;
  for (const [pendingKey, pendingTimer] of context.runtimeStatusTimers.entries()) {
    if (!pendingKey.startsWith(sessionTimerPrefix)) {
      continue;
    }
    clearTimeout(pendingTimer);
    context.runtimeStatusTimers.delete(pendingKey);
  }

  // A new round owns the session slot immediately, but remains visually hidden
  // until the delay elapses.
  clearRuntimeStatusState({ sessionId });

  const delayMs = options.delayMs ?? MODEL_RESPONSE_STATUS_DELAY_MS;
  const timer = setTimeout(() => {
    context.runtimeStatusTimers.delete(key);

    const turn = getDialogTurn(context, sessionId, turnId);
    if (!turn || hasVisibleOutputForRound(turn, roundId)) {
      return;
    }

    showRuntimeStatus({
      sessionId,
      turnId,
      roundId,
    });
  }, delayMs);

  context.runtimeStatusTimers.set(key, timer);
}

export function clearRuntimeStatus(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  options: ClearRuntimeStatusOptions = {},
): void {
  const timerPrefix = `${sessionId}:${turnId}:`;
  const targetTimerKey = options.roundId
    ? runtimeStatusKey(sessionId, turnId, options.roundId)
    : null;
  for (const [key, timer] of context.runtimeStatusTimers.entries()) {
    if (targetTimerKey ? key !== targetTimerKey : !key.startsWith(timerPrefix)) {
      continue;
    }
    clearTimeout(timer);
    context.runtimeStatusTimers.delete(key);
  }

  clearRuntimeStatusState({
    sessionId,
    turnId,
    roundId: options.roundId,
  });
}
