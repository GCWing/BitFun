import { util } from '@kit.ArkTS';
import { BusinessError } from '@kit.BasicServicesKit';
import { asset } from '@kit.AssetStoreKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

const CORE_ASSET_ALIAS = 'bitfun.feedback.credentials.v1';
const CAPABILITY_ASSET_PREFIX = 'bitfun.feedback.capability.v1.';
const CAPABILITY_GROUP = 'bitfun.feedback.capabilities.v1';
const ASSET_NOT_FOUND = 24000002;
const ASSET_ALREADY_EXISTS = 24000003;
const LOG_DOMAIN = 0x0000;
const LOG_TAG = 'FeedbackCredentialStore';

class AssetStoreOperationError extends Error {
  readonly code: number;

  constructor(code: number) {
    super('AssetStore operation failed');
    this.code = code;
  }
}

interface CredentialRequest {
  action: string;
  value?: string;
}

interface CredentialResponse {
  status: string;
  value?: string;
  code?: string;
}

interface StoredCredentialPayload {
  enroll_key?: string;
  refresh_token?: string;
  capabilities?: Record<string, string>;
}

interface CoreCredentialPayload {
  enroll_key: string;
  refresh_token?: string;
}

interface StoredCapabilityMetadata {
  feedbackId: string;
  alias: Uint8Array;
}

interface StoredCapability extends StoredCapabilityMetadata {
  token: string;
}

export class FeedbackCredentialStore {
  private readonly encoder = new util.TextEncoder();
  private readonly decoder = util.TextDecoder.create('utf-8');
  private readonly coreAlias = this.encoder.encodeInto(CORE_ASSET_ALIAS);
  private readonly capabilityGroup = this.encoder.encodeInto(CAPABILITY_GROUP);

  async handle(input: string): Promise<string> {
    let action = 'parse';
    try {
      const request: CredentialRequest = JSON.parse(input);
      action = request.action;
      if (request.action === 'load') {
        return JSON.stringify(await this.load());
      }
      if (request.action === 'store' && request.value !== undefined) {
        await this.store(request.value);
        return JSON.stringify({ status: 'ok' } as CredentialResponse);
      }
      return JSON.stringify({ status: 'error', code: 'REQUEST_INVALID' } as CredentialResponse);
    } catch (error) {
      const businessError = error as BusinessError;
      const code = businessError.code === undefined ? 'INTERNAL_ERROR' : String(businessError.code);
      hilog.error(
        LOG_DOMAIN,
        LOG_TAG,
        'Secure credential operation failed: action=%{public}s code=%{public}s',
        action,
        code
      );
      return JSON.stringify({
        status: 'error',
        code
      } as CredentialResponse);
    }
  }

  private async load(): Promise<CredentialResponse> {
    const core = await this.loadCore();
    if (core.status !== 'ok' || core.value === undefined) {
      return core;
    }
    const payload = JSON.parse(core.value) as StoredCredentialPayload;
    const capabilities: Record<string, string> = payload.capabilities ?? {};
    const storedCapabilities = await this.queryCapabilities();
    storedCapabilities.forEach((item: StoredCapability) => {
      capabilities[item.feedbackId] = item.token;
    });
    payload.capabilities = capabilities;
    return { status: 'ok', value: JSON.stringify(payload) };
  }

