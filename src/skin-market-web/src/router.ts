export interface MarketRoute {
  kind: 'catalog' | 'detail' | 'not-found';
  slug?: string;
}

export const SKIN_BASE_PATH = '/skin';

export function parseMarketRoute(pathname: string): MarketRoute {
  const withoutBase = pathname === SKIN_BASE_PATH
    ? '/'
    : pathname.startsWith(`${SKIN_BASE_PATH}/`)
      ? pathname.slice(SKIN_BASE_PATH.length)
      : pathname;
  const normalized = withoutBase.length > 1 ? withoutBase.replace(/\/+$/, '') : withoutBase;
  if (normalized === '/') return { kind: 'catalog' };
  const match = normalized.match(/^\/appearances\/([a-z0-9-]+)$/);
  if (match) return { kind: 'detail', slug: match[1] };
  return { kind: 'not-found' };
}

export function appearancePath(slug: string): string {
  return `${SKIN_BASE_PATH}/appearances/${encodeURIComponent(slug)}`;
}

export function catalogPath(search = ''): string {
  return `${SKIN_BASE_PATH}/${search}`;
}
