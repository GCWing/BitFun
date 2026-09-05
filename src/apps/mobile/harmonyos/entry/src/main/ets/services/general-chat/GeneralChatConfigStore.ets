import { preferences } from '@kit.ArkData';
import { huks } from '@kit.UniversalKeystoreKit';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { Encoding } from '../Encoding';
import { HarmonyUpgradeIdentityContract } from '../HarmonyUpgradeIdentityContract';
import { OhosError } from '../OhosError';
import { GeneralChatTokenProvider } from './GeneralChatHttpTransport';

const STORE_NAME = HarmonyUpgradeIdentityContract.GENERAL_CHAT_CONFIG_STORE;
const API_URL_KEY = 'api_url';
const MODEL_NAME_KEY = 'model_name';
const API_KEY_CIPHER_KEY = 'api_key_cipher';
const API_KEY_IV_KEY = 'api_key_iv';
const HUKS_ALIAS = HarmonyUpgradeIdentityContract.GENERAL_CHAT_HUKS_ALIAS;
const SELECTED_MODEL_ID_KEY = 'selected_model_id';
export const GENERAL_CHAT_LOCAL_MODEL_ID = 'local-general-chat';

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

export interface GeneralChatRuntimeModelConfig {
  modelId: string;
  name: string;
  provider: string;
  apiUrl: string;
  modelName: string;
  apiKey: string;
}

export class GeneralChatModelSelectionPolicy {
  static shouldActivateSavedLocalModel(catalog: RemoteModelCatalog): boolean {
    const selectedModelId = catalog.session_model_id || catalog.default_models.primary || '';
    return !catalog.models.some((model: RemoteModelConfig): boolean => model.id === selectedModelId);
  }
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
  private accountModels: GeneralChatRuntimeModelConfig[] = [];
  private preferredModelId: string = GENERAL_CHAT_LOCAL_MODEL_ID;
  private catalogRevision: number = 1;

  async init(context: Context): Promise<GeneralChatConfigSnapshot> {
    try {
      this.store = await preferences.getPreferences(context, STORE_NAME);
      this.preferredModelId = await this.getString(SELECTED_MODEL_ID_KEY) || GENERAL_CHAT_LOCAL_MODEL_ID;
      return this.snapshot();
    } catch (err) {
      throw OhosError.wrap('general chat config init', err);
    }
  }

  async snapshot(): Promise<GeneralChatConfigSnapshot> {
    try {
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
    } catch (err) {
      throw OhosError.wrap('general chat config snapshot', err);
    }
  }

