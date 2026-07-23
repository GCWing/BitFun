/**
 * HTTP client for communicating with the relay server.
 * All mobile-to-desktop communication goes through HTTP requests
 * that the relay bridges to the desktop via WebSocket.
 *
 * No WebSocket connection is maintained on the mobile side.
 */

import {
  generateKeyPair,
  deriveSharedKey,
  encrypt,
  decrypt,
  toB64,
  fromB64,
  type MobileKeyPair,
} from './E2EEncryption';

interface DelegatedIdentitySnapshot {
  token: string;
  masterKey: Uint8Array;
  userId: string | null;
  homeDeviceId: string | null;
  generation: number;
}

interface DelegatedAccountIdentity {
  userId: string | null;
  masterKey: Uint8Array;
  homeDeviceId: string | null;
}

export type DelegatedAccountOwnerChange = {
  kind: 'initial' | 'replacement' | 'unavailable';
  epoch: number;
  userId: string | null;
  homeDeviceId: string | null;
};

export type ControlTargetSnapshot = Readonly<{
  deviceId: string | null;
  homeDeviceId: string | null;
  epoch: number;
}>;

export class DelegatedIdentityChangedError extends Error {
  constructor(message = 'Delegated identity changed') {
    super(message);
    this.name = 'DelegatedIdentityChangedError';
  }
}

export class DelegatedAccountChangedError extends DelegatedIdentityChangedError {
  constructor() {
    super('Delegated account changed');
    this.name = 'DelegatedAccountChangedError';
  }
}

export function isDelegatedIdentityChangedError(
  value: unknown,
): value is DelegatedIdentityChangedError {
  return value instanceof DelegatedIdentityChangedError;
}

const RELAY_HTTP_MAX_ATTEMPTS = 5;
const RELAY_HTTP_RETRY_BASE_DELAY_MS = 300;
const RELAY_HTTP_RETRY_BUDGET_MS = 120_000;
const TRANSIENT_RELAY_STATUSES = new Set([408, 425, 500, 502, 503, 504]);

type RelayRequestOptions = {
  retryable?: boolean;
};

function equalBytesConstantTime(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= left[index] ^ right[index];
  }
  return difference === 0;
}

function delegatedAccountChanged(
  previous: DelegatedAccountIdentity | null,
  next: DelegatedAccountIdentity,
): boolean {
  if (!previous) return true;
  if (!equalBytesConstantTime(previous.masterKey, next.masterKey)) return true;
  if (previous.userId !== null && next.userId !== null) {
    return previous.userId !== next.userId;
  }
  // Older Desktop builds omit user_id. The account master key is the stable
  // identifier in that case; homeDeviceId separates distinct paired homes.
  return previous.homeDeviceId !== next.homeDeviceId;
}

export class RelayHttpClient {
  private relayUrl: string;
  private roomId: string;
  private sharedKey: Uint8Array | null = null;
  private keyPair: MobileKeyPair | null = null;
  /** Delegated credentials are committed as one immutable generation. */
  private delegatedIdentity: DelegatedIdentitySnapshot | null = null;
  private delegatedIdentityRequestEpoch = 0;
  private delegatedIdentityGenerationValue = 0;
  private delegatedIdentityRefreshOwner: number | null = null;
  private delegatedAccountIdentity: DelegatedAccountIdentity | null = null;
  private delegatedAccountEpochValue = 0;
  private delegatedAccountOwnerListeners = new Set<(
    change: DelegatedAccountOwnerChange,
  ) => void>();
  /** The current control-target device_id (for sendDeviceRpc). */
  private pairedDeviceIdValue: string | null = null;
  private controlTargetEpochValue = 0;
  private controlTargetListeners = new Set<(snapshot: ControlTargetSnapshot) => void>();
  /** The QR-paired desktop's device_id (the "home" device of this session). */
  public homeDeviceId: string | null = null;