  private async loadCore(): Promise<CredentialResponse> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, this.coreAlias);
    query.set(asset.Tag.RETURN_TYPE, asset.ReturnType.ALL);
    try {
      const results = await asset.query(query);
      if (results.length === 0) {
        return { status: 'not_found' };
      }
      const secret = results[0].get(asset.Tag.SECRET) as Uint8Array;
      if (secret === undefined) {
        return { status: 'error', code: 'SECRET_MISSING' };
      }
      return { status: 'ok', value: this.decoder.decodeToString(secret) };
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code === ASSET_NOT_FOUND) {
        return { status: 'not_found' };
      }
      throw new AssetStoreOperationError(businessError.code ?? -1);
    }
  }

  private async store(value: string): Promise<void> {
    const payload = JSON.parse(value) as StoredCredentialPayload;
    const capabilities: Record<string, string> = payload.capabilities ?? {};
    await this.synchronizeCapabilities(capabilities);

    const core: CoreCredentialPayload = {
      enroll_key: payload.enroll_key ?? ''
    };
    if (payload.refresh_token !== undefined) {
      core.refresh_token = payload.refresh_token;
    }
    await this.upsertAsset(this.coreAlias, JSON.stringify(core));
  }

  private async synchronizeCapabilities(capabilities: Record<string, string>): Promise<void> {
    const existing = await this.queryCapabilityMetadata();
    const desiredIds = Object.keys(capabilities);
    for (const feedbackId of desiredIds) {
      const alias = this.encoder.encodeInto(`${CAPABILITY_ASSET_PREFIX}${feedbackId}`);
      await this.upsertAsset(alias, capabilities[feedbackId], feedbackId);
    }
    for (const item of existing) {
      if (desiredIds.indexOf(item.feedbackId) < 0) {
        const query: asset.AssetMap = new Map();
        query.set(asset.Tag.ALIAS, item.alias);
        await asset.remove(query);
      }
    }
  }

  private async queryCapabilities(): Promise<StoredCapability[]> {
    const metadata = await this.queryCapabilityMetadata();
    const capabilities: StoredCapability[] = [];
    for (const item of metadata) {
      const token = await this.loadCapabilitySecret(item.alias);
      capabilities.push({
        feedbackId: item.feedbackId,
        token,
        alias: item.alias
      });
    }
    return capabilities;
  }

  private async queryCapabilityMetadata(): Promise<StoredCapabilityMetadata[]> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.DATA_LABEL_NORMAL_1, this.capabilityGroup);
    query.set(asset.Tag.RETURN_TYPE, asset.ReturnType.ATTRIBUTES);
    query.set(asset.Tag.RETURN_LIMIT, 1000);
    try {
      const results = await asset.query(query);
      const capabilities: StoredCapabilityMetadata[] = [];
      results.forEach((item: asset.AssetMap) => {
        const alias = item.get(asset.Tag.ALIAS) as Uint8Array;
        const feedbackId = item.get(asset.Tag.DATA_LABEL_NORMAL_2) as Uint8Array;
        if (alias !== undefined && feedbackId !== undefined) {
          capabilities.push({
            feedbackId: this.decoder.decodeToString(feedbackId),
            alias
          });
        }
      });
      return capabilities;
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code === ASSET_NOT_FOUND) {
        return [];
      }
      throw new AssetStoreOperationError(businessError.code ?? -1);
    }
  }

  private async loadCapabilitySecret(alias: Uint8Array): Promise<string> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, alias);
    query.set(asset.Tag.RETURN_TYPE, asset.ReturnType.ALL);
    const results = await asset.query(query);
    const secret = results[0]?.get(asset.Tag.SECRET) as Uint8Array;
    if (secret === undefined) {
      throw new Error('SECRET_MISSING');
    }
    return this.decoder.decodeToString(secret);
  }

  private async upsertAsset(alias: Uint8Array, value: string, feedbackId?: string): Promise<void> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, alias);
    const update: asset.AssetMap = new Map();
    update.set(asset.Tag.SECRET, this.encoder.encodeInto(value));
    try {
      await asset.update(query, update);
      return;
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code !== ASSET_NOT_FOUND) {
        throw new AssetStoreOperationError(businessError.code ?? -1);
      }
    }

    const attributes: asset.AssetMap = new Map();
    attributes.set(asset.Tag.ALIAS, alias);
    attributes.set(asset.Tag.SECRET, this.encoder.encodeInto(value));
    attributes.set(asset.Tag.ACCESSIBILITY, asset.Accessibility.DEVICE_FIRST_UNLOCKED);
    attributes.set(asset.Tag.AUTH_TYPE, asset.AuthType.NONE);
    attributes.set(asset.Tag.SYNC_TYPE, asset.SyncType.NEVER);
    if (feedbackId !== undefined) {
      attributes.set(asset.Tag.DATA_LABEL_NORMAL_1, this.capabilityGroup);
      attributes.set(asset.Tag.DATA_LABEL_NORMAL_2, this.encoder.encodeInto(feedbackId));
    }
    try {
      await asset.add(attributes);
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code !== ASSET_ALREADY_EXISTS) {
        throw new AssetStoreOperationError(businessError.code ?? -1);
      }
      await asset.update(query, update);
    }
  }
}
