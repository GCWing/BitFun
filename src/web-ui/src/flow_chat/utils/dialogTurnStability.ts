import type {
  DialogTurn,
  AnyFlowItem,
  FlowTextItem,
  FlowThinkingItem,
  FlowToolItem,
  ModelRound,
} from '../types/flow-chat';

const TRANSIENT_TURN_STATUSES = new Set(['pending', 'processing', 'finishing', 'image_analyzing', 'cancelling', 'inprogress']);
const TRANSIENT_ROUND_STATUSES = new Set(['pending', 'streaming', 'pending_confirmation']);
const TERMINAL_ROUND_STATUSES = new Set(['completed', 'cancelled', 'rejected', 'error']);
const TRANSIENT_TOOL_STATUSES = new Set([
  'pending',
  'preparing',
  'streaming',
  'running',
  'receiving',
  'starting',
  'analyzing',
  'pending_confirmation',
  'confirmed',
]);
const TERMINAL_TOOL_STATUSES = new Set(['completed', 'cancelled', 'rejected', 'error']);
const TERMINAL_ITEM_STATUSES = new Set(['completed', 'cancelled', 'rejected', 'error']);
const STABLE_ITEM_STATUSES = new Set(['completed', 'cancelled', 'rejected', 'error']);

type SettleInterruptedDialogTurnOptions = {
  interruptionReason?: FlowToolItem['interruptionReason'];
};

export function isTransientToolStatus(status: unknown): boolean {
  return typeof status === 'string' && TRANSIENT_TOOL_STATUSES.has(status);
}

function isTerminalTurnStatus(status: DialogTurn['status']): boolean {
  return status === 'completed' || status === 'cancelled' || status === 'error';
}

function getTurnFallbackStatus(turn: Pick<DialogTurn, 'error'>): DialogTurn['status'] {
  return turn.error ? 'error' : 'cancelled';
}

export function normalizeRecoveredTurnStatus(
  status: unknown,
  turn: Pick<DialogTurn, 'error'>,
): DialogTurn['status'] {
  if (status === 'completed' || status === 'cancelled' || status === 'error') {
    return status;
  }

  if (typeof status === 'string' && TRANSIENT_TURN_STATUSES.has(status)) {
    return getTurnFallbackStatus(turn);
  }

  return getTurnFallbackStatus(turn);
}

export function normalizeRecoveredRoundStatus(
  status: unknown,
  parentTurnStatus: DialogTurn['status'],
): ModelRound['status'] {
  if (status === 'completed' || status === 'cancelled' || status === 'rejected' || status === 'error') {
    return status;
  }

  if (typeof status === 'string' && (TRANSIENT_ROUND_STATUSES.has(status) || TERMINAL_ROUND_STATUSES.has(status))) {
    if (parentTurnStatus === 'completed' || parentTurnStatus === 'error' || parentTurnStatus === 'cancelled') {
      return parentTurnStatus;
    }
  }

  return parentTurnStatus === 'completed' ? 'completed' : parentTurnStatus === 'error' ? 'error' : 'cancelled';
}

export function normalizeRecoveredTextStatus(
  status: unknown,
  parentTurnStatus: DialogTurn['status'],
): FlowTextItem['status'] {
  if (status === 'completed' || status === 'cancelled' || status === 'rejected' || status === 'error') {
    return status;
  }

  if (parentTurnStatus === 'completed') {
    return 'completed';
  }

  if (parentTurnStatus === 'error') {
    return 'error';
  }

  return 'cancelled';
}

export function normalizeRecoveredThinkingStatus(
  status: unknown,
  parentTurnStatus: DialogTurn['status'],
): FlowThinkingItem['status'] {
  if (status === 'completed' || status === 'cancelled' || status === 'rejected' || status === 'error') {
    return status;
  }

  if (parentTurnStatus === 'completed') {
    return 'completed';
  }

  if (parentTurnStatus === 'error') {
    return 'error';
  }

  return 'cancelled';
}

