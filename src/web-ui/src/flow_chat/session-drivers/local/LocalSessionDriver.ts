/**
 * Local session driver: the default flavor backed by this machine's (or the
 * attached peer's) agent runtime via `agentAPI`.
 *
 * Peer Device Mode is invisible here by design: it swaps the transport
 * underneath `api.invoke`, so this driver must never consult it. Session
 * creation sends only an explicit model selection and projects the Runtime's
 * authoritative resolved model from the response.
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { worktreeAPI } from '@/infrastructure/api/service-api/WorktreeAPI';
import { createLogger } from '@/shared/utils/logger';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import type { FlowChatContext, SessionConfig, DialogTurn } from '../../services/flow-chat-manager/types';
import type {
  SessionCascadeRemoval,
  SessionCreationSeed,
  SessionDriver,
  StartTurnInput,
  StartTurnResult,
  SubmissionPlan,
  TurnTracker,
  UsageReportUiParams,
} from '../types';
import {
  getModelMaxTokens,
  resolveReasoningPresetForSessionCreation,
} from '../../utils/modelResolution';
import { syncSessionModelSelection } from '../../utils/modelSync';
import { markCurrentTurnItemsAsCancelled } from '../../utils/turnCancellation';
import {
  requireSessionProjectWorkspacePath,
  sessionProjectWorkspacePath,
} from '../../utils/sessionWorkspace';
import { sessionWorktreeMaterializationPlan } from '../../utils/sessionWorktree';
import { cleanupSaveState } from '../../services/flow-chat-manager/PersistenceModule';
import { cleanupSessionBuffers } from '../../services/flow-chat-manager/TextChunkModule';
import { applyGeneratingTitlePlaceholder } from '../shared';

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

    const explicitModelName = config.modelName?.trim() || undefined;
    const reasoningPreset = config.reasoningPreset
      ?? (explicitModelName
        ? await resolveReasoningPresetForSessionCreation(explicitModelName)
        : undefined);

    const response = await agentAPI.createSession({
      sessionName,
      agentType,
      workspacePath,
      projectWorkspacePath,
      executionTarget: config.executionTargetRequest,
      requestId: globalThis.crypto?.randomUUID?.() ?? `worktree-${Date.now()}-${Math.random()}`,
      workspaceId: workspaceId ?? config.workspaceId,
      remoteConnectionId,
      remoteSshHost,
      config: {
        modelName: explicitModelName,
        reasoningPreset,
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        enableContextCompression: true,
        remoteConnectionId,
        remoteSshHost,
      }
    });

    const sessionModelName = response.modelId ?? explicitModelName;
    const maxContextTokens = await getModelMaxTokens(sessionModelName, agentType);
    const mergedConfig: SessionConfig = {
      ...config,
      modelName: sessionModelName,
      reasoningPreset,
      workspaceId: workspaceId ?? config.workspaceId,
    };

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

  planSubmission(): SubmissionPlan {
    // Local sessions park messages while busy; the drain listener replays
    // them when the state machine returns to IDLE.
    return { kind: 'queue' };
  },

  async compactSession(context: FlowChatContext, sessionId: string): Promise<void> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session does not exist: ${sessionId}`);
    }
    await agentAPI.compactSession({
      sessionId,
      workspacePath: session.workspacePath,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    });
  },

  async runUsageReport(
    context: FlowChatContext,
    sessionId: string,
    uiParams: UsageReportUiParams,
  ): Promise<{ inserted: boolean }> {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session does not exist: ${sessionId}`);
    }
    const { runUsageReportCommand } = await import('../../services/usageReportService');
    const result = await runUsageReportCommand({ session, ...uiParams });
    return { inserted: result.inserted };
  },

  permissionRequestSource(): 'live' {
    return 'live';
  },

  async respondPermission(
    _sessionId: string,
    requestId: string,
    reply: 'once' | 'always' | 'reject',
    feedback?: string,
  ): Promise<void> {
    await agentAPI.respondPermission(requestId, reply, feedback);
  },

  async respondPermissionBatch(
    _sessionId: string,
    requestId: string,
    reply: 'once' | 'always' | 'reject',
    feedback?: string,
  ): Promise<string[]> {
    return agentAPI.respondPermissionBatch(requestId, reply, feedback);
  },

  async steer(): Promise<void> {
    throw new Error('Local sessions queue messages while a turn is running');
  },

  async startTurn(
    context: FlowChatContext,
    input: StartTurnInput,
    tracker: TurnTracker,
  ): Promise<StartTurnResult> {
    const {
      sessionId,
      message,
      displayMessage,
      currentAgentType,
      acpClientId,
      isFirstMessage,
      readySession,
      options,
    } = input;

    const dialogTurnId = options?.turnId?.trim() ||
      `dialog_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const hasImages = (options?.imageContexts?.length ?? 0) > 0;

    const dialogTurn: DialogTurn = {
      id: dialogTurnId,
      sessionId: sessionId,
      agentType: currentAgentType,
      userMessage: {
        id: `user_${Date.now()}`,
        content: displayMessage || message,
        timestamp: Date.now(),
        hasImages,
        images: options?.imageDisplayData,
        metadata: options?.userMessageMetadata,
      },
      modelRounds: [],
      // Images are attached for multimodal primary models or reduced to text placeholders for text-only models.
      // We don't run a separate frontend "image pre-analysis" phase here.
      status: 'pending',
      startTime: Date.now()
    };

    context.flowChatStore.addDialogTurn(sessionId, dialogTurn);
    tracker.createdLocalTurnId = dialogTurnId;
    const isRestoringHistoricalSession =
      readySession.isHistorical || context.pendingHistoryLoads.has(sessionId);
    if (isRestoringHistoricalSession) {
      context.processingManager.clearSessionStatus(sessionId);
      context.flowChatStore.deleteDialogTurn(sessionId, dialogTurnId);
      throw new Error('Session history is still restoring, please retry once loading finishes');
    }

    const startOk = await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
      taskId: sessionId,
      dialogTurnId,
    });
    if (!startOk) {
      const currentState = stateMachineManager.getCurrentState(sessionId);
      throw new Error(`Session is still busy finishing the previous turn (current state: ${currentState})`);
    }

    context.processingManager.registerStatus({
      sessionId: sessionId,
      status: 'thinking',
      message: '',
      metadata: { sessionId: sessionId, dialogTurnId }
    });

    if (readySession.config.worktreeIsolationRequested !== undefined) {
      const materialization = sessionWorktreeMaterializationPlan(readySession);
      if (materialization) {
        log.info('Materializing requested worktree after prompt submission', {
          sessionId,
          enabled: materialization.enabled,
          projectWorkspacePath: materialization.projectWorkspacePath,
        });
        const result = await worktreeAPI.bindSession(
          sessionId,
          materialization.enabled,
          globalThis.crypto?.randomUUID?.() ?? `worktree-first-turn-${Date.now()}`,
          materialization.projectWorkspacePath,
        );
        context.flowChatStore.updateSessionExecutionTarget(sessionId, {
          workspacePath: result.workspacePath,
          projectWorkspacePath: result.projectWorkspacePath,
          workspaceId: result.workspaceId,
          executionTarget: result.executionTarget,
        });
        if (result.retainedWorktreePath) {
          log.warn('Released worktree retained because it contains local work', {
            sessionId,
            retainedWorktreePath: result.retainedWorktreePath,
          });
        }
      }
      context.flowChatStore.setSessionWorktreeIsolationRequested(sessionId, undefined);
    }

    if (isFirstMessage) {
      applyGeneratingTitlePlaceholder(context, sessionId, message);
    }

    if (!acpClientId) {
      await syncSessionModelSelection(context, sessionId, currentAgentType);
    }

    const updatedSession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!updatedSession) {
      throw new Error(`Session lost after adding dialog turn: ${sessionId}`);
    }

    context.contentBuffers.set(sessionId, new Map());
    context.activeTextItems.set(sessionId, new Map());

    const workspacePath = updatedSession.workspacePath;
    const projectWorkspacePath = sessionProjectWorkspacePath(updatedSession);

    if (acpClientId) {
      await ACPClientAPI.startDialogTurn({
        sessionId,
        clientId: acpClientId,
        userInput: message,
        originalUserInput: displayMessage || message,
        turnId: dialogTurnId,
        workspacePath,
        imageContexts: options?.imageContexts,
        userMessageMetadata: options?.userMessageMetadata,
        remoteConnectionId: updatedSession.remoteConnectionId,
        remoteSshHost: updatedSession.remoteSshHost,
      });
      context.flowChatStore.updateSessionLastSubmittedMode(sessionId, currentAgentType);
    } else {
      try {
        await agentAPI.startDialogTurn({
          sessionId: sessionId,
          userInput: message,
          originalUserInput: displayMessage || message,
          turnId: dialogTurnId,
          agentType: currentAgentType,
          workspacePath,
          projectWorkspacePath,
          remoteConnectionId: updatedSession.remoteConnectionId,
          remoteSshHost: updatedSession.remoteSshHost,
          imageContexts: options?.imageContexts,
          userMessageMetadata: options?.userMessageMetadata,
          execution: options?.execution,
        });
        context.flowChatStore.updateSessionLastSubmittedMode(sessionId, currentAgentType);
      } catch (error: any) {
        if (error?.message?.includes('Session does not exist') || error?.message?.includes('Not found')) {
          log.warn('Backend session still not found, retrying creation', {
            sessionId: sessionId,
            dialogTurnsCount: updatedSession.dialogTurns.length
          });

          // Lazy import: SessionModule routes lifecycle calls through the
          // driver registry, so a static import would create a module cycle.
          const { retryCreateBackendSession } =
            await import('../../services/flow-chat-manager/SessionModule');
          await retryCreateBackendSession(context, sessionId);

          await agentAPI.startDialogTurn({
            sessionId: sessionId,
            userInput: message,
            originalUserInput: displayMessage || message,
            turnId: dialogTurnId,
            agentType: currentAgentType,
            workspacePath,
            projectWorkspacePath,
            remoteConnectionId: updatedSession.remoteConnectionId,
            remoteSshHost: updatedSession.remoteSshHost,
            imageContexts: options?.imageContexts,
            userMessageMetadata: options?.userMessageMetadata,
            execution: options?.execution,
          });
          context.flowChatStore.updateSessionLastSubmittedMode(sessionId, currentAgentType);
        } else {
          throw error;
        }
      }
    }

    const sessionStateMachine = stateMachineManager.get(sessionId);
    if (sessionStateMachine) {
      sessionStateMachine.getContext().taskId = sessionId;
    }
    return 'completed';
  },
};
