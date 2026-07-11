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
  /** The paired desktop's device_id (for sendDeviceRpc when in relay mode). */
  public pairedDeviceId: string | null = null;

  constructor(relayUrl: string, roomId: string) {
    this.relayUrl = relayUrl.replace(/\/$/, '');
    this.roomId = roomId;
  }

  /**
   * Pair with the desktop via two HTTP round-trips:
   * 1. POST /pair with our public key → receive encrypted challenge
   * 2. POST /command with encrypted challenge_echo → receive initial_sync
   */
  async pair(
    desktopPubKeyB64: string,
    identity: {
      userId: string;
      mobileInstallId: string;
    },
  ): Promise<any> {
    this.keyPair = await generateKeyPair();
    const desktopPub = fromB64(desktopPubKeyB64);
    this.sharedKey = await deriveSharedKey(this.keyPair, desktopPub);

    const deviceId = identity.mobileInstallId;
    const deviceName = this.getMobileDeviceName();
    const userId = identity.userId.trim();
    const mobileInstallId = identity.mobileInstallId.trim();

    // Step 1: POST /pair → encrypted challenge
    const pairResp = await fetch(
      `${this.relayUrl}/api/rooms/${this.roomId}/pair`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          public_key: toB64(this.keyPair.publicKey),
          device_id: deviceId,
          device_name: deviceName,
        }),
      },
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
    const challengeResponse = JSON.stringify({
      challenge_echo: challenge.challenge,
      device_id: deviceId,
      device_name: deviceName,
      mobile_install_id: mobileInstallId,
      user_id: userId,
    });
    const { data: encData, nonce: encNonce } = await encrypt(
      this.sharedKey,
      challengeResponse,
    );

    const cmdResp = await fetch(
      `${this.relayUrl}/api/rooms/${this.roomId}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
      },
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
    // Intercept delegated account identity from the paired desktop.
    // The desktop sends this when the user is logged into their account —
    // it allows the mobile-web to call /api/devices and /api/devices/:id/rpc
    // directly, without needing Argon2id key derivation.
    if (parsed?.resp === 'delegate_identity' && parsed.token && parsed.master_key) {
      this.delegatedToken = parsed.token;
      this.delegatedMasterKey = fromB64(parsed.master_key);
      if (parsed.device_id) {
        this.pairedDeviceId = parsed.device_id;
      }
      // Return a minimal initial_sync so the UI continues normally
      return {
        authenticated_user_id: parsed.user_id,
        has_workspace: false,
      } as any;
    }
    return parsed;
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

    const resp = await fetch(
      `${this.relayUrl}/api/rooms/${this.roomId}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      },
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
   * List all same-account devices via the relay HTTP API.
   * Requires a delegated identity (token + master_key from the paired desktop).
   */
  async listDevices(): Promise<Array<{ device_id: string; device_name: string; online: boolean }>> {
    if (!this.delegatedToken) throw new Error('No delegated identity');
    const resp = await fetch(`${this.relayUrl}/api/devices`, {
      headers: { 'Authorization': `Bearer ${this.delegatedToken}` },
    });
    if (!resp.ok) throw new Error(`List devices failed: HTTP ${resp.status}`);
    return resp.json();
  }

  /**
   * Send a RemoteCommand to a target device via the relay HTTP RPC endpoint.
   * The command is encrypted with the delegated master_key (same key the
   * desktop uses, shared via the room channel at pairing time).
   */
  async sendDeviceRpc<T = any>(targetDeviceId: string, command: object): Promise<T> {
    if (!this.delegatedToken || !this.delegatedMasterKey) {
      throw new Error('No delegated identity');
    }

    // Encrypt the command with the master_key
    const plaintext = JSON.stringify(command);
    const { data: encData, nonce: encNonce } = await encrypt(
      this.delegatedMasterKey,
      plaintext,
    );

    const resp = await fetch(
      `${this.relayUrl}/api/devices/${targetDeviceId}/rpc`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${this.delegatedToken}`,
        },
        body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
      },
    );

    if (!resp.ok) throw new Error(`Device RPC failed: HTTP ${resp.status}`);
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
  }

  private getMobileDeviceName(): string {
    const ua = navigator.userAgent;
    if (/iPhone/i.test(ua)) return 'iPhone';
    if (/iPad/i.test(ua)) return 'iPad';
    if (/Android/i.test(ua)) return 'Android';
    return 'Mobile Browser';
  }
}
