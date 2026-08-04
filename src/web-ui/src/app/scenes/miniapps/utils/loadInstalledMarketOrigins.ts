import {
  miniAppMarketAPI,
  type InstalledMarketOrigin,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('installedMarketOrigins');

/**
 * Marketplace origins for the installed catalog, keyed by app id.
 *
 * Every catalog load pairs with this so version labels can name the release a
 * user installed instead of the local edit counter. A failure degrades those
 * labels back to the local counter rather than taking the catalog down with it.
 */
export async function loadInstalledMarketOrigins(): Promise<
  Record<string, InstalledMarketOrigin>
> {
  try {
    return await miniAppMarketAPI.installedOrigins();
  } catch (error) {
    log.warn('Failed to read installed MiniApp market origins', error);
    return {};
  }
}
