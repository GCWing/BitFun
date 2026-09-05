import { huks } from '@kit.UniversalKeystoreKit';
import { preferences } from '@kit.ArkData';
import { Encoding } from './Encoding';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { OhosError } from './OhosError';

const STORE_NAME: string = HarmonyUpgradeIdentityContract.CLOUD_ACCOUNT_STORE;
const CIPHER_KEY: string = 'session_cipher';
const IV_KEY: string = 'session_iv';
const HUKS_ALIAS: string = HarmonyUpgradeIdentityContract.CLOUD_ACCOUNT_HUKS_ALIAS;

export interface PersistedCloudAccountSession {
  relayUrl: string;
  username: string;
  token: string;
  userId: string;
  masterKey: string;
  targetDeviceId?: string;
  targetDeviceName?: string;
}

/** Persists only an HUKS-encrypted account session, never plaintext secrets. */
export class CloudAccountSessionStore {
  private store?: preferences.Preferences;

  async init(context: Context): Promise<void> {
    try {
      this.store = await preferences.getPreferences(context, STORE_NAME);
    } catch (err) {
      throw OhosError.wrap('cloud account session init', err);
    }
  }

  async save(session: PersistedCloudAccountSession): Promise<void> {
    try {
      const store = this.requireStore();
      await this.ensureKey();
      const iv = Encoding.randomBytes(16);
      const options = this.options(huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_ENCRYPT, iv);
      const handle = await huks.initSession(HUKS_ALIAS, options);
      const encrypted = await huks.finishSession(handle.handle, {
        properties: options.properties,
        inData: Encoding.utf8ToBytes(JSON.stringify(session))
      });
      if (!encrypted.outData || encrypted.outData.length === 0) {
        throw new Error('Cloud account session encryption failed.');
      }
      await store.put(CIPHER_KEY, Encoding.bytesToBase64(encrypted.outData));
      await store.put(IV_KEY, Encoding.bytesToBase64(iv));
      await store.flush();
    } catch (err) {
      throw OhosError.wrap('cloud account session save', err);
    }
  }

  async load(): Promise<PersistedCloudAccountSession | undefined> {
    try {
      const store = this.requireStore();
      const cipher = await store.get(CIPHER_KEY, '');
      const ivText = await store.get(IV_KEY, '');
      if (typeof cipher !== 'string' || typeof ivText !== 'string' || cipher.length === 0 || ivText.length === 0) {
        return undefined;
      }
      const iv = Encoding.base64ToBytes(ivText);
      const options = this.options(huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_DECRYPT, iv);
      const handle = await huks.initSession(HUKS_ALIAS, options);
      const decrypted = await huks.finishSession(handle.handle, {
        properties: options.properties,
        inData: Encoding.base64ToBytes(cipher)
      });
      if (!decrypted.outData) {
        return undefined;
      }
      return JSON.parse(Encoding.bytesToUtf8(decrypted.outData)) as PersistedCloudAccountSession;
    } catch (err) {
      throw OhosError.wrap('cloud account session load', err);
    }
  }

  async clear(): Promise<void> {
    try {
      const store = this.requireStore();
      await store.delete(CIPHER_KEY);
      await store.delete(IV_KEY);
      await store.flush();
    } catch (err) {
      throw OhosError.wrap('cloud account session clear', err);
    }
  }

  private async ensureKey(): Promise<void> {
    if (await this.keyItemExists()) {
      return;
    }
    try {
      await huks.generateKeyItem(HUKS_ALIAS, {
        properties: [
          { tag: huks.HuksTag.HUKS_TAG_ALGORITHM, value: huks.HuksKeyAlg.HUKS_ALG_AES },
          { tag: huks.HuksTag.HUKS_TAG_KEY_SIZE, value: huks.HuksKeySize.HUKS_AES_KEY_SIZE_256 },
          { tag: huks.HuksTag.HUKS_TAG_PURPOSE, value: huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_ENCRYPT | huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_DECRYPT },
          { tag: huks.HuksTag.HUKS_TAG_BLOCK_MODE, value: huks.HuksCipherMode.HUKS_MODE_CBC },
          { tag: huks.HuksTag.HUKS_TAG_PADDING, value: huks.HuksKeyPadding.HUKS_PADDING_PKCS7 }
        ]
      });
    } catch (err) {
      throw OhosError.wrap('cloud account session generate key', err);
    }
  }

  private async keyItemExists(): Promise<boolean> {
    try {
      return await huks.isKeyItemExist(HUKS_ALIAS, {});
    } catch (err) {
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      if (message.indexOf('12000011') >= 0 || message.indexOf('-13') >= 0 ||
        message.toLowerCase().indexOf('does not exist') >= 0) {
        return false;
      }
      throw new Error(message || 'HUKS key lookup failed.');
    }
  }

  private options(purpose: huks.HuksKeyPurpose, iv: Uint8Array): huks.HuksOptions {
    return {
      properties: [
        { tag: huks.HuksTag.HUKS_TAG_ALGORITHM, value: huks.HuksKeyAlg.HUKS_ALG_AES },
        { tag: huks.HuksTag.HUKS_TAG_PURPOSE, value: purpose },
        { tag: huks.HuksTag.HUKS_TAG_BLOCK_MODE, value: huks.HuksCipherMode.HUKS_MODE_CBC },
        { tag: huks.HuksTag.HUKS_TAG_PADDING, value: huks.HuksKeyPadding.HUKS_PADDING_PKCS7 },
        { tag: huks.HuksTag.HUKS_TAG_IV, value: iv }
      ]
    };
  }

  private requireStore(): preferences.Preferences {
    if (!this.store) {
      throw new Error('Cloud account session store is not initialized.');
    }
    return this.store;
  }
}
