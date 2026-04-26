import { agentAPI, btwAPI, sessionAPI } from '@/infrastructure/api';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine';
import { flowChatManager } from './FlowChatManager';
import type { DialogTurn, ModelRound, FlowTextItem } from '../types/flow-chat';
import type { SessionKind } from '@/shared/types/session-history';
import { buildSessionMetadata } from '../utils/sessionMetadata';

const log = createLogger('BtwThreadService');

function safeUuid(prefix = 'btw'): string {
  try {
    const fn = (globalThis as any)?.crypto?.randomUUID as (() => string) | undefined;
    if (fn) return fn();
  } catch {
    // ignore
  }
  return `${prefix}_${Date.now()}_${Math.random().toString(16).slice(2)}`;
}

function toOneLine(input: string): string {
  return input.replace(/\s+/g, ' ').trim();
}

function buildChildSessionName(question: string): string {
  const one = toOneLine(question);
  const clipped = one.length > 48 ? `${one.slice(0, 48)}…` : one;
  // Child session title should not be prefixed with "/btw" (user command),
  // keep it as a clean thread title.
  return clipped || 'Side thread';
}

async function loadSessionMetadataWithRetry(
  sessionId: string,
  workspacePath: string,
  opts?: { retries?: number; delayMs?: number },
  remoteConnectionId?: string
): Promise<import('@/shared/types/session-history').SessionMetadata | null> {
  const retries = opts?.retries ?? 10;
  const delayMs = opts?.delayMs ?? 60;

  for (let i = 0; i < retries; i++) {
    try {
      const meta = await sessionAPI.loadSessionMetadata(sessionId, workspacePath, remoteConnectionId);
      if (meta) return meta;
    } catch (e) {
      // Ignore and retry; persistence write can lag behind create_session event.
      log.debug('loadSessionMetadata retry failed', { sessionId, attempt: i + 1, e });
    }
    await new Promise(r => window.setTimeout(r, delayMs));
  }
  return null;
}

function getParentInterruptionContext(parentSessionId: string): { parentDialogTurnId?: string; parentTurnIndex?: number } {
  const machine = stateMachineManager.get(parentSessionId);
  const ctx = machine?.getContext?.();
  const machineTurnId = ctx?.currentDialogTurnId || undefined;

  const session = flowChatStore.getState().sessions.get(parentSessionId);
  if (!session) {
    return { parentDialogTurnId: machineTurnId, parentTurnIndex: undefined };
  }

  const parentDialogTurnId = machineTurnId || session.dialogTurns[session.dialogTurns.length - 1]?.id;
  if (!parentDialogTurnId) return { parentDialogTurnId: undefined, parentTurnIndex: undefined };

  const idx = session.dialogTurns.findIndex(t => t.id === parentDialogTurnId);
  return { parentDialogTurnId, parentTurnIndex: idx >= 0 ? idx + 1 : undefined };
}

