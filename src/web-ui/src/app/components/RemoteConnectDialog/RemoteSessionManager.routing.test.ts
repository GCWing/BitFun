import { describe, expect, it, vi } from 'vitest';
import { RelayHttpClient } from '../../../../../mobile-web/src/services/RelayHttpClient';
import {
  RemoteControlTargetChangedError,
  RemoteSessionManager,
} from '../../../../../mobile-web/src/services/RemoteSessionManager';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('mobile RemoteSessionManager target routing', () => {
  it('invalidates an active room request and starts a usable generation after late identity', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const workspace = deferred<any>();
    const targetChanges: number[] = [];
    client.onControlTargetChange((snapshot) => targetChanges.push(snapshot.epoch));
    let workspaceRequestCount = 0;
    vi.spyOn(client, 'sendCommand').mockImplementation((command: any) => {
      if (command.cmd === 'get_workspace_info') {
        workspaceRequestCount += 1;
        if (workspaceRequestCount === 1) return workspace.promise;
        return Promise.resolve({
          resp: 'workspace_info',
          has_workspace: true,
          project_name: 'Current home workspace',
        });
      }
      if (command.cmd === 'get_delegated_identity') {
        return Promise.resolve({
          resp: 'delegate_identity',
          token: 'late-token',
          master_key: btoa(String.fromCharCode(...new Uint8Array(32).fill(7))),
          user_id: 'late-user',
          device_id: 'home-device',
        });
      }
      throw new Error(`Unexpected command: ${command.cmd}`);
    });
    const manager = new RemoteSessionManager(client);
    const initialEpoch = client.controlTargetEpoch;

    const activeRoomRequest = manager.getWorkspaceInfo();
    await expect(client.requestDelegatedIdentity()).resolves.toBe(true);
    expect(client.pairedDeviceId).toBe('home-device');
    expect(client.homeDeviceId).toBe('home-device');
    expect(client.controlTargetEpoch).toBeGreaterThan(initialEpoch);
    expect(targetChanges).toEqual([client.controlTargetEpoch]);

    workspace.resolve({
      resp: 'workspace_info',
      has_workspace: true,
      project_name: 'Home workspace',
    });
    await expect(activeRoomRequest).rejects.toBeInstanceOf(RemoteControlTargetChangedError);
    await expect(manager.getWorkspaceInfo()).resolves.toMatchObject({
      project_name: 'Current home workspace',
    });
  });

  it('invalidates an in-flight room request on disconnect without delegated credentials', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    const workspace = deferred<any>();
    vi.spyOn(client, 'sendCommand').mockImplementation(() => workspace.promise);
    const manager = new RemoteSessionManager(client);
    const initialEpoch = client.controlTargetEpoch;

    const activeRoomRequest = manager.getWorkspaceInfo();
    client.resetConnectionIdentity();
    expect(client.controlTargetEpoch).toBeGreaterThan(initialEpoch);
    workspace.resolve({ resp: 'workspace_info', has_workspace: true });

    await expect(activeRoomRequest).rejects.toBeInstanceOf(RemoteControlTargetChangedError);
  });

  it('never falls back to the home room when a remote target temporarily lacks credentials', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    client.homeDeviceId = 'home-device';
    client.setPairedDeviceId('remote-device');
    const remoteRequest = vi.spyOn(client, 'sendDeviceRpc')
      .mockRejectedValueOnce(new Error('No delegated identity'));
    const homeRequest = vi.spyOn(client, 'sendCommand')
      .mockResolvedValueOnce({ resp: 'workspace_info', has_workspace: true });
    const manager = new RemoteSessionManager(client);

    await expect(manager.getWorkspaceInfo()).rejects.toThrow('No delegated identity');
    expect(remoteRequest).toHaveBeenCalledWith(
      'remote-device',
      expect.objectContaining({ cmd: 'get_workspace_info' }),
      { retryable: true },
    );
    expect(homeRequest).not.toHaveBeenCalled();
  });

  it('rejects a deferred A response after the control target switches to B', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    client.homeDeviceId = 'home-device';
    client.setPairedDeviceId('device-a');
    const responseA = deferred<any>();
    vi.spyOn(client, 'sendDeviceRpc').mockImplementation((deviceId) => {
      if (deviceId === 'device-a') return responseA.promise;
      return Promise.resolve({
        resp: 'workspace_info',
        has_workspace: true,
        project_name: 'Device B',
      });
    });
    const manager = new RemoteSessionManager(client);

    const requestA = manager.getWorkspaceInfo();
    client.setPairedDeviceId('device-b');
    await expect(manager.getWorkspaceInfo()).resolves.toMatchObject({
      project_name: 'Device B',
    });
    responseA.resolve({
      resp: 'workspace_info',
      has_workspace: true,
      project_name: 'Device A',
    });

    await expect(requestA).rejects.toBeInstanceOf(RemoteControlTargetChangedError);
  });

  it('rejects a deferred A error after an A to B to A ABA switch', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    client.homeDeviceId = 'home-device';
    client.setPairedDeviceId('device-a');
    const responseA = deferred<any>();
    vi.spyOn(client, 'sendDeviceRpc').mockImplementation(() => responseA.promise);
    const manager = new RemoteSessionManager(client);
    const firstAEpoch = client.controlTargetEpoch;

    const requestA = manager.getWorkspaceInfo();
    client.setPairedDeviceId('device-b');
    client.setPairedDeviceId('device-a');
    expect(client.controlTargetEpoch).toBeGreaterThan(firstAEpoch);
    responseA.reject(new Error('Device A request failed'));

    await expect(requestA).rejects.toBeInstanceOf(RemoteControlTargetChangedError);
    await expect(requestA).rejects.not.toThrow('Device A request failed');
  });

  it('does not send a later file chunk to B after a download starts on A', async () => {
    const client = new RelayHttpClient('https://relay.example.com', 'room');
    client.homeDeviceId = 'home-device';
    client.setPairedDeviceId('device-a');
    const remoteRequest = vi.spyOn(client, 'sendDeviceRpc').mockResolvedValue({
      resp: 'file_chunk',
      name: 'from-a.txt',
      chunk_base64: 'QUFB',
      offset: 0,
      chunk_size: 3,
      total_size: 6,
      mime_type: 'text/plain',
    });
    const manager = new RemoteSessionManager(client);

    const download = manager.readFile('/tmp/from-a.txt', undefined, () => {
      client.setPairedDeviceId('device-b');
    });

    await expect(download).rejects.toBeInstanceOf(RemoteControlTargetChangedError);
    expect(remoteRequest).toHaveBeenCalledTimes(1);
    expect(remoteRequest).toHaveBeenCalledWith(
      'device-a',
      expect.objectContaining({
        cmd: 'read_file_chunk',
        offset: 0,
      }),
      { retryable: true },
    );
  });
});
