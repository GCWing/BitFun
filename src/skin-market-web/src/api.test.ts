import { afterEach, describe, expect, it, vi } from 'vitest';
import listingFixture from '../../shared/appearance-market-contract-fixtures/listing-detail.json';
import {
  csrfTokenFromCookie,
  sharedMarketAccountApi,
  sharedMarketLoginUrl,
} from './account';
import { buildListingPath, downloadUrl, skinMarketApi } from './api';
import type { AppearanceListingDetail, AppearanceMode } from './types';

function fixtureMode(value: string): AppearanceMode {
  if (value !== 'light' && value !== 'dark') throw new Error(`Invalid fixture mode: ${value}`);
  return value;
}

const typedListingFixture: AppearanceListingDetail = {
  ...listingFixture,
  mode: fixtureMode(listingFixture.mode),
};

describe('Skin Market API paths', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('encodes filters and omits the all-mode sentinel', () => {
    expect(buildListingPath({
      query: '  ocean night  ',
      mode: 'dark',
      sort: 'downloads',
      cursor: 'page/2',
      limit: 12,
    })).toBe('/listings?q=ocean+night&mode=dark&sort=downloads&cursor=page%2F2&limit=12');
    expect(buildListingPath({ mode: 'all' })).toBe('/listings');
  });

  it('builds public release downloads below the versioned Skin API', () => {
    expect(downloadUrl('ocean-night', 2)).toBe(
      '/skin/api/v1/listings/ocean-night/releases/2/download',
    );
  });

  it('type-checks the shared Rust and TypeScript listing fixture', () => {
    expect(typedListingFixture.packageId).toBe('community.ocean-night');
    expect(typedListingFixture.mode).toBe('dark');
    expect(typedListingFixture.releases[0].reviewBundleHash).toHaveLength(64);
  });

  it('uses the MiniApp auth broker and returns to the current Skin route', () => {
    expect(sharedMarketLoginUrl('/skin/appearances/ocean-night?q=dark')).toBe(
      '/miniapp/api/v1/auth/github/start?returnTo=%2Fskin%2Fappearances%2Focean-night%3Fq%3Ddark',
    );
  });

  it('forwards the Skin-scoped CSRF alias when signing out', async () => {
    expect(csrfTokenFromCookie('theme=dark; openbitfun_skin_csrf=shared-csrf; locale=zh')).toBe(
      'shared-csrf',
    );
    vi.stubGlobal('document', { cookie: 'openbitfun_skin_csrf=shared-csrf' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal('fetch', fetchMock);

    await sharedMarketAccountApi.logout();

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('/miniapp/api/v1/auth/logout');
    expect(init.method).toBe('POST');
    expect(init.credentials).toBe('include');
    expect(new Headers(init.headers).get('x-csrf-token')).toBe('shared-csrf');
  });

  it('uses the shared Skin session and CSRF token for submission writes', async () => {
    vi.stubGlobal('document', { cookie: 'openbitfun_skin_csrf=skin-write-token' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      submission: { submissionId: 'submission-1' },
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    vi.stubGlobal('fetch', fetchMock);

    await skinMarketApi.decideSubmission('submission/1', 'reject', 'Update the preview');

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('/skin/api/v1/admin/submissions/submission%2F1/decision');
    expect(init.method).toBe('POST');
    expect(init.credentials).toBe('same-origin');
    expect(new Headers(init.headers).get('x-csrf-token')).toBe('skin-write-token');
    expect(JSON.parse(String(init.body))).toEqual({
      decision: 'reject',
      reason: 'Update the preview',
    });
  });

  it('uses admin moderation routes with encoded ids, CSRF, and removal reasons', async () => {
    vi.stubGlobal('document', { cookie: 'openbitfun_skin_csrf=skin-admin-token' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal('fetch', fetchMock);

    await skinMarketApi.yankRelease('release/1', 'Unsafe package content');
    await skinMarketApi.unpublishListing('listing/1', 'Repeated policy violations');

    expect(fetchMock).toHaveBeenCalledTimes(2);
    const [yankUrl, yankInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(yankUrl).toBe('/skin/api/v1/admin/releases/release%2F1/yank');
    expect(yankInit.method).toBe('POST');
    expect(yankInit.credentials).toBe('same-origin');
    expect(new Headers(yankInit.headers).get('content-type')).toBe('application/json');
    expect(new Headers(yankInit.headers).get('x-csrf-token')).toBe('skin-admin-token');
    expect(JSON.parse(String(yankInit.body))).toEqual({ reason: 'Unsafe package content' });

    const [unpublishUrl, unpublishInit] = fetchMock.mock.calls[1] as [string, RequestInit];
    expect(unpublishUrl).toBe('/skin/api/v1/admin/listings/listing%2F1/unpublish');
    expect(unpublishInit.method).toBe('POST');
    expect(new Headers(unpublishInit.headers).get('x-csrf-token')).toBe('skin-admin-token');
    expect(JSON.parse(String(unpublishInit.body))).toEqual({
      reason: 'Repeated policy violations',
    });
  });
});
