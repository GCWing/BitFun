import { http } from '@kit.NetworkKit';
import { CloudAccountCrypto, CloudAccountKdfParams } from './CloudAccountCrypto';
import { Encoding } from './Encoding';
import { HarmonyRemoteCryptoCipher } from './RemoteCrypto';
import { RemoteLogger } from './RemoteLogger';
import { CommandStatusResponse, EncryptedPayload, RemoteCommand } from '../model/RemoteModels';

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

interface LoginChallengeRequest { username: string; }
interface LoginRequest { username: string; password_hash: string; device_id: string; device_name: string; }
interface RelayErrorResponse { error?: string; }

export class CloudAccountRequestError extends Error {
  readonly statusCode: number;

  constructor(message: string, statusCode: number) {
    super(message);
    this.name = 'CloudAccountRequestError';
    this.statusCode = statusCode;
  }
}

export interface CloudAccountSession {
  token: string;
  userId: string;
  masterKey: Uint8Array;
}

export interface CloudAccountDevice {
  deviceId: string;
  deviceName: string;
  online: boolean;
  lastSeenAt?: number;
}

interface CloudAccountDeviceWire {
  device_id: string;
  device_name: string;
  online: boolean;
  last_seen_at?: number;
}

export interface CloudSessionBundle {
  sessionId: string;
  metadata: Object;
  turns: Object[];
  sourceDeviceId?: string;
  sourceDeviceName?: string;
  version: number;
}

interface SyncSessionEntry {
  session_id: string;
  encrypted_data: string;
  nonce: string;
  version: number;
}

interface SyncSessionList { sessions: SyncSessionEntry[]; }
interface SessionBundleWire {
  session_id: string;
  metadata: Object;
  turns: Object[];
  source_device_id?: string;
  source_device_name?: string;
}
interface SyncSessionUpload {
  session_id: string;
  encrypted_data: string;
  nonce: string;
  version: number;
}

interface SyncSettingsEntry {
  encrypted_data: string;
  nonce: string;
  version: number;
}

export interface CloudSettingsBlob {
  plaintext: string;
  version: number;
}

/** Default BitFun cloud relay used by the desktop account flow. */
export const DEFAULT_CLOUD_RELAY_URL: string = 'https://remote.openbitfun.com/relay';

/** Client for the current desktop relay account protocol. */
export class CloudAccountClient {
  private readonly cipher: HarmonyRemoteCryptoCipher = new HarmonyRemoteCryptoCipher();

  async login(relayUrl: string, username: string, password: string, deviceId: string): Promise<CloudAccountSession> {
    const normalizedRelayUrl = relayUrl.trim() || DEFAULT_CLOUD_RELAY_URL;
    const normalizedUser = username.trim();
    const startedAt = Date.now();
    RemoteLogger.info(`cloud login start relay=${normalizedRelayUrl}`);
    if (normalizedUser.length === 0 || normalizedUser.length > 128 || password.length === 0 || password.length > 1024) {
      throw new Error('Invalid account credentials.');
    }
    const challengeRequest: LoginChallengeRequest = { username: normalizedUser };
    const challengeStartedAt = Date.now();
    const challenge = await this.post<AccountChallenge>(normalizedRelayUrl, '/api/auth/login/challenge', challengeRequest);
    RemoteLogger.info(`cloud login challenge received elapsed_ms=${Date.now() - challengeStartedAt}`);
    const params = JSON.parse(challenge.argon2_params) as CloudAccountKdfParams;
    const salt = Encoding.base64ToBytes(challenge.salt);
    const kdfSalt = Encoding.base64ToBytes(challenge.kdf_salt);
    const kek = await CloudAccountCrypto.derivePasswordHash(password, salt, params);
    RemoteLogger.info(`cloud login password proof derived elapsed_ms=${Date.now() - startedAt}`);
    const masterKey = await this.unwrapMasterKey(kek, challenge.wrapped_master_key);
    RemoteLogger.info(`cloud login master key unwrapped elapsed_ms=${Date.now() - startedAt}`);
    const passwordHash = await CloudAccountCrypto.derivePasswordHash(password, kdfSalt, params);
    const loginRequest: LoginRequest = {
      username: normalizedUser,
      password_hash: Encoding.bytesToBase64(passwordHash),
      device_id: deviceId,
      device_name: 'HarmonyOS Phone'
    };
    const auth = await this.post<AccountAuthResponse>(normalizedRelayUrl, '/api/auth/login', loginRequest);
    RemoteLogger.info(`cloud login authenticated elapsed_ms=${Date.now() - startedAt}`);
    return { token: auth.token, userId: auth.user_id, masterKey };
  }

