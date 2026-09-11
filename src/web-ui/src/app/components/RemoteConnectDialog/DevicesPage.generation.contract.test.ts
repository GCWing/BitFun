import { beforeEach, describe, expect, it } from 'vitest';
import { reconcileAccountOwner } from '../../../../../mobile-web/src/services/accountOwner';
import { useMobileStore } from '../../../../../mobile-web/src/services/store';

const sessionA = {
  session_id: 'session-a',
  name: 'Session A',
  agent_type: 'general',
  created_at: '2026-07-22T00:00:00Z',
  updated_at: '2026-07-22T00:00:00Z',
  message_count: 0,
};

describe('mobile account UI ownership', () => {
  beforeEach(() => {
    useMobileStore.getState().resetConnectionState();
  });

  it('adopts a initial identity without discarding matching account state', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);

    expect(reconcileAccountOwner({
      kind: 'initial',
      epoch: 1,
      userId: 'user-a',
    })).toBe(false);

    expect(useMobileStore.getState().sessions).toHaveLength(1);
    expect(useMobileStore.getState().controlTarget).toBeNull();
  });

  it('clears A-owned state when a initial identity proves account B', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);
    store.setControlTarget({ deviceId: 'home-a', deviceName: 'A', isHome: true });

    expect(reconcileAccountOwner({
      kind: 'initial',
      epoch: 1,
      userId: 'user-b',
    })).toBe(true);

    const next = useMobileStore.getState();
    expect(next.sessions).toEqual([]);
    expect(next.authenticatedUserId).toBe('user-b');
    expect(next.controlTarget).toBeNull();
  });

  it('clears known user and target when identity becomes unavailable', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);
    store.setControlTarget({ deviceId: 'peer-a', deviceName: 'Peer A', isHome: false });

    expect(reconcileAccountOwner({
      kind: 'unavailable',
      epoch: 2,
      userId: null,
    })).toBe(true);

    const next = useMobileStore.getState();
    expect(next.sessions).toEqual([]);
    expect(next.authenticatedUserId).toBeNull();
    expect(next.controlTarget).toBeNull();
  });
});
