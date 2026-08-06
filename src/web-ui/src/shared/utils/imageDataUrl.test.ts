// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';
import { createBrowserImageDataUrl, getImageMimeType, isTiffPath } from './imageDataUrl';

vi.mock('utif', () => ({
  decode: vi.fn(() => [{ width: 1, height: 1, data: new Uint8Array() }]),
  decodeImage: vi.fn(),
  toRGBA8: vi.fn(() => new Uint8Array([255, 0, 0, 255])),
}));

describe('imageDataUrl', () => {
  it('recognizes both TIFF extensions', () => {
    expect(isTiffPath('image.tif')).toBe(true);
    expect(isTiffPath('IMAGE.TIFF?version=1')).toBe(true);
    expect(isTiffPath('image.png')).toBe(false);
  });

  it('returns the TIFF MIME type', () => {
    expect(getImageMimeType('image.tif')).toBe('image/tiff');
    expect(getImageMimeType('image.tiff')).toBe('image/tiff');
  });

  it('keeps browser-supported images as base64 data URLs', async () => {
    await expect(createBrowserImageDataUrl('image.png', 'AQID')).resolves.toBe(
      'data:image/png;base64,AQID',
    );
  });

  it('decodes TIFF content and renders it as a PNG data URL', async () => {
    const putImageData = vi.fn();
    const toDataURL = vi.fn(() => 'data:image/png;base64,converted');
    class MockImageData {
      constructor(
        public data: Uint8ClampedArray,
        public width: number,
        public height: number,
      ) {}
    }
    vi.stubGlobal('ImageData', MockImageData);
    vi.spyOn(document, 'createElement').mockReturnValue({
      width: 0,
      height: 0,
      getContext: () => ({ putImageData }),
      toDataURL,
    } as unknown as HTMLCanvasElement);

    await expect(createBrowserImageDataUrl('image.tiff', 'AQID')).resolves.toBe(
      'data:image/png;base64,converted',
    );
    expect(putImageData).toHaveBeenCalledOnce();
    expect(toDataURL).toHaveBeenCalledWith('image/png');
  });
});
