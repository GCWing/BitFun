import { http } from '@kit.NetworkKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { RemoteCommandFactory } from './RemoteCommandFactory';
import { RemoteCrypto } from './RemoteCrypto';
import { RemoteLogger } from './RemoteLogger';
import { ChallengeCommand, CommandStatusResponse, DelegatedIdentityResponse, EncryptedPayload, InitialSyncResponse, PairChallengeResponse, PairRequest, PeerDeviceProvisionedResponse, RemoteCommand, RemoteDescriptor } from '../model/RemoteModels';

export interface PairIdentity {
  userId: string;
  password?: string;
}

/**
 * Result of a `provision_peer_device` round trip, reported rather than thrown
 * because the caller has to tell two failures apart and an `Error` cannot
 * carry that distinction back through the transport's error collapsing.
 *
 * `desktopReported` means the desktop answered and refused, and its message is
 * worth showing verbatim. Everything else — no answer at all — covers both a
 * desktop that is offline and a desktop too old to know the command: an older
 * build decrypts the payload, fails to deserialize the unknown variant, and
 * replies with nothing, which on this side is indistinguishable from silence.
 */
export interface PeerDeviceProvisionOutcome {
  ok: boolean;
  token: string;
  userId: string;
  masterKeyBase64: string;
  deviceId: string;
  failure: string;
  desktopReported: boolean;
}

export class RelayHttpClient {
  private descriptor?: RemoteDescriptor;
  private crypto?: RemoteCrypto;
  private readonly deviceName: string = 'HarmonyOS Phone';
  delegatedToken: string = '';
  delegatedMasterKey: string = '';
  delegatedUserId: string = '';
  pairedDeviceId: string = '';
  homeDeviceId: string = '';

  bind(descriptor: RemoteDescriptor, crypto: RemoteCrypto): void {
    this.descriptor = descriptor;
    this.crypto = crypto;
  }

  reset(): void {
    this.descriptor = undefined;
    this.crypto = undefined;
    this.clearDelegatedIdentity();
    this.pairedDeviceId = '';
    this.homeDeviceId = '';
  }

  async pair(deviceId: string, identity: PairIdentity): Promise<InitialSyncResponse> {
    const descriptor = this.requireDescriptor();
    const crypto = this.requireCrypto();
    const startedAt = Date.now();
    RemoteLogger.info(`pair start room=${RelayHttpClient.shortRoomId(descriptor.roomId)}`);
    crypto.deriveSharedKey(descriptor.publicKey);
    const userId = identity.userId.trim();

    const pairRequest: PairRequest = {
      public_key: crypto.getPublicKeyBase64(),
      device_id: deviceId,
      device_name: this.deviceName
    };
    const pairResponse = await this.postJson<EncryptedPayload>(`/api/rooms/${descriptor.roomId}/pair`, pairRequest);
    RemoteLogger.info(`pair challenge received ms=${Date.now() - startedAt}`);
    const challenge = await crypto.decryptJson<PairChallengeResponse>(pairResponse);
    const challengeCommand: ChallengeCommand = {
      challenge_echo: challenge.challenge,
      device_id: deviceId,
      device_name: this.deviceName,
      mobile_install_id: deviceId,
      user_id: userId
    };
    if (identity.password && identity.password.length > 0) {
      challengeCommand.password = identity.password;
    }
    const commandPayload = await crypto.encryptJson(challengeCommand);
    const encryptedInitialSync = await this.postJson<EncryptedPayload>(`/api/rooms/${descriptor.roomId}/command`, commandPayload);
    const initialSync = await crypto.decryptJson<InitialSyncResponse>(encryptedInitialSync);
    if (initialSync.resp === 'error') {
      throw new Error(initialSync.message || 'Desktop rejected the pairing request.');
    }
    RemoteLogger.info(`pair complete ms=${Date.now() - startedAt}`);
    return initialSync;
  }

  async requestDelegatedIdentity(force: boolean = false): Promise<boolean> {
    if (!force && this.hasDelegatedIdentity()) {
      return true;
    }
    if (force) {
      this.clearDelegatedIdentity();
    }
    try {
      const response = await this.sendCommand<DelegatedIdentityResponse>({ cmd: 'get_delegated_identity' }, 30000);
      if (response.resp === 'delegate_identity' && response.token && response.master_key) {
        this.delegatedToken = response.token;
        this.delegatedMasterKey = response.master_key;
        this.delegatedUserId = response.user_id || '';
        if (response.device_id) {
          this.homeDeviceId = response.device_id;
          if (!this.pairedDeviceId) {
            this.pairedDeviceId = response.device_id;
          }
        }
        return true;
      }
    } catch (_err) {
      return false;
    }
    return false;
  }

