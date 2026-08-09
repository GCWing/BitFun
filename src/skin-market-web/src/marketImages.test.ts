import { describe, expect, it, vi } from 'vitest';
import { marketImageSrcSet, marketImageUrl, retryOriginalMarketImage } from './marketImages';

describe('market image URLs', () => {
  it('adds a versioned image variant without losing existing URL state', () => {
    expect(marketImageUrl('/skin/api/v1/artifacts/previews/hash?token=one#focus', 'compact-v1'))
      .toBe('/skin/api/v1/artifacts/previews/hash?token=one&variant=compact-v1#focus');
  });

  it('replaces an existing variant and exposes responsive candidates', () => {
    const source = 'https://market.openbitfun.com/image?variant=old';
    expect(marketImageUrl(source, 'large-v1'))
      .toBe('https://market.openbitfun.com/image?variant=large-v1');
    expect(marketImageSrcSet(source)).toContain('variant=compact-v1 640w');
    expect(marketImageSrcSet(source)).toContain('variant=large-v1 1280w');
  });

  it('retries the original URL only once after an optimized image fails', () => {
    const image = {
      dataset: {} as Record<string, string>,
      removeAttribute: vi.fn(),
      src: 'optimized',
    } as unknown as HTMLImageElement;
    expect(retryOriginalMarketImage(image, '/original.webp')).toBe(true);
    expect(image.src).toBe('/original.webp');
    expect(retryOriginalMarketImage(image, '/original.webp')).toBe(false);
  });
});
