import { agentAPI, btwAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine';
import { flowChatManager } from './FlowChatManager';
import type { Session } from '../types/flow-chat';
import type { SessionKind, SessionRelationship } from '@/shared/types/session-history';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';
import type { ImagePayload } from '../utils/imagePayload';

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
  return session?.isTransient === true && session.sessionKind === 'btw' && session.agentBackedTransient !== true;
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
  isTransient?: boolean;
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
  const shouldPersistStandaloneSession =
    !params.isTransient && childSessionKind !== 'btw';
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
  const relationship: SessionRelationship | undefined =
    childSessionKind === 'btw'
      ? undefined
      : {
          kind: childSessionKind,
          parentSessionId,
          parentRequestId: requestId,
          parentDialogTurnId,
          parentTurnIndex,
        };

  const childSessionId = shouldPersistStandaloneSession
    ? (
        await agentAPI.createSession({
          sessionName: childSessionName,
          agentType,
          workspacePath,
          workspaceId: parentSession?.workspaceId,
          remoteConnectionId,
          remoteSshHost,
          relationship,
          deepReviewRunManifest: params.deepReviewRunManifest,
          config: {
            modelName,
            enableTools: params.enableTools ?? false,
            safeMode: params.safeMode ?? true,
            autoCompact: params.autoCompact ?? true,
            enableContextCompression: params.enableContextCompression ?? true,
            remoteConnectionId,
            remoteSshHost,
          },
        })
      ).sessionId
    : safeUuid('btw_session');
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
      isTransient: params.isTransient ?? false,
      agentBackedTransient: params.isTransient ?? false,
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
      agentBackedTransient: false,
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
  imagePayload?: ImagePayload;
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
    imageContexts: params.imagePayload?.imageContexts,
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
  imagePayload?: ImagePayload;
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
      imagePayload: params.imagePayload,
    });
    return { requestId, childSessionId };
  } catch (error) {
    flowChatManager.discardLocalSession(childSessionId);
    throw error;
  }
}
