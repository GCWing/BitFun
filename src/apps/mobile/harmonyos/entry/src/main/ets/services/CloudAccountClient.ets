import { http } from '@kit.NetworkKit';
import { CloudAccountCrypto, CloudAccountKdfParams } from './CloudAccountCrypto';
import { Encoding } from './Encoding';
import { HarmonyRemoteCryptoCipher } from './RemoteCrypto';
import { OhosError } from './OhosError';
import { RemoteLogger } from './RemoteLogger';
import { RemoteI18n } from '../i18n/RemoteI18n';
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
interface LoginRequest {
  username: string;
  password_hash: string;
  device_id: string;
  device_name: string;
  device_kind: string;
  request_id?: string;
}
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

/** The live account client plus the session it is currently signed in with. */
export interface CloudAccountBinding {
  client: CloudAccountClient;
  relayUrl: string;
  session: CloudAccountSession;
}

export interface CloudAccountDevice {
  deviceId: string;
  deviceName: string;
  deviceKind?: string;
  online: boolean;
  lastSeenAt?: number;
}

/** A device credential minted for another device, never for this one. */
export interface CloudProvisionedDevice {
  token: string;
  userId: string;
  deviceId: string;
}

interface ProvisionDeviceRequest {
  device_id: string;
  device_name: string;
  device_kind: string;
  request_id: string;
}

interface ProvisionDeviceWire {
  token: string;
  user_id: string;
  device_id: string;
}

interface CloudAccountDeviceWire {
  device_id: string;
  device_name: string;
  device_kind?: string;
  online: boolean;
  last_seen_at?: number;
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

/** Device kinds the relay accepts; mirrors `relay-service/src/db.rs::DEVICE_KINDS`. */
const DEVICE_KIND_DESKTOP: string = 'desktop';
const DEVICE_KIND_MOBILE: string = 'mobile';
const DEVICE_KIND_WATCH: string = 'watch';

/**
 * Device names our own non-desktop builds register under, lowercased.
 *
 * Only labels we hardcode ourselves belong here: the Android client reports
 * `Build.MODEL` (`android/.../DeviceInstall.kt:27`), an arbitrary string that
 * guessing at would hide desktops as readily as phones.
 *
 * Kept in step with `shared/core-transport/.../CloudAccountClient.kt`, which
 * carries the same list for the Android and iOS clients.
 */
const KNOWN_NON_DESKTOP_DEVICE_NAMES: string[] = ['harmonyos phone', 'harmonyos watch'];

/**
 * Whether a relay device row is a desktop, and so controllable from a phone.
 *
 * A row that reports its kind is taken at its word. A row without one predates
 * the relay learning about kinds, and is judged by two weaker signals: this
 * phone's own row is never a desktop, and neither is one carrying a name our
 * own builds register under. Anything else stays visible — hiding a real
 * desktop would strand the user, while a stale phone row disappears the next
 * time that phone logs in against a relay that stores kinds.
 */
function isDesktopDeviceRow(device: CloudAccountDeviceWire, selfDeviceId: string): boolean {
  const kind = (device.device_kind || '').trim();
  if (kind.length > 0) {
    return kind === DEVICE_KIND_DESKTOP;
  }
  if (selfDeviceId.length > 0 && device.device_id === selfDeviceId) {
    return false;
  }
  const name = (device.device_name || '').trim().toLowerCase();
  return KNOWN_NON_DESKTOP_DEVICE_NAMES.indexOf(name) < 0;
}

/** Client for the current desktop relay account protocol. */
export class CloudAccountClient {
  private readonly cipher: HarmonyRemoteCryptoCipher = new HarmonyRemoteCryptoCipher();

  async login(relayUrl: string, username: string, password: string, deviceId: string): Promise<CloudAccountSession> {
    return this.loginDevice(
      relayUrl, username, password, deviceId, 'HarmonyOS Phone', DEVICE_KIND_MOBILE);
  }

