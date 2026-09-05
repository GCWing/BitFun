import { preferences } from '@kit.ArkData';
import { Encoding } from './Encoding';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { OhosError } from './OhosError';
import { RemoteDescriptorParser } from './RemoteDescriptorParser';
import { RemoteLogger } from './RemoteLogger';
import { RemoteModelPreferenceStore } from './RemoteModelController';

// Persisted identifiers are compatibility contracts; the product rename must
// not strand an installed phone's pairing snapshot in a second store.
const STORE_NAME = HarmonyUpgradeIdentityContract.REMOTE_IDENTITY_STORE;
const INSTALL_ID_KEY = 'mobile_install_id';
const USER_ID_KEY = 'user_id';
const REMOTE_URL_KEY = 'remote_url';
const PAIRING_SAVED_AT_KEY = 'pairing_saved_at';
const CONTROL_TARGET_TYPE_KEY = 'control_target_type';
const LAST_MODEL_ID_KEY = 'last_model_id';
const USER_ID_LOCK_UNTIL_KEY = 'user_id_lock_until';
const USER_ID_FAILURE_COUNT_KEY = 'user_id_failure_count';

export interface MobileIdentitySnapshot {
  installId: string;
  userId: string;
  remoteUrl: string;
  pairingSavedAt: number;
  controlTargetType: string;
  userIdFailureCount: number;
  userIdLockUntil: number;
}

export class MobileIdentityStore implements RemoteModelPreferenceStore {
  private preferences?: preferences.Preferences;

  async init(context: Context): Promise<MobileIdentitySnapshot> {
    try {
      this.preferences = await preferences.getPreferences(context, STORE_NAME);
    } catch (err) {
      throw OhosError.wrap('mobile identity init', err);
    }
    const existingInstallId = await this.getString(INSTALL_ID_KEY);
    const installId = existingInstallId || Encoding.randomId('harmony');
    if (!existingInstallId) {
      await this.putString(INSTALL_ID_KEY, installId);
    }
    const snapshot: MobileIdentitySnapshot = {
      installId,
      userId: await this.getString(USER_ID_KEY),
      remoteUrl: await this.getString(REMOTE_URL_KEY),
      pairingSavedAt: await this.getNumber(PAIRING_SAVED_AT_KEY),
      controlTargetType: await this.getString(CONTROL_TARGET_TYPE_KEY),
      userIdFailureCount: await this.getNumber(USER_ID_FAILURE_COUNT_KEY),
      userIdLockUntil: await this.getNumber(USER_ID_LOCK_UNTIL_KEY)
    };
    if (snapshot.remoteUrl.length > 0) {
      RemoteLogger.info(
        `pairing snapshot restored room=${this.roomReference(snapshot.remoteUrl)} saved_at=${snapshot.pairingSavedAt}`
      );
    }
    return snapshot;
  }

  async savePairingInput(userId: string, remoteUrl: string): Promise<void> {
    if (!this.preferences) {
      throw new Error('Mobile identity store is not initialized.');
    }
    const normalizedUrl = remoteUrl.trim();
    const savedAt = Date.now();
    try {
      await this.preferences.put(USER_ID_KEY, userId.trim());
      await this.preferences.put(REMOTE_URL_KEY, normalizedUrl);
      await this.preferences.put(PAIRING_SAVED_AT_KEY, savedAt);
      await this.preferences.flush();
    } catch (err) {
      throw OhosError.wrap('mobile identity save pairing snapshot', err);
    }
    RemoteLogger.info(`pairing snapshot saved room=${this.roomReference(normalizedUrl)} saved_at=${savedAt}`);
  }

  async clearPairingInput(): Promise<void> {
    await this.putString(USER_ID_KEY, '');
    await this.putString(REMOTE_URL_KEY, '');
    await this.putNumber(PAIRING_SAVED_AT_KEY, 0);
    await this.clearUserIdProtection();
  }

  async saveControlTargetType(controlTargetType: string): Promise<void> {
    const normalized = controlTargetType === 'room' || controlTargetType === 'account_device' ?
      controlTargetType : '';
    await this.putString(CONTROL_TARGET_TYPE_KEY, normalized);
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
    try {
      const value = await this.preferences.get(key, '');
      return typeof value === 'string' ? value : '';
    } catch (err) {
      throw OhosError.wrap('mobile identity get string', err);
    }
  }

  private async getNumber(key: string): Promise<number> {
    if (!this.preferences) {
      return 0;
    }
    try {
      const value = await this.preferences.get(key, 0);
      return typeof value === 'number' ? value : Number(value || 0);
    } catch (err) {
      throw OhosError.wrap('mobile identity get number', err);
    }
  }

  private async putString(key: string, value: string): Promise<void> {
    if (!this.preferences) {
      return;
    }
    try {
      await this.preferences.put(key, value);
      await this.preferences.flush();
    } catch (err) {
      throw OhosError.wrap('mobile identity put string', err);
    }
  }

  private async putNumber(key: string, value: number): Promise<void> {
    if (!this.preferences) {
      return;
    }
    try {
      await this.preferences.put(key, value);
      await this.preferences.flush();
    } catch (err) {
      throw OhosError.wrap('mobile identity put number', err);
    }
  }

  private roomReference(remoteUrl: string): string {
    try {
      return RemoteDescriptorParser.parse(remoteUrl).roomId.slice(0, 8);
    } catch (_err) {
      return 'invalid';
    }
  }
}
