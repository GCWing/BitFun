/**
 * Local session driver: the default flavor backed by this machine's (or the
 * attached peer's) agent runtime via `agentAPI`.
 *
 * Bodies were moved verbatim from SessionModule/MessageModule; behavior is
 * unchanged. Peer Device Mode is invisible here by design — it swaps the
 * transport underneath `api.invoke`, so this driver must never consult it.
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { createLogger } from '@/shared/utils/logger';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import type { FlowChatContext, SessionConfig } from '../../services/flow-chat-manager/types';
import type { SessionCascadeRemoval, SessionCreationSeed, SessionDriver } from '../types';
import {
  getModelMaxTokens,
  resolveModelForSessionCreation,
} from '../../utils/modelResolution';
import { markCurrentTurnItemsAsCancelled } from '../../utils/turnCancellation';
import {
  requireSessionProjectWorkspacePath,
  sessionProjectWorkspacePath,
} from '../../utils/sessionWorkspace';
import { cleanupSaveState } from '../../services/flow-chat-manager/PersistenceModule';
import { cleanupSessionBuffers } from '../../services/flow-chat-manager/TextChunkModule';

const log = createLogger('LocalSessionDriver');

export const localSessionDriver: SessionDriver = {
  id: 'local',

  async createSession(context: FlowChatContext, seed: SessionCreationSeed): Promise<string> {
    const {
      config,
      agentType,
      sessionName,
      titleDescriptor,
      workspacePath,
      projectWorkspacePath,
      workspaceId,
      remoteConnectionId,
      remoteSshHost,
    } = seed;

    const sessionModelName = await resolveModelForSessionCreation(config.modelName);
    const maxContextTokens = await getModelMaxTokens(sessionModelName, agentType);
    const mergedConfig: SessionConfig = {
      ...config,
      modelName: sessionModelName,
      workspaceId: workspaceId ?? config.workspaceId,
    };

    const response = await agentAPI.createSession({
      sessionName,
      agentType,
      workspacePath,
      projectWorkspacePath,
      executionTarget: config.executionTargetRequest,
      requestId: globalThis.crypto?.randomUUID?.() ?? `worktree-${Date.now()}-${Math.random()}`,
      workspaceId: mergedConfig.workspaceId,
      remoteConnectionId,
      remoteSshHost,
      config: {
        modelName: sessionModelName,
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        maxContextTokens: maxContextTokens,
        enableContextCompression: true,
        remoteConnectionId,
        remoteSshHost,
      }
    });

    const effectiveWorkspacePath =
      response.workspacePath || response.executionTarget?.rootPath || workspacePath;
    const effectiveProjectWorkspacePath =
      response.projectWorkspacePath || projectWorkspacePath || workspacePath;
    const resolvedConfig: SessionConfig = {
      ...mergedConfig,
      workspacePath: effectiveWorkspacePath,
      projectWorkspacePath: effectiveProjectWorkspacePath,
      workspaceId: response.workspaceId ?? mergedConfig.workspaceId,
      executionTarget: response.executionTarget,
    };

    context.flowChatStore.createSession(
      response.sessionId,
      resolvedConfig,
      undefined,
      sessionName,
      maxContextTokens,
      agentType,
      effectiveWorkspacePath,
      remoteConnectionId,
      remoteSshHost,
      titleDescriptor,
    );

    return response.sessionId;
  },

  async deleteSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    log.info('Dispatch diagnostic: delete routed to persisted backend session', {
      sessionId,
      hasWorkspacePath: Boolean(session && sessionProjectWorkspacePath(session)),
    });
    await context.flowChatStore.deleteSession(
      sessionId,
      removal.removedActiveSession ? { nextActiveSessionId: null } : undefined,
    );

    removal.removedSessionIds.forEach(id => {
      context.processingManager.clearSessionStatus(id);
      cleanupSaveState(context, id);
    });
  },

  async archiveSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session does not exist: ${sessionId}`);
    }

    await sessionAPI.archiveSession(
      sessionId,
      requireSessionProjectWorkspacePath(session, sessionId),
      session.remoteConnectionId,
      session.remoteSshHost,
    );

    context.flowChatStore.removeSession(
      sessionId,
      removal.removedActiveSession ? { nextActiveSessionId: null } : undefined,
    );

    removal.removedSessionIds.forEach(id => {
      stateMachineManager.delete(id);
      context.processingManager.clearSessionStatus(id);
      cleanupSaveState(context, id);
      cleanupSessionBuffers(context, id);
    });
  },

  async renameSession(
    context: FlowChatContext,
    sessionId: string,
    title: string,
  ): Promise<string> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session does not exist: ${sessionId}`);
    }
    const updatedTitle = await agentAPI.updateSessionTitle({
      sessionId,
      title,
      workspacePath: sessionProjectWorkspacePath(session),
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    });

    await context.flowChatStore.updateSessionTitle(sessionId, updatedTitle, 'generated');
    return updatedTitle;
  },

  async ensureReady(context: FlowChatContext, sessionId: string): Promise<void> {
    // Deliberate lazy import: SessionModule routes lifecycle calls through the
    // driver registry, so a static import here would create a module cycle.
    const { ensureBackendSession } = await import('../../services/flow-chat-manager/SessionModule');
    await ensureBackendSession(context, sessionId);
  },

  async cancel(context: FlowChatContext, sessionId: string): Promise<boolean> {
    const currentState = stateMachineManager.getCurrentState(sessionId);
    const success = currentState === SessionExecutionState.PROCESSING
      ? await stateMachineManager.transition(sessionId, SessionExecutionEvent.USER_CANCEL)
      : false;

    if (success) {
      context.userCancelledSessionIds.add(sessionId);
      markCurrentTurnItemsAsCancelled(context, sessionId);
      cleanupSessionBuffers(context, sessionId);
    }

    return success;
  },
};
