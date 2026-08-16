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

import { isSurfaceChangedError } from '@/infrastructure/peer-device/deviceSurface';
import { isSurfaceReconcileEnabled } from '@/infrastructure/peer-device/deviceSurfaceReconcile';
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

  async function runRefresh(requestedSessionId?: string): Promise<void> {
    if (disposed || inFlight || !isSurfaceReconcileEnabled()) {
      if (inFlight) {
        queued = true;
      }
      return;
    }
    if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
      return;
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
    const replaceRunningSnapshot =
      machineState === SessionExecutionState.IDLE ||
      machineState === SessionExecutionState.ERROR ||
      Date.now() - lastUpdateTime >= PEER_SESSION_STREAM_STALE_MS;
    context.eventBatcher.flushNow();
    const machineVersion = machine?.getContext().version ?? 0;

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
        },
      );
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
    state => state.activeSessionId,
    sessionId => scheduleRefresh(sessionId ?? undefined),
  );
  const interval = setInterval(() => {
    void runRefresh();
  }, PEER_SESSION_REFRESH_INTERVAL_MS);

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
    if (typeof window !== 'undefined') {
      window.removeEventListener('peer-mode:changed', handlePeerModeChanged);
    }
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', handleVisibilityChanged);
    }
  };
}
