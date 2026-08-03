import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AppearanceMarketAPI } from './AppearanceMarketAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({ api: { invoke: invokeMock } }));

describe('AppearanceMarketAPI', () => {
  beforeEach(() => invokeMock.mockReset());

  it('uses structured requests for browsing and detail lookup', async () => {
    invokeMock
      .mockResolvedValueOnce({ items: [] })
      .mockResolvedValueOnce({ slug: 'tokyo-night', releases: [] });
    const market = new AppearanceMarketAPI();

    await market.browse({ query: 'tokyo', mode: 'dark', sort: 'downloads', limit: 20 });
    await market.getListing('tokyo-night');

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'appearance_market_browse', {
      request: { query: 'tokyo', mode: 'dark', sort: 'downloads', limit: 20 },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'appearance_market_get_listing', {
      request: { slug: 'tokyo-night' },
    });
  });

  it('keeps the package response binary and returns an isolated buffer', async () => {
    const source = new Uint8Array([9, 1, 2, 3, 8]);
    invokeMock.mockResolvedValue(source.subarray(1, 4));
    const market = new AppearanceMarketAPI();

    const request = {
      slug: 'tokyo-night',
      releaseNumber: 3,
      packageId: 'community.tokyo-night',
      packageVersion: '2.0.0',
      packageSha256: 'a'.repeat(64),
      packageSize: 3,
    };
    const bytes = await market.downloadRelease(request);

    expect([...new Uint8Array(bytes)]).toEqual([1, 2, 3]);
    expect(bytes).not.toBe(source.buffer);
    expect(invokeMock).toHaveBeenCalledWith(
      'appearance_market_download_release',
      { request },
      { timeout: 180_000 },
    );
  });
});