  /**
   * Mint a full account device credential for a device that cannot type a
   * password. Pinned to the room channel on purpose: only the desktop's room
   * loop holds the trusted pairing identity that authorizes provisioning, so
   * the account-device transport would answer "not available on this host".
   */
  async provisionPeerDevice(
    deviceId: string,
    deviceName: string,
    requestId: string,
    readTimeoutMs: number = 45000
  ): Promise<PeerDeviceProvisionOutcome> {
    const command: RemoteCommand = RemoteCommandFactory.provisionPeerDevice(deviceId, deviceName, requestId);
    command._request_id = `req_provision_${requestId}`;
    let response: PeerDeviceProvisionedResponse;
    try {
      response = await this.sendCommand<PeerDeviceProvisionedResponse>(command, readTimeoutMs, false);
    } catch (err) {
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      RemoteLogger.error(`provision peer device transport failure: ${message}`);
      return RelayHttpClient.provisionFailure(message, false);
    }
    if (response.resp === 'error') {
      return RelayHttpClient.provisionFailure(response.message || '', true);
    }
    const token = response.token || '';
    const userId = response.user_id || '';
    const masterKey = response.master_key || '';
    const provisionedDeviceId = response.device_id || '';
    if (response.resp !== 'peer_device_provisioned' || token.length === 0 || userId.length === 0 ||
      masterKey.length === 0) {
      // A well-formed reply that is missing the credential is a desktop we do
      // not understand, not a desktop that refused. Treat it as silence.
      RemoteLogger.error(`provision peer device unexpected response resp=${response.resp || 'unknown'}`);
      return RelayHttpClient.provisionFailure('', false);
    }
    if (provisionedDeviceId !== deviceId) {
      // The desktop already checks this, so reaching here means the reply did
      // not come from the desktop we asked. Refuse rather than hand the watch
      // a credential minted for some other device.
      RemoteLogger.error('provision peer device returned a mismatched device id');
      return RelayHttpClient.provisionFailure('', false);
    }
    return {
      ok: true,
      token,
      userId,
      masterKeyBase64: masterKey,
      deviceId: provisionedDeviceId,
      failure: '',
      desktopReported: false
    };
  }

  clearDelegatedIdentity(): void {
    this.delegatedToken = '';
    this.delegatedMasterKey = '';
    this.delegatedUserId = '';
  }

  hasDelegatedIdentity(): boolean {
    return this.delegatedToken.length > 0 && this.delegatedMasterKey.length > 0;
  }

  async sendCommand<T extends CommandStatusResponse>(
    command: RemoteCommand,
    readTimeoutMs: number = 30000,
    throwOnRemoteError: boolean = true
  ): Promise<T> {
    const descriptor = this.requireDescriptor();
    const crypto = this.requireCrypto();
    const commandName = command.cmd || 'unknown';
    const requestId = RelayHttpClient.shortRequestId(command._request_id || '');
    const startedAt = Date.now();
    let stage = 'encrypt';
    try {
      RemoteLogger.info(`command encrypt start cmd=${commandName} request=${requestId}`);
      const encrypted = await crypto.encryptJson(command);
      RemoteLogger.info(`command encrypt done cmd=${commandName} request=${requestId} ms=${Date.now() - startedAt}`);

      stage = 'http';
      const httpStartedAt = Date.now();
      RemoteLogger.info(`command http start cmd=${commandName} request=${requestId} timeout=${readTimeoutMs}`);
      const encryptedResponse = await this.postJson<EncryptedPayload>(
        `/api/rooms/${descriptor.roomId}/command`,
        encrypted,
        readTimeoutMs
      );
      RemoteLogger.info(`command http done cmd=${commandName} request=${requestId} ms=${Date.now() - httpStartedAt}`);

      stage = 'decrypt';
      const decryptStartedAt = Date.now();
      RemoteLogger.info(`command decrypt start cmd=${commandName} request=${requestId}`);
      const response = await crypto.decryptJson<T>(encryptedResponse);
      RemoteLogger.info(`command decrypt done cmd=${commandName} request=${requestId} resp=${response.resp || 'unknown'} ms=${Date.now() - decryptStartedAt}`);
      if (response.resp === 'error') {
        RemoteLogger.error(`command remote error cmd=${commandName} request=${requestId} message=${response.message || 'unknown'}`);
        if (throwOnRemoteError) {
          throw new Error(response.message || 'Remote command failed.');
        }
        return response;
      }
      RemoteLogger.info(`command complete cmd=${commandName} request=${requestId} ms=${Date.now() - startedAt}`);
      return response;
    } catch (err) {
      RemoteLogger.error(`command failed cmd=${commandName} request=${requestId} stage=${stage} ms=${Date.now() - startedAt}`);
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      throw new Error(message);
    }
  }

