import { expect, it, vi } from 'vitest';
import { installUpdateWithProgress } from './installUpdateWithProgress';

const mocks = vi.hoisted(() => ({ download: vi.fn(), install: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@/infrastructure/api', () => ({ systemAPI: { downloadUpdate: mocks.download, installUpdate: mocks.install } }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ error: vi.fn() }) }));

it('subscribes before downloading, returns the prepared version and never installs', async () => {
  mocks.listen.mockImplementation(async (_name, callback) => {
    callback({ payload: { downloaded: 12, total: null } });
    return mocks.unlisten;
  });
  mocks.download.mockResolvedValue({ version: '2.0.0' });
  const progress = vi.fn();
  await expect(installUpdateWithProgress(progress)).resolves.toEqual({ version: '2.0.0' });
  expect(progress).toHaveBeenCalledWith({ downloaded: 12, total: null });
  expect(mocks.unlisten).toHaveBeenCalledOnce();
  expect(mocks.install).not.toHaveBeenCalled();
});

it('cleans up its progress listener when signature verification fails', async () => {
  mocks.unlisten.mockClear();
  mocks.listen.mockResolvedValue(mocks.unlisten);
  mocks.download.mockRejectedValue(new Error('signature invalid'));
  await expect(installUpdateWithProgress(vi.fn())).rejects.toThrow('signature invalid');
  expect(mocks.unlisten).toHaveBeenCalledOnce();
});
