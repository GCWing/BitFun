import { describe, expect, it } from 'vitest';
import { getSmallImageDisplayScale } from './ImageViewer';

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
