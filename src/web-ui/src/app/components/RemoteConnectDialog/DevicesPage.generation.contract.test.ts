import { beforeEach, describe, expect, it } from 'vitest';
import { reconcileDelegatedAccountOwner } from '../../../../../mobile-web/src/services/delegatedAccountOwner';
import { useMobileStore } from '../../../../../mobile-web/src/services/store';

const sessionA = {
  session_id: 'session-a',
  name: 'Session A',
  agent_type: 'general',
  created_at: '2026-07-22T00:00:00Z',
  updated_at: '2026-07-22T00:00:00Z',
  message_count: 0,
};

describe('mobile delegated account UI ownership', () => {
  beforeEach(() => {
    useMobileStore.getState().resetConnectionState();
  });

  it('adopts a late initial delegation without discarding matching pairing state', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);

    expect(reconcileDelegatedAccountOwner({
      kind: 'initial',
      epoch: 1,
      userId: 'user-a',
      homeDeviceId: 'home-a',
    })).toBe(false);

    expect(useMobileStore.getState().sessions).toHaveLength(1);
    expect(useMobileStore.getState().controlTarget).toEqual({
      deviceId: 'home-a',
      deviceName: null,
      isHome: true,
    });
  });

  it('clears A-owned state when a late initial delegation proves account B', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);
    store.setControlTarget({ deviceId: 'home-a', deviceName: 'A', isHome: true });

    expect(reconcileDelegatedAccountOwner({
      kind: 'initial',
      epoch: 1,
      userId: 'user-b',
      homeDeviceId: 'home-b',
    })).toBe(true);

    const next = useMobileStore.getState();
    expect(next.sessions).toEqual([]);
    expect(next.authenticatedUserId).toBe('user-b');
    expect(next.controlTarget).toEqual({
      deviceId: 'home-b',
      deviceName: null,
      isHome: true,
    });
  });

  it('clears known user and target when delegation becomes unavailable', () => {
    const store = useMobileStore.getState();
    store.setAuthenticatedUserId('user-a');
    store.setSessions([sessionA]);
    store.setControlTarget({ deviceId: 'peer-a', deviceName: 'Peer A', isHome: false });

    expect(reconcileDelegatedAccountOwner({
      kind: 'unavailable',
      epoch: 2,
      userId: null,
      homeDeviceId: null,
    })).toBe(true);

    const next = useMobileStore.getState();
    expect(next.sessions).toEqual([]);
    expect(next.authenticatedUserId).toBeNull();
    expect(next.controlTarget).toBeNull();
  });
});
