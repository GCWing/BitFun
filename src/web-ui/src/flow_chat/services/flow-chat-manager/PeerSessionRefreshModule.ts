/**
 * Active-session snapshot reconciliation for the rendered device surface.
 *
 * DeviceEvent fan-out is the real-time path, but the relay protocol has no
 * ACK/replay recovery. A controller that attaches mid-turn can therefore miss
 * lifecycle events required by the local FlowChat state machine. The same gap
 * exists on the **local** surface: a turn that keeps running on this machine
 * while the UI renders another device produces events that surface routing
 * drops, so returning to it needs the same repair. This module periodically
 * reconciles a small host snapshot and also supports immediate refresh
 * requests when an event gap is detected.
 */

import {
  getActiveSurfaceScope,
  isSurfaceChangedError,
} from '@/infrastructure/peer-device/deviceSurface';
import { isSurfaceReconcileEnabled } from '@/infrastructure/peer-device/deviceSurfaceReconcile';
import {
  beginRuntimeSessionAttachment,
  subscribeRuntimeSessionEventGaps,
} from '@/infrastructure/peer-device/runtimeSessionEventGate';
import { createLogger } from '@/shared/utils/logger';
import {
  isBackendSessionActivelyProcessing,
} from '../../store/FlowChatStore';
import { stateMachineManager } from '../../state-machine';
import {
  SessionExecutionEvent,
  SessionExecutionState,
} from '../../state-machine/types';
import type { AnyFlowItem, DialogTurn } from '../../types/flow-chat';
import { installLiveSessionInteractionMailbox } from '../liveSessionInteractionStore';
import { agenticEventListener } from '../AgenticEventListener';
import type { FlowChatContext } from './types';

const log = createLogger('PeerSessionRefresh');

export const PEER_SESSION_REFRESH_INTERVAL_MS = 3000;
export const PEER_SESSION_STREAM_STALE_MS = 6000;

type RefreshRequester = (sessionId?: string) => void;
let installedRefreshRequester: RefreshRequester | null = null;

export function requestPeerSessionRefresh(sessionId?: string): void {
  if (!isSurfaceReconcileEnabled()) {
    return;
  }
  installedRefreshRequester?.(sessionId);
}

function streamKey(
  roundId: string,
  item: Pick<AnyFlowItem, 'attemptId' | 'attemptIndex'>,
): string {
  if (item.attemptId) {
    return item.attemptId;
  }
  if (typeof item.attemptIndex === 'number' && Number.isFinite(item.attemptIndex)) {
    return `${roundId}:attempt:${item.attemptIndex}`;
  }
  return roundId;
}

function seedActiveTurnBuffers(
  context: FlowChatContext,
  sessionId: string,
  turn: DialogTurn,
): void {
  const contentBuffers = new Map<string, string>();
  const activeTextItems = new Map<string, string>();

  for (const round of turn.modelRounds) {
    for (const item of round.items) {
      if (item.type !== 'text' && item.type !== 'thinking') {
        continue;
      }
      const baseKey = streamKey(round.id, item);
      const key = item.type === 'thinking' ? `thinking_${baseKey}` : baseKey;
      contentBuffers.set(key, item.content || '');
      activeTextItems.set(key, item.id);
    }
  }

  context.contentBuffers.set(sessionId, contentBuffers);
  context.activeTextItems.set(sessionId, activeTextItems);
}

function isTerminalTurn(turn: DialogTurn | undefined): boolean {
  return turn?.status === 'completed' ||
    turn?.status === 'cancelled' ||
    turn?.status === 'error';
}

async function alignStateMachineWithSnapshot(
  context: FlowChatContext,
  sessionId: string,
  backendState: string,
  latestTurnId?: string,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  const latestTurn = latestTurnId
    ? session?.dialogTurns.find(turn => turn.id === latestTurnId)
    : session?.dialogTurns[session.dialogTurns.length - 1];

  if (
    isBackendSessionActivelyProcessing(backendState) &&
    latestTurn &&
    !isTerminalTurn(latestTurn)
  ) {
    stateMachineManager.reset(sessionId);
    seedActiveTurnBuffers(context, sessionId, latestTurn);
    await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
      taskId: sessionId,
      dialogTurnId: latestTurn.id,
    });
    const latestRound = latestTurn.modelRounds[latestTurn.modelRounds.length - 1];
    if (latestRound) {
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.MODEL_ROUND_START, {
        modelRoundId: latestRound.id,
      });
    }
    return;
  }

  context.contentBuffers.delete(sessionId);
  context.activeTextItems.delete(sessionId);
  stateMachineManager.reset(sessionId);
}

