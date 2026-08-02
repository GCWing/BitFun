/**
 * Dispatch session driver: controller-side observer projections of jobs that
 * execute on a detached target (SSH host or paired device).
 *
 * The projection contract (see `features/dispatch/README.md`): the target CLI
 * owns the durable session and event log; this controller owns only the
 * outbound observer index and a rendered transcript cache. Nothing here may
 * call `agentAPI.createSession`, `start_dialog_turn`, restore, or local
 * session persistence for a projection.
 */

import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, SessionConfig } from '../../services/flow-chat-manager/types';
import type { Session } from '../../types/flow-chat';
import type { SessionCascadeRemoval, SessionCreationSeed, SessionDriver } from '../types';
import { isNonLocalDispatchTarget, type DispatchTarget } from '@/features/dispatch/types';
import { dispatchApi } from '@/features/dispatch/dispatchApi';
import { dispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import { forgetDispatchTranscript } from '@/features/dispatch/dispatchTranscriptCache';
import { requestDispatchJobRefresh } from '@/features/dispatch/DispatchJobObserver';
import { cleanupSaveState } from '../../services/flow-chat-manager/PersistenceModule';
import { cleanupSessionBuffers } from '../../services/flow-chat-manager/TextChunkModule';

const log = createLogger('DispatchSessionDriver');

/**
 * Fixed context budget for projections: no controller-side provider is ever
 * resolved for them, so there is no model to read a real window from.
 */
const DISPATCH_OBSERVER_MAX_CONTEXT_TOKENS = 128128;

function dismissDispatchObserverProjection(
  sessionId: string,
  session: Session | undefined,
): void {
  // Collect before dismissing: dismissSession removes the store entries that
  // are the only remaining link from this session to its cached transcripts.
  const jobIds = new Set(
    Object.values(dispatchJobStore.getState().jobs)
      .filter(job => job.sessionId === sessionId)
      .map(job => job.jobId),
  );
  const configuredJobId = session?.config.dispatchJobId?.trim();
  if (configuredJobId) {
    jobIds.add(configuredJobId);
  }

  dispatchJobStore.getState().dismissSession(
    sessionId,
    session?.config.dispatchJobId,
  );

  // The projection is gone for good, so its cached transcript must not stay
  // readable on disk until retention gets around to it.
  jobIds.forEach(jobId => {
    void forgetDispatchTranscript(jobId);
  });
}

function removeProjectionLocally(
  context: FlowChatContext,
  sessionId: string,
  removal: SessionCascadeRemoval,
): void {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  dismissDispatchObserverProjection(sessionId, session);
  context.flowChatStore.removeSession(
    sessionId,
    removal.removedActiveSession ? { nextActiveSessionId: null } : undefined,
  );
  removal.removedSessionIds.forEach(id => {
    context.processingManager.clearSessionStatus(id);
    cleanupSaveState(context, id);
    cleanupSessionBuffers(context, id);
  });
}

export const dispatchSessionDriver: SessionDriver = {
  id: 'dispatch',

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

    if (!isNonLocalDispatchTarget(config.dispatchTargetRequest)) {
      throw new Error('Dispatch driver requires a non-local dispatch target request');
    }
    const dispatchTarget: DispatchTarget = config.dispatchTarget
      ?? (
        config.dispatchTargetRequest.kind === 'ssh'
          ? {
              ...config.dispatchTargetRequest,
              displayName: config.dispatchTargetRequest.connectionId,
            }
          : {
              ...config.dispatchTargetRequest,
              displayName: config.dispatchTargetRequest.deviceId,
            }
      );
    const sessionId =
      globalThis.crypto?.randomUUID?.()
      ?? `dispatch-session-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const jobId =
      config.dispatchJobId?.trim()
      || `dispatch-${globalThis.crypto?.randomUUID?.()
        ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`}`;
    const approvalPolicy = config.dispatchApprovalPolicy;
    if (!approvalPolicy) {
      throw new Error('Dispatch approval policy must be selected before creating a session');
    }
    const resolvedConfig: SessionConfig = {
      ...config,
      // A dispatch projection must not inherit or resolve a controller-side
      // provider. The target selection, when explicit, lives in dispatchModel.
      modelName: undefined,
      workspaceId: workspaceId ?? config.workspaceId,
      workspacePath,
      projectWorkspacePath,
      dispatchTargetRequest: config.dispatchTargetRequest,
      dispatchTarget,
      dispatchJobId: jobId,
      dispatchApprovalPolicy: approvalPolicy,
      dispatchIncludeUncommitted: config.dispatchIncludeUncommitted ?? false,
      dispatchBaseRef: config.dispatchBaseRef?.trim() || 'HEAD',
      dispatchJobState: 'submitting',
      dispatchCursor: 0,
    };

    // This is an observer projection only. In particular, do not call
    // agentAPI.createSession: the target CLI owns the durable session.
    context.flowChatStore.createSession(
      sessionId,
      resolvedConfig,
      undefined,
      sessionName,
      DISPATCH_OBSERVER_MAX_CONTEXT_TOKENS,
      agentType,
      workspacePath,
      remoteConnectionId,
      remoteSshHost,
      titleDescriptor,
    );
    dispatchJobStore.getState().registerJob({
      jobId,
      sessionId,
      targetRequest: config.dispatchTargetRequest,
      target: dispatchTarget,
      sourceWorkspacePath: workspacePath,
      sourceWorkspaceId: resolvedConfig.workspaceId,
      title: sessionName,
      agentType,
      approvalPolicy,
      // Do not inherit the controller's model selector. An omitted target
      // model lets the probed target use its own configured default.
      model: config.dispatchModel?.trim() || undefined,
      availableModels: config.dispatchAvailableModels,
      defaultModel: config.dispatchDefaultModel,
      cursor: 0,
      state: 'submitting',
      appliedEventIds: [],
      pendingPermissions: [],
      eventLogComplete: true,
      historyTruncated: false,
      omittedEventCount: 0,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    });
    return sessionId;
  },

  async deleteSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    const observerJobIds = Object.values(dispatchJobStore.getState().jobs)
      .filter(job => job.sessionId === sessionId)
      .map(job => job.jobId);
    log.info('Dispatch diagnostic: projection delete evaluated', {
      sessionId,
      sessionFound: Boolean(session),
      dispatchTargetKind: session?.config.dispatchTarget?.kind,
      dispatchJobId: session?.config.dispatchJobId,
      observerJobIds,
      cascadeSessionIds: removal.removedSessionIds,
    });
    removeProjectionLocally(context, sessionId, removal);
    log.info('Dispatch diagnostic: projection removed from flow chat store', {
      sessionId,
      activeSessionId: context.flowChatStore.getState().activeSessionId,
    });
  },

  async archiveSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void> {
    // Archiving an observer projection is a local dismiss: the target keeps
    // its durable session; the tombstone stops reconciliation reviving it.
    removeProjectionLocally(context, sessionId, removal);
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
    await context.flowChatStore.updateSessionTitle(sessionId, title, 'generated');
    if (session.config.dispatchJobId) {
      dispatchJobStore.getState().updateTitle(session.config.dispatchJobId, title);
    }
    return title;
  },

  async ensureReady(): Promise<void> {
    // Nothing to prepare: the target owns the durable session, and submission
    // itself is what creates the job.
  },

  async cancel(context: FlowChatContext, sessionId: string): Promise<boolean> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    const jobId = session?.config.dispatchJobId;
    if (!jobId) {
      return false;
    }
    const response = await dispatchApi.cancel(jobId);
    if (response.cancelled) {
      context.userCancelledSessionIds.add(sessionId);
      requestDispatchJobRefresh(jobId);
    }
    return response.cancelled;
  },
};