  async fetchSessions(relayUrl: string, session: CloudAccountSession, since: number = 0): Promise<CloudSessionBundle[]> {
    const suffix = `/api/sync/sessions?since=${Math.max(0, since)}`;
    const payload = await this.request<SyncSessionList>(relayUrl, suffix, 'GET', undefined, session.token);
    const bundles: CloudSessionBundle[] = [];
    for (const entry of payload.sessions || []) {
      const plain = await this.decryptSyncPayload(session.masterKey, entry.encrypted_data, entry.nonce);
      const bundle = JSON.parse(plain) as SessionBundleWire;
      bundles.push({
        sessionId: bundle.session_id || entry.session_id,
        metadata: bundle.metadata,
        turns: bundle.turns,
        sourceDeviceId: bundle.source_device_id,
        sourceDeviceName: bundle.source_device_name,
        version: entry.version
      });
    }
    return bundles;
  }

  async fetchSettings(relayUrl: string, session: CloudAccountSession): Promise<CloudSettingsBlob | undefined> {
    let entry: SyncSettingsEntry;
    try {
      entry = await this.request<SyncSettingsEntry>(relayUrl, '/api/sync/settings', 'GET', undefined, session.token);
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 404) {
        return undefined;
      }
      throw err instanceof Error ? err : new Error('Cloud settings request failed.');
    }
    if (!entry || !entry.encrypted_data || !entry.nonce) {
      return undefined;
    }
    return {
      plaintext: await this.decryptSyncPayload(session.masterKey, entry.encrypted_data, entry.nonce),
      version: entry.version
    };
  }

  async listDevices(relayUrl: string, session: CloudAccountSession): Promise<CloudAccountDevice[]> {
    const devices = await this.request<CloudAccountDeviceWire[]>(relayUrl, '/api/devices', 'GET', undefined, session.token);
    return devices.map((device: CloudAccountDeviceWire): CloudAccountDevice => ({
      deviceId: device.device_id,
      deviceName: device.device_name || device.device_id,
      online: device.online,
      lastSeenAt: device.last_seen_at
    }));
  }

  async deviceRpc<T extends CommandStatusResponse>(
    relayUrl: string,
    session: CloudAccountSession,
    targetDeviceId: string,
    command: RemoteCommand
  ): Promise<T> {
    const target = targetDeviceId.trim();
    if (target.length === 0) {
      throw new Error('Remote target device is required.');
    }
    const nonce = Encoding.randomBytes(12);
    const encrypted = await this.cipher.encrypt(
      Encoding.utf8ToBytes(JSON.stringify(command)),
      session.masterKey,
      nonce
    );
    const body: EncryptedPayload = {
      encrypted_data: Encoding.bytesToBase64(encrypted),
      nonce: Encoding.bytesToBase64(nonce)
    };
    const response = await this.request<EncryptedPayload>(
      relayUrl,
      `/api/devices/${encodeURIComponent(target)}/rpc`,
      'POST',
      body,
      session.token,
      130000
    );
    const plain = await this.cipher.decrypt(
      Encoding.base64ToBytes(response.encrypted_data),
      session.masterKey,
      Encoding.base64ToBytes(response.nonce)
    );
    const parsed = Encoding.parseJsonObject<T>(Encoding.bytesToUtf8(plain));
    if (parsed.resp === 'error') {
      throw new Error(parsed.message || 'Remote device command failed.');
    }
    return parsed;
  }

  async deleteSession(relayUrl: string, session: CloudAccountSession, sessionId: string): Promise<void> {
    await this.request<Object>(relayUrl, `/api/sync/sessions/${encodeURIComponent(sessionId)}`, 'DELETE', undefined, session.token);
  }

  async uploadSession(relayUrl: string, session: CloudAccountSession, bundle: CloudSessionBundle): Promise<number> {
    const nonce = Encoding.randomBytes(12);
    const wire: SessionBundleWire = {
      session_id: bundle.sessionId,
      metadata: bundle.metadata,
      turns: bundle.turns,
      source_device_id: bundle.sourceDeviceId,
      source_device_name: bundle.sourceDeviceName
    };
    const encrypted = await this.cipher.encrypt(
      Encoding.utf8ToBytes(JSON.stringify(wire)), session.masterKey, nonce
    );
    const version = Date.now();
    const body: SyncSessionUpload = {
      session_id: bundle.sessionId,
      encrypted_data: Encoding.bytesToBase64(encrypted),
      nonce: Encoding.bytesToBase64(nonce),
      version
    };
    await this.request<Object>(relayUrl, '/api/sync/sessions', 'POST', body, session.token);
    return version;
  }

  private async decryptSyncPayload(masterKey: Uint8Array, data: string, nonceText: string): Promise<string> {
    const plain = await this.cipher.decrypt(Encoding.base64ToBytes(data), masterKey, Encoding.base64ToBytes(nonceText));
    return Encoding.bytesToUtf8(plain);
  }

  private async unwrapMasterKey(kek: Uint8Array, packed: string): Promise<Uint8Array> {
    const parts = packed.split('.');
    if (parts.length !== 2) {
      throw new Error('Invalid wrapped master key.');
    }
    const ciphertext = Encoding.base64ToBytes(parts[0]);
    const nonce = Encoding.base64ToBytes(parts[1]);
    if (kek.length !== 32 || nonce.length !== 12) {
      throw new Error('Invalid account encryption parameters.');
    }
    try {
      const plain = await this.cipher.decrypt(ciphertext, kek, nonce);
      if (plain.length !== 32) {
        throw new Error('Invalid master key length.');
      }
      return plain;
    } catch (_err) {
      throw new Error('Invalid username or password.');
    }
  }

  private async post<T>(relayUrl: string, path: string, body: Object): Promise<T> {
    return this.request<T>(relayUrl, path, 'POST', body);
  }

  private async request<T>(
    relayUrl: string,
    path: string,
    method: string,
    body?: Object,
    token: string = '',
    readTimeoutMs: number = 120000
  ): Promise<T> {
    const request = http.createHttp();
    const base = relayUrl.replace(/\/$/, '');
    try {
      const headers: Record<string, string> = { 'Content-Type': 'application/json', 'Accept': 'application/json' };
      if (token.length > 0) headers.Authorization = `Bearer ${token}`;
      const requestOptions: http.HttpRequestOptions = {
        method: method === 'GET' ? http.RequestMethod.GET : method === 'DELETE' ? http.RequestMethod.DELETE : http.RequestMethod.POST,
        header: headers,
        expectDataType: http.HttpDataType.STRING,
        connectTimeout: 15000,
        readTimeout: readTimeoutMs
      };
      if (body !== undefined) requestOptions.extraData = JSON.stringify(body);
      const response = await request.request(`${base}${path}`, requestOptions);
      const text = typeof response.result === 'string' ? response.result : JSON.stringify(response.result);
      if (response.responseCode < 200 || response.responseCode >= 300) {
        let message = `Relay login failed (HTTP ${response.responseCode}).`;
        try {
          const error = JSON.parse(text) as RelayErrorResponse;
          if (error.error) message = error.error;
        } catch (_err) {
          // Keep the status-based error when the relay did not return JSON.
        }
        throw new CloudAccountRequestError(message, response.responseCode);
      }
      return JSON.parse(text) as T;
    } finally {
      request.destroy();
    }
  }
}
