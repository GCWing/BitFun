import { agentAPI, btwAPI, sessionAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine';
import { flowChatManager } from './FlowChatManager';
import type { Session } from '../types/flow-chat';
import type { SessionKind } from '@/shared/types/session-history';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';
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

function requireSession(sessionId: string): Session {
  const session = flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session not found: ${sessionId}`);
  }
  return session;
}

export function isTransientBtwSession(session: Session | undefined): boolean {
  return session?.isTransient === true && session.sessionKind === 'btw';
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
  deepReviewRunManifest?: ReviewTeamRunManifest;
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
      deepReviewRunManifest: params.deepReviewRunManifest,
      isTransient: false,
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

export function createTransientBtwSession(params: {
  parentSessionId: string;
  workspacePath?: string;
  childSessionName: string;
}): { childSessionId: string } {
  const parentSession = requireSession(params.parentSessionId);
  const workspacePath = params.workspacePath || parentSession.workspacePath;
  if (!workspacePath) {
    throw new Error(`Workspace path is required for BTW child session: ${params.parentSessionId}`);
  }

  const childSessionId = safeUuid('btw_session');
  const childSessionName = params.childSessionName.trim() || 'Side thread';

  flowChatStore.addExternalSession(
    childSessionId,
    childSessionName,
    parentSession.mode || 'agentic',
    workspacePath,
    {
      parentSessionId: params.parentSessionId,
      sessionKind: 'btw',
      btwOrigin: {
        parentSessionId: params.parentSessionId,
      },
      isTransient: true,
    },
    parentSession.remoteConnectionId,
    parentSession.remoteSshHost
  );

  return { childSessionId };
}

export async function sendMessageToTransientBtwSession(params: {
  parentSessionId: string;
  childSessionId: string;
  question: string;
  childSessionName?: string;
  modelId?: string;
}): Promise<{ requestId: string }> {
  const question = params.question.trim();
  if (!question) {
    notificationService.warning('Please provide a question after /btw');
    throw new Error('Empty /btw question');
  }

  const childSession = requireSession(params.childSessionId);
  if (!isTransientBtwSession(childSession)) {
    throw new Error(`Session is not a transient /btw session: ${params.childSessionId}`);
  }

  const requestId = safeUuid('btw');
  await btwAPI.askStream({
    requestId,
    sessionId: params.parentSessionId,
    childSessionId: params.childSessionId,
    childSessionName: params.childSessionName || childSession.title || 'Side thread',
    question,
    modelId: params.modelId ?? childSession.config.modelName ?? 'fast',
  });
  if (params.modelId?.trim()) {
    flowChatStore.updateSessionModelName(params.childSessionId, params.modelId.trim());
  }

  return { requestId };
}

export async function startBtwThread(params: {
  parentSessionId: string;
  workspacePath: string;
  question: string;
  modelId?: string;
}): Promise<{ requestId: string; childSessionId: string }> {
  const question = params.question.trim();
  if (!question) {
    notificationService.warning('Please provide a question after /btw');
    throw new Error('Empty /btw question');
  }

  const childSessionName = buildChildSessionName(question);
  const { childSessionId } = createTransientBtwSession({
    parentSessionId: params.parentSessionId,
    workspacePath: params.workspacePath,
    childSessionName,
  });

  try {
    const { requestId } = await sendMessageToTransientBtwSession({
      parentSessionId: params.parentSessionId,
      childSessionId,
      question,
      childSessionName,
      modelId: params.modelId,
    });
    return { requestId, childSessionId };
  } catch (error) {
    flowChatManager.discardLocalSession(childSessionId);
    throw error;
  }
}
