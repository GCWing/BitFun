import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import { createLogger } from '@/shared/utils/logger';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { i18nService } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { agenticEventListener } from '@/flow_chat/services/AgenticEventListener';
import type { FlowChatContext } from '@/flow_chat/services/flow-chat-manager/types';
import { clearRuntimeStatus } from '@/flow_chat/services/flow-chat-manager/RuntimeStatusModule';
import { clearRuntimeStatusState } from '@/flow_chat/store/runtimeStatusStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import {
  SessionExecutionEvent,
  SessionExecutionState,
} from '@/flow_chat/state-machine/types';
import { dispatchApi } from './dispatchApi';
import { dispatchJobStore, type DispatchObserverJob } from './dispatchJobStore';
import type {
  DispatchAgentEventEnvelope,
  DispatchEvent,
  DispatchJobState,
  DispatchStatusResponse,
} from './types';
import { isDispatchJobTerminal } from './types';

const log = createLogger('DispatchJobObserver');

export const DISPATCH_JOB_POLL_INTERVAL_MS = 1800;

type RefreshRequester = (jobId?: string) => void;

interface DispatchObserverLease {
  requestRefresh: RefreshRequester;
  dispose: () => void;
}

type DispatchObserverGlobal = typeof globalThis & {
  __bitfunDispatchJobObserverLease__?: DispatchObserverLease;
};

function getDispatchObserverGlobal(): DispatchObserverGlobal {
  return globalThis as DispatchObserverGlobal;
}

export function requestDispatchJobRefresh(jobId?: string): void {
  getDispatchObserverGlobal()
    .__bitfunDispatchJobObserverLease__
    ?.requestRefresh(jobId);
}

const RAW_EVENT_NAMES: Record<string, string> = {
  SessionCreated: 'agentic://session-created',
  SessionDeleted: 'agentic://session-deleted',
  SessionStateChanged: 'agentic://session-state-changed',
  SessionTitleGenerated: 'session_title_generated',
  ImageAnalysisStarted: 'agentic://image-analysis-started',
  ImageAnalysisCompleted: 'agentic://image-analysis-completed',
  DialogTurnStarted: 'agentic://dialog-turn-started',
  // Detached dispatch has no child-observer ownership. Ignoring this link prevents an
  // unmarked child projection from being mistaken for a local session.
  ModelRoundStarted: 'agentic://model-round-started',
  ModelRoundCompleted: 'agentic://model-round-completed',
  ModelRoundAttemptSuperseded: 'agentic://model-round-attempt-superseded',
  TextChunk: 'agentic://text-chunk',
  ThinkingChunk: 'agentic://text-chunk',
  ToolEvent: 'agentic://tool-event',
  DialogTurnCompleted: 'agentic://dialog-turn-completed',
  DialogTurnFailed: 'agentic://dialog-turn-failed',
  DialogTurnCancelled: 'agentic://dialog-turn-cancelled',
  TokenUsageUpdated: 'agentic://token-usage-updated',
  ContextCompressionStarted: 'agentic://context-compression-started',
  ContextCompressionCompleted: 'agentic://context-compression-completed',
  ContextCompressionFailed: 'agentic://context-compression-failed',
  ThreadGoalUpdated: 'agentic://thread-goal-updated',
  DeepReviewQueueStateChanged: 'agentic://deep-review-queue-state-changed',
  SessionModelAutoMigrated: 'agentic://session-model-auto-migrated',
  UserSteeringInjected: 'agentic://user-steering-injected',
};

function camelKey(key: string): string {
  return key.replace(/_([a-z])/g, (_match, letter: string) => letter.toUpperCase());
}

function camelize(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(camelize);
  }
  if (!value || typeof value !== 'object') {
    return value;
  }
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [camelKey(key), camelize(nested)]),
  );
}

