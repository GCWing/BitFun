import distributedKVStore from '@ohos.data.distributedKVStore';
import { common } from '@kit.AbilityKit';
import { RemoteLogger } from './RemoteLogger';
import {
  WATCH_PROVISION_REQUEST_KEY,
  WATCH_PROVISION_RESPONSE_KEY
} from './WatchProvisionProtocol';

const STORE_ID: string = 'bitfun_harmony_handoff';

export type WatchProvisionRequestHandler = (payload: string) => void;

/**
 * Phone half of the distributed KV channel the watch already listens on.
 *
 * `distributedKVStore` isolates stores by bundle name *and* store id and only
 * replicates between installs of the same app on the same account, which is
 * why the phone carries the watch's bundle name (`com.bitfun.app`) rather than
 * a store id of its own. The store, the id and the option set below all have
 * to match the watch's `DistributedHandoffStore` exactly or the two apps end
 * up with private stores that never see each other's writes.
 */
export class WatchHandoffStore {
  private readonly context: common.UIAbilityContext;
  private kvManager?: distributedKVStore.KVManager;
  private kvStore?: distributedKVStore.SingleKVStore;
  private listener?: (change: distributedKVStore.ChangeNotification) => void;

  constructor(context: common.UIAbilityContext) {
    this.context = context;
  }

  async start(handler: WatchProvisionRequestHandler): Promise<void> {
    const store = await this.getStore();
    this.listener = (change: distributedKVStore.ChangeNotification): void => {
      const entries = change.insertEntries.concat(change.updateEntries);
      entries.forEach((entry: distributedKVStore.Entry) => {
        if (entry.key === WATCH_PROVISION_REQUEST_KEY &&
          entry.value.type === distributedKVStore.ValueType.STRING) {
          handler(String(entry.value.value));
        }
      });
    };
    store.on('dataChange', distributedKVStore.SubscribeType.SUBSCRIBE_TYPE_ALL, this.listener);
    // A watch that asked while the phone app was closed gets picked up here;
    // the request's own age decides whether it is still worth acting on.
    try {
      const value = await store.get(WATCH_PROVISION_REQUEST_KEY);
      if (typeof value === 'string' && value.trim().length > 0) {
        handler(value);
      }
    } catch (_err) {
      // No request pending. dataChange delivers the next one.
    }
  }

  async writeResponse(payload: string): Promise<void> {
    const store = await this.getStore();
    await store.put(WATCH_PROVISION_RESPONSE_KEY, payload);
  }

  stop(): void {
    if (this.kvStore && this.listener) {
      try {
        this.kvStore.off('dataChange', this.listener);
      } catch (err) {
        RemoteLogger.warn(`watch handoff unsubscribe failed: ${WatchHandoffStore.errorText(err)}`);
      }
    }
    this.listener = undefined;
  }

  private async getStore(): Promise<distributedKVStore.SingleKVStore> {
    if (this.kvStore) {
      return this.kvStore;
    }
    if (!this.kvManager) {
      this.kvManager = distributedKVStore.createKVManager({
        bundleName: this.context.abilityInfo.bundleName,
        context: this.context
      });
    }
    const options: distributedKVStore.Options = {
      createIfMissing: true,
      encrypt: true,
      backup: false,
      autoSync: true,
      kvStoreType: distributedKVStore.KVStoreType.SINGLE_VERSION,
      securityLevel: distributedKVStore.SecurityLevel.S2
    };
    this.kvStore = await this.kvManager.getKVStore<distributedKVStore.SingleKVStore>(STORE_ID, options);
    await this.kvStore.enableSync(true);
    return this.kvStore;
  }

  private static errorText(err: Object): string {
    return err instanceof Error ? err.message : JSON.stringify(err);
  }
}
