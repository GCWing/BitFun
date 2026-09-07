// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UserMessageImage } from './UserMessageImage';

const { readFileContent } = vi.hoisted(() => ({ readFileContent: vi.fn() }));
vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({ workspaceAPI: { readFileContent } }));
vi.mock('@/infrastructure/i18n', () => ({ useI18n: () => ({ t: (_key: string, values: { message: string }) => `Load failed: ${values.message}` }) }));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ warn: vi.fn(), debug: vi.fn(), info: vi.fn(), error: vi.fn() }) }));

const image = { id: 'image', name: 'Photo.png', imagePath: '/Lark images/Photo.png', mimeType: 'image/png' };

describe('UserMessageImage', () => {
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


  it('reads host image bytes through the transport and previews the same data URL', async () => {
    readFileContent.mockResolvedValue('aW1hZ2U=');
    const onPreview = vi.fn();
    await render(<UserMessageImage image={image} onPreview={onPreview} />);
    const thumbnail = container.querySelector('img')!;
    expect(readFileContent).toHaveBeenCalledWith(image.imagePath, 'base64');
    expect(thumbnail.getAttribute('src')).toBe('data:image/png;base64,aW1hZ2U=');
    act(() => thumbnail.click());
    expect(onPreview).toHaveBeenCalledWith(thumbnail.getAttribute('src'));
  });

  it('uses embedded clipboard data without reading a host file', async () => {
    await render(<UserMessageImage image={{ ...image, dataUrl: 'data:image/png;base64,AA==' }} onPreview={vi.fn()} />);
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,AA==');
    expect(readFileContent).not.toHaveBeenCalled();
  });

  it('displays read failures without falling back to controller-local URLs', async () => {
    readFileContent.mockRejectedValue(new Error('Host offline'));
    await render(<UserMessageImage image={image} onPreview={vi.fn()} />);
    expect(container.querySelector('[role=alert]')!.textContent).toContain('Host offline');
    expect(container.querySelector('img')).toBeNull();
    expect(readFileContent).toHaveBeenCalledTimes(1);
  });

  it('ignores a stale image read after the attachment changes', async () => {
    let finishOld!: (content: string) => void;
    readFileContent.mockImplementationOnce(() => new Promise<string>(resolve => { finishOld = resolve; }));
    readFileContent.mockResolvedValueOnce('NEW');
    await render(<UserMessageImage image={image} onPreview={vi.fn()} />);
    await render(<UserMessageImage image={{ ...image, imagePath: '/new.png' }} onPreview={vi.fn()} />);
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,NEW');
    await act(async () => { finishOld('OLD'); });
    expect(container.querySelector('img')!.getAttribute('src')).toBe('data:image/png;base64,NEW');
  });
});
