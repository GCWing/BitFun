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
    invokeMock.mockResolvedValueOnce({
      catalogDigest: 'digest',
      revision: 3,
      currentOptionValues: { 'prevent-sleep': false },
      controlAvailability: { status: 'available', adapter: 'desktop-native', readBack: true },
    });

    await expect(systemAPI.getPreventSleepEnabled()).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledWith('product_control_invoke', {
      request: { action: 'get', capabilityId: 'setting.application.general' },
    });
  });

  it('sends the requested app-wide state', async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await expect(systemAPI.setPreventSleepEnabled(true)).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith('product_control_invoke', {
      request: {
        action: 'configure',
        capabilityId: 'setting.application.general',
        optionId: 'prevent-sleep',
        value: true,
      },
    });
  });
});
