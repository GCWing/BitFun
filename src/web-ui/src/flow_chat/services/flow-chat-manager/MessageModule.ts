/**
 * Message handling module
 * Handles message sending, cancellation, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { worktreeAPI } from '@/infrastructure/api/service-api/WorktreeAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { AIModelConfig, AgentModelDefaultsConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { notificationService } from '../../../shared/notification-system';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { generateTempTitle } from '../../utils/titleUtils';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, DialogTurn } from './types';
import { ensureBackendSession, getModelMaxTokens, retryCreateBackendSession } from './SessionModule';
import { cleanupSessionBuffers } from './TextChunkModule';
import type { ImageContextData as ImageInputContextData } from '@/infrastructure/api/service-api/ImageContextTypes';
import { globalEventBus } from '@/infrastructure/event-bus';
import {
  FLOWCHAT_PIN_TURN_TO_TOP_EVENT,
  type FlowChatPinTurnToTopRequest,
} from '../../events/flowchatNavigation';
import { pendingQueueManager } from './PendingQueueModule';
import { sessionProjectWorkspacePath } from '../../utils/sessionWorkspace';
import { sessionWorktreeMaterializationPlan } from '../../utils/sessionWorktree';
import { dispatchApi } from '@/features/dispatch/dispatchApi';
import { dispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import { requestDispatchJobRefresh } from '@/features/dispatch/DispatchJobObserver';
import { isNonLocalDispatchTarget } from '@/features/dispatch/types';
import { isSessionInUseError } from '@/infrastructure/api/errors/TauriCommandError';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('MessageModule');

interface SessionConflictRetry {
  notificationId: string;
  active: boolean;
  inFlight: boolean;
}

const sessionConflictRetries = new Map<string, SessionConflictRetry>();
const latestSendBySession = new Map<string, symbol>();

function clearSessionConflictRetry(sessionId: string): void {
  const current = sessionConflictRetries.get(sessionId);
  if (!current) return;
  current.active = false;
  sessionConflictRetries.delete(sessionId);
  notificationService.dismiss(current.notificationId);
}

function beginSessionSend(sessionId: string): symbol {
  const attempt = Symbol(sessionId);
  latestSendBySession.set(sessionId, attempt);
  clearSessionConflictRetry(sessionId);
  return attempt;
}

function completeSessionSend(
  sessionId: string,
  attempt: symbol,
  retrySuccess?: () => void,
): void {
  if (latestSendBySession.get(sessionId) !== attempt) return;
  latestSendBySession.delete(sessionId);
  clearSessionConflictRetry(sessionId);
  retrySuccess?.();
}

function acpClientIdFromMode(mode: string | undefined): string | null {
  const value = mode?.trim();
  if (!value?.startsWith('acp:')) return null;
  const clientId = value.slice('acp:'.length).trim();
  return clientId || null;
}

function normalizeModelSelection(
  modelId: string | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): string {
  const value = modelId?.trim();
  if (!value || value === 'auto') return 'auto';

  if (value === 'primary' || value === 'fast') {
    const resolvedDefaultId = value === 'primary' ? defaultModels.primary : defaultModels.fast;
    const matchedModel = models.find(model => model.id === resolvedDefaultId);
    return matchedModel ? value : 'auto';
  }

  const matchedModel = models.find(model =>
    model.id === value || model.name === value || model.model_name === value,
  );
  return matchedModel ? value : 'auto';
}

export async function syncSessionModelSelection(
  context: FlowChatContext,
  sessionId: string,
  agentType: string,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }

  const sessionModelId = session.config.modelName?.trim();

  // Any stored selector, including "auto", belongs to the session. Still sync
  // it to the backend in case the restored runtime session lost that state.
  if (sessionModelId) {
    const desiredMaxContextTokens = await getModelMaxTokens(sessionModelId, agentType);
    if (session.maxContextTokens !== desiredMaxContextTokens) {
      context.flowChatStore.updateSessionMaxContextTokens(sessionId, desiredMaxContextTokens);
    }
    await agentAPI.updateSessionModel({
      sessionId,
      modelName: sessionModelId,
    });
    return;
  }

  const configData = await configManager.getConfigs([
    'ai.agent_model_defaults',
    'ai.models',
    'ai.default_models',
  ]);
  const agentModelDefaults = configData['ai.agent_model_defaults'] as AgentModelDefaultsConfig | undefined;
  const allModels = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
  const defaultModels = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};

  const desiredModelId = normalizeModelSelection(agentModelDefaults?.mode, allModels, defaultModels);
  const shouldForceAutoSync = desiredModelId === 'auto';
  const desiredMaxContextTokens = await getModelMaxTokens(desiredModelId, agentType);
  const shouldSyncContextWindow = session.maxContextTokens !== desiredMaxContextTokens;

  context.flowChatStore.updateSessionModelName(sessionId, desiredModelId);
  if (shouldSyncContextWindow) {
    context.flowChatStore.updateSessionMaxContextTokens(sessionId, desiredMaxContextTokens);
  }
  await agentAPI.updateSessionModel({
    sessionId,
    modelName: desiredModelId,
  });

  log.info('Session model synchronized before send', {
    sessionId,
    agentType,
    previousModelId: null,
    nextModelId: desiredModelId,
    forcedAutoSync: shouldForceAutoSync,
  });
}

/**
 * Send message and handle response
 * @param message - Message sent to backend
 * @param sessionId - Session ID
 * @param displayMessage - Optional, message for UI display
 * @param agentType - Agent type
 * @param switchToMode - Optional, switch UI mode selector to this mode (if not provided, mode remains unchanged)
 */
