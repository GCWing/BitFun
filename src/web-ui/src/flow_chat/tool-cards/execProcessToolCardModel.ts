import type { FlowToolItem } from '../types/flow-chat';
import type { ExecProcessCardModel } from './ExecProcessToolCardView';

interface ParsedExecResult {
  output: string;
  status?: string;
  workdir?: string;
  sessionId?: number;
  requestedSessionId?: number;
  exitCode?: number;
  wallTimeSeconds?: number;
  remote?: boolean;
  tty?: boolean;
}

function resultRecord(raw: unknown): Record<string, unknown> | null {
  if (raw == null) {
    return null;
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw);
      return parsed && typeof parsed === 'object' ? parsed as Record<string, unknown> : null;
    } catch {
      return null;
    }
  }

  return typeof raw === 'object' ? raw as Record<string, unknown> : null;
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key];
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === 'string' ? value : undefined;
}

function boolField(record: Record<string, unknown>, key: string): boolean | undefined {
  const value = record[key];
  return typeof value === 'boolean' ? value : undefined;
}

export function parseExecProcessResult(raw: unknown): ParsedExecResult {
  const record = resultRecord(raw);
  if (!record) {
    return { output: '' };
  }

  return {
    output: stringField(record, 'output') ?? '',
    status: stringField(record, 'status'),
    workdir: stringField(record, 'workdir'),
    sessionId: numberField(record, 'session_id'),
    requestedSessionId: numberField(record, 'requested_session_id'),
    exitCode: numberField(record, 'exit_code'),
    wallTimeSeconds: numberField(record, 'wall_time_seconds'),
    remote: boolField(record, 'remote'),
    tty: boolField(record, 'tty'),
  };
}

export function buildExecCommandCardModel(
  toolItem: FlowToolItem,
  t: (key: string, options?: Record<string, unknown>) => string,
): ExecProcessCardModel {
  const input = toolItem.toolCall?.input ?? {};
  const result = parseExecProcessResult(toolItem.toolResult?.result);
  const cmd = typeof input.cmd === 'string' ? input.cmd : '';

  return {
    kind: 'command',
    actionLabel: t('toolCards.execProcess.executeCommand'),
    primaryText: cmd,
    emptyText: t('toolCards.terminal.noCommand'),
    copyText: cmd,
    copyDisabled: !cmd.trim(),
    waitingText: t('toolCards.execProcess.executingCommand'),
    noOutputText: t('toolCards.execProcess.noOutput'),
    resultOutput: result.output,
    workdir: result.workdir,
    sessionId: result.sessionId,
    exitCode: result.exitCode,
    wallTimeSeconds: result.wallTimeSeconds,
    remote: result.remote,
    tty: result.tty,
  };
}

export function buildWriteStdinCardModel(
  toolItem: FlowToolItem,
  t: (key: string, options?: Record<string, unknown>) => string,
): ExecProcessCardModel {
  const input = toolItem.toolCall?.input ?? {};
  const result = parseExecProcessResult(toolItem.toolResult?.result);
  const sessionId = typeof input.session_id === 'number'
    ? input.session_id
    : result.sessionId;
  const displaySessionId = sessionId ?? result.requestedSessionId;
  const chars = typeof input.chars === 'string' ? input.chars : '';
  const appendEnter = Boolean(input.append_enter);
  const isPollOnly = chars.length === 0;
  const primaryText = isPollOnly
    ? t('toolCards.execProcess.pollSession', { id: displaySessionId ?? '?' })
    : appendEnter
      ? `${chars}\\n`
      : chars;
  const resultNoticeText = result.status === 'session_not_found'
    ? t('toolCards.execProcess.sessionNotFound', {
      id: displaySessionId ?? '?',
    })
    : undefined;

  return {
    kind: 'stdin',
    actionLabel: isPollOnly
      ? t('toolCards.execProcess.pollProcess')
      : t('toolCards.execProcess.writeStdin'),
    primaryText,
    emptyText: t('toolCards.execProcess.pollSession', { id: displaySessionId ?? '?' }),
    copyText: chars,
    copyDisabled: isPollOnly,
    waitingText: isPollOnly
      ? t('toolCards.execProcess.pollingOutput')
      : t('toolCards.execProcess.waitingForOutput'),
    noOutputText: t('toolCards.execProcess.noOutput'),
    resultNoticeText,
    resultOutput: result.output,
    sessionId: displaySessionId,
    exitCode: result.exitCode,
    wallTimeSeconds: result.wallTimeSeconds,
    remote: result.remote,
  };
}
