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

  it('uses shared-account commands for submission history and review actions', async () => {
    invokeMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ submissionId: 'submission-upload', status: 'submitted' })
      .mockResolvedValueOnce({ submissionId: 'submission-1', status: 'withdrawn' })
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ submission: { submissionId: 'submission-2' } })
      .mockResolvedValueOnce({ submission: { submissionId: 'submission-2', status: 'approved' } });
    const market = new AppearanceMarketAPI();

    await market.listSubmissions();
    await market.submitPackage({
      packagePath: '/tmp/tokyo-night.openbitfun-appearance',
      slug: 'tokyo-night',
      minOpenBitFunVersion: '1.0.0',
      changelog: 'Initial release',
      license: { spdxExpression: 'MIT' },
    });
    await market.withdrawSubmission('submission-1');
    await market.listReviewSubmissions();
    await market.getReviewSubmission('submission-2');
    await market.reviewSubmission('submission-2', 'approve');

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'appearance_market_list_submissions', {});
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'appearance_market_submit_package', {
      request: {
        packagePath: '/tmp/tokyo-night.openbitfun-appearance',
        slug: 'tokyo-night',
        minOpenBitFunVersion: '1.0.0',
        changelog: 'Initial release',
        license: { spdxExpression: 'MIT' },
      },
    }, { timeout: 300_000 });
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'appearance_market_withdraw_submission', {
      request: { submissionId: 'submission-1' },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'appearance_market_list_review_submissions', {});
    expect(invokeMock).toHaveBeenNthCalledWith(5, 'appearance_market_get_review_submission', {
      request: { submissionId: 'submission-2' },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(6, 'appearance_market_review_submission', {
      request: { submissionId: 'submission-2', decision: 'approve', reason: '' },
    });
  });
});
