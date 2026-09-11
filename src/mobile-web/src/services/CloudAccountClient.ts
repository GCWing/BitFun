import { generateKeyPair, fromB64, toB64 } from './E2EEncryption';
import { x25519 } from '@noble/curves/ed25519.js';

import { pairingRelayUrl } from './pairingLink';
export interface CloudAccountSession { token: string; userId: string; masterKey: Uint8Array; }
interface RelayErrorResponse { error?: string; retry_after_secs?: number; }
export class CloudAccountRequestError extends Error {
  readonly status: number;
  readonly retryAfterSeconds: number | null;

  constructor(message: string, status: number, retryAfterSeconds: number | null = null) {
    super(message);
    this.name = 'CloudAccountRequestError';
    this.status = status;
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

export function generateRequestId(): string {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

async function requestJson<T>(
  relayUrl: string,
  path: string,
  body: object,
): Promise<T> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 35_000);
  try {
    const response = await fetch(`${relayUrl.replace(/\/+$/, '')}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    });
    const text = await response.text();
    let parsed: T | RelayErrorResponse | null = null;
    try {
      parsed = text ? JSON.parse(text) as T | RelayErrorResponse : null;
    } catch {
      // Use the HTTP status below when a proxy returned a non-JSON body.
    }
    if (!response.ok) {
      const relayError = parsed as RelayErrorResponse | null;
      throw new CloudAccountRequestError(
        relayError?.error || `Relay request failed (HTTP ${response.status}).`,
        response.status,
        typeof relayError?.retry_after_secs === 'number'
          ? relayError.retry_after_secs
          : null,
      );
    }
    if (!parsed) throw new Error('Relay returned an empty account response.');
    return parsed as T;
  } catch (error: unknown) {
    if ((error as { name?: string })?.name === 'AbortError') {
      throw new Error('Account request timed out.');
    }
    throw error;
  } finally {
    window.clearTimeout(timeout);
  }
}


interface AuthStart {
  transactionId: string; transactionSecret: string; authorizationUrl: string;
  expiresAt: number; pollIntervalSeconds: number;
}

/** GitHub identity authorizes a separately keyed browser controller. */
export class CloudAccountClient {
  private readonly relayUrl: string;
  constructor(relayUrl: string) {
    const endpoint = pairingRelayUrl(relayUrl);
    if (!endpoint) throw new Error('Invalid Relay URL');
    this.relayUrl = endpoint;
  }
  async authorize(popup: Window, signal: AbortSignal): Promise<string> {
    const start = await requestJson<AuthStart>(this.relayUrl, '/api/auth/github/start', {});
    const url = new URL(start.authorizationUrl);
    if (url.origin !== 'https://github.com' || url.pathname !== '/login/oauth/authorize' || url.username || url.password) {
      throw new Error('Untrusted account authorization URL.');
    }
    if (signal.aborted) throw new Error('Sign-in cancelled.');
    popup.location.href = url.href;
    while (!signal.aborted && Date.now() < start.expiresAt * 1000) {
      await new Promise<void>((resolve) => setTimeout(resolve, Math.min(30, Math.max(1, start.pollIntervalSeconds)) * 1000));
      if (signal.aborted) break;
      const result = await requestJson<{ status: string; tokens?: { accessToken: string } }>(
        this.relayUrl, '/api/auth/github/poll', {
          transactionId: start.transactionId, transactionSecret: start.transactionSecret,
        },
      );
      if (result.status === 'authorized' && result.tokens?.accessToken) return result.tokens.accessToken;
      if (result.status === 'expired' || result.status === 'denied') break;
    }
    throw new Error(signal.aborted ? 'Sign-in cancelled.' : 'GitHub sign-in expired. Try again.');
  }

  async login(accessToken: string, deviceId: string): Promise<CloudAccountSession> {
    const storageKey = `openbitfun.mobile.device_key:${this.relayUrl}:${deviceId}`;
    const saved = sessionStorage.getItem(storageKey);
    let privateKey: Uint8Array;
    if (saved) {
      privateKey = fromB64(saved);
      if (privateKey.length !== 32) throw new Error('Stored device identity is invalid.');
    } else {
      const generated = await generateKeyPair();
      // Another login in this tab may have created the key while generation yielded.
      const existing = sessionStorage.getItem(storageKey);
      privateKey = existing ? fromB64(existing) : generated.privateKey;
      if (privateKey.length !== 32) throw new Error('Stored device identity is invalid.');
      if (!existing) sessionStorage.setItem(storageKey, toB64(privateKey));
      else generated.privateKey.fill(0);
    }
    const keys = { privateKey, publicKey: x25519.getPublicKey(privateKey) };
    try {
      const auth = await requestJson<{ token: string; user_id: string }>(this.relayUrl, '/api/auth/login', {
        access_token: accessToken, device_id: deviceId, device_name: 'Mobile Browser',
        device_kind: 'mobile', public_key: toB64(keys.publicKey), request_id: generateRequestId(),
      });
      if (!auth.token?.trim() || !auth.user_id?.trim()) throw new Error('Invalid account identity.');
      return { token: auth.token, userId: auth.user_id, masterKey: keys.privateKey };
    } catch (error) {
      keys.privateKey.fill(0);
      throw error;
    }
  }
}
