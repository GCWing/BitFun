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

  constructor(relayUrl: string, roomId: string) {
    this.relayUrl = relayUrl.replace(/\/$/, '');
    this.roomId = roomId;
  }

  /**
   * Pair with the desktop via two HTTP round-trips:
   * 1. POST /pair with our public key → receive encrypted challenge
   * 2. POST /command with encrypted challenge_echo → receive initial_sync
   */
  async pair(desktopPubKeyB64: string): Promise<any> {
    this.keyPair = await generateKeyPair();
    const desktopPub = fromB64(desktopPubKeyB64);
    this.sharedKey = await deriveSharedKey(this.keyPair, desktopPub);

    const deviceId = `mobile-${Date.now().toString(36)}`;
    const deviceName = this.getMobileDeviceName();

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
    return JSON.parse(initialSyncJson);
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

    const resp = await fetch(
      `${this.relayUrl}/api/rooms/${this.roomId}/command`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ encrypted_data: encData, nonce: encNonce }),
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

  private getMobileDeviceName(): string {
    const ua = navigator.userAgent;
    if (/iPhone/i.test(ua)) return 'iPhone';
    if (/iPad/i.test(ua)) return 'iPad';
    if (/Android/i.test(ua)) return 'Android';
    return 'Mobile Browser';
  }
}
