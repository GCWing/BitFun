// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MarkdownContent } from '../../../../../mobile-web/src/components/ChatMarkdown';
import { ArtifactImageReader } from '../../../../../mobile-web/src/components/RemoteArtifactImage';

vi.mock('../../../../../mobile-web/src/i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock('../../../../../mobile-web/src/theme', () => ({ useTheme: () => ({ isDark: false }) }));

describe('mobile output artifact rendering', () => {
  let container: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => { act(() => root.unmount()); container.remove(); vi.unstubAllGlobals(); });

  it.each(['preview.png', 'computer://preview.png', 'file:///workspace/preview.png', 'computer://preview%20%E5%9B%BE.png', 'openbitfun://runtime/artifacts/preview.png'])(
    'reads %s from the session and displays transferred bytes', async source => {
      const read = vi.fn().mockResolvedValue('data:image/png;base64,YQ==');
      await act(async () => root.render(<ArtifactImageReader.Provider value={read}><MarkdownContent content={`![Preview](${source})`} /></ArtifactImageReader.Provider>));
      expect(read).toHaveBeenCalledWith(source.startsWith('openbitfun:') ? source : source.startsWith('file:') ? '/workspace/preview.png' : source.includes('%') ? 'preview 图.png' : 'preview.png');
      expect(container.querySelector('img')?.src).toBe('data:image/png;base64,YQ==');
    },
  );

  it('projects image and HTML outputs once and excludes code examples', async () => {
    const read = vi.fn().mockResolvedValue('data:image/png;base64,YQ==');
    const info = vi.fn(async (path: string) => ({ name: path, size: 100, mimeType: 'image/png' }));
    const content = '![Preview](computer://preview.png)\n\n[Image](computer://preview.png)\n\n[Animation](animation.html)\n\n`[Example](computer://secret.png)`\n\n```markdown\n![Example](computer://hidden.png)\n```';
    await act(async () => root.render(<ArtifactImageReader.Provider value={read}><MarkdownContent content={content} onGetFileInfo={info} onFileDownload={vi.fn()} /></ArtifactImageReader.Provider>));
    expect(info.mock.calls.map(([path]) => path)).toEqual(['preview.png', 'animation.html']);
    expect(container.querySelectorAll('.file-card')).toHaveLength(2);
  });

  it('preserves an image DOM node while more text streams', async () => {
    const read = vi.fn().mockResolvedValue('data:image/png;base64,YQ==');
    const render = (content: string) => root.render(<ArtifactImageReader.Provider value={read}><MarkdownContent content={content} /></ArtifactImageReader.Provider>);
    await act(async () => render('![Preview](preview.png)'));
    const image = container.querySelector('img');
    await act(async () => render('![Preview](preview.png)\n\nMore text'));
    expect(container.querySelector('img')).toBe(image);
    expect(read).toHaveBeenCalledTimes(1);
  });

  it('hides old pixels and rejects stale responses when the session reader changes', async () => {
    let complete!: (value: string) => void;
    const first = vi.fn(() => new Promise<string>(resolve => { complete = resolve; }));
    const second = vi.fn().mockResolvedValue('data:image/png;base64,bmV3');
    const render = (read: (path: string) => Promise<string>) => root.render(<ArtifactImageReader.Provider value={read}><MarkdownContent content="![Preview](same.png)" /></ArtifactImageReader.Provider>);
    await act(async () => render(first));
    await act(async () => render(second));
    await act(async () => complete('data:image/png;base64,b2xk'));
    expect(container.querySelector('img')?.src).toBe('data:image/png;base64,bmV3');
  });

  it('offers retry and download after a failed preview instead of a broken image', async () => {
    const read = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue('data:image/png;base64,YQ==');
    const download = vi.fn().mockResolvedValue(undefined);
    await act(async () => root.render(<ArtifactImageReader.Provider value={read}><MarkdownContent content="![Preview](preview.png)" onFileDownload={download} /></ArtifactImageReader.Provider>));
    expect(container.querySelector('img')).toBeNull();
    const retry = [...container.querySelectorAll('button')].find(button => button.textContent === 'devices.retry');
    expect(retry).toBeDefined();
    expect(container.textContent).toContain('chat.clickToDownload');
    await act(async () => retry!.click());
    expect(read).toHaveBeenLastCalledWith('preview.png', true);
    expect(container.querySelector('img')?.src).toBe('data:image/png;base64,YQ==');
  });
});
