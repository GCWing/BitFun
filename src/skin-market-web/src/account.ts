import type {
  ApiErrorEnvelope,
  SharedMarketAccount,
  SharedMarketAccountConfig,
} from './types';

export const SHARED_ACCOUNT_API_BASE = '/miniapp/api/v1';

export class SharedMarketAccountError extends Error {
  readonly code: string;
  readonly requestId?: string;

  constructor(code: string, message: string, requestId?: string) {
    super(message);
    this.name = 'SharedMarketAccountError';
    this.code = code;
    this.requestId = requestId;
  }
}

export function csrfTokenFromCookie(cookieHeader: string): string | undefined {
  const prefix = 'openbitfun_skin_csrf=';
  return cookieHeader
    .split(';')
    .map((item) => item.trim())
    .find((item) => item.startsWith(prefix))
    ?.slice(prefix.length);
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('accept', 'application/json');
  const method = (init.method ?? 'GET').toUpperCase();
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = csrfTokenFromCookie(document.cookie);
    if (csrf) headers.set('x-csrf-token', csrf);
  }
  const response = await fetch(`${SHARED_ACCOUNT_API_BASE}${path}`, {
    ...init,
    credentials: 'include',
    headers,
  });
  if (!response.ok) {
    let body: ApiErrorEnvelope | undefined;
    try {
      body = (await response.json()) as ApiErrorEnvelope;
    } catch {
      // Keep the stable HTTP fallback below.
    }
    throw new SharedMarketAccountError(
      body?.error.code ?? `http_${response.status}`,
      body?.error.message ?? `Shared market account request failed (${response.status}).`,
      body?.error.requestId,
    );
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export const sharedMarketAccountApi = {
  config: () => request<SharedMarketAccountConfig>('/config'),
  me: () => request<SharedMarketAccount>('/me'),
  logout: () => request<void>('/auth/logout', { method: 'POST' }),
};

export function sharedMarketLoginUrl(
  returnTo = `${window.location.pathname}${window.location.search}`,
): string {
  return `${SHARED_ACCOUNT_API_BASE}/auth/github/start?returnTo=${encodeURIComponent(returnTo)}`;
}