  constructor(relayUrl: string, roomId: string) {
    this.relayUrl = relayUrl.replace(/\/+$/, '');
    this.roomId = roomId;
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
  ): Promise<Response> {
    let lastError: unknown = null;
    const deadlineMs = Date.now() + RELAY_HTTP_RETRY_BUDGET_MS;
    for (let attempt = 1; attempt <= RELAY_HTTP_MAX_ATTEMPTS; attempt += 1) {
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

  /**
   * Pair with the desktop via two HTTP round-trips:
   * 1. POST /pair with our public key → receive encrypted challenge
   * 2. POST /command with encrypted challenge_echo → receive initial_sync
   *
   * When the desktop is logged into a BitFun account, pass `password` so the
   * desktop can verify credentials (same challenge+unwrap path as desktop login).
   */
  async pair(
    desktopPubKeyB64: string,
    identity: {
      userId: string;
      mobileInstallId: string;
      password?: string;
    },
  ): Promise<any> {
    this.keyPair = await generateKeyPair();
    const desktopPub = fromB64(desktopPubKeyB64);
    this.sharedKey = await deriveSharedKey(this.keyPair, desktopPub);

    const deviceId = identity.mobileInstallId;
    const deviceName = this.getMobileDeviceName();
    const userId = identity.userId.trim();
    const mobileInstallId = identity.mobileInstallId.trim();
    // Passwords are opaque credentials. Never normalize whitespace here or
    // credentials accepted by Desktop can fail only on the mobile path.
    const password = identity.password && identity.password.length > 0
      ? identity.password
      : undefined;

    // Step 1: POST /pair → encrypted challenge
    const pairResp = await this.fetchWithRetry(
      `${this.relayUrl}/api/rooms/${encodeURIComponent(this.roomId)}/pair`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          public_key: toB64(this.keyPair.publicKey),
          device_id: deviceId,
          device_name: deviceName,
        }),
      },
      35_000,
    );

    if (!pairResp.ok) {
      throw new Error(`Pairing failed: HTTP ${pairResp.status}`);
    }

    const pairData = await pairResp.json();
    const challengeJson = await decrypt(
      this.sharedKey,
      pairData.encrypted_data,
      pairData.nonce,
    );
    const challenge = JSON.parse(challengeJson);

    // Step 2: POST /command with challenge_echo → initial_sync
    const challengeResponse: Record<string, string> = {
      challenge_echo: challenge.challenge,
      device_id: deviceId,
      device_name: deviceName,
      mobile_install_id: mobileInstallId,
      user_id: userId,
    };
    if (password) {
      challengeResponse.password = password;
    }
    const { data: encData, nonce: encNonce } = await encrypt(
      this.sharedKey,
      JSON.stringify(challengeResponse),
    );

