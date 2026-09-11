// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AccountIdentityService,
  type AccountIdentityChangedEvent,
  type AccountIdentityServiceDependencies,
  type AccountIdentitySyncPort,
} from './AccountIdentityService';

class FakeSyncPort implements AccountIdentitySyncPort {
  readonly published: AccountIdentityChangedEvent[] = [];
  private readonly listeners = new Set<(event: AccountIdentityChangedEvent) => void>();

  publish(event: AccountIdentityChangedEvent): void {
    this.published.push(event);
  }

  subscribe(listener: (event: AccountIdentityChangedEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit(event: AccountIdentityChangedEvent): void {
    this.listeners.forEach(listener => listener(event));
  }
}

const profile = {
  user: { githubId: 42, login: 'octocat', avatarUrl: 'https://example.com/avatar.png' },
  isAdmin: false,
};

function setup() {
  const syncPort = new FakeSyncPort();
  const api = {
    me: vi.fn<() => Promise<typeof profile | null>>().mockResolvedValue(null),
    authStart: vi.fn().mockResolvedValue({
      transactionId: 'transaction-1',
      authorizationUrl: 'https://github.com/login/oauth/authorize',
      expiresAt: 100,
      pollIntervalSeconds: 1,
    }),
    authPoll: vi.fn().mockResolvedValue('authorized' as const),
    logout: vi.fn().mockResolvedValue(undefined),
    onAccountChanged: vi.fn(() => () => undefined),
  };
  const dependencies: AccountIdentityServiceDependencies = {
    api,
    openExternal: vi.fn().mockResolvedValue(undefined),
    syncPort,
    now: () => 0,
    sleep: vi.fn().mockResolvedValue(undefined),
    sourceId: 'window-a',
  };
  const service = new AccountIdentityService(dependencies);
  return { api, dependencies, service, syncPort };
}

const activeServices: AccountIdentityService[] = [];

afterEach(() => {
  activeServices.splice(0).forEach(service => service.dispose());
});

describe('AccountIdentityService', () => {
  it('uses the MiniApp desktop OAuth flow, keeps tokens out of the renderer, and shares identity', async () => {
    const { api, dependencies, service, syncPort } = setup();
    activeServices.push(service);
    await service.initialize();
    api.me.mockResolvedValue(profile);

    await expect(service.signIn()).resolves.toEqual(profile);

    expect(dependencies.openExternal).toHaveBeenCalledWith(
      'https://github.com/login/oauth/authorize',
    );
    expect(api.authPoll).toHaveBeenCalledWith(expect.objectContaining({
      transactionId: 'transaction-1',
    }));
    expect(api.authPoll.mock.calls[0][0]).not.toHaveProperty('transactionSecret');
    expect(service.getSnapshot()).toMatchObject({
      resolved: true,
      status: 'signed-in',
      me: profile,
    });
    expect(syncPort.published).toEqual([
      expect.objectContaining({ kind: 'identity-changed', sourceId: 'window-a' }),
    ]);
  });

  it('refreshes from another window and broadcasts logout from the shared vault', async () => {
    const { api, service, syncPort } = setup();
    activeServices.push(service);
    await service.initialize();
    api.me.mockResolvedValue(profile);

    syncPort.emit({ kind: 'identity-changed', eventId: 'remote-1', sourceId: 'window-b' });
    await vi.waitFor(() => expect(service.getSnapshot().me).toEqual(profile));

    await service.logout();
    expect(api.logout).toHaveBeenCalledOnce();
    expect(service.getSnapshot()).toMatchObject({ status: 'signed-out', me: null });
    expect(syncPort.published.at(-1)).toEqual(expect.objectContaining({
      kind: 'identity-changed',
      sourceId: 'window-a',
    }));
  });
  it('ignores a profile request started before logout', async () => {
    const { api, service } = setup(); activeServices.push(service);
    await service.initialize();
    let resolve!: (value: typeof profile) => void;
    api.me.mockImplementationOnce(() => new Promise(done => { resolve = done; }));
    const stale = service.refresh();
    await service.logout();
    resolve(profile);
    await stale;
    expect(service.getSnapshot()).toMatchObject({ status: 'signed-out', me: null });
  });

  it('keeps a late initial profile failure from erasing a newer sign-in', async () => {
    const { api, service } = setup(); activeServices.push(service);
    let reject!: (reason: Error) => void;
    api.me.mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; }));
    const stale = service.initialize();
    api.me.mockResolvedValue(profile);
    await service.signIn();
    reject(new Error('old network failure'));
    await stale;
    expect(service.getSnapshot()).toMatchObject({ status: 'signed-in', me: profile });
  });

});
