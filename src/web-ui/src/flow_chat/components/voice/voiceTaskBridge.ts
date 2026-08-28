import { flowChatSessionConfigForWorkspace } from '@/app/utils/projectSessionWorkspace';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import type { DialogTurn, FlowTextItem, Session } from '@/flow_chat/types/flow-chat';
import {
  subscribeAgentCompanionActivity,
  type AgentCompanionTaskStatus,
} from '@/flow_chat/utils/agentCompanionActivity';
import type { WorkspaceInfo } from '@/shared/types';

const TASK_TIMEOUT_MS = 30 * 60 * 1000;
const HEARTBEAT_INTERVAL_MS = 20_000;
const TEXT_PROGRESS_INTERVAL_MS = 8_000;
const MAX_RESULT_CHARS = 6_000;
const MAX_PROGRESS_CHARS = 90;

export type VoiceTaskProgressPhase =
  | 'starting'
  | 'working'
  | 'using_tools'
  | 'waiting_approval'
  | 'needs_input'
  | 'finishing'
  | 'stopping';

export interface VoiceTaskProgress {
  sessionId: string;
  phase: VoiceTaskProgressPhase;
}

export interface VoiceTaskResult {
  sessionId: string;
  summary: string;
}

interface RunVoiceTaskOptions {
  workspace: WorkspaceInfo;
  showSession?: boolean;
  signal?: AbortSignal;
  onSessionCreated?: (sessionId: string) => void;
  onProgress?: (progress: VoiceTaskProgress) => void;
  onTextProgress?: (text: string) => void;
}

export class VoiceTaskCancelledError extends Error {
  readonly sessionId: string;

  constructor(sessionId: string) {
    super('BitFun task was cancelled');
    this.name = 'VoiceTaskCancelledError';
    this.sessionId = sessionId;
  }
}

function progressPhase(task: AgentCompanionTaskStatus | undefined): VoiceTaskProgressPhase {
  if (!task) return 'working';
  if (task.state === 'attention') {
    return task.labelKey.endsWith('needsInput') ? 'needs_input' : 'waiting_approval';
  }
  if (task.labelKey.endsWith('usingTools')) return 'using_tools';
  if (task.labelKey.endsWith('finishing')) return 'finishing';
  return 'working';
}

