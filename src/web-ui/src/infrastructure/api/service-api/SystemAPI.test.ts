import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SystemAPI } from './SystemAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('SystemAPI', () => {
  let systemAPI: SystemAPI;

  beforeEach(() => {
    systemAPI = new SystemAPI();
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('does not invoke the updater in development mode', async () => {
    vi.stubEnv('DEV', true);

    await expect(systemAPI.checkForUpdates()).rejects.toThrow(
      'Update checks are disabled in development mode',
    );
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('invokes the updater outside development mode', async () => {
    vi.stubEnv('DEV', false);
    const response = {
      updateAvailable: false,
      currentVersion: '1.0.0',
      latestVersion: null,
      releaseNotes: null,
      releaseDate: null,
    };
    invokeMock.mockResolvedValueOnce(response);

    await expect(systemAPI.checkForUpdates()).resolves.toEqual(response);
    expect(invokeMock).toHaveBeenCalledWith('check_for_updates', {
      request: {},
    });
  });

  it('reads the persisted desktop preference', async () => {
    invokeMock.mockResolvedValueOnce(false);

    await expect(systemAPI.getPreventSleepEnabled()).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledWith('get_prevent_sleep_enabled', {
      request: {},
    });
  });

  it('sends the requested app-wide state', async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await expect(systemAPI.setPreventSleepEnabled(true)).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith('set_prevent_sleep_enabled', {
      request: { enabled: true },
    });
  });

  it('checks path existence through an explicit remote workspace scope', async () => {
    invokeMock.mockResolvedValueOnce(true);

    await expect(systemAPI.checkPathExists(
      '/remote/workspace/src/existing.ts',
      'remote-connection-1',
    )).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('check_path_exists', {
      request: {
        path: '/remote/workspace/src/existing.ts',
        remoteConnectionId: 'remote-connection-1',
      },
    });
  });
});