export function installPeerSessionRefresh(context: FlowChatContext): () => void {
  // Blocking interactions are Runtime mailboxes, so their event projection
  // must exist for the whole FlowChat lifetime rather than only while a card
  // component is mounted.
  installLiveSessionInteractionMailbox();
  let disposed = false;
  let inFlight = false;
  let queued = false;
  let immediateTimer: ReturnType<typeof setTimeout> | null = null;

  async function runRefresh(
    requestedSessionId?: string,
    staleOnly = false,
  ): Promise<void> {
    if (disposed || inFlight || !isSurfaceReconcileEnabled()) {
      if (inFlight) {
        queued = true;
      }
      return;
    }
    if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
      return;
    }
    // A dead subscription is the strongest reason to reconcile, not a reason to
    // skip: bailing here meant a switch that tore the listener down disabled
    // the only path that could repair it, and the chat froze for good. Re-arm
    // and continue — the runtime cursor fence already covers the snapshot/live
    // race while the subscription is coming back up.
    if (!agenticEventListener.getIsListening()) {
      void context.ensureLiveSubscription?.();
    }

    const state = context.flowChatStore.getState();
    const sessionId = requestedSessionId || state.activeSessionId;
    if (!sessionId || state.activeSessionId !== sessionId) {
      return;
    }
    const session = state.sessions.get(sessionId);
    const workspacePath = session?.workspacePath?.trim();
    if (
      !session ||
      !workspacePath ||
      session.isTransient ||
      session.isHistorical ||
      session.historyState !== 'ready'
    ) {
      return;
    }

    const machine = stateMachineManager.get(sessionId);
    const machineState = machine?.getCurrentState() ?? SessionExecutionState.IDLE;
    if (machineState === SessionExecutionState.FINISHING) {
      return;
    }
    const lastUpdateTime = machine?.getContext().lastUpdateTime ?? 0;
    const forceRuntimeReplay =
      machineState === SessionExecutionState.IDLE ||
      machineState === SessionExecutionState.ERROR;
    const streamIsStale = Date.now() - lastUpdateTime >= PEER_SESSION_STREAM_STALE_MS;
    if (staleOnly && !forceRuntimeReplay && !streamIsStale) {
      return;
    }
    const replaceRunningSnapshot =
      forceRuntimeReplay || streamIsStale;
    context.eventBatcher.flushNow();
    const machineVersion = machine?.getContext().version ?? 0;
    const surfaceScope = getActiveSurfaceScope();
    const attachment = beginRuntimeSessionAttachment(surfaceScope.surfaceId, sessionId);
    let attachmentFinished = false;

    inFlight = true;
    try {
      const result = await context.flowChatStore.refreshPeerSessionSnapshot(
        sessionId,
        workspacePath,
        {
          replaceRunningSnapshot,
          requireActiveSession: true,
          shouldApply: () => {
            if (!isSurfaceReconcileEnabled()) {
              return false;
            }
            const currentMachine = stateMachineManager.get(sessionId);
            return (currentMachine?.getContext().version ?? 0) === machineVersion;
          },
          shouldReplayRuntimeSnapshot: snapshot =>
            forceRuntimeReplay || attachment.requiresReplay(snapshot),
        },
      );
      surfaceScope.assertCurrent('attachRuntimeSession');

      if (result.runtimeEventSnapshot) {
        if (result.runtimeEventReplayRequired === false) {
          // The rendered projection already includes this exact Runtime
          // cursor. Advance the in-flight fence without resetting a healthy
          // state machine every time the liveness timer runs.
          attachment.finish({
            streamId: result.runtimeEventSnapshot.streamId,
            cursor: result.runtimeEventSnapshot.cursor,
          });
          attachmentFinished = true;
          log.debug('Runtime session projection already current', {
            sessionId,
            cursor: result.runtimeEventSnapshot.cursor,
          });
          return;
        }

        // Establish an empty current-Turn base before replay. The journal is
        // authoritative for everything after DialogTurnStarted, so no
        // UI-written partial checkpoint is allowed to overlap it.
        context.eventBatcher.clear();
        context.contentBuffers.delete(sessionId);
        context.activeTextItems.delete(sessionId);
        stateMachineManager.reset(sessionId);
        await alignStateMachineWithSnapshot(
          context,
          sessionId,
          result.backendState,
          result.runtimeEventSnapshot.activeTurnId ?? result.latestTurnId,
        );

        for (const event of result.runtimeEventSnapshot.events) {
          surfaceScope.assertCurrent('replayRuntimeSessionProjection');
          if (!agenticEventListener.dispatchExternal(event.eventName, event.payload)) {
            throw new Error('Agentic event listener is unavailable during Runtime replay');
          }
        }
        context.eventBatcher.flushNow();
        context.flowChatStore.reconcilePendingUserQuestions(
          sessionId,
          result.pendingUserQuestions,
        );
        await alignStateMachineWithSnapshot(
          context,
          sessionId,
          result.backendState,
          result.runtimeEventSnapshot.activeTurnId ?? result.latestTurnId,
        );
        surfaceScope.assertCurrent('finishRuntimeSessionAttachment');
        attachment.finish({
          streamId: result.runtimeEventSnapshot.streamId,
          cursor: result.runtimeEventSnapshot.cursor,
        });
        attachmentFinished = true;
        log.debug('Runtime session projection attached', {
          sessionId,
          backendState: result.backendState,
          cursor: result.runtimeEventSnapshot.cursor,
          eventCount: result.runtimeEventSnapshot.events.length,
        });
        return;
      }

      // Older hosts have no cursor contract. Release their queued live events
      // and retain the existing persisted-snapshot reconciliation fallback.
      attachment.abort();
      attachmentFinished = true;
      if (!result.applied) {
        // A snapshot that changed nothing — or that was refused because it
        // would have dropped projected content — still reports whether the
        // host is executing. After a device-surface switch the rebuilt
        // projection has no state machine, so an executing turn would render
        // as static history and later chunks would be dropped. Re-attach on
        // that narrow case only: while a turn really is streaming the machine
        // is already processing, so this cannot churn it every tick.
        const machine = stateMachineManager.get(sessionId);
        const machineIsIdle =
          (machine?.getCurrentState() ?? SessionExecutionState.IDLE)
            === SessionExecutionState.IDLE;
        if (machineIsIdle && isBackendSessionActivelyProcessing(result.backendState)) {
          await alignStateMachineWithSnapshot(
            context,
            sessionId,
            result.backendState,
            result.latestTurnId,
          );
          log.debug('Re-attached an executing session after a surface switch', {
            sessionId,
            backendState: result.backendState,
          });
        }
        return;
      }
      await alignStateMachineWithSnapshot(
        context,
        sessionId,
        result.backendState,
        result.latestTurnId,
      );
      log.debug('Peer session snapshot reconciled', {
        sessionId,
        backendState: result.backendState,
        latestTurnId: result.latestTurnId,
      });
    } catch (error) {
      if (!attachmentFinished) {
        attachment.abort({ discard: !surfaceScope.isCurrent() });
        attachmentFinished = true;
      }
      if (isSurfaceChangedError(error)) {
        // The snapshot belongs to a device this window stopped rendering. Its
        // own container keeps the projection; the surface now on screen
        // reconciles itself on the next tick.
        log.debug('Discarded a snapshot for a device surface we left', { sessionId });
        return;
      }
      // Realtime DeviceEvents remain usable when a background refresh fails.
      // The next interval or gap-triggered request retries without forcing an
      // auto-exit from Peer Mode.
      log.warn('Peer session snapshot refresh failed', { sessionId, error });
    } finally {
      if (!attachmentFinished) {
        attachment.abort({ discard: !surfaceScope.isCurrent() });
      }
      inFlight = false;
      if (queued && !disposed) {
        queued = false;
        scheduleRefresh();
      }
    }
  }

  function scheduleRefresh(sessionId?: string): void {
    if (disposed) {
      return;
    }
    if (immediateTimer !== null) {
      clearTimeout(immediateTimer);
    }
    immediateTimer = setTimeout(() => {
      immediateTimer = null;
      void runRefresh(sessionId);
    }, 0);
  }

  installedRefreshRequester = scheduleRefresh;

  const unsubscribeActiveSession = context.flowChatStore.subscribeSelector(
    state => {
      const sessionId = state.activeSessionId;
      const session = sessionId ? state.sessions.get(sessionId) : undefined;
      return JSON.stringify([
        sessionId ?? '',
        session?.historyState ?? '',
        session?.workspacePath ?? '',
        session?.isTransient === true,
        session?.isHistorical === true,
      ]);
    },
    () => scheduleRefresh(),
  );
  const interval = setInterval(() => {
    void runRefresh(undefined, true);
  }, PEER_SESSION_REFRESH_INTERVAL_MS);
  const unsubscribeRuntimeGaps = subscribeRuntimeSessionEventGaps(
    (surfaceId, sessionId) => {
      if (getActiveSurfaceScope().surfaceId === surfaceId) {
        scheduleRefresh(sessionId);
      }
    },
  );

  const handlePeerModeChanged = (): void => scheduleRefresh();
  const handleVisibilityChanged = (): void => {
    if (typeof document === 'undefined' || document.visibilityState === 'visible') {
      scheduleRefresh();
    }
  };
  if (typeof window !== 'undefined') {
    window.addEventListener('peer-mode:changed', handlePeerModeChanged);
  }
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', handleVisibilityChanged);
  }

  scheduleRefresh();

  return () => {
    disposed = true;
    if (installedRefreshRequester === scheduleRefresh) {
      installedRefreshRequester = null;
    }
    if (immediateTimer !== null) {
      clearTimeout(immediateTimer);
    }
    clearInterval(interval);
    unsubscribeActiveSession();
    unsubscribeRuntimeGaps();
    if (typeof window !== 'undefined') {
      window.removeEventListener('peer-mode:changed', handlePeerModeChanged);
    }
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', handleVisibilityChanged);
    }
  };
}
