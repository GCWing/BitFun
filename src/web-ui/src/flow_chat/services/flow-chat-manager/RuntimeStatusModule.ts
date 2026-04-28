/**
 * Transient runtime status helpers.
 *
 * These items explain long-running work in the current UI only. They must not
 * be treated as assistant output or persisted into session history.
 */

import type { FlowTextItem } from '../../types/flow-chat';
import type { DialogTurn, FlowChatContext } from './types';

export const MODEL_RESPONSE_STATUS_DELAY_MS = 1000;
const RUNTIME_STATUS_CONTENT = '\u200B';
type RuntimeStatus = NonNullable<FlowTextItem['runtimeStatus']>;
type RuntimeStatusScope = RuntimeStatus['scope'];

interface RuntimeStatusOptions {
  delayMs?: number;
  scope?: RuntimeStatusScope;
  messageKey?: string;
  parentToolId?: string;
  parentTimestamp?: number;
  subagentSessionId?: string;
}

interface ClearRuntimeStatusOptions {
  roundId?: string;
  scope?: RuntimeStatusScope;
  subagentSessionId?: string;
}

export function isRuntimeStatusItem(item: unknown): item is FlowTextItem {
  return Boolean(
    item &&
      typeof item === 'object' &&
      (item as { type?: unknown }).type === 'text' &&
      (item as { runtimeStatus?: unknown }).runtimeStatus,
  );
}

function runtimeStatusKey(
  sessionId: string,
  turnId: string,
  roundId: string,
  scope: RuntimeStatusScope,
): string {
  return `${sessionId}:${turnId}:${roundId}:${scope}`;
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
  const round = turn.modelRounds.find(candidate => candidate.id === roundId);
  if (!round) {
    return true;
  }

  return round.items.some(item => !isRuntimeStatusItem(item));
}

function hasVisibleSubagentOutput(turn: DialogTurn, subagentSessionId: string): boolean {
  return turn.modelRounds.some(round =>
    round.items.some(item => {
      const maybeSubagentItem = item as { subagentSessionId?: string };
      return maybeSubagentItem.subagentSessionId === subagentSessionId && !isRuntimeStatusItem(item);
    }),
  );
}

function createRuntimeStatusItem(
  roundId: string,
  options: Required<Pick<RuntimeStatusOptions, 'scope'>> & RuntimeStatusOptions,
): FlowTextItem {
  const now = Date.now();
  return {
    id: `runtime-status-${options.scope}-${options.subagentSessionId || roundId}`,
    type: 'text',
    content: RUNTIME_STATUS_CONTENT,
    timestamp: options.parentTimestamp ? options.parentTimestamp + 1 : now,
    status: 'streaming',
    isStreaming: true,
    isMarkdown: false,
    runtimeStatus: {
      phase: 'waiting_model',
      scope: options.scope,
      messageKey: options.messageKey || 'toolCards.terminal.waitingForModelResponse',
    },
    ...(options.parentToolId && {
      isSubagentItem: true,
      parentTaskToolId: options.parentToolId,
      subagentSessionId: options.subagentSessionId,
    }),
  };
}

function ensureActiveTextItems(context: FlowChatContext, sessionId: string): Map<string, string> {
  if (!context.activeTextItems.has(sessionId)) {
    context.activeTextItems.set(sessionId, new Map());
  }
  return context.activeTextItems.get(sessionId)!;
}

export function scheduleModelResponseStatus(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  options: RuntimeStatusOptions = {},
): void {
  const scope = options.scope || 'main';
  const statusTargetId = options.subagentSessionId || roundId;
  const key = runtimeStatusKey(sessionId, turnId, statusTargetId, scope);
  const existingTimer = context.runtimeStatusTimers.get(key);
  if (existingTimer) {
    clearTimeout(existingTimer);
  }

  const delayMs = options.delayMs ?? MODEL_RESPONSE_STATUS_DELAY_MS;
  const timer = setTimeout(() => {
    context.runtimeStatusTimers.delete(key);

    const turn = getDialogTurn(context, sessionId, turnId);
    if (!turn) {
      return;
    }

    if (options.subagentSessionId) {
      if (hasVisibleSubagentOutput(turn, options.subagentSessionId)) {
        return;
      }
    } else if (hasVisibleOutputForRound(turn, roundId)) {
      return;
    }

    const statusItem = createRuntimeStatusItem(roundId, { ...options, scope });
    if (options.parentToolId) {
      context.flowChatStore.insertModelRoundItemAfterTool(sessionId, turnId, options.parentToolId, statusItem);
    } else {
      context.flowChatStore.addModelRoundItem(sessionId, turnId, statusItem, roundId);
      ensureActiveTextItems(context, sessionId).set(roundId, statusItem.id);
    }
  }, delayMs);

  context.runtimeStatusTimers.set(key, timer);
}

export function clearRuntimeStatus(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  options: ClearRuntimeStatusOptions = {},
): void {
  let cancelledTimer = false;
  for (const [key, timer] of context.runtimeStatusTimers.entries()) {
    const [timerSessionId, timerTurnId, timerTargetId, timerScope] = key.split(':');
    if (timerSessionId !== sessionId || timerTurnId !== turnId) {
      continue;
    }
    if (options.roundId && timerTargetId !== options.roundId) {
      continue;
    }
    if (options.subagentSessionId && timerTargetId !== options.subagentSessionId) {
      continue;
    }
    if (options.scope && timerScope !== options.scope) {
      continue;
    }
    clearTimeout(timer);
    context.runtimeStatusTimers.delete(key);
    cancelledTimer = true;
  }

  const activeItems = context.activeTextItems.get(sessionId);
  const turn = getDialogTurn(context, sessionId, turnId);
  const hasMatchingRuntimeStatus = turn?.modelRounds.some(round => {
    if (options.roundId && round.id !== options.roundId) {
      return false;
    }

    return round.items.some(item => {
      if (!isRuntimeStatusItem(item)) {
        return false;
      }
      if (options.scope && item.runtimeStatus?.scope !== options.scope) {
        return false;
      }
      if (options.subagentSessionId && item.subagentSessionId !== options.subagentSessionId) {
        return false;
      }
      return true;
    });
  }) ?? false;

  if (!hasMatchingRuntimeStatus) {
    if (cancelledTimer && options.roundId) {
      activeItems?.delete(options.roundId);
    }
    return;
  }

  context.flowChatStore.updateDialogTurn(sessionId, turnId, turn => {
    const removedIds: string[] = [];
    const modelRounds = turn.modelRounds.map(round => {
      if (options.roundId && round.id !== options.roundId) {
        return round;
      }

      const items = round.items.filter(item => {
        if (!isRuntimeStatusItem(item)) {
          return true;
        }
        if (options.scope && item.runtimeStatus?.scope !== options.scope) {
          return true;
        }
        if (options.subagentSessionId && item.subagentSessionId !== options.subagentSessionId) {
          return true;
        }
        removedIds.push(item.id);
        return false;
      });

      return items.length === round.items.length ? round : { ...round, items };
    });

    if (activeItems && removedIds.length > 0) {
      for (const [roundId, itemId] of activeItems.entries()) {
        if (removedIds.includes(itemId)) {
          activeItems.delete(roundId);
        }
      }
    }

    return { ...turn, modelRounds };
  });
}
