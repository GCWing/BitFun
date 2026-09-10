import { describe, expect, it, vi } from 'vitest';
import type { RemoteConnectStatus } from '../api/service-api/RemoteConnectAPI';
import { createRemoteConnectStatusSource } from './remoteConnectStatus';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const offline: RemoteConnectStatus = {
  is_connected: false,
  pairing_state: 'idle',
  active_method: null,
  peer_device_name: null,
  peer_user_id: null,
  bot_connected: null,
  bot_verbose_mode: false,
};
const online: RemoteConnectStatus = { ...offline, account_control_connected: true };

describe('shared remote status snapshot', () => {
  it('coalesces slow dialog and sidebar reads without starving the first response', async () => {
    const pending = deferred<RemoteConnectStatus>();
    const read = vi.fn(() => pending.promise);
    const source = createRemoteConnectStatusSource(read);
    const dialog = vi.fn();
    const sidebar = vi.fn();
    source.subscribe(dialog);
    source.subscribe(sidebar);
    const first = source.refresh();
    const second = source.refresh();
    const third = source.refresh();
    expect(read).toHaveBeenCalledOnce();
    expect(first).toBe(second);
    expect(first).toBe(third);
    pending.resolve(online);
    await Promise.all([first, second, third]);
    expect(source.getSnapshot()).toEqual({ status: online, state: 'ready' });
    expect(dialog).toHaveBeenCalledOnce();
    expect(sidebar).toHaveBeenCalledOnce();
  });

  it('fences replies started before and during a stop so late online data cannot resurrect a connection', async () => {
    const beforeStop = deferred<RemoteConnectStatus>();
    const duringStop = deferred<RemoteConnectStatus>();
    const read = vi.fn().mockReturnValueOnce(beforeStop.promise).mockReturnValueOnce(duringStop.promise).mockResolvedValue(offline);
    const source = createRemoteConnectStatusSource(read);
    const before = source.refresh();
    source.invalidateReads();
    const during = source.refresh();
    source.invalidateReads();
    await source.refresh();
    beforeStop.resolve(online);
    duringStop.resolve(online);
    expect(await before).toBeNull();
    expect(await during).toBeNull();
    expect(source.getSnapshot()).toEqual({ status: offline, state: 'ready' });
  });

  it('reports unavailable on a current failure and recovers on the next confirmed snapshot', async () => {
    const read = vi.fn().mockRejectedValueOnce(new Error('connection unavailable')).mockResolvedValue(online);
    const source = createRemoteConnectStatusSource(read);
    await expect(source.refresh()).rejects.toThrow('connection unavailable');
    expect(source.getSnapshot()).toEqual({ status: null, state: 'unavailable' });
    await source.refresh();
    expect(source.getSnapshot()).toEqual({ status: online, state: 'ready' });
  });

  it('does not let an old account response or error overwrite a replacement account', async () => {
    for (const fail of [false, true]) {
      const pending = deferred<RemoteConnectStatus>();
      const read = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(offline);
      const source = createRemoteConnectStatusSource(read);
      const old = source.refresh().catch(() => null);
      source.invalidate();
      expect(source.getSnapshot()).toEqual({ status: null, state: 'loading' });
      await source.refresh();
      if (fail) pending.reject(new Error('old account unavailable'));
      else pending.resolve(online);
      await old;
      expect(source.getSnapshot()).toEqual({ status: offline, state: 'ready' });
    }
  });
});
