import { preferences } from '@kit.ArkData';
import { Encoding } from './Encoding';

const STORE_NAME = 'bitfun_remote_identity';
const INSTALL_ID_KEY = 'mobile_install_id';
const USER_ID_KEY = 'user_id';
const REMOTE_URL_KEY = 'remote_url';
const LAST_MODEL_ID_KEY = 'last_model_id';
const USER_ID_LOCK_UNTIL_KEY = 'user_id_lock_until';
const USER_ID_FAILURE_COUNT_KEY = 'user_id_failure_count';

export interface MobileIdentitySnapshot {
  installId: string;
  userId: string;
  remoteUrl: string;
  userIdFailureCount: number;
  userIdLockUntil: number;
}

export class MobileIdentityStore {
  private preferences?: preferences.Preferences;

  async init(context: Context): Promise<MobileIdentitySnapshot> {
    this.preferences = await preferences.getPreferences(context, STORE_NAME);
    const existingInstallId = await this.getString(INSTALL_ID_KEY);
    const installId = existingInstallId || Encoding.randomId('harmony');
    if (!existingInstallId) {
      await this.putString(INSTALL_ID_KEY, installId);
    }
    return {
      installId,
      userId: await this.getString(USER_ID_KEY),
      remoteUrl: await this.getString(REMOTE_URL_KEY),
      userIdFailureCount: await this.getNumber(USER_ID_FAILURE_COUNT_KEY),
      userIdLockUntil: await this.getNumber(USER_ID_LOCK_UNTIL_KEY)
    };
  }

  async savePairingInput(userId: string, remoteUrl: string): Promise<void> {
    await this.putString(USER_ID_KEY, userId.trim());
    await this.putString(REMOTE_URL_KEY, remoteUrl.trim());
  }

  async clearPairingInput(): Promise<void> {
    await this.putString(USER_ID_KEY, '');
    await this.putString(REMOTE_URL_KEY, '');
    await this.clearUserIdProtection();
  }

  async saveUserIdProtection(failureCount: number, lockUntil: number): Promise<void> {
    await this.putNumber(USER_ID_FAILURE_COUNT_KEY, failureCount);
    await this.putNumber(USER_ID_LOCK_UNTIL_KEY, lockUntil);
  }

  async clearUserIdProtection(): Promise<void> {
    await this.putNumber(USER_ID_FAILURE_COUNT_KEY, 0);
    await this.putNumber(USER_ID_LOCK_UNTIL_KEY, 0);
  }

  async getLastModelId(): Promise<string> {
    return this.getString(LAST_MODEL_ID_KEY);
  }

  async saveLastModelId(modelId: string): Promise<void> {
    await this.putString(LAST_MODEL_ID_KEY, modelId.trim());
  }

  private async getString(key: string): Promise<string> {
    if (!this.preferences) {
      return '';
    }
    const value = await this.preferences.get(key, '');
    return typeof value === 'string' ? value : '';
  }

  private async getNumber(key: string): Promise<number> {
    if (!this.preferences) {
      return 0;
    }
    const value = await this.preferences.get(key, 0);
    return typeof value === 'number' ? value : Number(value || 0);
  }

  private async putString(key: string, value: string): Promise<void> {
    if (!this.preferences) {
      return;
    }
    await this.preferences.put(key, value);
    await this.preferences.flush();
  }

  private async putNumber(key: string, value: number): Promise<void> {
    if (!this.preferences) {
      return;
    }
    await this.preferences.put(key, value);
    await this.preferences.flush();
  }
}