  private async postJson<T>(path: string, payload: Object, readTimeoutMs: number = 30000): Promise<T> {
    const descriptor = this.requireDescriptor();
    const request = http.createHttp();
    try {
      const response = await request.request(`${descriptor.relayUrl}${path}`, {
        method: http.RequestMethod.POST,
        header: {
          'Content-Type': 'application/json',
          'Accept': 'application/json'
        },
        extraData: JSON.stringify(payload),
        expectDataType: http.HttpDataType.STRING,
        connectTimeout: 15000,
        readTimeout: readTimeoutMs
      });
      if (response.responseCode < 200 || response.responseCode >= 300) {
        throw new Error(this.httpErrorMessage(response.responseCode));
      }
      const text = typeof response.result === 'string' ? response.result : JSON.stringify(response.result);
      try {
        return JSON.parse(text) as T;
      } catch (_err) {
        throw new Error(RemoteI18n.t('errors.remoteDataInvalid'));
      }
    } catch (err) {
      throw new Error(this.networkErrorMessage(err));
    } finally {
      request.destroy();
    }
  }

  private httpErrorMessage(code: number): string {
    if (code === 401 || code === 403) {
      return RemoteI18n.t('errors.pairRejected');
    }
    if (code === 404) {
      return RemoteI18n.t('errors.roomNotFound');
    }
    if (code === 408 || code === 504) {
      return RemoteI18n.t('errors.relayTimeout');
    }
    if (code === 429) {
      return RemoteI18n.t('errors.rateLimited');
    }
    if (code >= 500) {
      return RemoteI18n.f('errors.relayUnavailable', `${code}`);
    }
    return RemoteI18n.f('errors.relayHttp', `${code}`);
  }

  private networkErrorMessage(err: Object): string {
    const raw = err instanceof Error ? err.message : JSON.stringify(err);
    if (raw === RemoteI18n.t('errors.pairRejected') ||
      raw === RemoteI18n.t('errors.roomNotFound') ||
      raw === RemoteI18n.t('errors.relayTimeout') ||
      raw === RemoteI18n.t('errors.rateLimited') ||
      raw === RemoteI18n.t('errors.remoteDataInvalid') ||
      raw.indexOf('HTTP ') >= 0) {
      return raw;
    }
    const text = raw.toLowerCase();
    if (text.indexOf('timeout') >= 0 || text.indexOf('timed out') >= 0) {
      return RemoteI18n.t('errors.relayTimeoutDetail');
    }
    if (text.indexOf('refused') >= 0 || text.indexOf('unreachable') >= 0 || text.indexOf('failed') >= 0 ||
      text.indexOf('network') >= 0 || text.indexOf('dns') >= 0 || text.indexOf('resolve') >= 0) {
      return RemoteI18n.t('errors.relayNetwork');
    }
    return raw || RemoteI18n.t('errors.remoteConnectFailed');
  }

  private requireDescriptor(): RemoteDescriptor {
    if (!this.descriptor) {
      throw new Error('Remote descriptor is not configured.');
    }
    return this.descriptor;
  }

  private requireCrypto(): RemoteCrypto {
    if (!this.crypto) {
      throw new Error('Remote crypto is not configured.');
    }
    return this.crypto;
  }

  private static provisionFailure(message: string, desktopReported: boolean): PeerDeviceProvisionOutcome {
    return {
      ok: false,
      token: '',
      userId: '',
      masterKeyBase64: '',
      deviceId: '',
      failure: message,
      desktopReported
    };
  }

  private static shortRequestId(requestId: string): string {
    if (requestId.length === 0) {
      return 'none';
    }
    return requestId.length <= 12 ? requestId : requestId.slice(requestId.length - 12);
  }

  private static shortRoomId(roomId: string): string {
    return roomId.length <= 8 ? roomId : roomId.slice(0, 8);
  }
}
