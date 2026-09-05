import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useUpdateInstallStore } from './updateInstallStore';

const mocks = vi.hoisted(() => ({ pending: vi.fn(), download: vi.fn(), install: vi.fn() }));
vi.mock('@/infrastructure/api', () => ({ systemAPI: {
  getPendingUpdate: mocks.pending,
  installPendingUpdate: mocks.install,
} }));
vi.mock('./installUpdateWithProgress', () => ({ installUpdateWithProgress: mocks.download }));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ error: vi.fn() }) }));

const state = () => useUpdateInstallStore.getState();

beforeEach(() => {
  vi.resetAllMocks();
  mocks.pending.mockResolvedValue(null);
  mocks.download.mockResolvedValue({ version: '2.0.0' });
  mocks.install.mockResolvedValue(undefined);
  useUpdateInstallStore.setState({
    status: 'idle', progress: { downloaded: 0, total: null }, error: null,
    startedAt: null, version: null, promptOpen: false, initialized: false,
  });
});

describe('staged app update', () => {
  it('downloads in the background, then requests confirmation without installing', async () => {
    mocks.download.mockImplementation(async (progress) => {
      expect(state().status).toBe('downloading');
      expect(state().promptOpen).toBe(false);
      progress({ downloaded: 100, total: 100 });
      return { version: '2.0.0' };
    });
    await state().startInstall();
    expect(state()).toMatchObject({ status: 'ready', version: '2.0.0', promptOpen: true });
    expect(mocks.install).not.toHaveBeenCalled();
  });

  it('defers without discarding the package and installs from About without downloading again', async () => {
    await state().startInstall();
    state().deferInstall();
    expect(state()).toMatchObject({ status: 'ready', version: '2.0.0', promptOpen: false });
    await state().confirmInstall();
    expect(mocks.install).not.toHaveBeenCalled();
    state().requestInstall();
    await state().confirmInstall();
    expect(mocks.install).toHaveBeenCalledWith('2.0.0');
    expect(mocks.download).toHaveBeenCalledTimes(1);
    expect(state().status).toBe('installing');
  });

  it('restores a downloaded update after restart without forcing a new prompt', async () => {
    mocks.pending.mockResolvedValue({ version: '2.0.0' });
    await state().initialize();
    await state().startInstall();
    expect(state()).toMatchObject({ status: 'ready', version: '2.0.0', promptOpen: false });
    expect(mocks.download).not.toHaveBeenCalled();
    expect(mocks.install).not.toHaveBeenCalled();
  });

  it('does not mark a failed download or signature check ready', async () => {
    mocks.download.mockRejectedValue(new Error('signature invalid'));
    await state().startInstall();
    expect(state()).toMatchObject({ status: 'error', version: null, promptOpen: false });
    await state().confirmInstall();
    expect(mocks.install).not.toHaveBeenCalled();
  });

  it('retains the pending version after an install failure and permits retry', async () => {
    await state().startInstall();
    mocks.install.mockRejectedValueOnce(new Error('installer unavailable'));
    await state().confirmInstall();
    expect(state()).toMatchObject({ status: 'ready', version: '2.0.0', promptOpen: true, error: 'installer unavailable' });
    await state().confirmInstall();
    expect(mocks.install).toHaveBeenCalledTimes(2);
    expect(mocks.download).toHaveBeenCalledTimes(1);
  });

  it('coalesces startup reads and prevents duplicate downloads and installs', async () => {
    let finish!: (value: { version: string }) => void;
    mocks.download.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    const first = state().startInstall();
    const second = state().startInstall();
    await vi.waitFor(() => expect(mocks.download).toHaveBeenCalledTimes(1));
    finish({ version: '2.0.0' });
    await Promise.all([first, second]);
    expect(mocks.pending).toHaveBeenCalledTimes(1);
    await Promise.all([state().confirmInstall(), state().confirmInstall()]);
    expect(mocks.install).toHaveBeenCalledTimes(1);
    state().deferInstall();
    expect(state().status).toBe('installing');
  });

  it('allows an explicit replacement download after a cached package fails installation', async () => {
    await state().startInstall();
    mocks.install.mockRejectedValueOnce(new Error('package corrupt'));
    await state().confirmInstall();
    mocks.download.mockResolvedValueOnce({ version: '2.1.0' });
    await state().startInstall(true);
    expect(state()).toMatchObject({ status: 'ready', version: '2.1.0', error: null, promptOpen: true });
    expect(mocks.download).toHaveBeenCalledTimes(2);
  });
});
