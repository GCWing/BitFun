import { describe, expect, it } from 'vitest';
import { getImagePreviewLimit, getSmallImageDisplayScale, isImagePreviewAllowed } from './ImageViewer';

function getDisplayedSize(width: number, height: number, zoom: number) {
  const displayScale = getSmallImageDisplayScale(width, height);
  return {
    width: width * displayScale * zoom / 100,
    height: height * displayScale * zoom / 100,
  };
}

describe('ImageViewer small image scaling', () => {
  it('makes a 1 by 1 image visible without changing its reported dimensions', () => {
    expect(getSmallImageDisplayScale(1, 1)).toBe(32);
  });

  it('keeps a 1 by 1 image square at low zoom levels', () => {
    expect(getDisplayedSize(1, 1, 25)).toEqual({ width: 8, height: 8 });
    expect(getDisplayedSize(1, 1, 100)).toEqual({ width: 32, height: 32 });
  });

  it('preserves the natural scale for normal images', () => {
    expect(getSmallImageDisplayScale(640, 480)).toBe(1);
  });

  it('does not scale invalid dimensions', () => {
    expect(getSmallImageDisplayScale(0, 1)).toBe(1);
  });
});

describe('ImageViewer preview limits', () => {
  it('rejects a 300 MB image before reading its content', () => {
    expect(isImagePreviewAllowed('large.png', 300 * 1024 * 1024)).toBe(false);
  });

  it('allows regular images up to 64 MB', () => {
    const limit = getImagePreviewLimit('photo.jpg');
    expect(limit).toBe(64 * 1024 * 1024);
    expect(isImagePreviewAllowed('photo.jpg', limit)).toBe(true);
    expect(isImagePreviewAllowed('photo.jpg', limit + 1)).toBe(false);
  });

  it('uses a lower limit for TIFF images decoded on the main thread', () => {
    const limit = getImagePreviewLimit('scan.tiff');
    expect(limit).toBe(16 * 1024 * 1024);
    expect(isImagePreviewAllowed('scan.tif', limit + 1)).toBe(false);
  });
});
