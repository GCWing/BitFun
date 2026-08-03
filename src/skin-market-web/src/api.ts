import type {
  ApiErrorEnvelope,
  AppearanceListingDetail,
  AppearanceListingSummary,
  CursorPage,
  ListAppearancesRequest,
} from './types';

export const API_BASE = '/skin/api/v1';

export class SkinMarketApiError extends Error {
  readonly code: string;
  readonly requestId?: string;

  constructor(code: string, message: string, requestId?: string) {
    super(message);
    this.name = 'SkinMarketApiError';
    this.code = code;
    this.requestId = requestId;
  }
}

async function request<T>(path: string, signal?: AbortSignal): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    credentials: 'same-origin',
    headers: { accept: 'application/json' },
    signal,
  });

  if (!response.ok) {
    let body: ApiErrorEnvelope | undefined;
    try {
      body = (await response.json()) as ApiErrorEnvelope;
    } catch {
      // Fall through to the stable HTTP status fallback.
    }
    throw new SkinMarketApiError(
      body?.error.code ?? `http_${response.status}`,
      body?.error.message ?? `Skin Market request failed (${response.status}).`,
      body?.error.requestId,
    );
  }

  return response.json() as Promise<T>;
}

export function buildListingPath(options: ListAppearancesRequest): string {
  const query = new URLSearchParams();
  const search = options.query?.trim();
  if (search) query.set('q', search);
  if (options.mode && options.mode !== 'all') query.set('mode', options.mode);
  if (options.sort) query.set('sort', options.sort);
  if (options.cursor) query.set('cursor', options.cursor);
  if (options.limit) query.set('limit', String(options.limit));
  const suffix = query.toString();
  return `/listings${suffix ? `?${suffix}` : ''}`;
}

export function downloadUrl(slug: string, releaseNumber: number): string {
  return `${API_BASE}/listings/${encodeURIComponent(slug)}/releases/${releaseNumber}/download`;
}

export const skinMarketApi = {
  list: (options: ListAppearancesRequest, signal?: AbortSignal) =>
    request<CursorPage<AppearanceListingSummary>>(buildListingPath(options), signal),
  detail: (slug: string, signal?: AbortSignal) =>
    request<AppearanceListingDetail>(`/listings/${encodeURIComponent(slug)}`, signal),
};
