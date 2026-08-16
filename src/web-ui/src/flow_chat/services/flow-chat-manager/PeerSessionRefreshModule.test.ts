import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const peerModeMock = vi.hoisted(() => ({ active: true }));
const stateMachineMock = vi.hoisted(() => ({
  get: vi.fn(() => ({
    getCurrentState: () => 'idle',
    getContext: () => ({ lastUpdateTime: 0 }),
  })),
  reset: vi.fn(),
  transition: vi.fn(async () => true),
}));
const agenticListenerMock = vi.hoisted(() => ({
  getIsListening: vi.fn(() => true),
  dispatchExternal: vi.fn(() => true),
}));

vi.mock('@/infrastructure/peer-device/peerModeFlag', () => ({
  isPeerDeviceModeActive: () => peerModeMock.active,
}));

vi.mock('../../state-machine', () => ({
  stateMachineManager: stateMachineMock,
}));

vi.mock('../liveSessionInteractionStore', () => ({
  installLiveSessionInteractionMailbox: vi.fn(),
}));

vi.mock('../AgenticEventListener', () => ({
  agenticEventListener: agenticListenerMock,
}));

import {
  installPeerSessionRefresh,
  PEER_SESSION_REFRESH_INTERVAL_MS,
  requestPeerSessionRefresh,
} from './PeerSessionRefreshModule';
import { resetRuntimeSessionEventGateForTest } from '@/infrastructure/peer-device/runtimeSessionEventGate';

