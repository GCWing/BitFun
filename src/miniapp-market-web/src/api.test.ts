import { describe, expect, it } from 'vitest';
import listingFixture from '../../shared/miniapp-market-contract-fixtures/listing-detail.json';
import { downloadUrl, loginUrl } from './api';
import type { MarketListingDetail } from './types';

const typedListingFixture: MarketListingDetail = listingFixture;

describe('market API paths', () => {
  it('encodes public release downloads under the versioned API', () => {
    expect(downloadUrl('regex-playground', 2)).toBe(
      '/miniapp/api/v1/listings/regex-playground/releases/2/download',
    );
  });

  it('shares the listing detail fixture with the Rust contract', () => {
    expect(typedListingFixture.permissions.node?.enabled).toBe(false);
    expect(typedListingFixture.releases[0].reviewBundleHash).toHaveLength(64);
  });

  it('uses the broker camelCase return target contract', () => {
    expect(loginUrl('/miniapp/admin')).toBe(
      '/miniapp/api/v1/auth/github/start?returnTo=%2Fminiapp%2Fadmin',
    );
  });
});
