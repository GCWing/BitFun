import { util } from '@kit.ArkTS';
import { BusinessError } from '@kit.BasicServicesKit';
import { asset } from '@kit.AssetStoreKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

const MARKET_ASSET_ALIAS = 'bitfun.market.credentials.v1';
const ASSET_NOT_FOUND = 24000002;
const ASSET_ALREADY_EXISTS = 24000003;
const LOG_DOMAIN = 0x0000;
const LOG_TAG = 'MarketCredentialStore';

class MarketAssetOperationError extends Error {
  readonly code: number;

  constructor(code: number) {
    super('Market asset operation failed');
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

export class MarketCredentialStore {
  private readonly encoder = new util.TextEncoder();
  private readonly decoder = util.TextDecoder.create('utf-8');
  private readonly alias = this.encoder.encodeInto(MARKET_ASSET_ALIAS);

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
      if (request.action === 'clear') {
        return JSON.stringify(await this.clear());
      }
      return JSON.stringify({ status: 'error', code: 'REQUEST_INVALID' } as CredentialResponse);
    } catch (error) {
      const businessError = error as BusinessError;
      const code = businessError.code === undefined ? 'INTERNAL_ERROR' : String(businessError.code);
      hilog.error(
        LOG_DOMAIN,
        LOG_TAG,
        'Secure market credential operation failed: action=%{public}s code=%{public}s',
        action,
        code
      );
      return JSON.stringify({ status: 'error', code } as CredentialResponse);
    }
  }

  private async load(): Promise<CredentialResponse> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, this.alias);
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
      throw new MarketAssetOperationError(businessError.code ?? -1);
    }
  }

  private async store(value: string): Promise<void> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, this.alias);
    const update: asset.AssetMap = new Map();
    update.set(asset.Tag.SECRET, this.encoder.encodeInto(value));
    try {
      await asset.update(query, update);
      return;
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code !== ASSET_NOT_FOUND) {
        throw new MarketAssetOperationError(businessError.code ?? -1);
      }
    }

    const attributes: asset.AssetMap = new Map();
    attributes.set(asset.Tag.ALIAS, this.alias);
    attributes.set(asset.Tag.SECRET, this.encoder.encodeInto(value));
    attributes.set(asset.Tag.ACCESSIBILITY, asset.Accessibility.DEVICE_FIRST_UNLOCKED);
    attributes.set(asset.Tag.AUTH_TYPE, asset.AuthType.NONE);
    attributes.set(asset.Tag.SYNC_TYPE, asset.SyncType.NEVER);
    try {
      await asset.add(attributes);
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code !== ASSET_ALREADY_EXISTS) {
        throw new MarketAssetOperationError(businessError.code ?? -1);
      }
      await asset.update(query, update);
    }
  }

  private async clear(): Promise<CredentialResponse> {
    const query: asset.AssetMap = new Map();
    query.set(asset.Tag.ALIAS, this.alias);
    try {
      await asset.remove(query);
      return { status: 'ok' };
    } catch (error) {
      const businessError = error as BusinessError;
      if (businessError.code === ASSET_NOT_FOUND) {
        return { status: 'not_found' };
      }
      throw new MarketAssetOperationError(businessError.code ?? -1);
    }
  }
}