function normalizeAssistantText(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}#{1,6}\s+/gm, '')
    .replace(/^\s{0,3}>\s?/gm, '')
    .replace(/[*_~]{1,3}/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function latestTurn(session: Session): DialogTurn | undefined {
  return session.dialogTurns[session.dialogTurns.length - 1];
}

function truncateProgressText(text: string): string {
  if (text.length <= MAX_PROGRESS_CHARS) return text;
  const candidate = text.slice(0, MAX_PROGRESS_CHARS);
  const boundaries = ['。', '！', '？', '. ', '! ', '? ', '；', '; ']
    .map(mark => candidate.lastIndexOf(mark))
    .filter(index => index >= Math.floor(MAX_PROGRESS_CHARS * 0.45));
  const boundary = boundaries.length ? Math.max(...boundaries) + 1 : -1;
  return `${candidate.slice(0, boundary > 0 ? boundary : MAX_PROGRESS_CHARS - 1).trim()}…`;
}

function progressSentences(text: string): string[] {
  return text
    .match(/[^\u3002.\uFF01\uFF1F!?\uFF1B;]+[\u3002.\uFF01\uFF1F!?\uFF1B;]?/g)
    ?.map(sentence => sentence.trim())
    .filter(Boolean) ?? [];
}

function rewriteProgressSentence(sentence: string): string {
  const punctuation = sentence.match(/[\u3002.\uFF01\uFF1F!?\uFF1B;]$/)?.[0] ?? '';
  let value = punctuation ? sentence.slice(0, -1).trim() : sentence.trim();
  value = value
    .replace(/^(?:progress|update|status)\s*[:\uFF1A-]?\s*/i, '')
    .replace(/^(?:\u8FDB\u5C55|\u66F4\u65B0|\u72B6\u6001)\s*[:\uFF1A-]?\s*/, '')
    .replace(/^(?:\u6211|\u6211\u4EEC|BitFun)\s*/, '');

  const completedChinese = value.match(
    /^(?:\u5DF2\u7ECF|\u5DF2)\u5B8C\u6210\u4E86?(.+?)(?:\uFF0C(.+))?$/,
  );
  if (completedChinese) {
    value = `${completedChinese[1]}\u5DF2\u5B8C\u6210${completedChinese[2] ? `\uFF0C${completedChinese[2]}` : ''}`;
  }
  value = value
    .replace(
      /^(?:\u63A5\u4E0B\u6765|\u4E0B\u4E00\u6B65)(?:\u6211)?(?:\u4F1A|\u5C06|\u51C6\u5907)?\s*/,
      '\u4E0B\u4E00\u6B65',
    )
    .replace(
      /^(?:\u76EE\u524D|\u73B0\u5728)?(?:\u6B63\u5728|\u5728)\s*/,
      '\u6B63\u5728',
    );

  value = value
    .replace(/^(?:I|we)(?:'ve| have) (?:finished|completed) (.+)$/i, 'Completed $1')
    .replace(/^(?:I am|I'm|we are|we're) (.+)$/i, 'Now $1')
    .replace(/^Next,?\s+(?:I|we) (?:will|'ll)\s+/i, 'Next, ')
    .replace(/^Finished (.+?) and (?:I )?am (.+)$/i, 'Finished $1. Now $2');

  if (!value) return '';
  return `${value}${punctuation || (/[\u3400-\u9fff]/.test(value) ? '\u3002' : '.')}`;
}

export function summarizeVoiceTaskProgress(text: string): string {
  const normalized = normalizeAssistantText(text);
  if (!normalized) return '';

  const rewritten = progressSentences(normalized)
    .map(rewriteProgressSentence)
    .filter(Boolean)
    .join(/[\u3400-\u9fff]/.test(normalized) ? '' : ' ');
  const candidates = progressSentences(rewritten);
  if (!candidates.length) return '';

  const selected = [candidates[0]];
  if (candidates.length > 1) {
    const nextStep = candidates.slice(1).find(sentence =>
      /(?:\u4E0B\u4E00\u6B65|\u63A5\u4E0B\u6765|\u6B63\u5728|\u7EE7\u7EED|now|next|checking|running|testing)/i.test(sentence));
    selected.push(nextStep ?? candidates[1]);
  }
  const separator = /[\u3400-\u9fff]/.test(normalized) ? '' : ' ';
  const prefix = /[\u3400-\u9fff]/.test(normalized) ? '\u8FDB\u5C55\uFF1A' : 'Progress: ';
  return truncateProgressText(`${prefix}${selected.join(separator)}`);
}

export function extractVoiceTaskProgressTexts(session: Session): Array<{ id: string; text: string }> {
  const turn = latestTurn(session);
  if (!turn || !['processing', 'finishing'].includes(turn.status)) return [];

  const updates: Array<{ id: string; text: string }> = [];
  turn.modelRounds.forEach(round => {
    round.items.forEach(item => {
      if (item.type !== 'text') return;
      const textItem = item as FlowTextItem;
      if (textItem.isStreaming || textItem.status !== 'completed') return;
      const text = summarizeVoiceTaskProgress(textItem.content);
      if (!text) return;
      updates.push({ id: `${round.id}:${textItem.id}:${text}`, text });
    });
  });
  return updates;
}

export function extractVoiceTaskSummary(session: Session): string {
  const turn = latestTurn(session);
  if (!turn) {
    return 'BitFun completed the task without a text response.';
  }
  const parts: string[] = [];
  turn.modelRounds.forEach(round => {
    round.items.forEach(item => {
      if (item.type !== 'text') return;
      const text = normalizeAssistantText((item as FlowTextItem).content);
      if (text) parts.push(text);
    });
  });
  const summary = parts.join(' ').trim();
  if (summary.length <= MAX_RESULT_CHARS) {
    return summary || 'BitFun completed the task without a text response.';
  }
  return `${summary.slice(0, MAX_RESULT_CHARS - 1)}…`;
}

async function waitForSettledSession(sessionId: string): Promise<void> {
  const isSettled = () => {
    const state = stateMachineManager.getCurrentState(sessionId);
    if (state !== SessionExecutionState.IDLE && state !== SessionExecutionState.ERROR) {
      return false;
    }
    const session = FlowChatManager.getInstance().getFlowChatState().sessions.get(sessionId);
    const turn = session ? latestTurn(session) : undefined;
    return Boolean(turn && !['pending', 'processing', 'finishing', 'cancelling'].includes(turn.status));
  };

  if (isSettled()) return;
  await new Promise<void>((resolve, reject) => {
    const subscription = { dispose: () => undefined as void };
    let finished = false;
    const finish = (activeTimeoutId: number) => {
      if (finished) return;
      finished = true;
      window.clearTimeout(activeTimeoutId);
      subscription.dispose();
      resolve();
    };
    const timeoutId = window.setTimeout(() => {
      if (finished) return;
      finished = true;
      subscription.dispose();
      reject(new Error('BitFun task timed out after 30 minutes'));
    }, TASK_TIMEOUT_MS);
    subscription.dispose = stateMachineManager.subscribeGlobal((changedSessionId) => {
      if (changedSessionId !== sessionId || !isSettled()) return;
      finish(timeoutId);
    });
    // Close the check/subscribe race: a very short task or cancellation can
    // settle after the first check and before the global listener is attached.
    if (finished) subscription.dispose();
    else if (isSettled()) finish(timeoutId);
  });
}

export async function runBitFunVoiceTask(
  task: string,
  options: RunVoiceTaskOptions,
): Promise<VoiceTaskResult> {
  const normalizedTask = task.trim();
  if (!normalizedTask) {
    throw new Error('BitFun task description is empty');
  }

  const manager = FlowChatManager.getInstance();
  const sessionId = await manager.createChatSession(
    flowChatSessionConfigForWorkspace(options.workspace),
    'agentic',
  );
  options.onSessionCreated?.(sessionId);
  if (options.showSession !== false) {
    await openMainSession(sessionId);
  }

  let lastUserUpdateAt = Date.now();
  const emitProgress = (phase: VoiceTaskProgressPhase) => {
    lastUserUpdateAt = Date.now();
    options.onProgress?.({ sessionId, phase });
  };
  emitProgress('starting');

  let latestPhase: VoiceTaskProgressPhase = 'starting';
  const emitFromActivity = (taskStatus?: AgentCompanionTaskStatus) => {
    const phase = progressPhase(taskStatus);
    if (phase === latestPhase) return;
    latestPhase = phase;
    emitProgress(phase);
  };
  const unsubscribeActivity = subscribeAgentCompanionActivity(payload => {
    emitFromActivity(payload.tasks.find(item => item.sessionId === sessionId));
  });

  const seenTextProgress = new Set<string>();
  let lastTextProgress = '';
  let pendingTextProgress = '';
  let textProgressTimer: number | null = null;
  const publishTextProgress = () => {
    textProgressTimer = null;
    const text = pendingTextProgress;
    pendingTextProgress = '';
    if (!text || text === lastTextProgress) return;
    lastTextProgress = text;
    lastUserUpdateAt = Date.now();
    options.onTextProgress?.(text);
  };
  const queueTextProgress = (text: string) => {
    if (!options.onTextProgress || text === lastTextProgress) return;
    pendingTextProgress = text;
    const delay = Math.max(0, TEXT_PROGRESS_INTERVAL_MS - (Date.now() - lastUserUpdateAt));
    if (delay === 0) {
      if (textProgressTimer !== null) window.clearTimeout(textProgressTimer);
      publishTextProgress();
    } else if (textProgressTimer === null) {
      textProgressTimer = window.setTimeout(publishTextProgress, delay);
    }
  };
  const unsubscribeState = manager.onFlowChatStateChange(state => {
    const session = state.sessions.get(sessionId) as Session | undefined;
    if (!session) return;
    extractVoiceTaskProgressTexts(session).forEach(update => {
      if (seenTextProgress.has(update.id)) return;
      seenTextProgress.add(update.id);
      queueTextProgress(update.text);
    });
  });

  const heartbeatId = window.setInterval(() => {
    if (Date.now() - lastUserUpdateAt < HEARTBEAT_INTERVAL_MS) return;
    emitProgress(latestPhase === 'starting' ? 'working' : latestPhase);
  }, HEARTBEAT_INTERVAL_MS);

  let cancellationInFlight: Promise<boolean> | null = null;
  const requestCancellation = (): Promise<boolean> => {
    if (cancellationInFlight) return cancellationInFlight;
    cancellationInFlight = manager.cancelSessionTask(sessionId).finally(() => {
      cancellationInFlight = null;
    });
    return cancellationInFlight;
  };
  const handleAbort = () => {
    void requestCancellation();
  };
  options.signal?.addEventListener('abort', handleAbort);

  try {
    if (options.signal?.aborted) {
      throw new VoiceTaskCancelledError(sessionId);
    }
    await manager.sendMessage(
      normalizedTask,
      sessionId,
      normalizedTask,
      'agentic',
      undefined,
      { userMessageMetadata: { source: 'realtime_voice' } },
    );
    if (options.signal?.aborted) {
      await requestCancellation();
    }
    await waitForSettledSession(sessionId);
    const session = manager.getFlowChatState().sessions.get(sessionId);
    if (!session) {
      throw new Error('BitFun task session disappeared before completion');
    }
    const turn = latestTurn(session);
    if (!turn || turn.status === 'error') {
      throw new Error(turn?.error || session.error || 'BitFun task failed');
    }
    if (turn.status === 'cancelled') {
      throw new VoiceTaskCancelledError(sessionId);
    }
    return { sessionId, summary: extractVoiceTaskSummary(session) };
  } finally {
    options.signal?.removeEventListener('abort', handleAbort);
    window.clearInterval(heartbeatId);
    if (textProgressTimer !== null) window.clearTimeout(textProgressTimer);
    unsubscribeState();
    unsubscribeActivity();
  }
}
