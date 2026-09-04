import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import type {
  InstalledMarketOrigin,
  MarketListingSummary,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';

export type MiniAppLibraryAction = 'get' | 'open' | 'update';

export interface MiniAppLibraryItem {
  key: string;
  action: MiniAppLibraryAction;
  app?: MiniAppMeta;
  listing?: MarketListingSummary;
  origin?: InstalledMarketOrigin;
}

const ACTION_PRIORITY: Record<MiniAppLibraryAction, number> = {
  update: 0,
  open: 1,
  get: 2,
};

/**
 * Projects the local catalog and the marketplace into one App Store-style
 * library. A marketplace origin is the durable join key: local app ids may
 * differ from listing ids and local version counters are not market releases.
 */
export function buildMiniAppLibraryItems(
  listings: MarketListingSummary[],
  apps: MiniAppMeta[],
  origins: Record<string, InstalledMarketOrigin>,
): MiniAppLibraryItem[] {
  const seenListingIds = new Set<string>();
  const uniqueListings = listings.filter((listing) => {
    if (seenListingIds.has(listing.listingId)) return false;
    seenListingIds.add(listing.listingId);
    return true;
  });
  const installedByListingId = new Map<
    string,
    { app: MiniAppMeta; origin: InstalledMarketOrigin }
  >();

  for (const app of apps) {
    const origin = origins[app.id];
    if (origin && !installedByListingId.has(origin.listingId)) {
      installedByListingId.set(origin.listingId, { app, origin });
    }
  }

  const consumedAppIds = new Set<string>();
  const projected: Array<{ item: MiniAppLibraryItem; order: number }> = uniqueListings.map((listing, index) => {
    const installed = installedByListingId.get(listing.listingId);
    if (installed) consumedAppIds.add(installed.app.id);

    const action: MiniAppLibraryAction = !installed
      ? 'get'
      : installed.origin.releaseNumber < listing.latestRelease
        ? 'update'
        : 'open';

    return {
      item: {
        key: `market:${listing.listingId}`,
        action,
        app: installed?.app,
        listing,
        origin: installed?.origin,
      } satisfies MiniAppLibraryItem,
      order: index,
    };
  });

  for (const [index, app] of apps.entries()) {
    if (consumedAppIds.has(app.id)) continue;
    projected.push({
      item: {
        key: `local:${app.id}`,
        action: 'open',
        app,
        origin: origins[app.id],
      },
      order: uniqueListings.length + index,
    });
  }

  return projected
    .sort((left, right) => (
      ACTION_PRIORITY[left.item.action] - ACTION_PRIORITY[right.item.action]
      || left.order - right.order
    ))
    .map(({ item }) => item);
}
