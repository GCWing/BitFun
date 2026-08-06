import { BUILTIN_PROVIDER_TEMPLATES } from './builtinProviderCatalog';

export interface ProviderUrlCatalogItem {
  id: string;
  baseUrl: string;
  baseUrlOptions?: string[];
}

export const PROVIDER_URL_CATALOG: ProviderUrlCatalogItem[] = Object.values(
  BUILTIN_PROVIDER_TEMPLATES,
).map(template => ({
  id: template.id,
  baseUrl: template.baseUrl,
  baseUrlOptions: template.baseUrlOptions?.map(option => option.url),
}));

export function normalizeProviderBaseUrl(url: string): string {
  let normalized = url.trim().replace(/#$/, '').replace(/\/+$/, '');

  normalized = normalized
    .replace(/\/chat\/completions$/i, '')
    .replace(/\/responses$/i, '')
    .replace(/\/v1\/messages$/i, '');

  const geminiModelEndpointMatch = normalized.match(/^(.*)\/models\/[^/?#:]+(?::[^/?#]+)?(?:\?.*)?$/i);
  if (geminiModelEndpointMatch?.[1]) {
    normalized = geminiModelEndpointMatch[1].replace(/\/+$/, '');
  }

  normalized = normalized.replace(/\/v1beta$/i, '');

  return normalized;
}

export function matchProviderCatalogItemByBaseUrl(
  baseUrl?: string,
  catalog: ProviderUrlCatalogItem[] = PROVIDER_URL_CATALOG
): ProviderUrlCatalogItem | undefined {
  const configBaseUrl = baseUrl?.trim();
  if (!configBaseUrl) return undefined;

  const normalizedConfigBaseUrl = normalizeProviderBaseUrl(configBaseUrl);
  const candidates = catalog.flatMap(item => {
    const urls = [item.baseUrl, ...(item.baseUrlOptions || [])];
    return urls.map(url => ({
      item,
      normalizedUrl: normalizeProviderBaseUrl(url),
    }));
  });

  const matched = candidates
    .filter(candidate => (
      normalizedConfigBaseUrl === candidate.normalizedUrl ||
      normalizedConfigBaseUrl.startsWith(`${candidate.normalizedUrl}/`) ||
      candidate.normalizedUrl.startsWith(`${normalizedConfigBaseUrl}/`)
    ))
    .sort((a, b) => b.normalizedUrl.length - a.normalizedUrl.length)[0];

  return matched?.item;
}

export function extractProviderSegmentFromBaseUrl(baseUrl?: string): string | undefined {
  const rawBaseUrl = baseUrl?.trim();
  if (!rawBaseUrl) return undefined;

  try {
    const hostname = new URL(rawBaseUrl).hostname.toLowerCase();
    const parts = hostname.split('.').filter(Boolean);
    if (parts.length >= 2) {
      return parts[parts.length - 2];
    }
    return parts[0];
  } catch {
    return undefined;
  }
}