describe('PeerSessionRefreshModule', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    peerModeMock.active = true;
    vi.stubGlobal('document', {
      visibilityState: 'visible',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    vi.stubGlobal('window', {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
  });

  afterEach(() => {
    resetRuntimeSessionEventGateForTest();
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('refreshes immediately, periodically, and on an event-gap request', async () => {
    const refreshPeerSessionSnapshot = vi.fn(async () => ({
      applied: false,
      backendState: 'Processing',
      latestTurnId: 'turn-1',
      latestTurnStatus: 'processing',
    }));
    const state = {
      activeSessionId: 'session-1',
      sessions: new Map([
        ['session-1', {
          sessionId: 'session-1',
          workspacePath: '/peer/project',
          historyState: 'ready',
          isHistorical: false,
          isTransient: false,
        }],
      ]),
    };
    const context = {
      flowChatStore: {
        getState: () => state,
        subscribeSelector: vi.fn(() => () => {}),
        refreshPeerSessionSnapshot,
      },
      eventBatcher: {
        flushNow: vi.fn(),
        clear: vi.fn(),
      },
      contentBuffers: new Map(),
      activeTextItems: new Map(),
    } as any;

    const cleanup = installPeerSessionRefresh(context);

    await vi.advanceTimersByTimeAsync(0);
    expect(refreshPeerSessionSnapshot).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);
    expect(refreshPeerSessionSnapshot).toHaveBeenCalledTimes(2);

    requestPeerSessionRefresh('session-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(refreshPeerSessionSnapshot).toHaveBeenCalledTimes(3);

    cleanup();
  });

  it('does not poll when Peer Device Mode is inactive', async () => {
    peerModeMock.active = false;
    const refreshPeerSessionSnapshot = vi.fn();
    const context = {
      flowChatStore: {
        getState: () => ({
          activeSessionId: 'session-1',
          sessions: new Map(),
        }),
        subscribeSelector: vi.fn(() => () => {}),
        refreshPeerSessionSnapshot,
      },
      eventBatcher: {
        flushNow: vi.fn(),
        clear: vi.fn(),
      },
      contentBuffers: new Map(),
      activeTextItems: new Map(),
    } as any;

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS * 2);

    expect(refreshPeerSessionSnapshot).not.toHaveBeenCalled();
    cleanup();
  });
});

describe('PeerSessionRefreshModule re-attach after a surface switch', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    peerModeMock.active = true;
    vi.stubGlobal('document', {
      visibilityState: 'visible',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    vi.stubGlobal('window', { addEventListener: vi.fn(), removeEventListener: vi.fn() });
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  function contextWithSnapshot(
    refreshPeerSessionSnapshot: ReturnType<typeof vi.fn>,
  ) {
    const state = {
      activeSessionId: 'session-1',
      sessions: new Map([
        ['session-1', {
          sessionId: 'session-1',
          workspacePath: '/repo/BitFun',
          historyState: 'ready',
          isHistorical: false,
          isTransient: false,
          dialogTurns: [{
            id: 'turn-live',
            status: 'processing',
            modelRounds: [{ id: 'round-0', items: [] }],
          }],
        }],
      ]),
    };
    return {
      flowChatStore: {
        getState: () => state,
        subscribeSelector: vi.fn(() => () => {}),
        refreshPeerSessionSnapshot,
        reconcilePendingUserQuestions: vi.fn(),
      },
      eventBatcher: { flushNow: vi.fn(), clear: vi.fn() },
      contentBuffers: new Map(),
      activeTextItems: new Map(),
    } as any;
  }

  it('re-attaches an executing turn when the snapshot was refused or unchanged', async () => {
    // The content-safety guard can refuse a snapshot that would gut the turn.
    // The host is still executing, so the rebuilt surface must reattach anyway.
    stateMachineMock.get.mockReturnValue({
      getCurrentState: () => 'idle',
      getContext: () => ({ lastUpdateTime: 0, version: 0 }),
    });
    const refresh = vi.fn(async () => ({
      applied: false,
      backendState: 'Processing { current_turn_id: "turn-live", phase: Streaming }',
      latestTurnId: 'turn-live',
      latestTurnStatus: 'processing',
    }));

    const cleanup = installPeerSessionRefresh(contextWithSnapshot(refresh));
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);

    expect(refresh).toHaveBeenCalled();
    expect(stateMachineMock.transition).toHaveBeenCalled();
    cleanup();
  });

  it('does not poll or churn while a turn is already streaming fresh events', async () => {
    stateMachineMock.get.mockReturnValue({
      getCurrentState: () => 'processing',
      getContext: () => ({ lastUpdateTime: Date.now(), version: 0 }),
    });
    const refresh = vi.fn(async () => ({
      applied: false,
      backendState: 'Processing { current_turn_id: "turn-live", phase: Streaming }',
      latestTurnId: 'turn-live',
      latestTurnStatus: 'processing',
    }));

    const cleanup = installPeerSessionRefresh(contextWithSnapshot(refresh));
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);

    expect(refresh).toHaveBeenCalledTimes(1);
    expect(stateMachineMock.reset).not.toHaveBeenCalled();
    cleanup();
  });

  it('replays the Runtime projection before reconciling the blocking mailbox', async () => {
    stateMachineMock.get.mockReturnValue({
      getCurrentState: () => 'idle',
      getContext: () => ({ lastUpdateTime: 0, version: 0 }),
    });
    const runtimeEventSnapshot = {
      sessionId: 'session-1',
      streamId: 'runtime-a',
      cursor: 8,
      activeTurnId: 'turn-live',
      events: [
        {
          eventName: 'agentic://dialog-turn-started',
          payload: { sessionId: 'session-1', turnId: 'turn-live' },
        },
        {
          eventName: 'agentic://text-chunk',
          payload: {
            sessionId: 'session-1',
            turnId: 'turn-live',
            roundId: 'round-0',
            text: 'materialized output',
          },
        },
      ],
    };
    const refresh = vi.fn(async () => ({
      applied: true,
      backendState: 'Processing { current_turn_id: "turn-live", phase: Streaming }',
      latestTurnId: 'turn-live',
      latestTurnStatus: 'processing',
      runtimeEventSnapshot,
      pendingUserQuestions: { revision: 2, questions: [] },
    }));
    const context = contextWithSnapshot(refresh);

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(1);

    expect(agenticListenerMock.dispatchExternal).toHaveBeenNthCalledWith(
      1,
      'agentic://dialog-turn-started',
      runtimeEventSnapshot.events[0].payload,
    );
    expect(agenticListenerMock.dispatchExternal).toHaveBeenNthCalledWith(
      2,
      'agentic://text-chunk',
      runtimeEventSnapshot.events[1].payload,
    );
    expect(context.eventBatcher.clear).toHaveBeenCalled();
    expect(context.flowChatStore.reconcilePendingUserQuestions).toHaveBeenCalledWith(
      'session-1',
      { revision: 2, questions: [] },
    );
    cleanup();
  });

  it('advances a healthy Runtime fence without resetting and replaying it', async () => {
    stateMachineMock.get.mockReturnValue({
      getCurrentState: () => 'processing',
      getContext: () => ({ lastUpdateTime: Date.now(), version: 0 }),
    });
    const refresh = vi.fn(async () => ({
      applied: false,
      backendState: 'Processing { current_turn_id: "turn-live", phase: Streaming }',
      latestTurnId: 'turn-live',
      latestTurnStatus: 'processing',
      runtimeEventReplayRequired: false,
      runtimeEventSnapshot: {
        sessionId: 'session-1',
        streamId: 'runtime-a',
        cursor: 8,
        activeTurnId: 'turn-live',
        events: [{
          eventName: 'agentic://text-chunk',
          payload: {
            sessionId: 'session-1',
            turnId: 'turn-live',
            roundId: 'round-0',
            text: 'already rendered',
          },
        }],
      },
    }));
    const context = contextWithSnapshot(refresh);

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(1);

    expect(agenticListenerMock.dispatchExternal).not.toHaveBeenCalled();
    expect(context.eventBatcher.clear).not.toHaveBeenCalled();
    expect(stateMachineMock.reset).not.toHaveBeenCalled();
    cleanup();
  });
});

describe('PeerSessionRefreshModule dead subscription recovery', () => {
  /**
   * The reported freeze: repeated device switching tore the agentic
   * subscription down, and the reconcile loop refused to run while it was
   * down — so the only path that could repair the session view was disabled by
   * the very condition it existed to repair.
   */
  function contextWithDeadSubscription() {
    const refreshPeerSessionSnapshot = vi.fn(async () => ({
      applied: false,
      backendState: 'Processing { current_turn_id: "turn-live", phase: Streaming }',
      latestTurnId: 'turn-live',
      latestTurnStatus: 'processing',
    }));
    const ensureLiveSubscription = vi.fn(async () => {});
    const state = {
      activeSessionId: 'session-1',
      sessions: new Map([
        ['session-1', {
          sessionId: 'session-1',
          workspacePath: '/repo/BitFun',
          historyState: 'ready',
          isHistorical: false,
          isTransient: false,
          dialogTurns: [{ id: 'turn-live', status: 'processing', modelRounds: [] }],
        }],
      ]),
    };
    return {
      refreshPeerSessionSnapshot,
      ensureLiveSubscription,
      context: {
        flowChatStore: {
          getState: () => state,
          subscribeSelector: vi.fn(() => () => {}),
          refreshPeerSessionSnapshot,
        },
        eventBatcher: { flushNow: vi.fn() },
        contentBuffers: new Map(),
        activeTextItems: new Map(),
        ensureLiveSubscription,
      } as any,
    };
  }

  beforeEach(() => {
    vi.useFakeTimers();
    peerModeMock.active = true;
    resetRuntimeSessionEventGateForTest();
    stateMachineMock.get.mockReturnValue({
      getCurrentState: () => 'idle',
      getContext: () => ({ lastUpdateTime: 0, version: 0 }),
    });
    vi.stubGlobal('document', {
      visibilityState: 'visible',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    vi.stubGlobal('window', { addEventListener: vi.fn(), removeEventListener: vi.fn() });
  });

  afterEach(() => {
    agenticListenerMock.getIsListening.mockReturnValue(true);
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('still reconciles when the subscription is down', async () => {
    agenticListenerMock.getIsListening.mockReturnValue(false);
    const { context, refreshPeerSessionSnapshot } = contextWithDeadSubscription();

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);

    expect(refreshPeerSessionSnapshot).toHaveBeenCalled();
    cleanup();
  });

  it('re-arms the subscription it found dead', async () => {
    agenticListenerMock.getIsListening.mockReturnValue(false);
    const { context, ensureLiveSubscription } = contextWithDeadSubscription();

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);

    expect(ensureLiveSubscription).toHaveBeenCalled();
    cleanup();
  });

  it('does not re-arm a subscription that is already live', async () => {
    agenticListenerMock.getIsListening.mockReturnValue(true);
    const { context, ensureLiveSubscription } = contextWithDeadSubscription();

    const cleanup = installPeerSessionRefresh(context);
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(PEER_SESSION_REFRESH_INTERVAL_MS);

    expect(ensureLiveSubscription).not.toHaveBeenCalled();
    cleanup();
  });
});