  /**
   * Compatibility path for relays whose dedicated device-provisioning route is
   * present but cannot mint against their existing account database.
   *
   * This is still a real device login: the watch receives its own token and
   * device row. Only the password proof travels to the relay, while the
   * plaintext password and the account master key stay on this phone.
   */
  async loginWatch(
    relayUrl: string,
    username: string,
    password: string,
    currentSession: CloudAccountSession,
    deviceId: string,
    deviceName: string,
    requestId: string
  ): Promise<CloudProvisionedDevice> {
    const watchSession = await this.loginDevice(
      relayUrl, username, password, deviceId, deviceName, DEVICE_KIND_WATCH, requestId);
    if (watchSession.userId !== currentSession.userId ||
      !CloudAccountClient.sameBytes(watchSession.masterKey, currentSession.masterKey)) {
      throw new Error('The confirmed account does not match the signed-in account.');
    }
    return { token: watchSession.token, userId: watchSession.userId, deviceId };
  }

  private async loginDevice(
    relayUrl: string,
    username: string,
    password: string,
    deviceId: string,
    deviceName: string,
    deviceKind: string,
    requestId: string = ''
  ): Promise<CloudAccountSession> {
    const normalizedRelayUrl = relayUrl.trim() || DEFAULT_CLOUD_RELAY_URL;
    const normalizedUser = username.trim();
    const startedAt = Date.now();
    RemoteLogger.info(`cloud device login start relay=${normalizedRelayUrl} kind=${deviceKind}`);
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
      device_name: deviceName,
      device_kind: deviceKind
    };
    if (requestId.length > 0) {
      loginRequest.request_id = requestId;
    }
    const auth = await this.post<AccountAuthResponse>(normalizedRelayUrl, '/api/auth/login', loginRequest);
    RemoteLogger.info(`cloud login authenticated elapsed_ms=${Date.now() - startedAt}`);
    return { token: auth.token, userId: auth.user_id, masterKey };
  }

  private static sameBytes(left: Uint8Array, right: Uint8Array): boolean {
    if (left.length !== right.length) {
      return false;
    }
    let difference = 0;
    for (let index = 0; index < left.length; index += 1) {
      difference |= left[index] ^ right[index];
    }
    return difference === 0;
  }