export async function sendMessage(
  context: FlowChatContext,
  message: string,
  sessionId: string,
  displayMessage?: string,
  agentType?: string,
  switchToMode?: string,
  options?: {
    imageContexts?: ImageInputContextData[];
    imageDisplayData?: Array<{ id: string; name: string; dataUrl?: string; imagePath?: string; mimeType?: string }>;
    /**
     * When true, bypass the pending-queue check. Used by the queue drain path
     * to actually start a new dialog turn after the previous one finished.
     * Callers should not set this directly.
     */
    bypassPendingQueue?: boolean;
    userMessageMetadata?: Record<string, unknown>;
    turnId?: string;
    preserveTurnOnStartError?: boolean;
    /** One-shot UI confirmation for unattended auto approval. Never persist this flag. */
    dispatchAutoConfirmed?: boolean;
    onSessionConflictRetryStart?: () => void;
    onSessionConflictRetrySuccess?: () => void;
    fromSessionConflictRetry?: boolean;
  }
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  const sendAttempt = beginSessionSend(sessionId);

  if (!options?.bypassPendingQueue) {
    const machineState = stateMachineManager.getCurrentState(sessionId);
    const sessionBusy =
      machineState === SessionExecutionState.PROCESSING ||
      machineState === SessionExecutionState.FINISHING;
    const hasPendingQueue = pendingQueueManager.list(sessionId).length > 0;

    if (sessionBusy || hasPendingQueue) {
      try {
        const item = pendingQueueManager.enqueue({
          sessionId,
          content: message,
          displayMessage,
          agentType,
          imageContexts: options?.imageContexts,
          imageDisplayData: options?.imageDisplayData,
          userMessageMetadata: options?.userMessageMetadata,
        });
        log.info('Message enqueued: session busy or queue non-empty', {
          sessionId,
          state: machineState,
          queuedItemId: item.id,
          queueDepth: pendingQueueManager.list(sessionId).length,
        });
      } catch (error) {
        const reason = error instanceof Error ? error.message : 'Failed to queue message';
        log.error('Failed to enqueue pending message', { sessionId, error });
        notificationService.error(reason, {
          title: 'Queue full',
          duration: 4000,
        });
        throw error;
      }
      completeSessionSend(
        sessionId,
        sendAttempt,
        options?.fromSessionConflictRetry
          ? options.onSessionConflictRetrySuccess
          : undefined,
      );
      return;
    }
  }

  // Switch UI mode if specified
  if (switchToMode && switchToMode !== session.mode) {
    context.flowChatStore.updateSessionMode(sessionId, switchToMode);
    window.dispatchEvent(new CustomEvent('bitfun:session-switched', {
      detail: { sessionId, mode: switchToMode }
    }));
  }

  let createdLocalTurnId: string | null = null;

  try {
    const refreshedSession = context.flowChatStore.getState().sessions.get(sessionId) ?? session;
    const currentAgentType = (agentType?.trim() || refreshedSession.mode || 'agentic').trim();
    const acpClientId = acpClientIdFromMode(currentAgentType);
    const isDispatched = isNonLocalDispatchTarget(refreshedSession.config.dispatchTarget);

    if (
      !acpClientId &&
      agentType?.trim() &&
      refreshedSession.mode !== currentAgentType
    ) {
      context.flowChatStore.updateSessionMode(sessionId, currentAgentType);
    }

    if (context.pendingHistoryLoads.has(sessionId)) {
      throw new Error('Session history is still restoring, please retry once loading finishes');
    }

    if (!acpClientId && !isDispatched) {
      await ensureBackendSession(context, sessionId);
    }

    const readySession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!readySession) {
      throw new Error(`Session lost before starting dialog turn: ${sessionId}`);
    }

    const isFirstMessage = readySession.dialogTurns.length === 0 && readySession.titleStatus !== 'generated';

    if (isDispatched) {
      const targetRequest = readySession.config.dispatchTargetRequest;
      const jobId = readySession.config.dispatchJobId;
      const approvalPolicy = readySession.config.dispatchApprovalPolicy;
      if (!targetRequest || targetRequest.kind === 'local' || !jobId || !approvalPolicy) {
        throw new Error('Dispatch session is missing its immutable target or approval policy');
      }
      if (targetRequest.kind !== 'ssh') {
        throw new Error('Phase-one dispatch supports SSH targets only');
      }
      if ((options?.imageContexts?.length ?? 0) > 0) {
        throw new Error('Image attachments are not supported for SSH dispatch yet');
      }
      if (readySession.dialogTurns.length > 0) {
        throw new Error('Phase-one dispatch sessions accept one detached task');
      }
      if (
        readySession.config.dispatchJobState !== 'submitting' &&
        readySession.config.dispatchJobState !== 'submission_unknown'
      ) {
        throw new Error('This detached dispatch job has already been submitted');
      }
      if (approvalPolicy === 'auto' && options?.dispatchAutoConfirmed !== true) {
        throw new Error('Auto-approval dispatch requires an explicit confirmation before submit');
      }
      if (isFirstMessage) {
        handleTitleGeneration(context, sessionId, message);
      }

      const response = await dispatchApi.submit({
        target: targetRequest,
        jobId,
        sessionId,
        agentType: currentAgentType,
        prompt: message,
        approvalPolicy,
        model: readySession.config.dispatchModel?.trim() || undefined,
      });
      if (!response.accepted || response.jobId !== jobId || response.sessionId !== sessionId) {
        throw new Error('Dispatch target returned a mismatched acknowledgement');
      }
      context.flowChatStore.applyDispatchSnapshot(sessionId, {
        jobId,
        state: response.state,
        cursor: readySession.config.dispatchCursor ?? 0,
        expectedCursor: readySession.config.dispatchCursor ?? 0,
      });
      dispatchJobStore.getState().updateProgress(jobId, {
        state: response.state,
      });
      context.flowChatStore.updateSessionLastSubmittedMode(sessionId, currentAgentType);
      requestDispatchJobRefresh(jobId);
      completeSessionSend(
        sessionId,
        sendAttempt,
        options?.fromSessionConflictRetry
          ? options.onSessionConflictRetrySuccess
          : undefined,
      );
      return;
    }

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
    createdLocalTurnId = dialogTurnId;
    const pinRequest: FlowChatPinTurnToTopRequest = {
      sessionId,
      turnId: dialogTurnId,
      behavior: 'auto',
      source: 'send-message',
      pinMode: 'sticky-latest',
    };
    globalEventBus.emit(FLOWCHAT_PIN_TURN_TO_TOP_EVENT, pinRequest, 'MessageModule');

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
      handleTitleGeneration(context, sessionId, message);
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
        });
        context.flowChatStore.updateSessionLastSubmittedMode(sessionId, currentAgentType);
      } catch (error: any) {
        if (error?.message?.includes('Session does not exist') || error?.message?.includes('Not found')) {
          log.warn('Backend session still not found, retrying creation', {
            sessionId: sessionId,
            dialogTurnsCount: updatedSession.dialogTurns.length
          });

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
    completeSessionSend(
      sessionId,
      sendAttempt,
      options?.fromSessionConflictRetry
        ? options.onSessionConflictRetrySuccess
        : undefined,
    );

  } catch (error) {
    log.error('Failed to send message', { sessionId: sessionId, error });
    
    const errorMessage = error instanceof Error ? error.message : 'Failed to send message';
    
    const currentState = stateMachineManager.getCurrentState(sessionId);
    const activeDialogTurnId = stateMachineManager
      .get(sessionId)
      ?.getContext().currentDialogTurnId;
    const ownsProcessingTurn =
      createdLocalTurnId !== null &&
      activeDialogTurnId === createdLocalTurnId;
    if (currentState === SessionExecutionState.PROCESSING && ownsProcessingTurn) {
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.ERROR_OCCURRED, {
        error: errorMessage
      });
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.RESET);
    }
    
    const state = context.flowChatStore.getState();
    const currentSession = state.sessions.get(sessionId);
    if (createdLocalTurnId && currentSession && !options?.preserveTurnOnStartError) {
      context.flowChatStore.deleteDialogTurn(sessionId, createdLocalTurnId);
    }
    
    if (!options?.preserveTurnOnStartError) {
      if (isSessionInUseError(error)) {
        if (latestSendBySession.get(sessionId) !== sendAttempt) {
          throw error;
        }
        clearSessionConflictRetry(sessionId);
        const retry: SessionConflictRetry = {
          notificationId: '',
          active: true,
          inFlight: false,
        };
        retry.notificationId = notificationService.error(
          i18nService.t('flow-chat:session.inUseMessage'), {
          title: i18nService.t('flow-chat:session.inUseTitle'),
          duration: 0,
          actions: [{
            label: i18nService.t('flow-chat:session.retry'),
            variant: 'primary',
            onClick: () => {
              if (
                !retry.active ||
                retry.inFlight ||
                sessionConflictRetries.get(sessionId) !== retry
              ) {
                return;
              }
              retry.inFlight = true;
              options?.onSessionConflictRetryStart?.();
              void sendMessage(
                context,
                message,
                sessionId,
                displayMessage,
                agentType,
                switchToMode,
                { ...options, fromSessionConflictRetry: true },
              )
                .catch(() => undefined);
            },
          }],
        });
        sessionConflictRetries.set(sessionId, retry);
      } else {
        if (latestSendBySession.get(sessionId) === sendAttempt) {
          latestSendBySession.delete(sessionId);
          notificationService.error(errorMessage, {
            title: 'Thinking process error',
            duration: 5000
          });
        }
      }
    } else if (latestSendBySession.get(sessionId) === sendAttempt) {
      latestSendBySession.delete(sessionId);
    }
    
    throw error;
  }
}