export async function createBtwChildSession(params: {
  parentSessionId: string;
  workspacePath?: string;
  childSessionName: string;
  agentType?: string;
  modelName?: string;
  enableTools?: boolean;
  safeMode?: boolean;
  autoCompact?: boolean;
  enableContextCompression?: boolean;
  requestId?: string;
  addMarker?: boolean;
  sessionKind?: Extract<SessionKind, 'btw' | 'review' | 'deep_review'>;
}): Promise<{
  requestId: string;
  childSessionId: string;
  parentDialogTurnId?: string;
  parentTurnIndex?: number;
}> {
  const { parentSessionId } = params;
  const requestId = params.requestId || safeUuid('btw');
  const childSessionKind = params.sessionKind ?? 'btw';
  const createdAt = Date.now();
  const { parentDialogTurnId, parentTurnIndex } = getParentInterruptionContext(parentSessionId);

  const parentSession = flowChatStore.getState().sessions.get(parentSessionId);
  const workspacePath = params.workspacePath || parentSession?.workspacePath;
  if (!workspacePath) {
    throw new Error(`Workspace path is required for BTW child session: ${parentSessionId}`);
  }

  const agentType = params.agentType || parentSession?.mode || 'agentic';
  const modelName = params.modelName || parentSession?.config?.modelName || 'default';
  const childSessionName = params.childSessionName.trim() || 'Side thread';
  const remoteConnectionId = parentSession?.remoteConnectionId;
  const remoteSshHost = parentSession?.remoteSshHost;

  const created = await agentAPI.createSession({
    sessionName: childSessionName,
    agentType,
    workspacePath,
    remoteConnectionId,
    remoteSshHost,
    config: {
      modelName,
      enableTools: params.enableTools ?? false,
      safeMode: params.safeMode ?? true,
      autoCompact: params.autoCompact ?? true,
      enableContextCompression: params.enableContextCompression ?? true,
      remoteConnectionId,
      remoteSshHost,
    },
  });

  const childSessionId = created.sessionId;
  flowChatStore.addExternalSession(
    childSessionId,
    childSessionName,
    agentType,
    workspacePath,
    {
      parentSessionId,
      sessionKind: childSessionKind,
      btwOrigin: {
        requestId,
        parentSessionId,
        parentDialogTurnId,
        parentTurnIndex,
      },
    },
    remoteConnectionId,
    remoteSshHost
  );
  flowChatStore.updateSessionRelationship(childSessionId, {
    parentSessionId,
    sessionKind: childSessionKind,
  });
  flowChatStore.updateSessionBtwOrigin(childSessionId, {
    requestId,
    parentSessionId,
    parentDialogTurnId,
    parentTurnIndex,
  }, childSessionKind);

  if (params.addMarker ?? false) {
    flowChatStore.addBtwThreadMarker(parentSessionId, {
      requestId,
      childSessionId,
      title: childSessionName,
      status: 'running',
      createdAt,
      parentDialogTurnId,
      parentTurnIndex,
    });
  }

  const meta = await loadSessionMetadataWithRetry(
    childSessionId,
    workspacePath,
    undefined,
    remoteConnectionId
  );
  if (meta) {
    const childSession = flowChatStore.getState().sessions.get(childSessionId);

    if (childSession) {
      await sessionAPI.saveSessionMetadata(
        buildSessionMetadata(childSession, meta),
        workspacePath,
        remoteConnectionId
      );
    }
  }

  return {
    requestId,
    childSessionId,
    parentDialogTurnId,
    parentTurnIndex,
  };
}

