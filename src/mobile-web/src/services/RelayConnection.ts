/**
 * WebSocket connection to the relay server from the mobile client.
 * Handles join_room, message relay, heartbeat, reconnection,
 * and HTTP polling for buffered messages.
 */

import { generateKeyPair, deriveSharedKey, encrypt, decrypt, toB64, fromB64, MobileKeyPair } from './E2EEncryption';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'paired' | 'error';

export interface RelayCallbacks {
  onStateChange: (state: ConnectionState) => void;
  onMessage: (json: string) => void;
  onError: (msg: string) => void;
}

export interface BufferedMessage {
  seq: number;
  timestamp: number;
  direction: string;
  encrypted_data: string;
  nonce: string;
}

export class RelayConnection {
  private keyPair: MobileKeyPair | null = null;
  private sharedKey: Uint8Array | null = null;
  private roomId: string;
  private desktopPubKey: Uint8Array;
  private desktopDeviceId: string;
  private mobileDeviceId: string;
  private callbacks: RelayCallbacks;
  private wsUrl: string;
  private httpBaseUrl: string;
  private destroyed = false;
  private _lastSeq = 0;
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  constructor(
    wsUrl: string,
    roomId: string,
    desktopPubKeyB64: string,
    desktopDeviceId: string,
    callbacks: RelayCallbacks,
  ) {
    this.wsUrl = wsUrl;
    this.roomId = roomId;
    this.desktopPubKey = fromB64(desktopPubKeyB64);
    this.desktopDeviceId = desktopDeviceId;
    this.mobileDeviceId = `mobile-${Date.now().toString(36)}`;
    this.callbacks = callbacks;

    // Derive HTTP base URL from wsUrl for polling
    this.httpBaseUrl = wsUrl
      .replace(/^wss:\/\//, 'https://')
      .replace(/^ws:\/\//, 'http://')
      .replace(/\/ws\/?$/, '')
      .replace(/\/$/, '');
  }

  get lastSeq(): number {
    return this._lastSeq;
  }

  async connect() {
    this.callbacks.onStateChange('connecting');
    this.keyPair = await generateKeyPair();

    try {
      this.sharedKey = await deriveSharedKey(this.keyPair, this.desktopPubKey);
    } catch (e: any) {
      this.callbacks.onError(`Key derivation failed: ${e?.message || e}`);
      return;
    }

    try {
      const resp = await fetch(`${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/join`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          device_id: this.mobileDeviceId,
          device_type: 'mobile',
          public_key: toB64(this.keyPair!.publicKey),
        }),
      });

      if (!resp.ok) {
        throw new Error('Failed to join room');
      }

      const data = await resp.json();
      if (data.status === 'joined') {
        this.callbacks.onStateChange('connected');
        // Must start polling immediately so we can receive the desktop's challenge
        // and complete the pairing handshake (challenge → challenge_echo → 'paired').
        this.startPolling(1000);
      }
    } catch (e: any) {
      this.callbacks.onError(`Join failed: ${e?.message || e}`);
      this.callbacks.onStateChange('disconnected');
    }
  }

  async sendEncrypted(plaintext: string) {
    if (!this.sharedKey) return;
    const { data, nonce } = await encrypt(this.sharedKey, plaintext);
    
    try {
      const resp = await fetch(`${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/message`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          device_id: this.mobileDeviceId,
          encrypted_data: data,
          nonce,
        }),
      });
      if (!resp.ok) {
        throw new Error(`Relay message rejected: HTTP ${resp.status}`);
      }
    } catch (e: any) {
      console.error('[Relay] Failed to send encrypted message', e);
      this.callbacks.onError(`Send failed: ${e?.message || e}`);
    }
  }

  async sendCommand(cmd: object) {
    await this.sendEncrypted(JSON.stringify(cmd));
  }

  private getMobileDeviceName(): string {
    const ua = navigator.userAgent;
    if (/iPhone/i.test(ua)) return 'iPhone';
    if (/iPad/i.test(ua)) return 'iPad';
    if (/Android/i.test(ua)) return 'Android';
    return 'Mobile Browser';
  }

  setMessageHandler(handler: (json: string) => void) {
    this.callbacks.onMessage = handler;
  }

  // ── HTTP Polling API ──────────────────────────────────────────

  /** Poll the relay server for buffered messages via HTTP. */
  async pollMessages(): Promise<{ messages: BufferedMessage[], peer_connected: boolean }> {
    try {
      const url = `${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/poll?since_seq=${this._lastSeq}&device_type=mobile`;
      const resp = await fetch(url);
      if (!resp.ok) return { messages: [], peer_connected: false };
      const data = await resp.json();
      const messages: BufferedMessage[] = data.messages || [];

      if (messages.length > 0) {
        const maxSeq = Math.max(...messages.map((m: BufferedMessage) => m.seq));
        this._lastSeq = maxSeq;
      }
      return { messages, peer_connected: data.peer_connected || false };
    } catch {
      return { messages: [], peer_connected: false };
    }
  }

  /** Acknowledge receipt of messages up to the current lastSeq. */
  async ackMessages(): Promise<void> {
    if (this._lastSeq === 0) return;
    try {
      await fetch(`${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/ack`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ack_seq: this._lastSeq, device_type: 'mobile' }),
      });
    } catch {
      // best effort
    }
  }

  /** Start periodic polling (call after pairing). */
  startPolling(intervalMs = 2000) {
    this.stopPolling();
    this.pollTimer = setInterval(async () => {
      const { messages, peer_connected } = await this.pollMessages();
      
      if (!peer_connected && this.sharedKey) {
        // Desktop disconnected — stop polling before notifying so the callback
        // doesn't race with further poll ticks.
        this.stopPolling();
        this.sharedKey = null;
        this.callbacks.onStateChange('disconnected');
        return;
      }

      if (!this.sharedKey) return;
      
      for (const msg of messages) {
        try {
          const plaintext = await decrypt(this.sharedKey, msg.encrypted_data, msg.nonce);
          const parsed = JSON.parse(plaintext);
          
          if (parsed.challenge && parsed.timestamp) {
            const response = JSON.stringify({
              challenge_echo: parsed.challenge,
              device_id: this.mobileDeviceId,
              device_name: this.getMobileDeviceName(),
            });
            await this.sendEncrypted(response);
            this.callbacks.onStateChange('paired');
          } else {
            this.callbacks.onMessage(plaintext);
          }
        } catch {
          // skip messages that fail to decrypt
        }
      }
      if (messages.length > 0) {
        await this.ackMessages();
      }
    }, intervalMs);
  }

  stopPolling() {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  disconnect() {
    this.destroyed = true;
    this.stopPolling();
    this.sharedKey = null;
    this.callbacks.onStateChange('disconnected');
  }

  get isPaired(): boolean {
    return this.sharedKey !== null;
  }
}
