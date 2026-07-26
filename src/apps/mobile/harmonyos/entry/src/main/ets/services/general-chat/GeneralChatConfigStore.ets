import { preferences } from '@kit.ArkData';
import { huks } from '@kit.UniversalKeystoreKit';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { Encoding } from '../Encoding';
import { GeneralChatTokenProvider } from './GeneralChatHttpTransport';

const STORE_NAME = 'bitfun_general_chat_config';
const API_URL_KEY = 'api_url';
const MODEL_NAME_KEY = 'model_name';
const API_KEY_CIPHER_KEY = 'api_key_cipher';
const API_KEY_IV_KEY = 'api_key_iv';
const HUKS_ALIAS = 'bitfun_general_chat_api_key';

export interface GeneralChatConfigSnapshot {
  apiUrl: string;
  modelName: string;
  hasApiKey: boolean;
}

export interface GeneralChatConfigUpdate {
  apiUrl: string;
  modelName: string;
  apiKey: string;
  clearApiKey: boolean;
}

export class GeneralChatConfigValidator {
  static validate(update: GeneralChatConfigUpdate, hasExistingApiKey: boolean): string {
    const apiUrl = update.apiUrl.trim();
    const modelName = update.modelName.trim();
    if (!GeneralChatConfigValidator.isHttpUrl(apiUrl)) {
      return RemoteI18n.t('settings.modelService.invalidUrl');
    }
    if (modelName.length === 0) {
      return RemoteI18n.t('settings.modelService.modelRequired');
    }
    if (!update.clearApiKey && update.apiKey.trim().length === 0 && !hasExistingApiKey) {
      return RemoteI18n.t('settings.modelService.apiKeyRequired');
    }
    return '';
  }

  private static isHttpUrl(value: string): boolean {
    const lower = value.toLowerCase();
    if (!(lower.startsWith('https://') || lower.startsWith('http://'))) {
      return false;
    }
    const schemeLength = lower.startsWith('https://') ? 8 : 7;
    const hostAndPath = value.slice(schemeLength);
    return hostAndPath.length > 0 && hostAndPath.indexOf(' ') < 0 && hostAndPath.indexOf('#') < 0;
  }
}

export class GeneralChatConfigStore implements GeneralChatTokenProvider {
  private store?: preferences.Preferences;

  async init(context: Context): Promise<GeneralChatConfigSnapshot> {
    this.store = await preferences.getPreferences(context, STORE_NAME);
    return this.snapshot();
  }

  async snapshot(): Promise<GeneralChatConfigSnapshot> {
    const store = this.requireStore();
    const cipher = await store.get(API_KEY_CIPHER_KEY, '');
    const iv = await store.get(API_KEY_IV_KEY, '');
    const hasStoredSecret = typeof cipher === 'string' && cipher.length > 0 &&
      typeof iv === 'string' && iv.length > 0;
    const hasKeyItem = hasStoredSecret ? await this.keyItemExists() : false;
    return {
      apiUrl: await this.getString(API_URL_KEY),
      modelName: await this.getString(MODEL_NAME_KEY),
      hasApiKey: hasStoredSecret && hasKeyItem
    };
  }

  async save(update: GeneralChatConfigUpdate): Promise<GeneralChatConfigSnapshot> {
    const current = await this.snapshot();
    const validationError = GeneralChatConfigValidator.validate(update, current.hasApiKey);
    if (validationError.length > 0) {
      throw new Error(validationError);
    }
    const store = this.requireStore();
    await store.put(API_URL_KEY, GeneralChatConfigStore.normalizeBaseUrl(update.apiUrl));
    await store.put(MODEL_NAME_KEY, update.modelName.trim());
    if (update.clearApiKey) {
      await store.delete(API_KEY_CIPHER_KEY);
      await store.delete(API_KEY_IV_KEY);
    } else if (update.apiKey.trim().length > 0) {
      try {
        const encrypted = await this.encryptApiKey(update.apiKey.trim());
        await store.put(API_KEY_CIPHER_KEY, encrypted.cipherText);
        await store.put(API_KEY_IV_KEY, encrypted.iv);
      } catch (_err) {
        throw new Error(RemoteI18n.t('settings.modelService.secureStoreFailed'));
      }
    }
    await store.flush();
    return this.snapshot();
  }