    const cmdResp = await this.fetchWithTimeout(
      `${this.relayUrl}/api/rooms/${encodeURIComponent(this.roomId)}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
      },
      65_000,
    );

    if (!cmdResp.ok) {
      throw new Error(`Pairing verification failed: HTTP ${cmdResp.status}`);
    }

    const cmdData = await cmdResp.json();
    const initialSyncJson = await decrypt(
      this.sharedKey,
      cmdData.encrypted_data,
      cmdData.nonce,
    );
    const parsed = JSON.parse(initialSyncJson);
    if (parsed?.resp === 'error') {
      throw new Error(parsed?.message || 'Pairing rejected');
    }
    return parsed;
  }

  /**
   * Ask the paired desktop to delegate its logged-in account identity
   * (token + master_key). Allows this client to call /api/devices and
   * /api/devices/:id/rpc directly and control any same-account device.
   *
   * Returns true when an identity was delegated; false when the desktop is
   * not logged into an account (or delegation failed). Never throws for the
   * not-logged-in case.
   */
  async requestDelegatedIdentity(options?: { force?: boolean }): Promise<boolean> {
    if (!options?.force && this.hasDelegatedIdentity) return true;
    const requestGeneration = ++this.delegatedIdentityRequestEpoch;
    if (options?.force) {
      // Keep the last committed credential present for target routing, but
      // suspend its use until this refresh settles. On transport failure the
      // last confirmed identity becomes usable again.
      this.delegatedIdentityRefreshOwner = requestGeneration;
    }
    try {
      const resp = await this.sendCommand<{
        resp: string;
        token?: string;
        master_key?: string;
        user_id?: string;
        device_id?: string;
        message?: string;
      }>({ cmd: 'get_delegated_identity' }, { retryable: true });
      if (this.delegatedIdentityRequestEpoch !== requestGeneration) return false;
      if (resp?.resp === 'delegate_identity' && resp.token && resp.master_key) {
        const homeDeviceId = resp.device_id ?? null;
        const nextIdentity: DelegatedIdentitySnapshot = {
          token: resp.token,
          masterKey: fromB64(resp.master_key),
          userId: resp.user_id ?? null,
          homeDeviceId,
          generation: ++this.delegatedIdentityGenerationValue,
        };
        this.delegatedIdentity = nextIdentity;

        const nextAccountIdentity: DelegatedAccountIdentity = {
          userId: nextIdentity.userId,
          masterKey: nextIdentity.masterKey.slice(),
          homeDeviceId: nextIdentity.homeDeviceId,
        };
        const accountChanged = delegatedAccountChanged(
          this.delegatedAccountIdentity,
          nextAccountIdentity,
        );
        const hadAccountIdentity = this.delegatedAccountIdentity !== null;
        if (accountChanged) this.delegatedAccountEpochValue += 1;
        this.delegatedAccountIdentity = nextAccountIdentity;

        const previousHomeDeviceId = this.homeDeviceId;
        const wasUsingDefaultRoom = this.pairedDeviceIdValue === null
          || this.pairedDeviceIdValue === previousHomeDeviceId;
        this.homeDeviceId = homeDeviceId;
        if (accountChanged) {
          // Every semantic account-owner commit starts a new UI/data
          // generation, including a late initial delegation. Explicit React
          // epoch subscribers re-initialize same-owner screens safely.
          this.setPairedDeviceId(homeDeviceId);
        } else if (wasUsingDefaultRoom) {
          // Token/home metadata refresh for the same committed account does
          // not change the effective QR-room route.
          this.pairedDeviceIdValue = homeDeviceId;
        }
        if (accountChanged) {
          this.emitDelegatedAccountOwnerChange({
            kind: hadAccountIdentity ? 'replacement' : 'initial',
            epoch: this.delegatedAccountEpochValue,
            userId: nextIdentity.userId,
            homeDeviceId,
          });
        }
        return true;
      }
      this.commitDelegatedAccountUnavailable();
      return false;
    } finally {
      if (this.delegatedIdentityRefreshOwner === requestGeneration) {
        this.delegatedIdentityRefreshOwner = null;
      }
    }
  }

  /** Drop cached delegated credentials so the next request can refresh them. */
  clearDelegatedIdentity(): void {
    this.delegatedIdentityRequestEpoch += 1;
    this.delegatedIdentityRefreshOwner = null;
    if (this.delegatedIdentity) this.delegatedIdentityGenerationValue += 1;
    this.delegatedIdentity = null;
  }

  /** Fully discard account/control-target state when the mobile disconnects. */
  resetConnectionIdentity(): void {
    const targetEpoch = this.controlTargetEpochValue;
    this.clearDelegatedIdentity();
    this.commitDelegatedAccountUnavailable();
    if (this.controlTargetEpochValue === targetEpoch) {
      // Disconnect is an explicit ownership boundary even when this session
      // never received delegated credentials and was using only the QR room.
      this.setPairedDeviceId(null);
    }
  }

  /**
   * Observe semantic delegated-account owner changes. Token-only refreshes do
   * not emit. The listener is synchronous with the credential commit so UI
   * state is cleared before an operation can publish data for the new owner.
   */
  onDelegatedAccountOwnerChange(
    listener: (change: DelegatedAccountOwnerChange) => void,
    options?: { emitCurrent?: boolean },
  ): () => void {
    this.delegatedAccountOwnerListeners.add(listener);
    if (options?.emitCurrent && this.delegatedAccountIdentity) {
      listener({
        kind: 'initial',
        epoch: this.delegatedAccountEpochValue,
        userId: this.delegatedAccountIdentity.userId,
        homeDeviceId: this.delegatedAccountIdentity.homeDeviceId,
      });
    }
    return () => this.delegatedAccountOwnerListeners.delete(listener);
  }

  private emitDelegatedAccountOwnerChange(change: DelegatedAccountOwnerChange): void {
    for (const listener of this.delegatedAccountOwnerListeners) {
      listener(change);
    }
  }

  private commitDelegatedAccountUnavailable(): void {
    if (this.delegatedIdentity) this.delegatedIdentityGenerationValue += 1;
    this.delegatedIdentity = null;
    if (!this.delegatedAccountIdentity) {
      const wasUsingDefaultRoom = this.pairedDeviceIdValue === null
        || this.pairedDeviceIdValue === this.homeDeviceId;
      this.homeDeviceId = null;
      if (wasUsingDefaultRoom) {
        // A late "not logged in" result with no committed owner changes no
        // route. Do not manufacture a target event that can freeze consumers.
        this.pairedDeviceIdValue = null;
      } else {
        this.setPairedDeviceId(null);
      }
      return;
    }
    this.delegatedAccountEpochValue += 1;
    this.delegatedAccountIdentity = null;
    this.homeDeviceId = null;
    this.setPairedDeviceId(null);
    this.emitDelegatedAccountOwnerChange({
      kind: 'unavailable',
      epoch: this.delegatedAccountEpochValue,
      userId: null,
      homeDeviceId: null,
    });
  }

  /**
   * Send an encrypted command to the desktop and return the decrypted response.
   */
  async sendCommand<T = any>(
    cmd: object,
    options: RelayRequestOptions = {},
  ): Promise<T> {
    if (!this.sharedKey) throw new Error('Not paired');

    const plaintext = JSON.stringify(cmd);
    const { data: encData, nonce: encNonce } = await encrypt(
      this.sharedKey,
      plaintext,
    );

    const body = JSON.stringify({ encrypted_data: encData, nonce: encNonce });

    const resp = await (options.retryable ? this.fetchWithRetry : this.fetchWithTimeout).call(
      this,
      `${this.relayUrl}/api/rooms/${encodeURIComponent(this.roomId)}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      },
      options.retryable ? 20_000 : 65_000,
    );

    if (!resp.ok) {
      throw new Error(`Command failed: HTTP ${resp.status}`);
    }

    const data = await resp.json();
    const decrypted = await decrypt(
      this.sharedKey,
      data.encrypted_data,
      data.nonce,
    );
    return JSON.parse(decrypted) as T;
  }

  get isPaired(): boolean {
    return this.sharedKey !== null;
  }

  get hasDelegatedIdentity(): boolean {
    return this.delegatedIdentity !== null;
  }

  get pairedDeviceId(): string | null {
    return this.pairedDeviceIdValue;
  }

  /**
   * Commit a control target and advance its ownership epoch. Advancing even
   * when the device id repeats is intentional: A -> B -> A must invalidate
   * requests that were issued during the first A ownership interval.
   */
  setPairedDeviceId(deviceId: string | null): void {
    this.pairedDeviceIdValue = deviceId;
    this.controlTargetEpochValue += 1;
    const snapshot = this.getControlTargetSnapshot();
    for (const listener of this.controlTargetListeners) {
      listener(snapshot);
    }
  }

  get controlTargetEpoch(): number {
    return this.controlTargetEpochValue;
  }

  getControlTargetSnapshot(): ControlTargetSnapshot {
    return {
      deviceId: this.pairedDeviceIdValue,
      homeDeviceId: this.homeDeviceId,
      epoch: this.controlTargetEpochValue,
    };
  }

  isControlTargetCurrent(snapshot: ControlTargetSnapshot): boolean {
    // The epoch represents route ownership. Device/home ids are immutable
    // routing inputs captured by the request, but metadata-only home binding
    // intentionally leaves an in-flight QR-room request current.
    return snapshot.epoch === this.controlTargetEpochValue;
  }

  onControlTargetChange(
    listener: (snapshot: ControlTargetSnapshot) => void,
  ): () => void {
    this.controlTargetListeners.add(listener);
    return () => this.controlTargetListeners.delete(listener);
  }

  get delegatedIdentityGeneration(): number {
    return this.delegatedIdentityGenerationValue;
  }

  /** Changes only when the delegated account/home identity changes. */
  get delegatedAccountEpoch(): number {
    return this.delegatedAccountEpochValue;
  }

  /** Canonical delegated account user, or null for legacy Desktop responses. */
  get delegatedUserId(): string | null {
    return this.delegatedIdentity?.userId ?? null;
  }

  private requireDelegatedIdentity(): DelegatedIdentitySnapshot {
    const identity = this.delegatedIdentity;
    if (!identity) throw new Error('No delegated identity');
    return identity;
  }

  private isDelegatedIdentityCurrent(identity: DelegatedIdentitySnapshot): boolean {
    return this.delegatedIdentity?.generation === identity.generation
      && this.delegatedIdentityGenerationValue === identity.generation
      && this.delegatedIdentityRefreshOwner === null;
  }

  private ensureDelegatedIdentityCurrent(
    identity: DelegatedIdentitySnapshot,
    accountEpoch: number,
  ): void {
    if (!this.isDelegatedIdentityCurrent(identity)) {
      throw this.delegatedIdentityChangeError(accountEpoch);
    }
  }

  private delegatedIdentityChangeError(accountEpoch: number): DelegatedIdentityChangedError {
    return this.delegatedAccountEpochValue === accountEpoch
      ? new DelegatedIdentityChangedError()
      : new DelegatedAccountChangedError();
  }

  /**
   * Refresh delegated identity from the paired desktop after a 401, then
   * retry the caller once.
   */
  private async refreshDelegatedIdentityAfterUnauthorized(
    failedIdentity: DelegatedIdentitySnapshot,
  ): Promise<boolean> {
    // A 401 from an old generation must not clear credentials that were
    // delegated by a newer desktop account in the meantime.
    if (!this.isDelegatedIdentityCurrent(failedIdentity)) {
      return this.hasDelegatedIdentity;
    }
    try {
      return await this.requestDelegatedIdentity({ force: true });
    } catch {
      return false;
    }
  }

  /**
   * List all same-account devices via the relay HTTP API.
   * Requires a delegated identity (token + master_key from the paired desktop).
   * On HTTP 401, refreshes identity from the paired desktop and retries once.
   */
  async listDevices(): Promise<Array<{ device_id: string; device_name: string; online: boolean }>> {
    return this.withDelegatedAuthRetry(async (identity) => {
      const resp = await this.fetchWithRetry(`${this.relayUrl}/api/devices`, {
        headers: { 'Authorization': `Bearer ${identity.token}` },
      }, 20_000);
      if (!resp.ok) {
        const err = new Error(`List devices failed: HTTP ${resp.status}`) as Error & {
          status?: number;
        };
        err.status = resp.status;
        throw err;
      }
      return resp.json();
    }, { allowAccountReplacementRetry: true });
  }

  /**
   * Send a RemoteCommand to a target device via the relay HTTP RPC endpoint.
   * The command is encrypted with the delegated master_key (same key the
   * desktop uses, shared via the room channel at pairing time).
   * On HTTP 401, refreshes identity from the paired desktop and retries once.
   */
  async sendDeviceRpc<T = any>(
    targetDeviceId: string,
    command: object,
    options: RelayRequestOptions = {},
  ): Promise<T> {
    return this.withDelegatedAuthRetry(async (identity) => {
      const plaintext = JSON.stringify(command);
      const { data: encData, nonce: encNonce } = await encrypt(
        identity.masterKey,
        plaintext,
      );

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
        options.retryable ? 20_000 : 130_000,
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
        identity.masterKey,
        data.encrypted_data,
        data.nonce,
      );
      const parsed = JSON.parse(decrypted);
      if (parsed?.resp === 'error') {
        throw new Error(parsed.message || 'Remote error');
      }
      return parsed as T;
    }, { allowAccountReplacementRetry: false });
  }

  private async withDelegatedAuthRetry<T>(
    operation: (identity: DelegatedIdentitySnapshot) => Promise<T>,
    options: { allowAccountReplacementRetry: boolean },
  ): Promise<T> {
    let identity = this.requireDelegatedIdentity();
    const accountEpoch = this.delegatedAccountEpochValue;
    // A forced refresh keeps the last committed bytes only so routing remains
    // explicit; it suspends their authority. Fence before invoking the caller
    // because checking only after a device RPC returns is too late for commands
    // that may already have produced side effects on the old account.
    this.ensureDelegatedIdentityCurrent(identity, accountEpoch);
    try {
      const result = await operation(identity);
      this.ensureDelegatedIdentityCurrent(identity, accountEpoch);
      return result;
    } catch (e: unknown) {
      if (!this.isDelegatedIdentityCurrent(identity)) {
        throw this.delegatedIdentityChangeError(accountEpoch);
      }
      const status = (e as { status?: number })?.status;
      const message = String((e as { message?: string })?.message || e);
      const unauthorized =
        status === 401
        || message.includes('HTTP 401')
        || message.includes('Unauthorized');
      if (!unauthorized) throw e;

      const refreshed = await this.refreshDelegatedIdentityAfterUnauthorized(identity);
      if (!refreshed) {
        throw new Error('No delegated identity');
      }
      if (
        !options.allowAccountReplacementRetry
        && this.delegatedAccountEpochValue !== accountEpoch
      ) {
        // Never replay an A-owned RPC target/command with B's credentials.
        // The account-owner listener has already cleared A's UI state.
        throw new DelegatedAccountChangedError();
      }
      identity = this.requireDelegatedIdentity();
      const result = await operation(identity);
      this.ensureDelegatedIdentityCurrent(identity, this.delegatedAccountEpochValue);
      return result;
    }
  }

  private getMobileDeviceName(): string {
    const ua = navigator.userAgent;
    if (/iPhone/i.test(ua)) return 'iPhone';
    if (/iPad/i.test(ua)) return 'iPad';
    if (/Android/i.test(ua)) return 'Android';
    return 'Mobile Browser';
  }
}
