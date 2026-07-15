import { http } from '@kit.NetworkKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { Encoding } from './Encoding';
import { RemoteCrypto } from './RemoteCrypto';
import { ChallengeCommand, CommandStatusResponse, EncryptedPayload, InitialSyncResponse, PairChallengeResponse, PairRequest, RemoteCommand, RemoteDescriptor } from '../model/RemoteModels';

export class RelayHttpClient {
  private descriptor?: RemoteDescriptor;
  private crypto?: RemoteCrypto;
  private readonly deviceName: string = 'HarmonyOS Phone';

  bind(descriptor: RemoteDescriptor, crypto: RemoteCrypto): void {
    this.descriptor = descriptor;
    this.crypto = crypto;
  }

  reset(): void {
    this.descriptor = undefined;
    this.crypto = undefined;
  }

  async pair(deviceId: string, userId: string): Promise<InitialSyncResponse> {
    const descriptor = this.requireDescriptor();
    const crypto = this.requireCrypto();
    crypto.deriveSharedKey(descriptor.publicKey);

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
    const commandPayload = await crypto.encryptJson(challengeCommand);
    const encryptedInitialSync = await this.postJson<EncryptedPayload>(`/api/rooms/${descriptor.roomId}/command`, commandPayload);
    const initialSync = await crypto.decryptJson<InitialSyncResponse>(encryptedInitialSync);
    if (initialSync.resp === 'error') {
      throw new Error(initialSync.message || 'Desktop rejected the pairing request.');
    }
    return initialSync;
  }

  async sendCommand<T extends CommandStatusResponse>(command: RemoteCommand, readTimeoutMs: number = 30000): Promise<T> {
    const descriptor = this.requireDescriptor();
    const crypto = this.requireCrypto();
    const encrypted = await crypto.encryptJson(command);
    const encryptedResponse = await this.postJson<EncryptedPayload>(`/api/rooms/${descriptor.roomId}/command`, encrypted, readTimeoutMs);
    const response = await crypto.decryptJson<T>(encryptedResponse);
    if (response.resp === 'error') {
      throw new Error(response.message || 'Remote command failed.');
    }
    return response;
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
}