  async accessToken(): Promise<string> {
    const store = this.requireStore();
    const cipherText = await this.getString(API_KEY_CIPHER_KEY);
    const iv = await this.getString(API_KEY_IV_KEY);
    if (cipherText.length === 0 || iv.length === 0) {
      return '';
    }
    const exists = await this.keyItemExists();
    if (!exists) {
      return '';
    }
    return this.decryptApiKey(cipherText, iv);
  }

  private async encryptApiKey(apiKey: string): Promise<EncryptedApiKey> {
    await this.ensureHuksKey();
    const iv = Encoding.randomBytes(16);
    const options = this.operationOptions(huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_ENCRYPT, iv);
    const session = await huks.initSession(HUKS_ALIAS, options);
    const result = await huks.finishSession(session.handle, {
      properties: options.properties,
      inData: Encoding.utf8ToBytes(apiKey)
    });
    if (!result.outData || result.outData.length === 0) {
      throw new Error(RemoteI18n.t('settings.modelService.encryptFailed'));
    }
    return {
      cipherText: Encoding.bytesToBase64(result.outData),
      iv: Encoding.bytesToBase64(iv)
    };
  }

  private async decryptApiKey(cipherText: string, ivText: string): Promise<string> {
    const iv = Encoding.base64ToBytes(ivText);
    const options = this.operationOptions(huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_DECRYPT, iv);
    const session = await huks.initSession(HUKS_ALIAS, options);
    const result = await huks.finishSession(session.handle, {
      properties: options.properties,
      inData: Encoding.base64ToBytes(cipherText)
    });
    if (!result.outData) {
      return '';
    }
    return Encoding.bytesToUtf8(result.outData);
  }

  private async ensureHuksKey(): Promise<void> {
    if (await this.keyItemExists()) {
      return;
    }
    await huks.generateKeyItem(HUKS_ALIAS, {
      properties: [
        { tag: huks.HuksTag.HUKS_TAG_ALGORITHM, value: huks.HuksKeyAlg.HUKS_ALG_AES },
        { tag: huks.HuksTag.HUKS_TAG_KEY_SIZE, value: huks.HuksKeySize.HUKS_AES_KEY_SIZE_256 },
        {
          tag: huks.HuksTag.HUKS_TAG_PURPOSE,
          value: huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_ENCRYPT |
            huks.HuksKeyPurpose.HUKS_KEY_PURPOSE_DECRYPT
        },
        { tag: huks.HuksTag.HUKS_TAG_BLOCK_MODE, value: huks.HuksCipherMode.HUKS_MODE_CBC },
        { tag: huks.HuksTag.HUKS_TAG_PADDING, value: huks.HuksKeyPadding.HUKS_PADDING_PKCS7 }
      ]
    });
  }

  private async keyItemExists(): Promise<boolean> {
    try {
      return await huks.isKeyItemExist(HUKS_ALIAS, {});
    } catch (err) {
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      if (message.indexOf('12000011') >= 0 || message.toLowerCase().indexOf('does not exist') >= 0) {
        return false;
      }
      throw new Error(message || 'HUKS key lookup failed.');
    }
  }

  private operationOptions(purpose: huks.HuksKeyPurpose, iv: Uint8Array): huks.HuksOptions {
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

  private async getString(key: string): Promise<string> {
    const value = await this.requireStore().get(key, '');
    return typeof value === 'string' ? value : '';
  }

  private requireStore(): preferences.Preferences {
    if (!this.store) {
      throw new Error('General chat configuration store is not initialized.');
    }
    return this.store;
  }

  private static normalizeBaseUrl(value: string): string {
    const trimmed = value.trim();
    return trimmed.endsWith('/') ? trimmed.slice(0, trimmed.length - 1) : trimmed;
  }
}

interface EncryptedApiKey {
  cipherText: string;
  iv: string;
}
