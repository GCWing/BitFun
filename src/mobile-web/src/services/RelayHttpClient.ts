/** Account-authenticated HTTP device directory and encrypted device RPC. */
import { deriveDeviceMessageKey, encrypt, decrypt, fromB64 } from './E2EEncryption';
import { normalizeRelayUrl } from './pairingLink';

export interface AccountIdentity {
  token: string;
  masterKey: Uint8Array;
  userId: string;
  deviceId: string;
}
interface AccountIdentitySnapshot extends AccountIdentity { generation: number; }
export type AccountOwnerChange = {
  kind: 'initial' | 'replacement' | 'unavailable';
  epoch: number;
  userId: string | null;
};
export type ControlTargetSnapshot = Readonly<{ deviceId: string | null; epoch: number }>;
export class AccountIdentityChangedError extends Error {
  constructor() { super('Account identity changed'); this.name = 'AccountIdentityChangedError'; }
}
export function isAccountIdentityChangedError(value: unknown): value is AccountIdentityChangedError {
  return value instanceof AccountIdentityChangedError;
}
const RELAY_HTTP_MAX_ATTEMPTS = 5;
const RELAY_HTTP_RETRY_BASE_DELAY_MS = 300;
const RELAY_HTTP_RETRY_BUDGET_MS = 120_000;
const TRANSIENT_RELAY_STATUSES = new Set([408, 425, 500, 502, 503, 504]);
type RelayRequestOptions = { retryable?: boolean; timeoutMs?: number };

export class RelayHttpClient {
  private readonly relayUrl: string;
  private identity: AccountIdentitySnapshot | null = null;
  private identityGeneration = 0;
  private accountEpochValue = 0;
  private ownerListeners = new Set<(change: AccountOwnerChange) => void>();
  private targetDeviceIdValue: string | null = null;
  private controlTargetEpochValue = 0;
  private controlTargetListeners = new Set<(snapshot: ControlTargetSnapshot) => void>();
  private deviceMessageKeys = new Map<string, { expires: number; key: Promise<Uint8Array> }>();

  constructor(relayUrl: string, identity: AccountIdentity) {
    const endpoint = normalizeRelayUrl(relayUrl);
    if (!endpoint) throw new Error('Invalid Relay URL');
    this.relayUrl = endpoint;
    this.setAccountIdentity(identity);
  }

  setAccountIdentity(identity: AccountIdentity): void {
    if (!identity.token.trim() || !identity.userId.trim() || !identity.deviceId.trim() || identity.masterKey.length !== 32) {
      throw new Error('Relay returned an invalid account identity.');
    }
    const kind = this.identity ? 'replacement' : 'initial';
    this.identity?.masterKey.fill(0);
    this.identity = { ...identity, masterKey: identity.masterKey.slice(), generation: ++this.identityGeneration };
    this.accountEpochValue += 1;
    this.deviceMessageKeys.clear();
    this.setTargetDeviceId(null);
    for (const listener of this.ownerListeners) listener({ kind, epoch: this.accountEpochValue, userId: identity.userId });
  }

  resetConnectionIdentity(): void {
    this.identity?.masterKey.fill(0);
    this.identity = null;
    this.identityGeneration += 1;
    this.accountEpochValue += 1;
    this.deviceMessageKeys.clear();
    this.setTargetDeviceId(null);
    for (const listener of this.ownerListeners) listener({ kind: 'unavailable', epoch: this.accountEpochValue, userId: null });
  }

  onAccountOwnerChange(listener: (change: AccountOwnerChange) => void, options?: { emitCurrent?: boolean }): () => void {
    this.ownerListeners.add(listener);
    if (options?.emitCurrent && this.identity) listener({ kind: 'initial', epoch: this.accountEpochValue, userId: this.identity.userId });
    return () => this.ownerListeners.delete(listener);
  }
  get hasAccountIdentity(): boolean { return this.identity !== null; }
  get accountEpoch(): number { return this.accountEpochValue; }
  get accountUserId(): string | null { return this.identity?.userId ?? null; }
  get controllerDeviceId(): string | null { return this.identity?.deviceId ?? null; }
  get targetDeviceId(): string | null { return this.targetDeviceIdValue; }
  get controlTargetEpoch(): number { return this.controlTargetEpochValue; }
  setTargetDeviceId(deviceId: string | null): void {
    this.targetDeviceIdValue = deviceId;
    this.controlTargetEpochValue += 1;
    const snapshot = this.getControlTargetSnapshot();
    for (const listener of this.controlTargetListeners) listener(snapshot);
  }
  getControlTargetSnapshot(): ControlTargetSnapshot {
    return { deviceId: this.targetDeviceIdValue, epoch: this.controlTargetEpochValue };
  }
  isControlTargetCurrent(snapshot: ControlTargetSnapshot): boolean { return snapshot.epoch === this.controlTargetEpochValue; }
  onControlTargetChange(listener: (snapshot: ControlTargetSnapshot) => void): () => void {
    this.controlTargetListeners.add(listener);
    return () => this.controlTargetListeners.delete(listener);
  }

