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

// 'complete' 是 BitFun 正常收尾码；'stop' 是模型原生正常终止码，防御其他事件源。
// 'eos' / 'tool_calls' 同为模型原生正常终止码：流式 EOF 或模型最终转向工具调用，
// 都不是"没有可用结果"的异常终止。
const NORMAL_FINISH_REASONS = new Set(['complete', 'stop', 'eos', 'tool_calls']);

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