export function projectDispatchAgentEvent(
  dispatchEvent: Extract<DispatchEvent, { type: 'agentEvent' }>,
): { eventName: string; payload: Record<string, unknown>; envelopeId?: string } | null {
  const outer = dispatchEvent as unknown as Record<string, unknown>;
  const envelope = dispatchEvent.event as DispatchAgentEventEnvelope;
  const eventRecord = envelope?.event && typeof envelope.event === 'object'
    ? envelope.event
    : dispatchEvent.event;
  const projectedName =
    (typeof outer.frontendEventName === 'string' && outer.frontendEventName)
    || (typeof envelope?.frontendEventName === 'string' && envelope.frontendEventName);
  const projectedPayload =
    (outer.frontendPayload && typeof outer.frontendPayload === 'object'
      ? outer.frontendPayload
      : undefined)
    || (envelope?.frontendPayload && typeof envelope.frontendPayload === 'object'
      ? envelope.frontendPayload
      : undefined);
  if (projectedName === 'agentic://subagent-session-linked') {
    return null;
  }
  if (projectedName && projectedPayload) {
    return {
      eventName: projectedName,
      payload: projectedPayload as Record<string, unknown>,
      envelopeId: typeof envelope?.id === 'string' ? envelope.id : undefined,
    };
  }

  if (!eventRecord || typeof eventRecord !== 'object') {
    return null;
  }
  const raw = eventRecord as Record<string, unknown>;
  const rawType = typeof raw.type === 'string' ? raw.type : '';
  const eventName = RAW_EVENT_NAMES[rawType];
  if (!eventName) {
    return null;
  }
  const payload = camelize(raw) as Record<string, unknown>;
  delete payload.type;
  if (rawType === 'ThinkingChunk') {
    payload.text = payload.content;
    payload.contentType = 'thinking';
    payload.isThinkingEnd = payload.isEnd;
    delete payload.content;
    delete payload.isEnd;
  }
  if (rawType === 'SessionTitleGenerated' && payload.timestamp === undefined) {
    payload.timestamp = Date.now();
  }
  return {
    eventName,
    payload,
    envelopeId: typeof envelope?.id === 'string' ? envelope.id : undefined,
  };
}