  private async fetchWithTimeout(
    input: RequestInfo | URL,
    init: RequestInit,
    timeoutMs: number,
  ): Promise<Response> {
    const controller = new AbortController();
    const timer = window.setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(input, { ...init, signal: controller.signal });
      // Keep the deadline active until the full body has been received.
      // `fetch()` resolves after response headers, so returning that Response
      // directly would let a stalled body wait forever outside the timeout.
      const body = await response.arrayBuffer();
      return new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers: response.headers,
      });
    } catch (error: unknown) {
      if ((error as { name?: string })?.name === 'AbortError') {
        throw new Error('Request timed out');
      }
      throw error;
    } finally {
      window.clearTimeout(timer);
    }
  }

  private async fetchWithRetry(
    input: RequestInfo | URL,
    init: RequestInit,
    timeoutMs: number,
    assertCurrent: () => void = () => {},
  ): Promise<Response> {
    let lastError: unknown = null;
    const deadlineMs = Date.now() + RELAY_HTTP_RETRY_BUDGET_MS;
    for (let attempt = 1; attempt <= RELAY_HTTP_MAX_ATTEMPTS; attempt += 1) {
      assertCurrent();
      try {
        const remainingMs = deadlineMs - Date.now();
        if (remainingMs <= 0) {
          throw lastError ?? new Error('Relay request retry budget exceeded');
        }
        const response = await this.fetchWithTimeout(
          input,
          init,
          Math.min(timeoutMs, remainingMs),
        );
        if (
          TRANSIENT_RELAY_STATUSES.has(response.status)
          && attempt < RELAY_HTTP_MAX_ATTEMPTS
        ) {
          lastError = new Error(`Relay returned HTTP ${response.status}`);
          void response.body?.cancel();
        } else {
          return response;
        }
      } catch (error) {
        lastError = error;
        if (attempt === RELAY_HTTP_MAX_ATTEMPTS) throw error;
      }
      const delayMs = RELAY_HTTP_RETRY_BASE_DELAY_MS * (2 ** (attempt - 1));
      if (Date.now() + delayMs >= deadlineMs) {
        throw lastError ?? new Error('Relay request retry budget exceeded');
      }
      await new Promise(resolve => window.setTimeout(resolve, delayMs));
    }
    throw lastError;
  }

  async listDevices(): Promise<Array<{ device_id: string; device_name: string; online: boolean }>> {
    return this.withAccount(async (identity) => {
      const resp = await this.fetchWithRetry(`${this.relayUrl}/api/devices`, {
        headers: { 'Authorization': `Bearer ${identity.token}` },
      }, 20_000, () => {
        if (identity.generation !== this.identityGeneration) throw new AccountIdentityChangedError();
      });
      if (!resp.ok) {
        const err = new Error(`List devices failed: HTTP ${resp.status}`) as Error & {
          status?: number;
        };
        err.status = resp.status;
        throw err;
      }
      return resp.json();
    });
  }

  /** Send a command encrypted with the two account devices' X25519 key agreement. */
  async sendDeviceRpc<T = any>(
    targetDeviceId: string,
    command: object,
    options: RelayRequestOptions = {},
  ): Promise<T> {
    const targetEpoch = this.controlTargetEpochValue;
    return this.withAccount(async (identity) => {
      const cacheId = `${identity.generation}:${targetDeviceId}`;
      let cached = this.deviceMessageKeys.get(cacheId);
      if (!cached || cached.expires < Date.now()) {
        const key = (async () => {
          const response = await this.fetchWithTimeout(
            `${this.relayUrl}/api/devices/${encodeURIComponent(targetDeviceId)}/key`,
            { headers: { Authorization: `Bearer ${identity.token}` } }, 20_000,
          );
          if (!response.ok) {
            const error = new Error(`Device key unavailable: HTTP ${response.status}`) as Error & { status?: number };
            error.status = response.status;
            throw error;
          }
          const peer = await response.json();
          if (peer.device_id !== targetDeviceId) throw new Error('Relay returned a different device identity.');
          return deriveDeviceMessageKey(identity.masterKey, fromB64(peer.public_key));
        })();
        cached = { expires: Date.now() + 60_000, key };
        this.deviceMessageKeys.set(cacheId, cached);
      }
      const messageKey = await cached.key;
      const plaintext = JSON.stringify(command);
      const { data: encData, nonce: encNonce } = await encrypt(
        messageKey,
        plaintext,
      );

      if (identity.generation !== this.identityGeneration || targetEpoch !== this.controlTargetEpochValue) {
        throw new AccountIdentityChangedError();
      }
      const timeoutMs = options.timeoutMs ?? (options.retryable ? 20_000 : 130_000);
      const resp = await (options.retryable ? this.fetchWithRetry : this.fetchWithTimeout).call(
        this,
        `${this.relayUrl}/api/devices/${encodeURIComponent(targetDeviceId)}/rpc`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${identity.token}`,
          },
          body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
        },
        timeoutMs,
        () => {
          if (identity.generation !== this.identityGeneration || targetEpoch !== this.controlTargetEpochValue) {
            throw new AccountIdentityChangedError();
          }
        },
      );

      if (!resp.ok) {
        const err = new Error(`Device RPC failed: HTTP ${resp.status}`) as Error & {
          status?: number;
        };
        err.status = resp.status;
        throw err;
      }
      const data = await resp.json();
      const decrypted = await decrypt(
        messageKey,
        data.encrypted_data,
        data.nonce,
      );
      const parsed = JSON.parse(decrypted);
      if (parsed?.resp === 'error') {
        throw new Error(parsed.message || 'Remote error');
      }
      return parsed as T;
    }).catch((error) => {
      this.deviceMessageKeys.clear();
      throw error;
    });
  }

  private async withAccount<T>(operation: (identity: AccountIdentitySnapshot) => Promise<T>): Promise<T> {
    const identity = this.identity;
    if (!identity) throw new Error('Sign in with GitHub to continue');
    try {
      const result = await operation(identity);
      if (this.identity !== identity) throw new AccountIdentityChangedError();
      return result;
    } catch (error) {
      if (this.identity !== identity) throw new AccountIdentityChangedError();
      throw error;
    }
  }
}
