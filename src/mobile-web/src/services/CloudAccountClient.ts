import { argon2idAsync } from '@noble/hashes/argon2.js';
import { decryptBytes, fromB64, toB64 } from './E2EEncryption';

interface AccountChallenge {
  salt: string;
  kdf_salt: string;
  argon2_params: string;
  wrapped_master_key: string;
}

interface AccountAuthResponse {
  token: string;
  user_id: string;
}

interface AccountKdfParams {
  m: number;
  t: number;
  p: number;
}

interface RelayErrorResponse {
  error?: string;
  retry_after_secs?: number;
}

export interface CloudAccountSession {
  token: string;
  userId: string;
  masterKey: Uint8Array;
}

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

const KDF_TIMEOUT_MS = 30_000;

function generateRequestId(): string {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function validateKdfParams(params: AccountKdfParams, salt: Uint8Array): void {
  if (salt.length < 8 || salt.length > 64
    || !Number.isInteger(params.m) || params.m < 8 * 1024 || params.m > 256 * 1024
    || !Number.isInteger(params.t) || params.t < 1 || params.t > 10
    || !Number.isInteger(params.p) || params.p < 1 || params.p > 16) {
    throw new Error('Relay returned invalid account encryption parameters.');
  }
}

async function derivePasswordHash(
  password: string,
  salt: Uint8Array,
  params: AccountKdfParams,
): Promise<Uint8Array> {
  validateKdfParams(params, salt);
  let timeoutId = 0;
  try {
    return await Promise.race([
      argon2idAsync(password, salt, {
        m: params.m,
        t: params.t,
        p: params.p,
        dkLen: 32,
        asyncTick: 10,
      }),
      new Promise<Uint8Array>((_, reject) => {
        timeoutId = window.setTimeout(
          () => reject(new Error('Account password derivation timed out.')),
          KDF_TIMEOUT_MS,
        );
      }),
    ]);
  } finally {
    window.clearTimeout(timeoutId);
  }
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

/** Browser implementation of the same zero-knowledge login used by HarmonyOS. */
export class CloudAccountClient {
  async login(
    relayUrl: string,
    username: string,
    password: string,
    deviceId: string,
  ): Promise<CloudAccountSession> {
    const normalizedUser = username.trim();
    if (!normalizedUser || normalizedUser.length > 128
      || !password || password.length > 1024) {
      throw new Error('Invalid account credentials.');
    }

    const challenge = await requestJson<AccountChallenge>(
      relayUrl,
      '/api/auth/login/challenge',
      { username: normalizedUser },
    );
    const params = JSON.parse(challenge.argon2_params) as AccountKdfParams;
    const salt = fromB64(challenge.salt);
    const kdfSalt = fromB64(challenge.kdf_salt);
    const kek = await derivePasswordHash(password, salt, params);
    let passwordHash: Uint8Array | null = null;
    try {
      const wrappedParts = challenge.wrapped_master_key.split('.');
      if (wrappedParts.length !== 2) {
        throw new Error('Relay returned an invalid wrapped master key.');
      }
      let masterKey: Uint8Array;
      try {
        masterKey = decryptBytes(kek, fromB64(wrappedParts[0]), fromB64(wrappedParts[1]));
      } catch {
        throw new Error('Invalid username or password.');
      }
      if (masterKey.length !== 32) {
        masterKey.fill(0);
        throw new Error('Invalid username or password.');
      }

      passwordHash = await derivePasswordHash(password, kdfSalt, params);
      const requestId = generateRequestId();
      try {
        const auth = await requestJson<AccountAuthResponse>(
          relayUrl,
          '/api/auth/login',
          {
            username: normalizedUser,
            password_hash: toB64(passwordHash),
            device_id: deviceId,
            device_name: 'Mobile Browser',
            device_kind: 'mobile',
            request_id: requestId,
          },
        );
        if (!auth.token?.trim() || !auth.user_id?.trim()) {
          masterKey.fill(0);
          throw new Error('Relay returned an invalid account identity.');
        }
        return {
          token: auth.token,
          userId: auth.user_id,
          masterKey,
        };
      } catch (error) {
        masterKey.fill(0);
        throw error;
      }
    } finally {
      kek.fill(0);
      passwordHash?.fill(0);
    }
  }
}