function hashText(value: string): string {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function transportErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isJobStillObserved(job: DispatchObserverJob): boolean {
  const state = dispatchJobStore.getState();
  return (
    state.jobs[job.jobId]?.sessionId === job.sessionId
    && !state.dismissedJobIds.includes(job.jobId)
    && !state.dismissedSessionIds.includes(job.sessionId)
  );
}

export function dispatchEventId(event: DispatchEvent): string {
  if (event.type === 'agentEvent') {
    const envelope = event.event as DispatchAgentEventEnvelope;
    if (typeof envelope?.id === 'string' && envelope.id) {
      return envelope.id;
    }
  }
  return `${event.type}:${event.timestamp}:${hashText(JSON.stringify(event))}`;
}

function ensureProjection(context: FlowChatContext, job: DispatchObserverJob): boolean {
  const sourceWorkspacePath = job.sourceWorkspacePath?.trim() || undefined;
  const existing = context.flowChatStore.getState().sessions.get(job.sessionId);
  if (existing) {
    if (existing.config.dispatchJobId !== job.jobId) {
      log.info('Dispatch diagnostic: observer adopted an existing flow chat session', {
        jobId: job.jobId,
        sessionId: job.sessionId,
        previousDispatchJobId: existing.config.dispatchJobId,
        wasHistorical: existing.isHistorical,
        historyState: existing.historyState,
      });
    }
    // Reconcile both immutable target identity and controller-side ownership.
    // The observer can start before FlowChat knows its workspace, so a legacy
    // outbound record may only gain its source path on a later poll.
    context.flowChatStore.updateSessionDispatchTarget(job.sessionId, {
      targetRequest: job.targetRequest,
      target: job.target,
      jobId: job.jobId,
      approvalPolicy: job.approvalPolicy,
      model: job.model,
      availableModels: job.availableModels,
      defaultModel: job.defaultModel,
      state: job.state,
      cursor: job.cursor,
      sourceWorkspacePath,
      sourceWorkspaceId: job.sourceWorkspaceId,
    });
    return true;
  }

  // Never create a workspace-less projection. SessionsSection renders once
  // per workspace, and an unowned projection must not be allowed to appear in
  // every navigation group while startup workspace state is still loading.
  if (!sourceWorkspacePath) {
    return false;
  }

  // The persisted cursor represents a transcript that lived only in the old
  // renderer process. Rebuild a fresh in-memory projection by replaying from
  // byte zero; never skip straight to that cursor.
  dispatchJobStore.getState().resetReplay(job.jobId);
  log.info('Dispatch diagnostic: observer created a flow chat projection', {
    jobId: job.jobId,
    sessionId: job.sessionId,
    sourceWorkspaceId: job.sourceWorkspaceId,
    state: job.state,
  });
  context.flowChatStore.addExternalSession(
    job.sessionId,
    job.title,
    job.agentType,
    sourceWorkspacePath,
    {
      projectWorkspacePath: sourceWorkspacePath,
      workspaceId: job.sourceWorkspaceId,
    },
  );
  context.flowChatStore.updateSessionDispatchTarget(job.sessionId, {
    targetRequest: job.targetRequest,
    target: job.target,
    jobId: job.jobId,
    approvalPolicy: job.approvalPolicy,
    model: job.model,
    availableModels: job.availableModels,
    defaultModel: job.defaultModel,
    state: job.state,
    cursor: 0,
    sourceWorkspacePath,
    sourceWorkspaceId: job.sourceWorkspaceId,
  });
  return context.flowChatStore.getState().sessions.has(job.sessionId);
}

function applyEvent(context: FlowChatContext, event: DispatchEvent): boolean {
  if (event.type !== 'agentEvent') {
    return true;
  }
  const projected = projectDispatchAgentEvent(event);
  if (!projected) {
    log.debug('Ignoring unprojectable target agent event', { event });
    return true;
  }
  const applied = agenticEventListener.dispatchExternal(
    projected.eventName,
    projected.payload,
  );
  if (!applied) {
    return false;
  }
  context.eventBatcher.flushNow();
  return true;
}

function isStreamingExecutionState(state: SessionExecutionState): boolean {
  return (
    state === SessionExecutionState.PROCESSING ||
    state === SessionExecutionState.FINISHING
  );
}

function reconcileDispatchTerminalRuntime(
  context: FlowChatContext,
  sessionId: string,
  state: DispatchJobState,
  lastError?: string,
): void {
  if (!isDispatchJobTerminal(state)) return;

  const pendingCompletion = context.pendingTurnCompletions?.get(sessionId);
  if (pendingCompletion?.timer) {
    clearTimeout(pendingCompletion.timer);
  }
  context.pendingTurnCompletions?.delete(sessionId);
  const runtimeStatusTimerPrefix = `${sessionId}:`;
  for (const [key, timer] of context.runtimeStatusTimers?.entries() ?? []) {
    if (!key.startsWith(runtimeStatusTimerPrefix)) {
      continue;
    }
    clearTimeout(timer);
    context.runtimeStatusTimers.delete(key);
  }
  const dialogTurns = context.flowChatStore
    .getState()
    .sessions
    .get(sessionId)
    ?.dialogTurns ?? [];
  const lastTurn = dialogTurns[dialogTurns.length - 1];
  if (lastTurn) {
    clearRuntimeStatus(context, sessionId, lastTurn.id);
  } else {
    clearRuntimeStatusState({ sessionId });
  }
  context.activeTextItems?.get(sessionId)?.clear();
  context.contentBuffers?.get(sessionId)?.clear();
  context.processingManager?.clearSessionStatus(sessionId);
  context.userCancelledSessionIds?.delete(sessionId);

  const settleStateMachine = async () => {
    const currentState = stateMachineManager.getCurrentState(sessionId);
    if (state === 'failed') {
      if (isStreamingExecutionState(currentState)) {
        await stateMachineManager.transition(
          sessionId,
          SessionExecutionEvent.ERROR_OCCURRED,
          { error: lastError || 'Dispatched task failed' },
        );
      }
      if (
        stateMachineManager.getCurrentState(sessionId) ===
        SessionExecutionState.ERROR
      ) {
        await stateMachineManager.transition(
          sessionId,
          SessionExecutionEvent.RESET,
        );
      }
      return;
    }

    if (isStreamingExecutionState(currentState)) {
      await stateMachineManager.transition(
        sessionId,
        SessionExecutionEvent.FINISHING_SETTLED,
      );
    }
  };

  void settleStateMachine().catch(error => {
    log.warn('Failed to settle dispatch terminal state machine', {
      sessionId,
      state,
      error,
    });
  });
}

async function refreshJob(
  context: FlowChatContext,
  requestedJobId: string,
  isObserverCurrent: () => boolean,
): Promise<void> {
  if (!isObserverCurrent()) {
    return;
  }
  let job = dispatchJobStore.getState().jobs[requestedJobId];
  if (!job) {
    return;
  }
  // `submitting` is a local pre-ack state. There is no durable target job to
  // query yet, and a failed submit intentionally remains retryable.
  if (job.state === 'submitting') {
    return;
  }
  const projectionExisted = context.flowChatStore.getState().sessions.has(job.sessionId);
  if (!ensureProjection(context, job)) {
    return;
  }
  if (projectionExisted && isDispatchJobTerminal(job.state) && job.terminalDrained) {
    return;
  }
  if (!projectionExisted) {
    job = dispatchJobStore.getState().jobs[requestedJobId] ?? job;
  }

  const requestCursor = job.cursor;
  let response: DispatchStatusResponse;
  try {
    response = await dispatchApi.status(job.jobId, requestCursor);
  } catch (error) {
    if (!isObserverCurrent() || !isJobStillObserved(job)) {
      return;
    }
    dispatchJobStore.getState().setTransportState(
      job.jobId,
      'unreachable',
      transportErrorMessage(error),
    );
    throw error;
  }
  // Deleting a dispatch session writes a projection tombstone while an
  // already-issued target poll may still be in flight. Never let that stale
  // response project SessionCreated/DialogTurnStarted and recreate the row.
  if (!isObserverCurrent() || !isJobStillObserved(job)) {
    return;
  }
  // A successful target status request is the only authoritative signal that
  // clears a transient transport failure. It does not alter the durable job
  // state beyond the snapshot applied below.
  dispatchJobStore.getState().setTransportState(job.jobId, 'reachable');
  // Terminal event handling and snapshot reconciliation clear this marker.
  // Capture it first so a controller-initiated cancellation cannot be
  // misreported as a newly completed background task.
  const userCancelledBeforeRefresh =
    context.userCancelledSessionIds?.has(job.sessionId) ?? false;
  for (const event of response.events) {
    if (!isObserverCurrent() || !isJobStillObserved(job)) {
      return;
    }
    const eventId = dispatchEventId(event);
    if (dispatchJobStore.getState().hasAppliedEvent(job.jobId, eventId)) {
      continue;
    }
    if (!applyEvent(context, event)) {
      return;
    }
    // Persist each applied id immediately. If a later event in this response
    // fails, the cursor stays put but already-applied chunks are not duplicated.
    dispatchJobStore.getState().updateProgress(job.jobId, {
      appliedEventIds: [eventId],
    });
  }
  if (!isObserverCurrent() || !isJobStillObserved(job)) {
    return;
  }

  const terminalDrained =
    isDispatchJobTerminal(response.state) &&
    requestCursor === response.cursor &&
    response.events.length === 0;
  const sessionBeforeSnapshot = context.flowChatStore
    .getState()
    .sessions
    .get(job.sessionId);
  const dialogTurnsBeforeSnapshot = sessionBeforeSnapshot?.dialogTurns ?? [];
  const lastTurnBeforeSnapshot =
    dialogTurnsBeforeSnapshot[dialogTurnsBeforeSnapshot.length - 1];
  const terminalEventHandled =
    !!lastTurnBeforeSnapshot &&
    (
      context.handledTerminalTurnEvents?.has(
        `${job.sessionId}:${lastTurnBeforeSnapshot.id}`,
      ) ?? false
    );
  const needsTerminalFallback = terminalDrained && !terminalEventHandled;
  const applied = context.flowChatStore.applyDispatchSnapshot(job.sessionId, {
    jobId: job.jobId,
    state: response.state,
    cursor: response.cursor,
    lastError: response.lastError,
    expectedCursor: requestCursor,
    cursorReset: response.cursorReset,
    terminalDrained: needsTerminalFallback,
  });
  if (!applied.applied) {
    return;
  }
  const background =
    typeof document !== 'undefined'
    && (document.hidden || !document.hasFocus());
  const knownPermissionIds = new Set(
    (job.pendingPermissions ?? [])
      .map(request => request.requestId)
      .filter((requestId): requestId is string => typeof requestId === 'string'),
  );
  const newPermissionCount = response.pendingPermissions.filter(request => (
    typeof request.requestId === 'string'
    && !knownPermissionIds.has(request.requestId)
  )).length;
  dispatchJobStore.getState().updateProgress(job.jobId, {
    cursor: response.cursor,
    state: response.state,
    lastError: response.lastError,
    cursorReset: response.cursorReset,
    terminalDrained,
    pendingPermissions: response.pendingPermissions,
    eventLogComplete: response.eventLogComplete,
    historyTruncated: response.historyTruncated,
    omittedEventCount: response.omittedEventCount,
  });
  if (
    job.eventLogComplete !== false
    && response.eventLogComplete === false
  ) {
    notificationService.warning(
      i18nService.t('common:dispatch.eventHistoryIncomplete'),
      { duration: 6000 },
    );
  }
  if (background && newPermissionCount > 0) {
    void systemAPI.sendSystemNotification(
      i18nService.t('common:dispatch.permissionTitle'),
      i18nService.t('common:dispatch.permissionBody', {
        task: job.title,
        count: newPermissionCount,
      }),
    ).catch(error => {
      log.warn('Failed to send dispatched permission notification', {
        jobId: job.jobId,
        error,
      });
    });
  }
  const becameTerminal =
    !isDispatchJobTerminal(job.state)
    && isDispatchJobTerminal(response.state);
  if (
    becameTerminal
    && response.state !== 'cancelled'
    && !userCancelledBeforeRefresh
  ) {
    const activeSessionId = context.flowChatStore.getState().activeSessionId;
    if (activeSessionId !== job.sessionId || background) {
      context.flowChatStore.markSessionUnreadCompletion(
        job.sessionId,
        response.state === 'failed' ? 'error' : 'completed',
      );
    }
    if (background) {
      const title = response.state === 'failed'
        ? i18nService.t('common:dispatch.completionFailedTitle')
        : i18nService.t('common:dispatch.completionTitle');
      const body = i18nService.t('common:dispatch.completionBody', {
        task: job.title,
        target:
          job.target.kind === 'ssh' || job.target.kind === 'device'
            ? job.target.displayName
            : i18nService.t('common:dispatch.localTarget'),
      });
      void systemAPI.sendSystemNotification(title, body).catch(error => {
        log.warn('Failed to send dispatched task completion notification', {
          jobId: job.jobId,
          error,
        });
      });
    }
  }
  if (needsTerminalFallback) {
    const effectiveSession = context.flowChatStore
      .getState()
      .sessions
      .get(job.sessionId);
    const effectiveState = effectiveSession?.config.dispatchJobState;
    if (!effectiveState || !isDispatchJobTerminal(effectiveState)) {
      return;
    }
    reconcileDispatchTerminalRuntime(
      context,
      job.sessionId,
      effectiveState,
      effectiveSession.config.dispatchLastError ||
        effectiveSession.error ||
        response.lastError,
    );
  }
}

export function installDispatchJobObserver(context: FlowChatContext): () => void {
  const observerGlobal = getDispatchObserverGlobal();
  const previousLease = observerGlobal.__bitfunDispatchJobObserverLease__;
  if (previousLease) {
    log.info('Replacing an existing dispatch job observer');
    previousLease.dispose();
  }

  let disposed = false;
  let inFlight = false;
  let queuedJobId: string | undefined;
  let immediateTimer: ReturnType<typeof setTimeout> | null = null;
  let interval: ReturnType<typeof setInterval> | null = null;
  let handleVisibilityChanged: (() => void) | null = null;
  let lease: DispatchObserverLease;

  const ownsLease = (): boolean => (
    !disposed
    && observerGlobal.__bitfunDispatchJobObserverLease__ === lease
  );

  async function run(requestedJobId?: string): Promise<void> {
    if (!ownsLease() || isPeerDeviceModeActive()) return;
    if (inFlight) {
      queuedJobId = requestedJobId;
      return;
    }

    inFlight = true;
    try {
      const records = await dispatchApi.listJobs();
      if (!ownsLease()) {
        return;
      }
      dispatchJobStore.getState().mergeOutboundRecords(records);
      const jobs = Object.values(dispatchJobStore.getState().jobs)
        .filter(job => !requestedJobId || job.jobId === requestedJobId);
      for (const job of jobs) {
        if (!ownsLease()) {
          return;
        }
        try {
          await refreshJob(context, job.jobId, ownsLease);
        } catch (error) {
          if (!ownsLease()) {
            return;
          }
          log.warn('Dispatch job refresh failed', { jobId: job.jobId, error });
        }
      }
    } catch (error) {
      if (!ownsLease()) {
        return;
      }
      const message = transportErrorMessage(error);
      const jobs = Object.values(dispatchJobStore.getState().jobs)
        .filter(job => (
          job.state !== 'submitting' &&
          (!requestedJobId || job.jobId === requestedJobId) &&
          !(isDispatchJobTerminal(job.state) && job.terminalDrained)
        ));
      for (const job of jobs) {
        dispatchJobStore.getState().setTransportState(
          job.jobId,
          'unreachable',
          message,
        );
      }
      log.warn('Failed to reconcile outbound dispatch jobs', { error });
    } finally {
      inFlight = false;
      if (queuedJobId !== undefined && ownsLease()) {
        const next = queuedJobId;
        queuedJobId = undefined;
        schedule(next);
      }
    }
  }

  function schedule(jobId?: string): void {
    if (!ownsLease()) return;
    if (immediateTimer !== null) {
      clearTimeout(immediateTimer);
    }
    immediateTimer = setTimeout(() => {
      immediateTimer = null;
      void run(jobId);
    }, 0);
  }

  const dispose = (): void => {
    if (disposed) {
      return;
    }
    disposed = true;
    if (observerGlobal.__bitfunDispatchJobObserverLease__ === lease) {
      delete observerGlobal.__bitfunDispatchJobObserverLease__;
    }
    if (immediateTimer !== null) {
      clearTimeout(immediateTimer);
      immediateTimer = null;
    }
    if (interval !== null) {
      clearInterval(interval);
      interval = null;
    }
    if (typeof document !== 'undefined' && handleVisibilityChanged) {
      document.removeEventListener('visibilitychange', handleVisibilityChanged);
    }
  };
  lease = {
    requestRefresh: schedule,
    dispose,
  };
  observerGlobal.__bitfunDispatchJobObserverLease__ = lease;

  interval = setInterval(() => {
    void run();
  }, DISPATCH_JOB_POLL_INTERVAL_MS);
  handleVisibilityChanged = () => {
    if (typeof document === 'undefined' || document.visibilityState === 'visible') {
      schedule();
    }
  };
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', handleVisibilityChanged);
  }
  schedule();

  return dispose;
}