  /**
   * Adds another device to this account and returns its own credential.
   *
   * The relay gates `/api/auth/provision-device` on nothing but
   * `AuthToken::is_device_token()` (`relay-service/src/routes/auth.rs`), so a
   * phone that signed in with the account password is as entitled to mint here
   * as the desktop is — the desktop was never special, it just happened to be
   * the one holding a session. A token delegated by a room pairing is
   * `delegated_control` rather than `device` and is refused with 403; that is
   * the caller's cue to fall back to asking the desktop.
   *
   * The account master key is deliberately not part of this exchange. The relay
   * never sees it, and the caller already holds its own copy — a watch gets it
   * sealed to its ephemeral key, not from here.
   */
  async provisionDevice(
    relayUrl: string,
    session: CloudAccountSession,
    deviceId: string,
    deviceName: string,
    requestId: string
  ): Promise<CloudProvisionedDevice> {
    const body: ProvisionDeviceRequest = {
      device_id: deviceId,
      device_name: deviceName,
      device_kind: DEVICE_KIND_WATCH,
      request_id: requestId
    };
    const wire = await this.request<ProvisionDeviceWire>(
      relayUrl, '/api/auth/provision-device', 'POST', body, session.token);
    const token = (wire.token || '').trim();
    const userId = (wire.user_id || '').trim();
    const provisionedDeviceId = (wire.device_id || '').trim();
    if (token.length === 0 || userId !== session.userId || provisionedDeviceId !== deviceId) {
      // A credential naming a different account or device would sign the watch
      // in as somebody else. Refuse rather than pass it on.
      throw new Error('Relay returned a mismatched provisioned device identity.');
    }
    return { token, userId, deviceId: provisionedDeviceId };
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

  /**
   * The account's controllable devices — desktops only.
   *
   * A relay that stores device kinds already filters this list; the client
   * repeats the judgement so a phone stops listing itself and its peers before
   * that relay is deployed. [selfDeviceId] is this install's own device id.
   */
  async listDevices(
    relayUrl: string,
    session: CloudAccountSession,
    selfDeviceId: string = ''
  ): Promise<CloudAccountDevice[]> {
    const devices = await this.request<CloudAccountDeviceWire[]>(relayUrl, '/api/devices', 'GET', undefined, session.token);
    const desktops = devices.filter((device: CloudAccountDeviceWire): boolean =>
      isDesktopDeviceRow(device, selfDeviceId));
    return desktops.map((device: CloudAccountDeviceWire): CloudAccountDevice => ({
      deviceId: device.device_id,
      deviceName: device.device_name || device.device_id,
      deviceKind: device.device_kind,
      online: device.online,
      lastSeenAt: device.last_seen_at
    }));
  }

  /**
   * [timeoutMs] defaults to the relay's own patience for a device that is slow
   * to answer. A caller with a reason to give up earlier — a handshake, where
   * silence means the desktop is gone rather than busy — passes its own.
   */
  async deviceRpc<T extends CommandStatusResponse>(
    relayUrl: string,
    session: CloudAccountSession,
    targetDeviceId: string,
    command: RemoteCommand,
    timeoutMs: number = 130000
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
    RemoteLogger.info(`device rpc cmd=${command.cmd || 'unknown'} bytes=${body.encrypted_data.length}`);
    const response = await this.request<EncryptedPayload>(
      relayUrl,
      `/api/devices/${encodeURIComponent(target)}/rpc`,
      'POST',
      body,
      session.token,
      timeoutMs
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
    // Timed so the watch has something to be slow relative to. Phone and watch
    // reach the same relay and the same desktop, so a gap between them is the
    // client's own cost and a shared floor is the round trip's.
    const startedAt = Date.now();
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
      let bodyBytes = 0;
      if (body !== undefined) {
        const encoded = JSON.stringify(body);
        bodyBytes = encoded.length;
        requestOptions.extraData = encoded;
      }
      const response = await request.request(`${base}${path}`, requestOptions);
      const text = typeof response.result === 'string' ? response.result : JSON.stringify(response.result);
      // `tls` is cumulative from request start: near zero means the connection
      // was reused, and anything large is a handshake this call paid for alone.
      const timing = response.performanceTiming;
      RemoteLogger.info(`relay ${method} ${path} status=${response.responseCode} elapsed_ms=${Date.now() - startedAt}` +
        ` dns=${Math.round(timing.dnsTiming)} tcp=${Math.round(timing.tcpTiming)}` +
        ` tls=${Math.round(timing.tlsTiming)} ttfb=${Math.round(timing.firstReceiveTiming)}`);
      if (response.responseCode < 200 || response.responseCode >= 300) {
        // The size belongs in the log: a body the relay refuses for being too
        // big is otherwise indistinguishable from one it refuses for auth.
        RemoteLogger.error(`relay request failed ${method} ${path} status=${response.responseCode} bytes=${bodyBytes}`);
        let message = response.responseCode === 413 ? RemoteI18n.t('errors.payloadTooLarge') :
          `Relay request failed (HTTP ${response.responseCode}).`;
        try {
          const error = JSON.parse(text) as RelayErrorResponse;
          if (error.error) message = error.error;
        } catch (_err) {
          // Keep the status-based error when the relay did not return JSON.
        }
        throw new CloudAccountRequestError(message, response.responseCode);
      }
      return JSON.parse(text) as T;
    } catch (err) {
      throw OhosError.wrap('relay request', err);
    } finally {
      request.destroy();
    }
  }
}
