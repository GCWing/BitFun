// @vitest-environment jsdom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppearanceBackgroundMediaLayer } from './AppearanceBackgroundMedia';

describe('AppearanceBackgroundMedia', () => {
  let container: HTMLDivElement;
  let root: Root;
  let hidden = false;
  let presentedFrame: (() => void) | undefined;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    hidden = false;
    presentedFrame = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    Object.defineProperty(document, 'hidden', {
      configurable: true,
      get: () => hidden,
    });
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    });
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined);
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
    Object.defineProperty(HTMLVideoElement.prototype, 'requestVideoFrameCallback', {
      configurable: true,
      value: vi.fn((callback: () => void) => {
        presentedFrame = callback;
        return 1;
      }),
    });
    Object.defineProperty(HTMLVideoElement.prototype, 'cancelVideoFrameCallback', {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it('renders a muted looping video with a host-owned poster fallback', () => {
    const html = renderToStaticMarkup(<AppearanceBackgroundMediaLayer media={{
      kind: 'video',
      assetId: 'motion',
      posterAssetId: 'poster',
      fit: 'cover',
      position: 'center',
      url: 'blob:test-motion',
      posterUrl: 'blob:test-poster',
    }} />);

    expect(html).toContain('data-openbitfun-background-media="video"');
    expect(html).toContain('src="blob:test-motion"');
    expect(html).toContain('poster="blob:test-poster"');
    expect(html).toContain('autoplay=""');
    expect(html).toContain('loop=""');
    expect(html).toContain('muted=""');
    expect(html).toContain('data-media-ready="false"');
  });

  it('keeps the poster visible until a video frame is presented and hides the video again when suspended', async () => {
    const releaseRevision = vi.fn();
    const retainRevision = vi.fn(() => releaseRevision);
    const media = {
      kind: 'video' as const,
      assetId: 'motion',
      posterAssetId: 'poster',
      fit: 'cover' as const,
      position: 'center',
      url: 'blob:test-motion',
      posterUrl: 'blob:test-poster',
    };
    await act(async () => {
      root.render(<AppearanceBackgroundMediaLayer
        media={media}
        revision={7}
        retainRevision={retainRevision}
      />);
      await Promise.resolve();
    });

    const video = container.querySelector<HTMLVideoElement>('video');
    expect(retainRevision).toHaveBeenCalledWith(7);
    expect(video?.dataset.mediaReady).toBe('false');

    act(() => presentedFrame?.());
    expect(video?.dataset.mediaReady).toBe('true');

    hidden = true;
    act(() => document.dispatchEvent(new Event('visibilitychange')));
    expect(video?.dataset.mediaReady).toBe('false');
    expect(HTMLMediaElement.prototype.pause).toHaveBeenCalled();

    act(() => root.render(<AppearanceBackgroundMediaLayer
      media={media}
      revision={8}
      retainRevision={retainRevision}
    />));
    expect(releaseRevision).toHaveBeenCalledTimes(1);
  });
});