  async save(update: GeneralChatConfigUpdate): Promise<GeneralChatConfigSnapshot> {
    const current = await this.snapshot();
    const validationError = GeneralChatConfigValidator.validate(update, current.hasApiKey);
    if (validationError.length > 0) {
      throw new Error(validationError);
    }
    try {
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
    } catch (err) {
      throw OhosError.wrap('general chat config save', err);
    }
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

  replaceAccountModels(models: GeneralChatRuntimeModelConfig[]): void {
    this.accountModels = models.map((model: GeneralChatRuntimeModelConfig): GeneralChatRuntimeModelConfig => ({
      modelId: model.modelId,
      name: model.name,
      provider: model.provider,
      apiUrl: model.apiUrl,
      modelName: model.modelName,
      apiKey: model.apiKey
    }));
    this.catalogRevision += 1;
  }

  async modelCatalog(): Promise<RemoteModelCatalog> {
    const local = await this.snapshot();
    const models: RemoteModelConfig[] = [];
    if (GeneralChatConfigStore.isComplete(local)) {
      models.push({
        id: GENERAL_CHAT_LOCAL_MODEL_ID,
        name: local.modelName,
        provider: 'local',
        base_url: local.apiUrl,
        model_name: local.modelName,
        enabled: true,
        capabilities: ['text_chat']
      });
    }
    this.accountModels.forEach((model: GeneralChatRuntimeModelConfig) => {
      models.push({
        id: model.modelId,
        name: model.name || model.modelName,
        provider: model.provider || 'account',
        base_url: model.apiUrl,
        model_name: model.modelName,
        enabled: true,
        capabilities: ['text_chat']
      });
    });
    const selectedModelId = this.effectiveSelectedModelId(models);
    return {
      version: this.catalogRevision,
      models,
      default_models: { primary: selectedModelId || undefined },
      session_model_id: selectedModelId || undefined
    };
  }

  async selectModel(modelId: string): Promise<boolean> {
    const catalog = await this.modelCatalog();
    if (!catalog.models.some((model: RemoteModelConfig): boolean => model.id === modelId)) {
      return false;
    }
    this.preferredModelId = modelId;
    try {
      const store = this.requireStore();
      await store.put(SELECTED_MODEL_ID_KEY, modelId);
      await store.flush();
    } catch (err) {
      throw OhosError.wrap('general chat config select model', err);
    }
    this.catalogRevision += 1;
    return true;
  }

  async selectLocalModel(): Promise<void> {
    this.preferredModelId = GENERAL_CHAT_LOCAL_MODEL_ID;
    try {
      const store = this.requireStore();
      await store.put(SELECTED_MODEL_ID_KEY, GENERAL_CHAT_LOCAL_MODEL_ID);
      await store.flush();
    } catch (err) {
      throw OhosError.wrap('general chat config select local model', err);
    }
    this.catalogRevision += 1;
  }

  async activeSnapshot(): Promise<GeneralChatConfigSnapshot> {
    const selectedModelId = this.effectiveSelectedModelId((await this.modelCatalog()).models);
    const accountModel = this.accountModels.find((model: GeneralChatRuntimeModelConfig): boolean => {
      return model.modelId === selectedModelId;
    });
    if (accountModel) {
      return { apiUrl: accountModel.apiUrl, modelName: accountModel.modelName, hasApiKey: true };
    }
    return this.snapshot();
  }

  async activeAccessToken(): Promise<string> {
    const selectedModelId = this.effectiveSelectedModelId((await this.modelCatalog()).models);
    const accountModel = this.accountModels.find((model: GeneralChatRuntimeModelConfig): boolean => {
      return model.modelId === selectedModelId;
    });
    return accountModel ? accountModel.apiKey : this.accessToken();
  }

  private async encryptApiKey(apiKey: string): Promise<EncryptedApiKey> {
    try {
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
    } catch (err) {
      throw OhosError.wrap('general chat encrypt api key', err);
    }
  }

  private async decryptApiKey(cipherText: string, ivText: string): Promise<string> {
    try {
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
    } catch (err) {
      throw OhosError.wrap('general chat decrypt api key', err);
    }
  }

  private async ensureHuksKey(): Promise<void> {
    if (await this.keyItemExists()) {
      return;
    }
    try {
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
    } catch (err) {
      throw OhosError.wrap('general chat generate huks key', err);
    }
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
    try {
      const value = await this.requireStore().get(key, '');
      return typeof value === 'string' ? value : '';
    } catch (err) {
      throw OhosError.wrap('general chat config get string', err);
    }
  }

  private requireStore(): preferences.Preferences {
    if (!this.store) {
      throw new Error('General chat configuration store is not initialized.');
    }
    return this.store;
  }

  private effectiveSelectedModelId(models: RemoteModelConfig[]): string {
    if (models.some((model: RemoteModelConfig): boolean => model.id === this.preferredModelId)) {
      return this.preferredModelId;
    }
    const local = models.find((model: RemoteModelConfig): boolean => model.id === GENERAL_CHAT_LOCAL_MODEL_ID);
    return local ? local.id : (models.length > 0 ? models[0].id : '');
  }

  private static isComplete(snapshot: GeneralChatConfigSnapshot): boolean {
    return snapshot.apiUrl.trim().length > 0 && snapshot.modelName.trim().length > 0 && snapshot.hasApiKey;
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
