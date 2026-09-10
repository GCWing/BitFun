// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatInputImagePreview } from './ChatInputImagePreview';
import type { ImageContext } from '@/types/context';
vi.mock('@/infrastructure/peer-device/deviceSurface', () => ({ getActiveSurfaceScope: () => ({ epoch: 1 }) }));

const { readFileContent } = vi.hoisted(() => ({ readFileContent: vi.fn() }));
vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({ workspaceAPI: { readFileContent } }));
vi.mock('@/infrastructure/i18n', () => ({ useI18n: () => ({ t: (_key: string, values: { message: string }) => `Load failed: ${values.message}` }) }));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ warn: vi.fn(), debug: vi.fn(), info: vi.fn(), error: vi.fn() }) }));

const image = { id: 'image', imageName: 'Photo.png', imagePath: '/Lark images/Photo.png', mimeType: 'image/png' } as ImageContext;

describe('ChatInputImagePreview', () => {
  let container: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    readFileContent.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => { act(() => root.unmount()); container.remove(); });
  const render = async (element: React.ReactNode) => { await act(async () => { root.render(element); }); };


  it('reads path-only dropped screenshots through the host transport', async () => {
    readFileContent.mockResolvedValue('aW1hZ2U=');
    await render(<ChatInputImagePreview image={image} surfaceEpoch={1} />);
    const thumbnail = container.querySelector('img')!;
    expect(readFileContent).toHaveBeenCalledWith(image.imagePath, 'base64');
    expect(thumbnail.getAttribute('src')).toBe('data:image/png;base64,aW1hZ2U=');
  });

  it('uses embedded clipboard data without reading a host file', async () => {
    await render(<ChatInputImagePreview image={{ ...image, dataUrl: 'data:image/png;base64,AA==' }} surfaceEpoch={1} />);
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,AA==');
    expect(readFileContent).not.toHaveBeenCalled();
  });

  it('displays read failures without falling back to controller-local URLs', async () => {
    readFileContent.mockRejectedValue(new Error('Host offline'));
    await render(<ChatInputImagePreview image={image} surfaceEpoch={1} />);
    expect(container.querySelector('[role=img]')!.getAttribute('aria-label')).toContain('Host offline');
    expect(container.querySelector('img')).toBeNull();
    expect(readFileContent).toHaveBeenCalledTimes(1);
  });

  it('ignores a stale image read after the attachment changes', async () => {
    let finishOld!: (content: string) => void;
    readFileContent.mockImplementationOnce(() => new Promise<string>(resolve => { finishOld = resolve; }));
    readFileContent.mockResolvedValueOnce('NEW');
    await render(<ChatInputImagePreview image={image} surfaceEpoch={1} />);
    await render(<ChatInputImagePreview image={{ ...image, imagePath: '/new.png' }} surfaceEpoch={1} />);
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,NEW');
    await act(async () => { finishOld('OLD'); });
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,NEW');
  });
});
