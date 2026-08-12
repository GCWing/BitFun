/**
 * Message handling module
 * Shared submission choreography: busy-gate planning, queueing, mode
 * switching, conflict retries, and the error path. Flavor-specific transport
 * work (optimistic turns, dialog-turn start, steering) lives in the session
 * drivers.
 */

import { notificationService } from '../../../shared/notification-system';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext } from './types';
import { isProjectedSessionEmpty } from '../../utils/flowChatTurnIdentity';
import type { ImageContextData as ImageInputContextData } from '@/infrastructure/api/service-api/ImageContextTypes';
import { pendingQueueManager } from './PendingQueueModule';
import { isSessionInUseError } from '@/infrastructure/api/errors/TauriCommandError';
import { i18nService } from '@/infrastructure/i18n';
import { driverForSession } from '../../session-drivers/registry';
import type { SendMessageOptions, SubmissionDraft, TurnTracker } from '../../session-drivers/types';

export { syncSessionModelSelection } from '../../utils/modelSync';
export { markCurrentTurnItemsAsCancelled } from '../../utils/turnCancellation';

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
  options?: SendMessageOptions
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  const sendAttempt = beginSessionSend(sessionId);
  const draft: SubmissionDraft = {
    message,
    displayMessage,
    imageContexts: options?.imageContexts,
  };

  if (!options?.bypassPendingQueue) {
    const machineState = stateMachineManager.getCurrentState(sessionId);
    const sessionBusy =
      machineState === SessionExecutionState.PROCESSING ||
      machineState === SessionExecutionState.FINISHING;
    const hasPendingQueue = pendingQueueManager.list(sessionId).length > 0;

    if (sessionBusy || hasPendingQueue) {
      if (options?.execution?.kind === 'fresh_external_subagent') {
        throw new Error('External subagent command delegation requires an idle session');
      }
      // Steer-eligibility must be decided before queueing: a steerable
      // message that gets queued never drains for flavors that do not drive
      // the local state machine.
      const plan = driverForSession(sessionId, session)
        .planSubmission(context, sessionId, draft);
      if (plan.kind === 'reject') {
        throw new Error(plan.reason);
      }
      if (plan.kind === 'steer') {
        await driverForSession(sessionId, session).steer(context, sessionId, draft);
        return;
      }
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

  const turnTracker: TurnTracker = { createdLocalTurnId: null };

  try {
    const refreshedSession = context.flowChatStore.getState().sessions.get(sessionId) ?? session;
    const currentAgentType = (agentType?.trim() || refreshedSession.mode || 'agentic').trim();
    const acpClientId = acpClientIdFromMode(currentAgentType);
    const driver = driverForSession(sessionId, refreshedSession);
    if (
      options?.execution?.kind === 'fresh_external_subagent'
      && (acpClientId || driver.id !== 'local')
    ) {
      throw new Error('External subagent command delegation requires the local BitFun runtime');
    }

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

    if (!acpClientId) {
      // A driver with nothing to prepare returns void; awaiting only real
      // promises keeps the projection's optimistic turn synchronous with the
      // user's send action.
      const readiness = driver.ensureReady(context, sessionId);
      if (readiness) {
        await readiness;
      }
    }

    const readySession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!readySession) {
      throw new Error(`Session lost before starting dialog turn: ${sessionId}`);
    }

    const isFirstMessage = isProjectedSessionEmpty(readySession)
      && readySession.titleStatus !== 'generated';

    const outcome = await driver.startTurn(
      context,
      {
        sessionId,
        message,
        displayMessage,
        currentAgentType,
        acpClientId,
        isFirstMessage,
        readySession,
        options,
      },
      turnTracker,
    );
    if (outcome === 'detached') {
      // The message steered or continued target-owned work; the shared
      // post-submission bookkeeping does not apply.
      return;
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
      turnTracker.createdLocalTurnId !== null &&
      activeDialogTurnId === turnTracker.createdLocalTurnId;
    if (currentState === SessionExecutionState.PROCESSING && ownsProcessingTurn) {
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.ERROR_OCCURRED, {
        error: errorMessage
      });
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.RESET);
    }

    const state = context.flowChatStore.getState();
    const currentSession = state.sessions.get(sessionId);
    if (turnTracker.createdLocalTurnId && currentSession && !options?.preserveTurnOnStartError) {
      context.flowChatStore.deleteDialogTurn(sessionId, turnTracker.createdLocalTurnId);
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

export async function cancelSessionTask(context: FlowChatContext, requestedSessionId?: string): Promise<boolean> {
  try {
    const state = context.flowChatStore.getState();
    const sessionId = requestedSessionId || state.activeSessionId;

    if (!sessionId) {
      log.debug('No active session to cancel');
      return false;
    }

    const session = state.sessions.get(sessionId);
    return await driverForSession(sessionId, session).cancel(context, sessionId);
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
