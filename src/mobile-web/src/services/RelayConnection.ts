/**
 * WebSocket connection to the relay server from the mobile client.
 * Handles join_room, message relay, heartbeat, and reconnection via WebSocket.
 *
 * Uses WebSocket for all communication since the relay server's /ws endpoint
 * is properly proxied by nginx on the BitFun server. The HTTP polling approach
 * was replaced because it requires nginx to proxy POST API requests, which is
 * not guaranteed in all deployment configurations.
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
  private ws: WebSocket | null = null;
  private destroyed = false;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  // Kept for API compatibility with callers that expect these properties
  readonly httpBaseUrl: string;
  private _lastSeq = 0;

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

    // Compute HTTP base URL for backward compat (not used in WS mode)
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
      this.callbacks.onStateChange('disconnected');
      return;
    }

    // Build the WebSocket endpoint URL: append /ws if not already present
    const wsEndpoint = this.wsUrl.replace(/\/ws\/?$/, '') + '/ws';


    try {
      this.ws = new WebSocket(wsEndpoint);
    } catch (e: any) {
      this.callbacks.onError(`WebSocket connection failed: ${e?.message || e}`);
      this.callbacks.onStateChange('disconnected');
      return;
    }

    this.ws.onopen = () => {
      if (this.destroyed) return;
      this.ws!.send(JSON.stringify({
        type: 'join_room',
        room_id: this.roomId,
        device_id: this.mobileDeviceId,
        device_type: 'mobile',
        public_key: toB64(this.keyPair!.publicKey),
      }));
      this.callbacks.onStateChange('connected');
      this.startHeartbeat();
    };

    this.ws.onmessage = async (event: MessageEvent) => {
      if (this.destroyed) return;
      try {
        const msg = JSON.parse(event.data as string);


        await this.handleWsMessage(msg);
      } catch {
        // ignore parse errors
      }
    };

    this.ws.onclose = (_event: CloseEvent) => {
      if (this.destroyed) return;
      this.stopHeartbeat();
      this.sharedKey = null;
      this.callbacks.onStateChange('disconnected');
    };

    this.ws.onerror = (_event: Event) => {
      if (this.destroyed) return;
      this.callbacks.onError('WebSocket connection error');
    };
  }

  private async handleWsMessage(msg: any) {
    switch (msg.type) {
      case 'relay': {
        if (!this.sharedKey) return;
        try {
          const plaintext = await decrypt(this.sharedKey, msg.encrypted_data, msg.nonce);
          const parsed = JSON.parse(plaintext);

          if (parsed.challenge && parsed.timestamp) {
            // Pairing challenge from desktop — send challenge echo back
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
          // Decryption failed — skip this message
        }
        break;
      }

      case 'peer_disconnected': {
        this.stopHeartbeat();
        this.sharedKey = null;
        this.callbacks.onStateChange('disconnected');
        break;
      }

      case 'error': {
        this.callbacks.onError(`Join failed: ${msg.message}`);
        this.callbacks.onStateChange('disconnected');
        break;
      }

      case 'peer_joined':
        // Desktop's info — we already have the public key from the QR code, nothing to do
        break;

      case 'heartbeat_ack':
        // Heartbeat acknowledged by server
        break;

      default:
        break;
    }
  }

  async sendEncrypted(plaintext: string) {
    if (!this.sharedKey || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const { data, nonce } = await encrypt(this.sharedKey, plaintext);
    this.ws.send(JSON.stringify({
      type: 'relay',
      room_id: this.roomId,
      encrypted_data: data,
      nonce,
    }));
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

  // ── Heartbeat ──────────────────────────────────────────────────────

  private startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'heartbeat' }));
      }
    }, 25_000);
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  // ── HTTP Polling API (kept for API compatibility, no-ops in WS mode) ──

  async pollMessages(): Promise<{ messages: BufferedMessage[], peer_connected: boolean }> {
    return { messages: [], peer_connected: !!this.sharedKey };
  }

  async ackMessages(): Promise<void> {}

  /** No-op: WebSocket receives messages in real-time without polling. */
  startPolling(_intervalMs = 2000) {}

  /** No-op: no polling timer to stop in WebSocket mode. */
  stopPolling() {}

  disconnect() {
    this.destroyed = true;
    this.stopHeartbeat();
    this.ws?.close();
    this.ws = null;
    this.sharedKey = null;
    this.callbacks.onStateChange('disconnected');
  }

  get isPaired(): boolean {
    return this.sharedKey !== null;
  }
}
