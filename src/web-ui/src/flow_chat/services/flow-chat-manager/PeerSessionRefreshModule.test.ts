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
