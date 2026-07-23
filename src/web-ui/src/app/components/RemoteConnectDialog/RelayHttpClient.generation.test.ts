import { describe, expect, it, vi } from 'vitest';
import {
  DelegatedAccountChangedError,
  RelayHttpClient,
} from '../../../../../mobile-web/src/services/RelayHttpClient';

const masterKey = btoa(String.fromCharCode(...new Uint8Array(32).fill(7)));
const replacementMasterKey = btoa(String.fromCharCode(...new Uint8Array(32).fill(8)));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('RelayHttpClient delegated identity generations', () => {
  it('advances the target epoch for an initial delegated account owner', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const changes: number[] = [];
    client.onControlTargetChange((snapshot) => changes.push(snapshot.epoch));
    vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
      resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
      user_id: 'user-a', device_id: 'home-a',
    });
    const epoch = client.controlTargetEpoch;

    await expect(client.requestDelegatedIdentity()).resolves.toBe(true);

    expect(client.controlTargetEpoch).toBeGreaterThan(epoch);
    expect(client.getControlTargetSnapshot()).toMatchObject({
      deviceId: 'home-a',
      homeDeviceId: 'home-a',
      epoch: client.controlTargetEpoch,
    });
    expect(changes).toEqual([client.controlTargetEpoch]);
  });

  it('forces a target epoch advance when the account is replaced on the same home', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'shared-home',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-b', master_key: replacementMasterKey,
        user_id: 'user-b', device_id: 'shared-home',
      });
    await client.requestDelegatedIdentity();
    const firstOwnerEpoch = client.controlTargetEpoch;

    await client.requestDelegatedIdentity({ force: true });

    expect(client.pairedDeviceId).toBe('shared-home');
    expect(client.controlTargetEpoch).toBeGreaterThan(firstOwnerEpoch);
  });

  it('does not manufacture a target change for an initial unavailable identity', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const changes: number[] = [];
    client.onControlTargetChange((snapshot) => changes.push(snapshot.epoch));
    vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
      resp: 'error', message: 'Not logged in',
    });
    const epoch = client.controlTargetEpoch;

    await expect(client.requestDelegatedIdentity()).resolves.toBe(false);

    expect(client.controlTargetEpoch).toBe(epoch);
    expect(client.pairedDeviceId).toBeNull();
    expect(client.homeDeviceId).toBeNull();
    expect(changes).toEqual([]);
  });

  it('commits only the latest concurrent delegation response', async () => {
    const client = new RelayHttpClient('https://relay.example.com///', 'room');
    const first = deferred<any>();
    const second = deferred<any>();
    vi.spyOn(client, 'sendCommand')
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const firstRequest = client.requestDelegatedIdentity({ force: true });
    const secondRequest = client.requestDelegatedIdentity({ force: true });
    second.resolve({
      resp: 'delegate_identity', token: 'token-b', master_key: masterKey,
      user_id: 'user-b', device_id: 'home-b',
    });
    await expect(secondRequest).resolves.toBe(true);
    first.resolve({
      resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
      user_id: 'user-a', device_id: 'home-a',
    });
    await expect(firstRequest).resolves.toBe(false);

    expect(client.homeDeviceId).toBe('home-b');
    expect(client.pairedDeviceId).toBe('home-b');
  });

  it('keeps the last committed identity available while a forced refresh is pending', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const refresh = deferred<any>();
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      })
      .mockImplementationOnce(() => refresh.promise);
    await client.requestDelegatedIdentity({ force: true });

    const pendingRefresh = client.requestDelegatedIdentity({ force: true });
    expect(client.hasDelegatedIdentity).toBe(true);
    expect(client.delegatedUserId).toBe('user-a');
    const fetchMock = vi.fn().mockResolvedValueOnce(new Response(
      JSON.stringify([{ device_id: 'old-a', device_name: 'Old A', online: true }]),
      { status: 200, headers: { 'Content-Type': 'application/json' } },
    ));
    (client as any).fetchWithTimeout = fetchMock;
    await expect(client.listDevices()).rejects.toThrow('Delegated identity changed');
    expect(fetchMock).not.toHaveBeenCalled();

    refresh.resolve({
      resp: 'delegate_identity', token: 'token-a2', master_key: masterKey,
      user_id: 'user-a', device_id: 'home-a',
    });
    await expect(pendingRefresh).resolves.toBe(true);
  });

  it('restores the last confirmed identity after a forced refresh transport failure', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      })
      .mockRejectedValueOnce(new Error('Desktop temporarily unavailable'));
    await client.requestDelegatedIdentity({ force: true });

    await expect(client.requestDelegatedIdentity({ force: true }))
      .rejects.toThrow('Desktop temporarily unavailable');
    (client as any).fetchWithTimeout = vi.fn().mockResolvedValueOnce(new Response(
      JSON.stringify([{ device_id: 'home-a', device_name: 'Home A', online: true }]),
      { status: 200, headers: { 'Content-Type': 'application/json' } },
    ));

    await expect(client.listDevices()).resolves.toHaveLength(1);
  });

  it('does not let an old 401 clear a newly delegated account', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
      resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
      user_id: 'user-a', device_id: 'home-a',
    });
    await client.requestDelegatedIdentity({ force: true });
    const oldAccountEpoch = client.delegatedAccountEpoch;

    const oldList = deferred<Response>();
    (client as any).fetchWithTimeout = vi.fn(() => oldList.promise);
    const staleRequest = client.listDevices();

    vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
      resp: 'delegate_identity', token: 'token-b', master_key: masterKey,
      user_id: 'user-b', device_id: 'home-b',
    });
    await client.requestDelegatedIdentity({ force: true });

    const unauthorized = new Error('List devices failed: HTTP 401') as Error & { status: number };
    unauthorized.status = 401;
    oldList.reject(unauthorized);
    await expect(staleRequest).rejects.toBeInstanceOf(DelegatedAccountChangedError);
    expect(client.hasDelegatedIdentity).toBe(true);
    expect(client.homeDeviceId).toBe('home-b');
    expect(client.pairedDeviceId).toBe('home-b');
    expect(client.delegatedAccountEpoch).toBeGreaterThan(oldAccountEpoch);
    expect(client.delegatedUserId).toBe('user-b');
  });

  it('returns a successful device list after refreshing an unauthorized identity', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'expired-token', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'fresh-token', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      });
    await client.requestDelegatedIdentity({ force: true });
    const expiredGeneration = client.delegatedIdentityGeneration;

    (client as any).fetchWithTimeout = vi.fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(new Response(JSON.stringify([
        { device_id: 'peer-a', device_name: 'Peer A', online: true },
      ]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }));

    await expect(client.listDevices()).resolves.toEqual([
      { device_id: 'peer-a', device_name: 'Peer A', online: true },
    ]);
    expect(client.delegatedIdentityGeneration).toBeGreaterThan(expiredGeneration);
  });

  it('accepts a 401 retry while advancing the account epoch for a replacement account', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'expired-token', master_key: masterKey,
        user_id: 'user-a', device_id: 'shared-home',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'replacement-token', master_key: replacementMasterKey,
        user_id: 'user-b', device_id: 'shared-home',
      });
    await client.requestDelegatedIdentity({ force: true });
    const accountEpoch = client.delegatedAccountEpoch;
    client.setPairedDeviceId('peer-from-user-a');

    (client as any).fetchWithTimeout = vi.fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(new Response(JSON.stringify([
        { device_id: 'peer-b', device_name: 'Peer B', online: true },
      ]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }));

    await expect(client.listDevices()).resolves.toEqual([
      { device_id: 'peer-b', device_name: 'Peer B', online: true },
    ]);
    expect(client.delegatedAccountEpoch).toBeGreaterThan(accountEpoch);
    expect(client.pairedDeviceId).toBe('shared-home');
    expect(client.delegatedUserId).toBe('user-b');
  });

  it('advances the account epoch when the user changes on the same home device', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'shared-home',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-b', master_key: masterKey,
        user_id: 'user-b', device_id: 'shared-home',
      });

    await client.requestDelegatedIdentity({ force: true });
    const accountEpoch = client.delegatedAccountEpoch;
    client.setPairedDeviceId('peer-from-user-a');
    await client.requestDelegatedIdentity({ force: true });

    expect(client.delegatedAccountEpoch).toBeGreaterThan(accountEpoch);
    expect(client.pairedDeviceId).toBe('shared-home');
    expect(client.delegatedUserId).toBe('user-b');
  });

  it('keeps the account epoch stable across token refresh for the same account', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a2', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      });

    await client.requestDelegatedIdentity({ force: true });
    const accountEpoch = client.delegatedAccountEpoch;
    const targetEpoch = client.controlTargetEpoch;
    await client.requestDelegatedIdentity({ force: true });

    expect(client.delegatedAccountEpoch).toBe(accountEpoch);
    expect(client.controlTargetEpoch).toBe(targetEpoch);
  });

  it('does not replay an A-owned device RPC after a 401 delegates account B', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'expired-token', master_key: masterKey,
        user_id: 'user-a', device_id: 'shared-home',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'replacement-token', master_key: replacementMasterKey,
        user_id: 'user-b', device_id: 'shared-home',
      });
    await client.requestDelegatedIdentity({ force: true });
    client.setPairedDeviceId('peer-owned-by-a');
    const changes: string[] = [];
    client.onDelegatedAccountOwnerChange((change) => changes.push(change.kind));
    const fetchMock = vi.fn().mockResolvedValueOnce(new Response(null, { status: 401 }));
    (client as any).fetchWithTimeout = fetchMock;

    await expect(client.sendDeviceRpc('peer-owned-by-a', { cmd: 'get_sessions' }))
      .rejects.toBeInstanceOf(DelegatedAccountChangedError);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(changes).toEqual(['replacement']);
    expect(client.delegatedUserId).toBe('user-b');
    expect(client.pairedDeviceId).toBe('shared-home');
  });

  it('notifies a listener when a soft-timed-out initial delegation commits later', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const delegation = deferred<any>();
    vi.spyOn(client, 'sendCommand').mockImplementationOnce(() => delegation.promise);
    const pending = client.requestDelegatedIdentity();
    const changes: Array<{ kind: string; userId: string | null }> = [];
    client.onDelegatedAccountOwnerChange((change) => changes.push({
      kind: change.kind,
      userId: change.userId,
    }));

    delegation.resolve({
      resp: 'delegate_identity', token: 'late-token', master_key: masterKey,
      user_id: 'late-user', device_id: 'late-home',
    });

    await expect(pending).resolves.toBe(true);
    expect(changes).toEqual([{ kind: 'initial', userId: 'late-user' }]);
  });

  it('can replay an identity committed before the app owner listener attaches', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
      resp: 'delegate_identity', token: 'token-b', master_key: replacementMasterKey,
      user_id: 'user-b', device_id: 'home-b',
    });
    await client.requestDelegatedIdentity();

    const changes: Array<{ kind: string; userId: string | null }> = [];
    client.onDelegatedAccountOwnerChange((change) => changes.push({
      kind: change.kind,
      userId: change.userId,
    }), { emitCurrent: true });

    expect(changes).toEqual([{ kind: 'initial', userId: 'user-b' }]);
  });

  it('publishes unavailable only after a confirmed no-identity response', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        user_id: 'user-a', device_id: 'home-a',
      })
      .mockResolvedValueOnce({ resp: 'error', message: 'Not logged in' });
    await client.requestDelegatedIdentity({ force: true });
    const changes: string[] = [];
    client.onDelegatedAccountOwnerChange((change) => changes.push(change.kind));

    await expect(client.requestDelegatedIdentity({ force: true })).resolves.toBe(false);
    expect(changes).toEqual(['unavailable']);
    expect(client.hasDelegatedIdentity).toBe(false);
    expect(client.homeDeviceId).toBeNull();
    expect(client.pairedDeviceId).toBeNull();
  });

  it('uses the master key when legacy delegated responses omit user_id', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-a', master_key: masterKey,
        device_id: 'shared-home',
      })
      .mockResolvedValueOnce({
        resp: 'delegate_identity', token: 'token-b', master_key: replacementMasterKey,
        device_id: 'shared-home',
      });

    await client.requestDelegatedIdentity({ force: true });
    const accountEpoch = client.delegatedAccountEpoch;
    client.setPairedDeviceId('legacy-peer-a');
    await client.requestDelegatedIdentity({ force: true });

    expect(client.delegatedAccountEpoch).toBeGreaterThan(accountEpoch);
    expect(client.pairedDeviceId).toBe('shared-home');
    expect(client.delegatedUserId).toBeNull();
  });
});
