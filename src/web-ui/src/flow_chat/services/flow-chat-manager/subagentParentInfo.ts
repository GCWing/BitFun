import type { SubagentParentInfo } from '../EventBatcher';

type RawSubagentParentInfo = Partial<SubagentParentInfo> & {
  tool_call_id?: unknown;
  session_id?: unknown;
  dialog_turn_id?: unknown;
};

function normalizeString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value : undefined;
}

export function normalizeSubagentParentInfo(event: unknown): SubagentParentInfo | undefined {
  if (!event || typeof event !== 'object') {
    return undefined;
  }

  const record = event as Record<string, unknown>;
  // Backend events may arrive from snake_case Rust serialization or camelCase web adapters.
  const raw = (record.subagentParentInfo ?? record.subagent_parent_info) as
    | RawSubagentParentInfo
    | undefined;

  if (!raw || typeof raw !== 'object') {
    return undefined;
  }

  const toolCallId = normalizeString(raw.toolCallId) ?? normalizeString(raw.tool_call_id);
  const sessionId = normalizeString(raw.sessionId) ?? normalizeString(raw.session_id);
  const dialogTurnId = normalizeString(raw.dialogTurnId) ?? normalizeString(raw.dialog_turn_id);

  if (!toolCallId || !sessionId || !dialogTurnId) {
    return undefined;
  }

  return {
    toolCallId,
    sessionId,
    dialogTurnId,
  };
}
