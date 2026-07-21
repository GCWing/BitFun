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

export class RelayHttpClient {
  private relayUrl: string;
  private roomId: string;
  private sharedKey: Uint8Array | null = null;
  private keyPair: MobileKeyPair | null = null;
  /** Delegated account identity (token + master_key) from the paired desktop. */
  public delegatedToken: string | null = null;
  public delegatedMasterKey: Uint8Array | null = null;
  /** The current control-target device_id (for sendDeviceRpc). */
  public pairedDeviceId: string | null = null;
  /** The QR-paired desktop's device_id (the "home" device of this session). */
  public homeDeviceId: string | null = null;

  constructor(relayUrl: string, roomId: string) {
    this.relayUrl = relayUrl.replace(/\/$/, '');
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
      return await fetch(input, { ...init, signal: controller.signal });
    } catch (error: unknown) {
      if ((error as { name?: string })?.name === 'AbortError') {
        throw new Error('Request timed out');
      }
      throw error;
    } finally {
      window.clearTimeout(timer);
    }
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
    const pairResp = await this.fetchWithTimeout(
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
    if (options?.force) {
      this.clearDelegatedIdentity();
    }
    const resp = await this.sendCommand<{
      resp: string;
      token?: string;
      master_key?: string;
      device_id?: string;
      message?: string;
    }>({ cmd: 'get_delegated_identity' });
    if (resp?.resp === 'delegate_identity' && resp.token && resp.master_key) {
      this.delegatedToken = resp.token;
      this.delegatedMasterKey = fromB64(resp.master_key);
      if (resp.device_id) {
        this.homeDeviceId = resp.device_id;
        if (!this.pairedDeviceId) {
          this.pairedDeviceId = resp.device_id;
        }
      }
      return true;
    }
    return false;
  }

  /** Drop cached delegated credentials so the next request can refresh them. */
  clearDelegatedIdentity(): void {
    this.delegatedToken = null;
    this.delegatedMasterKey = null;
  }

  /**
   * Send an encrypted command to the desktop and return the decrypted response.
   */
  async sendCommand<T = any>(cmd: object): Promise<T> {
    if (!this.sharedKey) throw new Error('Not paired');

    const plaintext = JSON.stringify(cmd);
    const { data: encData, nonce: encNonce } = await encrypt(
      this.sharedKey,
      plaintext,
    );

    const body = JSON.stringify({ encrypted_data: encData, nonce: encNonce });

    const resp = await this.fetchWithTimeout(
      `${this.relayUrl}/api/rooms/${encodeURIComponent(this.roomId)}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      },
      65_000,
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
    return this.delegatedToken !== null && this.delegatedMasterKey !== null;
  }

  /**
   * Refresh delegated identity from the paired desktop after a 401, then
   * retry the caller once.
   */
  private async refreshDelegatedIdentityAfterUnauthorized(): Promise<boolean> {
    this.clearDelegatedIdentity();
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
    return this.withDelegatedAuthRetry(async () => {
      if (!this.delegatedToken) throw new Error('No delegated identity');
      const resp = await this.fetchWithTimeout(`${this.relayUrl}/api/devices`, {
        headers: { 'Authorization': `Bearer ${this.delegatedToken}` },
      }, 20_000);
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

  /**
   * Send a RemoteCommand to a target device via the relay HTTP RPC endpoint.
   * The command is encrypted with the delegated master_key (same key the
   * desktop uses, shared via the room channel at pairing time).
   * On HTTP 401, refreshes identity from the paired desktop and retries once.
   */
  async sendDeviceRpc<T = any>(targetDeviceId: string, command: object): Promise<T> {
    return this.withDelegatedAuthRetry(async () => {
      if (!this.delegatedToken || !this.delegatedMasterKey) {
        throw new Error('No delegated identity');
      }

      const plaintext = JSON.stringify(command);
      const { data: encData, nonce: encNonce } = await encrypt(
        this.delegatedMasterKey,
        plaintext,
      );

      const resp = await this.fetchWithTimeout(
        `${this.relayUrl}/api/devices/${encodeURIComponent(targetDeviceId)}/rpc`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${this.delegatedToken}`,
          },
          body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
        },
        130_000,
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
        this.delegatedMasterKey,
        data.encrypted_data,
        data.nonce,
      );
      const parsed = JSON.parse(decrypted);
      if (parsed?.resp === 'error') {
        throw new Error(parsed.message || 'Remote error');
      }
      return parsed as T;
    });
  }

  private async withDelegatedAuthRetry<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (e: unknown) {
      const status = (e as { status?: number })?.status;
      const message = String((e as { message?: string })?.message || e);
      const unauthorized =
        status === 401
        || message.includes('HTTP 401')
        || message.includes('Unauthorized');
      if (!unauthorized) throw e;

      const refreshed = await this.refreshDelegatedIdentityAfterUnauthorized();
      if (!refreshed) {
        throw new Error('No delegated identity');
      }
      return operation();
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
