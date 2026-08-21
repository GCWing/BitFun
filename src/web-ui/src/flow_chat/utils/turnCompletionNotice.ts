import type { DialogTurn } from '../types/flow-chat';

export type TurnCompletionNoticeTone = 'info' | 'warning' | 'error';

export interface TurnCompletionNotice {
  reasonCode: string;
  tone: TurnCompletionNoticeTone;
  titleKey: string;
  bodyKey?: string;
}

interface TurnCompletionNoticeConfig {
  tone: TurnCompletionNoticeTone;
  titleKey: string;
}

interface NormalizedTurnCompletionNoticeState {
  reasonCode: string;
  hasFinalResponse?: boolean;
}

// 'complete' is BitFun's normal finish code; 'stop' is a model-native normal
// termination code, defensive against other event sources.
// 'eos' / 'tool_calls' are also model-native normal termination codes: stream
// EOF or the model finally turning to tool calls — neither is an abnormal
// termination meaning "no usable result".
// 'completed' is the DialogTurn.status value mirrored into finishReason by
// group chat message projection (GroupChatView.groupMessageToDialogTurn) and
// other local turn builders - a normal completion, not an abnormal reason.
const NORMAL_FINISH_REASONS = new Set(['complete', 'stop', 'eos', 'tool_calls', 'completed']);

const TURN_COMPLETION_NOTICE_CONFIG: Record<string, TurnCompletionNoticeConfig> = {
  repeated_tool_failures: {
    tone: 'warning',
    titleKey: 'turnCompletionNotice.repeatedToolFailures.title',
  },
  max_rounds: {
    tone: 'warning',
    titleKey: 'turnCompletionNotice.maxRounds.title',
  },
  empty_round: {
    tone: 'warning',
    titleKey: 'turnCompletionNotice.emptyRound.title',
  },
  interrupted: {
    tone: 'warning',
    titleKey: 'turnCompletionNotice.interrupted.title',
  },
};

const DEFAULT_TURN_COMPLETION_NOTICE: TurnCompletionNoticeConfig = {
  tone: 'warning',
  titleKey: 'turnCompletionNotice.generic.title',
};

function normalizeFinishReason(finishReason?: string): string | null {
  if (typeof finishReason !== 'string') {
    return null;
  }

  const normalized = finishReason.trim();
  return normalized.length > 0 ? normalized : null;
}

function normalizeNoticeState(
  turn: Pick<DialogTurn, 'finishReason' | 'hasFinalResponse'>,
): NormalizedTurnCompletionNoticeState | null {
  const reasonCode = normalizeFinishReason(turn.finishReason);
  if (!reasonCode || NORMAL_FINISH_REASONS.has(reasonCode)) {
    return null;
  }

  switch (reasonCode) {
    case 'repeated_tool_failures':
      return {
        reasonCode,
        hasFinalResponse:
          typeof turn.hasFinalResponse === 'boolean' ? turn.hasFinalResponse : true,
      };
    default:
      return {
        reasonCode,
        hasFinalResponse:
          typeof turn.hasFinalResponse === 'boolean' ? turn.hasFinalResponse : undefined,
      };
  }
}

export function getTurnCompletionNotice(
  turn: Pick<DialogTurn, 'status' | 'finishReason' | 'hasFinalResponse'>,
): TurnCompletionNotice | null {
  if (turn.status !== 'completed') {
    return null;
  }

  const normalized = normalizeNoticeState(turn);
  if (!normalized) {
    return null;
  }

  const config =
    TURN_COMPLETION_NOTICE_CONFIG[normalized.reasonCode] ?? DEFAULT_TURN_COMPLETION_NOTICE;
  return {
    reasonCode: normalized.reasonCode,
    tone: config.tone,
    titleKey: config.titleKey,
    bodyKey:
      normalized.hasFinalResponse === true
        ? 'turnCompletionNotice.finalResponseProvided'
        : undefined,
  };
}
