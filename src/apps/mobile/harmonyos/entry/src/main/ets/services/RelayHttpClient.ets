import { http } from '@kit.NetworkKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { RemoteCrypto } from './RemoteCrypto';
import { RemoteLogger } from './RemoteLogger';
import { ChallengeCommand, CommandStatusResponse, DelegatedIdentityResponse, EncryptedPayload, InitialSyncResponse, PairChallengeResponse, PairRequest, RemoteCommand, RemoteDescriptor } from '../model/RemoteModels';

export interface PairIdentity {
  userId: string;
  password?: string;
}

export class RelayHttpClient {
  private descriptor?: RemoteDescriptor;
  private crypto?: RemoteCrypto;
  private readonly deviceName: string = 'HarmonyOS Phone';
  delegatedToken: string = '';
  delegatedMasterKey: string = '';
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
    crypto.deriveSharedKey(descriptor.publicKey);
    const userId = identity.userId.trim();

    const pairRequest: PairRequest = {
      public_key: crypto.getPublicKeyBase64(),
      device_id: deviceId,
      device_name: this.deviceName
    };
    const pairResponse = await this.postJson<EncryptedPayload>(`/api/rooms/${descriptor.roomId}/pair`, pairRequest);
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

  clearDelegatedIdentity(): void {
    this.delegatedToken = '';
    this.delegatedMasterKey = '';
  }

  hasDelegatedIdentity(): boolean {
    return this.delegatedToken.length > 0 && this.delegatedMasterKey.length > 0;
  }

  async sendCommand<T extends CommandStatusResponse>(command: RemoteCommand, readTimeoutMs: number = 30000): Promise<T> {
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
        throw new Error(response.message || 'Remote command failed.');
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

  private static shortRequestId(requestId: string): string {
    if (requestId.length === 0) {
      return 'none';
    }
    return requestId.length <= 12 ? requestId : requestId.slice(requestId.length - 12);
  }
}
