// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { loadLocalImages } from './loadLocalImages';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

const { readFileContent } = vi.hoisted(() => ({ readFileContent: vi.fn() }));
vi.mock('@/infrastructure/api', () => ({ workspaceAPI: { readFileContent } }));
vi.mock('@/infrastructure/i18n', () => ({ i18nService: { t: (key: string) => key } }));
vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ warn: vi.fn(), error: vi.fn() }),
}));

describe('local image source changes', () => {
  it.each(['resolve', 'reject'] as const)('ignores a stale request that later %ss', async outcome => {
    let resolveOld!: (value: string) => void;
    let rejectOld!: (error: Error) => void;
    const oldPath = `/workspace/${outcome}-old.png`;
    const newPath = `/workspace/${outcome}-new.png`;
    const oldRead = new Promise<string>((resolve, reject) => { resolveOld = resolve; rejectOld = reject; });
    readFileContent.mockImplementation((path: string) => path === oldPath ? oldRead : Promise.resolve('bmV3'));
    const container = document.createElement('div');
    const image = document.createElement('img');
    container.append(image);
    image.dataset.localImage = 'true';
    image.dataset.localPath = oldPath;
    const oldLoad = loadLocalImages(container);
    await vi.waitFor(() => expect(readFileContent).toHaveBeenCalledWith(oldPath));
    image.dataset.localPath = newPath;
    image.alt = 'New image';
    await loadLocalImages(container);
    if (outcome === 'resolve') resolveOld('b2xk');
    else rejectOld(new Error('Old request failed'));
    await oldLoad;
    expect(image.getAttribute('src')).toBe('data:image/png;base64,bmV3');
    expect(image.alt).toBe('New image');
    expect(image.classList.contains('local-image-error')).toBe(false);
  });
  it('keeps identical image paths isolated by their document origin', async () => {
    activateSurface('local');
    const renderImage = () => {
      const container = document.createElement('div');
      const image = document.createElement('img');
      image.dataset.localImage = 'true';
      image.dataset.localPath = '/workspace/scoped-image.png';
      container.append(image);
      return { container, image };
    };
    const a = renderImage();
    const b = renderImage();
    const localRead = vi.fn(async () => 'bG9jYWw=');
    const remoteRead = vi.fn(async () => 'cmVtb3Rl');
    await loadLocalImages(a.container, { scope: { surfaceId: 'local' }, files: { readFileContent: localRead } });
    await loadLocalImages(b.container, { scope: { surfaceId: 'local', remoteConnectionId: 'ssh-a' }, files: { readFileContent: remoteRead } });
    expect(a.image.src).toBe('data:image/png;base64,bG9jYWw=');
    expect(b.image.src).toBe('data:image/png;base64,cmVtb3Rl');
    expect(localRead).toHaveBeenCalledOnce();
    expect(remoteRead).toHaveBeenCalledOnce();
  });

});
