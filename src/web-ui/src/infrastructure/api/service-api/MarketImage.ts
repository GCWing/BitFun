export type MarketImageVariant = 'compact-v1' | 'large-v1';

export function marketImageUrl(source: string, variant: MarketImageVariant): string {
  if (!source) return source;

  const hashIndex = source.indexOf('#');
  const hash = hashIndex >= 0 ? source.slice(hashIndex) : '';
  const withoutHash = hashIndex >= 0 ? source.slice(0, hashIndex) : source;
  const queryIndex = withoutHash.indexOf('?');
  const path = queryIndex >= 0 ? withoutHash.slice(0, queryIndex) : withoutHash;
  const params = new URLSearchParams(queryIndex >= 0 ? withoutHash.slice(queryIndex + 1) : '');
  params.set('variant', variant);
  return `${path}?${params.toString()}${hash}`;
}

export function marketImageSrcSet(source: string): string {
  return [
    `${marketImageUrl(source, 'compact-v1')} 640w`,
    `${marketImageUrl(source, 'large-v1')} 1280w`,
  ].join(', ');
}

export function retryOriginalMarketImage(image: HTMLImageElement, source: string): boolean {
  if (!source || image.dataset.marketImageOriginal === source) return false;
  image.dataset.marketImageOriginal = source;
  image.removeAttribute('srcset');
  image.removeAttribute('sizes');
  image.src = source;
  return true;
}
