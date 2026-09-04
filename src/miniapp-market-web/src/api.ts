import type {
  AdminSubmission,
  AdminSubmissionDetail,
  ApiErrorEnvelope,
  CursorPage,
  MarketConfig,
  MarketListingDetail,
  MarketListingSummary,
  MarketSubmission,
  Me,
  SubmissionStatus,
} from './types';

const API = '/miniapp/api/v1';

export class MarketApiError extends Error {
  readonly code: string;
  readonly requestId?: string;

  constructor(code: string, message: string, requestId?: string) {
    super(message);
    this.name = 'MarketApiError';
    this.code = code;
    this.requestId = requestId;
  }
}

function csrfToken(): string | undefined {
  const prefix = 'openbitfun_market_csrf=';
  return document.cookie
    .split(';')
    .map((item) => item.trim())
    .find((item) => item.startsWith(prefix))
    ?.slice(prefix.length);
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !(init.body instanceof Blob) && typeof init.body === 'string') {
    headers.set('content-type', 'application/json');
  }
  const method = (init.method || 'GET').toUpperCase();
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = csrfToken();
    if (csrf) headers.set('x-csrf-token', csrf);
  }
  const response = await fetch(`${API}${path}`, {
    ...init,
    headers,
    credentials: 'include',
  });
  if (!response.ok) {
    let body: ApiErrorEnvelope | undefined;
    try {
      body = (await response.json()) as ApiErrorEnvelope;
    } catch {
      // Keep the status fallback below.
    }
    throw new MarketApiError(
      body?.error.code || `http_${response.status}`,
      body?.error.message || `Marketplace request failed (${response.status}).`,
      body?.error.requestId,
    );
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export const marketApi = {
  config: () => request<MarketConfig>('/config'),
  me: () => request<Me>('/me'),
  logout: () => request<void>('/auth/logout', { method: 'POST' }),
  list: (query: URLSearchParams) =>
    request<CursorPage<MarketListingSummary>>(`/listings?${query.toString()}`),
  detail: (slug: string) => request<MarketListingDetail>(`/listings/${encodeURIComponent(slug)}`),
  rate: (slug: string, value: number) =>
    request<{ average: number; count: number; myRating?: number }>(
      `/listings/${encodeURIComponent(slug)}/rating`,
      { method: 'PUT', body: JSON.stringify({ value }) },
    ),
  deleteRating: (slug: string) =>
    request<{ average: number; count: number; myRating?: number }>(
      `/listings/${encodeURIComponent(slug)}/rating`,
      { method: 'DELETE' },
    ),
  favorite: (slug: string, active: boolean) =>
    request<{ count: number; isFavorited: boolean }>(
      `/listings/${encodeURIComponent(slug)}/favorite`,
      { method: active ? 'PUT' : 'DELETE' },
    ),
  createSubmission: (payload: Record<string, unknown>) =>
    request<MarketSubmission>('/submissions', {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  uploadPackage: (id: string, file: File) =>
    request<MarketSubmission>(`/submissions/${id}/package`, {
      method: 'PUT',
      body: file,
      headers: { 'content-type': 'application/vnd.openbitfun.miniapp+zip' },
    }),
  uploadScreenshot: (id: string, position: number, file: File) =>
    request<MarketSubmission>(`/submissions/${id}/screenshots/${position}`, {
      method: 'PUT',
      body: file,
      headers: { 'content-type': file.type || 'application/octet-stream' },
    }),
  submit: (id: string) =>
    request<MarketSubmission>(`/submissions/${id}/submit`, { method: 'POST' }),
  submissions: (status?: SubmissionStatus) =>
    request<CursorPage<MarketSubmission>>(
      `/submissions${status ? `?status=${encodeURIComponent(status)}` : ''}`,
    ),
  withdrawSubmission: (id: string) =>
    request<void>(`/submissions/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  adminSubmissions: (status: SubmissionStatus = 'submitted') =>
    request<CursorPage<AdminSubmission>>(
      `/admin/submissions?status=${encodeURIComponent(status)}`,
    ),
  adminDetail: (id: string) =>
    request<AdminSubmissionDetail>(`/admin/submissions/${encodeURIComponent(id)}`),
  review: (id: string, decision: 'approve' | 'reject', reason = '') =>
    request<MarketSubmission>(`/admin/submissions/${id}/decision`, {
      method: 'POST',
      body: JSON.stringify({ decision, reason }),
    }),
  yankRelease: (id: string, reason: string) =>
    request<void>(`/admin/releases/${encodeURIComponent(id)}/yank`, {
      method: 'POST',
      body: JSON.stringify({ reason }),
    }),
  unpublishListing: (id: string, reason: string) =>
    request<void>(`/admin/listings/${encodeURIComponent(id)}/unpublish`, {
      method: 'POST',
      body: JSON.stringify({ reason }),
    }),
};

export function loginUrl(returnTo = window.location.pathname): string {
  return `${API}/auth/github/start?returnTo=${encodeURIComponent(returnTo)}`;
}

export function downloadUrl(slug: string, release: number): string {
  return `${API}/listings/${encodeURIComponent(slug)}/releases/${release}/download`;
}