function handleTitleGeneration(
  context: FlowChatContext,
  sessionId: string,
  message: string
): void {
  const tempTitle = generateTempTitle(message, 20);
  // Show a readable placeholder immediately; backend later confirms the
  // authoritative title via AI or local fallback generation.
  context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generating');
}

export async function cancelSessionTask(context: FlowChatContext, requestedSessionId?: string): Promise<boolean> {
  try {
    const state = context.flowChatStore.getState();
    const sessionId = requestedSessionId || state.activeSessionId;
    
    if (!sessionId) {
      log.debug('No active session to cancel');
      return false;
    }

    const session = state.sessions.get(sessionId);
    if (isNonLocalDispatchTarget(session?.config.dispatchTarget)) {
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
    }

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
    
  } catch (error) {
    log.error('Failed to cancel current task', error);
    return false;
  }
}

export async function cancelCurrentTask(context: FlowChatContext): Promise<boolean> {
  return cancelSessionTask(context);
}

/**
 * Drain a single head item from the pending queue if the session is currently IDLE.
 * Called by the global state-machine subscriber after a turn completes.
 */
export async function drainPendingQueue(
  context: FlowChatContext,
  sessionId: string,
): Promise<void> {
  const machineState = stateMachineManager.getCurrentState(sessionId);
  if (machineState !== SessionExecutionState.IDLE) {
    return;
  }
  // Find the head item *that is still eligible for auto-drain*. Items with
  // `retryCount > 0` (e.g. restored from a failed turn) are deliberately
  // skipped here — the user must explicitly act on them to avoid re-entering
  // the same failure mode automatically.
  const allItems = pendingQueueManager.list(sessionId);
  const next = allItems.find(
    (item) => (item.retryCount ?? 0) === 0 && item.status === 'queued',
  );
  if (!next) return;

  // If there are blocking failed items in front of this one, also skip — the
  // user expects FIFO order, so we should not silently jump ahead of a failed
  // entry. Once they clear / send-now the failed entry, the listener will
  // re-fire on the next IDLE state event.
  const blockedByFailed = allItems
    .slice(0, allItems.indexOf(next))
    .some((item) => (item.retryCount ?? 0) > 0 || item.status === 'failed');
  if (blockedByFailed) {
    log.debug('Auto-drain blocked by a failed item ahead of head', {
      sessionId,
      pending: allItems.length,
    });
    return;
  }

  pendingQueueManager.setStatus(sessionId, next.id, 'sending');

  try {
    await sendMessage(
      context,
      next.content,
      sessionId,
      next.displayMessage,
      next.agentType,
      undefined,
      {
        imageContexts: next.imageContexts as ImageInputContextData[] | undefined,
        imageDisplayData: next.imageDisplayData as
          | Array<{
              id: string;
              name: string;
              dataUrl?: string;
              imagePath?: string;
              mimeType?: string;
            }>
          | undefined,
        userMessageMetadata: next.userMessageMetadata,
        bypassPendingQueue: true,
      },
    );
    // Only remove the item AFTER sendMessage completes successfully so we keep
    // the original id / timestamp / retryCount on failure (no UI flicker, no
    // reset of the retry counter, and FIFO order is preserved).
    pendingQueueManager.remove(sessionId, next.id);
  } catch (error) {
    log.error('Failed to drain pending queue item', { sessionId, itemId: next.id, error });
    // Mark in place. The auto-drain listener skips `failed` items so the user
    // can edit / send-now / delete without entering a tight retry loop.
    pendingQueueManager.setStatus(sessionId, next.id, 'failed');
  }
}

