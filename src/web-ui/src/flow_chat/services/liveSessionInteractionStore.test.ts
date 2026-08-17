import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  PermissionRequest,
  PermissionRequestSnapshot,
} from '@/infrastructure/api/service-api/AgentAPI';
import {
  activateSurface,
  LOCAL_SURFACE_ID,
} from '@/infrastructure/peer-device/deviceSurface';
import {
  PEER_EVENT_SOURCE_KEY,
  routeSurfaceEvent,
  setActiveSurfaceDeviceId,
} from '@/infrastructure/peer-device/deviceSurfaceRouting';
import {
  installLiveSessionInteractionMailbox,
  liveSessionInteractionStore,
  resetLiveSessionInteractionStoreForTest,
} from './liveSessionInteractionStore';

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    onPermissionRequestEvent: vi.fn(() => () => {}),
    subscribePermissionRequests: vi.fn(async () => undefined),
    listPendingPermissionRequests: vi.fn(async () => []),
  },
}));

const PEER_SURFACE_ID = 'peer-device-b';

function request(requestId: string, sessionId = 'session-1'): PermissionRequest {
  return {
    requestId,
    roundId: 'round-1',
    order: 0,
    sessionId,
    toolCallId: `${requestId}-tool`,
    projectId: 'project-1',
    agentId: 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
    source: { kind: 'tool_call', identity: 'Write' },
  };
}

function snapshot(
  revision: number,
  requests: PermissionRequest[],
): PermissionRequestSnapshot {
  return { revision, requests };
}

function activateTestSurface(surfaceId: string): void {
  activateSurface(surfaceId);
  setActiveSurfaceDeviceId(surfaceId === LOCAL_SURFACE_ID ? null : surfaceId);
}

describe('liveSessionInteractionStore', () => {
  beforeEach(() => {
    activateTestSurface(LOCAL_SURFACE_ID);
    resetLiveSessionInteractionStoreForTest();
    installLiveSessionInteractionMailbox();
  });

  it('retains an inactive local-host request while a peer Surface is rendered', () => {
    activateTestSurface(PEER_SURFACE_ID);
    const localRequest = request('local-request');

    const route = routeSurfaceEvent('permission://event', {
      event: 'asked',
      request: localRequest,
    });

    expect(route.deliver).toBe(false);
    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([]);

    activateTestSurface(LOCAL_SURFACE_ID);
    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      localRequest,
    ]);
  });

  it('keeps equal session ids isolated by their device Surface', () => {
    const localRequest = request('local-request', 'shared-session');
    const peerRequest = request('peer-request', 'shared-session');
    const localEventVersion = liveSessionInteractionStore.captureEventVersion(
      LOCAL_SURFACE_ID,
    );
    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      'shared-session',
      snapshot(1, [localRequest]),
      localEventVersion,
    );

    routeSurfaceEvent('permission://event', {
      [PEER_EVENT_SOURCE_KEY]: PEER_SURFACE_ID,
      event: 'asked',
      request: peerRequest,
    });

    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      localRequest,
    ]);
    activateTestSurface(PEER_SURFACE_ID);
    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      peerRequest,
    ]);
  });

  it('repairs a missed request and removes it from a newer authoritative snapshot', () => {
    const pending = request('missed-request');
    const eventVersion = liveSessionInteractionStore.captureEventVersion(
      LOCAL_SURFACE_ID,
    );

    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      pending.sessionId,
      snapshot(4, [pending]),
      eventVersion,
    );
    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      pending,
    ]);

    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      pending.sessionId,
      snapshot(5, []),
      eventVersion,
    );
    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([]);
  });

  it('does not let an in-flight stale snapshot erase a newer asked event', () => {
    const eventVersionBeforeRestore = liveSessionInteractionStore.captureEventVersion(
      LOCAL_SURFACE_ID,
    );
    const arrivedDuringRestore = request('new-request');
    liveSessionInteractionStore.applyPermissionEvent(LOCAL_SURFACE_ID, {
      event: 'asked',
      request: arrivedDuringRestore,
    });

    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      arrivedDuringRestore.sessionId,
      snapshot(8, []),
      eventVersionBeforeRestore,
    );

    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      arrivedDuringRestore,
    ]);
  });

  it('does not revive a resolved request from an in-flight stale snapshot', () => {
    const resolved = request('resolved-request');
    liveSessionInteractionStore.applyPermissionEvent(LOCAL_SURFACE_ID, {
      event: 'asked',
      request: resolved,
    });
    const eventVersionBeforeRestore = liveSessionInteractionStore.captureEventVersion(
      LOCAL_SURFACE_ID,
    );
    liveSessionInteractionStore.applyPermissionEvent(LOCAL_SURFACE_ID, {
      event: 'replied',
      requestId: resolved.requestId,
      reply: { reply: 'once' },
      source: 'user',
    });

    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      resolved.sessionId,
      snapshot(12, [resolved]),
      eventVersionBeforeRestore,
    );

    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([]);
  });

  it('rejects a snapshot no newer than the last applied session revision', () => {
    const pending = request('pending-request');
    const eventVersion = liveSessionInteractionStore.captureEventVersion(
      LOCAL_SURFACE_ID,
    );
    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      pending.sessionId,
      snapshot(20, [pending]),
      eventVersion,
    );
    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      pending.sessionId,
      snapshot(20, []),
      eventVersion,
    );
    liveSessionInteractionStore.reconcilePermissionSnapshot(
      LOCAL_SURFACE_ID,
      pending.sessionId,
      snapshot(19, []),
      eventVersion,
    );

    expect(liveSessionInteractionStore.getActiveSnapshot().requests).toEqual([
      pending,
    ]);
  });
});
