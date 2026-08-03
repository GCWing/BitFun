import { describe, expect, it } from 'vitest';
import listingFixture from '../../shared/appearance-market-contract-fixtures/listing-detail.json';
import { buildListingPath, downloadUrl } from './api';
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
});
