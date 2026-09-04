import { describe, expect, it } from 'vitest';

import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import type {
  InstalledMarketOrigin,
  MarketListingSummary,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import { buildMiniAppLibraryItems } from './miniAppLibraryItems';

function app(id: string, name = id, createdAt = 1): MiniAppMeta {
  return {
    id,
    name,
    description: `${name} description`,
    icon: 'box',
    category: 'utilities',
    tags: [],
    version: 1,
    created_at: createdAt,
    updated_at: 1,
    permissions: {},
  };
}

function listing(
  listingId: string,
  latestRelease: number,
): MarketListingSummary {
  return {
    listingId,
    slug: listingId,
    name: listingId,
    description: `${listingId} description`,
    icon: 'box',
    category: 'utilities',
    tags: [],
    owner: { githubId: 1, login: 'owner', avatarUrl: '' },
    latestRelease,
    minOpenBitFunVersion: '1.0.0',
    permissions: {},
    screenshotUrls: [],
    ratingAverage: 0,
    ratingCount: 0,
    favoriteCount: 0,
    downloadCount: 0,
    publishedAt: 1,
  };
}

function origin(listingId: string, releaseNumber: number): InstalledMarketOrigin {
  return {
    listingId,
    releaseId: `${listingId}-release-${releaseNumber}`,
    releaseNumber,
    packageSha256: 'sha256',
  };
}

describe('buildMiniAppLibraryItems', () => {
  it('joins installed marketplace apps and projects App Store actions', () => {
    const installed = app('local-market-id');
    const result = buildMiniAppLibraryItems(
      [listing('needs-update', 3), listing('available', 1)],
      [installed],
      { [installed.id]: origin('needs-update', 2) },
    );

    expect(result).toHaveLength(2);
    expect(result[0]).toMatchObject({
      key: 'market:needs-update',
      action: 'update',
      app: { id: installed.id },
      origin: { releaseNumber: 2 },
    });
    expect(result[1]).toMatchObject({
      key: 'market:available',
      action: 'get',
    });
  });

  it('deduplicates current marketplace installs and keeps local-only apps', () => {
    const current = app('current');
    const localOnly = app('local-only');
    const result = buildMiniAppLibraryItems(
      [listing('current-listing', 4)],
      [current, localOnly],
      { [current.id]: origin('current-listing', 4) },
    );

    expect(result.map((item) => item.key)).toEqual([
      'market:current-listing',
      'local:local-only',
    ]);
    expect(result.every((item) => item.action === 'open')).toBe(true);
  });

  it('surfaces updates first, installed apps next, and new apps last', () => {
    const current = app('current');
    const update = app('update');
    const result = buildMiniAppLibraryItems(
      [listing('new-app', 1), listing('current-listing', 2), listing('update-listing', 3)],
      [current, update],
      {
        [current.id]: origin('current-listing', 2),
        [update.id]: origin('update-listing', 2),
      },
    );

    expect(result.map((item) => item.action)).toEqual(['update', 'open', 'get']);
  });

  it('sorts local apps by import time with zero downloads and a default rating of three', () => {
    const localOnly = app('local-only', 'Local only', 1_720_000_002_000);
    const market = listing('market-app', 1);
    market.publishedAt = 1_720_000_001;
    market.downloadCount = 8;
    market.ratingAverage = 2.5;
    market.ratingCount = 4;

    const newest = buildMiniAppLibraryItems([market], [localOnly], {}, 'newest');
    expect(newest.map((item) => item.key)).toEqual([
      'local:local-only',
      'market:market-app',
    ]);
    expect(newest[0]).toMatchObject({
      downloadCount: 0,
      ratingAverage: 3,
      ratingCount: 0,
      sortPublishedAtMs: localOnly.created_at,
    });

    const downloads = buildMiniAppLibraryItems([market], [localOnly], {}, 'downloads');
    expect(downloads.map((item) => item.key)).toEqual([
      'market:market-app',
      'local:local-only',
    ]);

    const rating = buildMiniAppLibraryItems([market], [localOnly], {}, 'rating');
    expect(rating.map((item) => item.key)).toEqual([
      'local:local-only',
      'market:market-app',
    ]);
  });

  it('keeps off-page installs and ignores duplicate marketplace rows', () => {
    const installed = app('off-page-install');
    const duplicate = listing('duplicate', 2);
    const installedOrigin = origin('off-page-listing', 4);
    const result = buildMiniAppLibraryItems(
      [duplicate, duplicate],
      [installed],
      { [installed.id]: installedOrigin },
    );

    expect(result.map((item) => item.key)).toEqual([
      'market:duplicate',
      'local:off-page-install',
    ]);
    expect(result[1]).toMatchObject({
      action: 'open',
      app: { id: installed.id },
      origin: installedOrigin,
    });
  });
});