export async function startBtwThread(params: {
  parentSessionId: string;
  workspacePath: string;
  question: string;
  modelId?: string;
  maxContextMessages?: number;
}): Promise<{ requestId: string; childSessionId: string }> {
  const { parentSessionId, workspacePath } = params;
  const question = params.question.trim();
  if (!question) {
    notificationService.warning('Please provide a question after /btw');
    throw new Error('Empty /btw question');
  }

  const childSessionName = buildChildSessionName(question);
  const {
    requestId,
    childSessionId,
    parentDialogTurnId,
    parentTurnIndex,
  } = await createBtwChildSession({
    parentSessionId,
    workspacePath,
    childSessionName,
    requestId: safeUuid('btw'),
    enableTools: false,
    safeMode: true,
    autoCompact: true,
    enableContextCompression: true,
    addMarker: true,
  });

  // Insert a lightweight in-stream marker into the parent flow chat, and split the
  // currently streaming text so subsequent chunks continue after the marker.
  try {
    flowChatManager.insertBtwMarkerIntoActiveStream({
      parentSessionId,
      requestId,
      childSessionId,
      title: childSessionName,
    });
  } catch (e) {
    log.warn('Failed to insert /btw marker into parent stream', { parentSessionId, e });
  }

  // Seed an in-memory dialog turn so the child session is not empty when opened.
  // Persistence is handled on completion as a backup for reloads.
  const childTurnId = `btw-turn-${requestId}`;
  const childRoundId = `btw-round-${requestId}`;
  const childTextId = `btw-text-${requestId}`;
  const childNow = Date.now();

  const textItem: FlowTextItem = {
    id: childTextId,
    type: 'text',
    content: '',
    isStreaming: true,
    isMarkdown: true,
    timestamp: childNow,
    status: 'streaming',
  };

  const round: ModelRound = {
    id: childRoundId,
    index: 0,
    items: [textItem],
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: childNow,
  };

  const childTurn: DialogTurn = {
    id: childTurnId,
    sessionId: childSessionId,
    userMessage: {
      id: `btw-user-${requestId}`,
      content: question,
      timestamp: childNow,
    },
    modelRounds: [round],
    status: 'processing' as const,
    startTime: childNow,
    backendTurnIndex: 0,
  };

  flowChatStore.addDialogTurn(childSessionId, childTurn);

  let answerAcc = '';

  const unlistenChunk = api.listen<import('@/infrastructure/api/service-api/BtwAPI').BtwTextChunkEvent>(
    'btw://text-chunk',
    (evt) => {
      if (evt.requestId !== requestId) return;
      if (!evt.text) return;
      answerAcc += evt.text;

      // Update child session live.
      flowChatStore.updateModelRoundItem(childSessionId, childTurnId, childTextId, {
        content: answerAcc,
        isStreaming: true,
        status: 'streaming',
      } as any);
    }
  );

  let unlistenCompleted: (() => void) | null = null;
  let unlistenError: (() => void) | null = null;

  const cleanup = () => {
    try { unlistenChunk(); } catch {}
    try { unlistenCompleted?.(); } catch {}
    try { unlistenError?.(); } catch {}
  };

  unlistenCompleted = api.listen<import('@/infrastructure/api/service-api/BtwAPI').BtwCompletedEvent>(
    'btw://completed',
    async (evt) => {
      if (evt.requestId !== requestId) return;
      const fullText = (evt.fullText && evt.fullText.length >= answerAcc.length) ? evt.fullText : answerAcc;
      answerAcc = fullText;

      const completedAt = Date.now();

      // Finalize child session live.
      flowChatStore.updateDialogTurn(childSessionId, childTurnId, (turn) => {
        const updatedRounds = turn.modelRounds.map(r => {
          if (r.id !== childRoundId) return r;
          const updatedItems = r.items.map(it => {
            if (it.id !== childTextId) return it as any;
            return { ...(it as any), content: fullText, isStreaming: false, status: 'completed' } as any;
          });
          return {
            ...r,
            items: updatedItems,
            isStreaming: false,
            isComplete: true,
            status: 'completed' as const,
            endTime: completedAt,
          };
        });
        return {
          ...turn,
          modelRounds: updatedRounds,
          status: 'completed' as const,
          endTime: completedAt,
        };
      });
      flowChatStore.markSessionFinished(childSessionId, completedAt);

      flowChatStore.updateBtwThreadMarker(parentSessionId, requestId, { status: 'done' });

      cleanup();

    }
  );

  unlistenError = api.listen<import('@/infrastructure/api/service-api/BtwAPI').BtwErrorEvent>(
    'btw://error',
    (evt) => {
      if (evt.requestId !== requestId) return;
      cleanup();

      const failedAt = Date.now();
      flowChatStore.updateDialogTurn(childSessionId, childTurnId, (turn) => ({
        ...turn,
        status: 'error' as const,
        endTime: failedAt,
        error: evt.error || 'Unknown error',
      }));
      flowChatStore.updateModelRoundItem(childSessionId, childTurnId, childTextId, {
        isStreaming: false,
        status: 'error',
      } as any);

      flowChatStore.updateBtwThreadMarker(parentSessionId, requestId, {
        status: 'error',
        error: evt.error || 'Unknown error',
      });
      notificationService.error(evt.error || 'BTW failed');
    }
  );

  // Kick off streaming after listeners are ready.
  try {
    await btwAPI.askStream({
      requestId,
      sessionId: parentSessionId,
      childSessionId,
      workspacePath,
      question,
      modelId: params.modelId ?? 'fast',
      maxContextMessages: params.maxContextMessages ?? 60,
      parentDialogTurnId,
      parentTurnIndex,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    cleanup();
    flowChatStore.updateBtwThreadMarker(parentSessionId, requestId, { status: 'error', error: msg });
    flowChatStore.updateDialogTurn(childSessionId, childTurnId, (turn) => ({
      ...turn,
      status: 'error' as const,
      endTime: Date.now(),
      error: msg,
    }));
    flowChatStore.updateModelRoundItem(childSessionId, childTurnId, childTextId, {
      isStreaming: false,
      status: 'error',
    } as any);
    throw e;
  }

  return { requestId, childSessionId };
}
