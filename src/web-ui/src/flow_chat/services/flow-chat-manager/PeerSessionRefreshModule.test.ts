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

vi.mock('@/infrastructure/peer-device/peerModeFlag', () => ({
  isPeerDeviceModeActive: () => peerModeMock.active,
}));

vi.mock('../../state-machine', () => ({
  stateMachineManager: stateMachineMock,
}));

import {
  installPeerSessionRefresh,
  PEER_SESSION_REFRESH_INTERVAL_MS,
  requestPeerSessionRefresh,
} from './PeerSessionRefreshModule';

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
      },
      eventBatcher: { flushNow: vi.fn() },
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

  it('does not churn the state machine while a turn is already streaming', async () => {
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

    expect(refresh).toHaveBeenCalled();
    expect(stateMachineMock.reset).not.toHaveBeenCalled();
    cleanup();
  });
});