export function normalizeRecoveredToolStatus(
  status: unknown,
  parentTurnStatus: DialogTurn['status'],
  toolResult?: Pick<NonNullable<FlowToolItem['toolResult']>, 'success' | 'error'> | null,
): FlowToolItem['status'] {
  if (status === 'completed' || status === 'cancelled' || status === 'rejected' || status === 'error') {
    return status;
  }

  if (parentTurnStatus === 'cancelled') {
    return 'cancelled';
  }

  if (parentTurnStatus === 'error') {
    return toolResult?.success === false && toolResult.error ? 'error' : 'cancelled';
  }

  if (parentTurnStatus === 'completed') {
    if (toolResult?.success === false) {
      return 'error';
    }
    return 'completed';
  }

  if (typeof status === 'string' && (TRANSIENT_TOOL_STATUSES.has(status) || TERMINAL_TOOL_STATUSES.has(status))) {
    return 'cancelled';
  }

  if (toolResult?.success === false) {
    return 'error';
  }

  return 'cancelled';
}

function settleInterruptedItem(
  item: AnyFlowItem,
  finalTurnStatus: DialogTurn['status'],
  settledAt: number,
  options?: SettleInterruptedDialogTurnOptions,
): AnyFlowItem {
  if (item.type === 'text') {
    const nextStatus = TERMINAL_ITEM_STATUSES.has(item.status)
      ? item.status
      : normalizeRecoveredTextStatus(item.status, finalTurnStatus);
    return {
      ...item,
      status: nextStatus,
      isStreaming: false,
    };
  }

  if (item.type === 'thinking') {
    const nextStatus = TERMINAL_ITEM_STATUSES.has(item.status)
      ? item.status
      : normalizeRecoveredThinkingStatus(item.status, finalTurnStatus);
    return {
      ...item,
      status: nextStatus,
      isStreaming: false,
      isCollapsed: true,
    };
  }

  if (item.type === 'tool') {
    const wasTransient = isTransientToolStatus(item.status);
    const nextStatus = normalizeRecoveredToolStatus(
      item.status,
      finalTurnStatus,
      item.toolResult,
    );
    return {
      ...item,
      status: nextStatus,
      interruptionReason:
        options?.interruptionReason === 'app_restart' && wasTransient && nextStatus === 'cancelled'
          ? 'app_restart'
          : item.interruptionReason,
      isParamsStreaming: false,
      endTime: item.endTime ?? settledAt,
    };
  }

  return item;
}

export function settleInterruptedDialogTurn(
  dialogTurn: DialogTurn,
  settledAt: number,
  options?: SettleInterruptedDialogTurnOptions,
): DialogTurn {
  const finalTurnStatus = normalizeRecoveredTurnStatus(dialogTurn.status, dialogTurn);

  if (
    isTerminalTurnStatus(dialogTurn.status) &&
    dialogTurn.modelRounds.every(round =>
      TERMINAL_ROUND_STATUSES.has(round.status) &&
      round.items.every(item => STABLE_ITEM_STATUSES.has(item.status))
    )
  ) {
    return dialogTurn;
  }

  const settledTurn = {
    ...dialogTurn,
    status: finalTurnStatus,
    endTime: dialogTurn.endTime ?? settledAt,
    modelRounds: dialogTurn.modelRounds.map(round => {
      const finalRoundStatus = normalizeRecoveredRoundStatus(round.status, finalTurnStatus);
      const nextItems = round.items.map(item =>
        settleInterruptedItem(item, finalTurnStatus, settledAt, options),
      );
      const nextAttempts = round.attempts?.map(attempt => ({
        ...attempt,
        items: attempt.items.map(item =>
          settleInterruptedItem(item, finalTurnStatus, settledAt, options),
        ),
      }));

      return {
        ...round,
        status: finalRoundStatus,
        isStreaming: false,
        isComplete: true,
        endTime: round.endTime ?? settledAt,
        items: nextItems,
        attempts: nextAttempts,
      };
    }),
  };

  return settledTurn;
}
