import type {
  ApiErrorEnvelope,
  AppearanceAdminSubmissionDetail,
  AppearanceListingDetail,
  AppearanceListingSummary,
  AppearanceSubmission,
  AppearanceSubmissionStatus,
  CursorPage,
  ListAppearancesRequest,
} from './types';
import { csrfTokenFromCookie } from './account';

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

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('accept', 'application/json');
  const method = (init.method ?? 'GET').toUpperCase();
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = csrfTokenFromCookie(document.cookie);
    if (csrf) headers.set('x-csrf-token', csrf);
  }
  const response = await fetch(`${API_BASE}${path}`, {
    ...init,
    credentials: 'same-origin',
    headers,
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

  if (response.status === 204) return undefined as T;
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

async function allSubmissionPages(
  path: string,
  signal?: AbortSignal,
): Promise<CursorPage<AppearanceSubmission>> {
  const items: AppearanceSubmission[] = [];
  let cursor: string | undefined;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const query = new URLSearchParams(path.includes('?') ? path.split('?')[1] : '');
    query.set('limit', '50');
    if (cursor) query.set('cursor', cursor);
    const basePath = path.split('?')[0];
    const page = await request<CursorPage<AppearanceSubmission>>(
      `${basePath}?${query.toString()}`,
      { signal },
    );
    items.push(...page.items);
    cursor = page.nextCursor;
    if (!cursor) return { items };
  }
  throw new SkinMarketApiError(
    'submission_history_too_large',
    'Skin Market submission history exceeds the browser pagination safety limit.',
  );
}

export const skinMarketApi = {
  list: (options: ListAppearancesRequest, signal?: AbortSignal) =>
    request<CursorPage<AppearanceListingSummary>>(buildListingPath(options), { signal }),
  detail: (slug: string, signal?: AbortSignal) =>
    request<AppearanceListingDetail>(`/listings/${encodeURIComponent(slug)}`, { signal }),
  submissions: (signal?: AbortSignal) =>
    allSubmissionPages('/submissions', signal),
  withdrawSubmission: (submissionId: string) =>
    request<AppearanceSubmission>(`/submissions/${encodeURIComponent(submissionId)}`, {
      method: 'DELETE',
    }),
  reviewSubmissions: (status: AppearanceSubmissionStatus = 'submitted', signal?: AbortSignal) =>
    allSubmissionPages(`/admin/submissions?status=${status}`, signal),
  reviewSubmission: (submissionId: string, signal?: AbortSignal) =>
    request<AppearanceAdminSubmissionDetail>(
      `/admin/submissions/${encodeURIComponent(submissionId)}`,
      { signal },
    ),
  decideSubmission: (submissionId: string, decision: 'approve' | 'reject', reason = '') =>
    request<AppearanceAdminSubmissionDetail>(
      `/admin/submissions/${encodeURIComponent(submissionId)}/decision`,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ decision, reason }),
      },
    ),
  yankRelease: (releaseId: string, reason: string) =>
    request<void>(`/admin/releases/${encodeURIComponent(releaseId)}/yank`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ reason }),
    }),
  unpublishListing: (listingId: string, reason: string) =>
    request<void>(`/admin/listings/${encodeURIComponent(listingId)}/unpublish`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ reason }),
    }),
};