let queueDrainListenerInstalled = false;
let queueDrainContext: FlowChatContext | null = null;

/** Install (once) the state-machine listener that drains the queue when a session returns to IDLE. */
export function installPendingQueueDrainListener(context: FlowChatContext): void {
  queueDrainContext = context;
  if (queueDrainListenerInstalled) {
    return;
  }
  queueDrainListenerInstalled = true;
  stateMachineManager.subscribeGlobal((sessionId, machine) => {
    if (machine.currentState !== SessionExecutionState.IDLE) return;
    if (!queueDrainContext) return;
    if (pendingQueueManager.list(sessionId).length === 0) return;
    void drainPendingQueue(queueDrainContext, sessionId);
  });
}

export function markCurrentTurnItemsAsCancelled(
  context: FlowChatContext,
  sessionId: string
): void {
  const state = context.flowChatStore.getState();
  const session = state.sessions.get(sessionId);
  if (!session) return;
  
  const lastDialogTurn = session.dialogTurns[session.dialogTurns.length - 1];
  if (!lastDialogTurn) return;
  
  if (lastDialogTurn.status === 'completed' || lastDialogTurn.status === 'cancelled') {
    return;
  }
  
  lastDialogTurn.modelRounds.forEach(round => {
    round.items.forEach(item => {
      if (item.status === 'completed' || item.status === 'cancelled' || item.status === 'error') {
        return;
      }
      
      context.flowChatStore.updateModelRoundItem(sessionId, lastDialogTurn.id, item.id, {
        status: 'cancelled',
        ...(item.type === 'text' && { isStreaming: false }),
        ...(item.type === 'tool' && { 
          isParamsStreaming: false,
          endTime: Date.now()
        })
      } as any);
    });
  });
  
  context.flowChatStore.updateDialogTurn(sessionId, lastDialogTurn.id, turn => ({
    ...turn,
    status: 'cancelled',
    endTime: Date.now()
  }));
}
