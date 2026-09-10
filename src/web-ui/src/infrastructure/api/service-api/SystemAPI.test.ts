import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SystemAPI } from './SystemAPI';

const invokeMock = vi.hoisted(() => vi.fn());
const copyTextToClipboardMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

vi.mock('@/shared/utils/textSelection', () => ({
  copyTextToClipboard: copyTextToClipboardMock,
}));

describe('SystemAPI', () => {
  let systemAPI: SystemAPI;

  beforeEach(() => {
    systemAPI = new SystemAPI();
    invokeMock.mockReset();
    copyTextToClipboardMock.mockReset();
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

  it('allows a background download to outlive the default request timeout without replaying it', async () => {
    invokeMock.mockResolvedValueOnce({ version: '2.0.0' });
    await expect(systemAPI.downloadUpdate()).resolves.toEqual({ version: '2.0.0' });
    expect(invokeMock).toHaveBeenCalledWith('download_update', { request: {} }, {
      timeout: 3600000, retries: 0,
    });
  });

  it('installs only the version the user confirmed and disables automatic mutation retries', async () => {
    await systemAPI.installPendingUpdate('2.0.0');
    expect(invokeMock).toHaveBeenCalledWith('install_pending_update', {
      request: { version: '2.0.0' },
    }, { timeout: 120000, retries: 0 });
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

  it('writes clipboard text on the controller without invoking a host command', async () => {
    copyTextToClipboardMock.mockResolvedValueOnce(true);

    await expect(systemAPI.setClipboard('device-code')).resolves.toBeUndefined();

    expect(copyTextToClipboardMock).toHaveBeenCalledWith('device-code');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('reports a clipboard helper failure to the caller', async () => {
    copyTextToClipboardMock.mockResolvedValueOnce(false);

    await expect(systemAPI.setClipboard('device-code')).rejects.toThrow('Clipboard write failed');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('checks path existence on the active host', async () => {
    invokeMock.mockResolvedValueOnce(true);

    await expect(systemAPI.checkPathExists(
      '/workspace/src/existing.ts',
    )).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('check_path_exists', {
      request: {
        path: '/workspace/src/existing.ts',
      },
    });
  });
});
