import distributedKVStore from '@ohos.data.distributedKVStore';
import { common } from '@kit.AbilityKit';
import { distributedDeviceManager } from '@kit.DistributedServiceKit';
import { OhosError } from './OhosError';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { RemoteLogger } from './RemoteLogger';
import {
  WATCH_PROVISION_REQUEST_KEY,
  WATCH_PROVISION_RESPONSE_KEY
} from './WatchProvisionProtocol';

/**
 * Versioned because a store's `securityLevel` is fixed at creation: it can only
 * ever be raised, and for a store that syncs across devices it cannot be changed
 * at all. The pre-release S2 handoff store had to be
 * abandoned rather than relabelled — see the S1 note on `securityLevel` below.
 * Must stay identical to the watch's `DistributedHandoffStore`.
 */
const STORE_ID: string = HarmonyUpgradeIdentityContract.WATCH_HANDOFF_STORE;

export type WatchProvisionRequestHandler = (payload: string) => void;

/**
 * Phone half of the distributed KV channel the watch already listens on.
 *
 * `distributedKVStore` isolates stores by bundle name *and* store id and only
 * replicates between installs of the same app on the same account, which is
 * why the phone and watch retain one stable compatibility bundle id rather
 * than using platform-specific ids.
 *
 * The isolation key is really bundle name + **AppID**, and AppID is derived from
 * the signing certificate — so this project and the watch's must also be built
 * with the same signing material. Two DevEco-generated debug identities under
 * one bundle name produce two stores that never see each other, with no error on
 * either side. The store, the id and the option set below all have
 * to match the watch's `DistributedHandoffStore` exactly or the two apps end
 * up with private stores that never see each other's writes.
 */
export class WatchHandoffStore {
  private readonly context: common.UIAbilityContext;
  private kvManager?: distributedKVStore.KVManager;
  private kvStore?: distributedKVStore.SingleKVStore;
  private listener?: (change: distributedKVStore.ChangeNotification) => void;
  private syncListener?: (results: Array<[string, number]>) => void;
  private deviceManager?: distributedDeviceManager.DeviceManager;

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
          RemoteLogger.info('watch provision request arrived over KV');
          handler(String(entry.value.value));
        }
      });
    };
    try {
      store.on('dataChange', distributedKVStore.SubscribeType.SUBSCRIBE_TYPE_ALL, this.listener);
    } catch (err) {
      throw OhosError.wrap('watch handoff subscribe', err);
    }
    // A watch that asked while the phone app was closed gets picked up here;
    // the request's own age decides whether it is still worth acting on. The
    // pull first: the watch pushes on write, but a write that happened while
    // this app was not running was pushed at a replica that was not listening,
    // and only an explicit pull goes back for it.
    this.sync(store, distributedKVStore.SyncMode.PULL_ONLY);
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
    try {
      await store.put(WATCH_PROVISION_RESPONSE_KEY, payload);
    } catch (err) {
      throw OhosError.wrap('watch handoff write response', err);
    }
    // The watch is waiting on a foreground screen that blanks after nine
    // seconds; `autoSync`'s own schedule is far too slack to catch that window.
    this.sync(store, distributedKVStore.SyncMode.PUSH_ONLY);
  }

  /**
   * `autoSync` is a hint, not a guarantee — it batches on the platform's
   * schedule, which on this pair can be never. Both halves of the handoff push
   * their own writes explicitly, so a missing answer means a missing answer
   * rather than a sync that was still pending.
   */
  private sync(store: distributedKVStore.SingleKVStore, mode: distributedKVStore.SyncMode): void {
    const peers = this.peerNetworkIds();
    if (peers.length === 0) {
      RemoteLogger.warn('watch handoff: no trusted device online to sync with');
      return;
    }
    try {
      store.sync(peers, mode, 0);
      RemoteLogger.info(`watch handoff sync requested mode=${mode} peers=${peers.length}`);
    } catch (err) {
      RemoteLogger.warn(`watch handoff sync failed: ${WatchHandoffStore.errorText(err)}`);
    }
  }

  /**
   * Network ids of the trusted devices currently online. `sync` addresses
   * devices by network id, which is per-session, so this is resolved per call
   * rather than cached.
   */
  private peerNetworkIds(): string[] {
    try {
      if (!this.deviceManager) {
        this.deviceManager = distributedDeviceManager.createDeviceManager(
          this.context.abilityInfo.bundleName);
      }
      const ids: string[] = [];
      this.deviceManager.getAvailableDeviceListSync()
        .forEach((device: distributedDeviceManager.DeviceBasicInfo) => {
          const networkId = device.networkId;
          if (networkId && networkId.length > 0) {
            ids.push(networkId);
          }
        });
      return ids;
    } catch (err) {
      RemoteLogger.warn(`watch handoff device list failed: ${WatchHandoffStore.errorText(err)}`);
      return [];
    }
  }

  stop(): void {
    if (this.kvStore) {
      try {
        if (this.listener) {
          this.kvStore.off('dataChange', this.listener);
        }
        // By reference, never bare: an argument-less `off` drops every ArkTS
        // instance's listener on this store, not just this one's.
        if (this.syncListener) {
          this.kvStore.off('syncComplete', this.syncListener);
        }
      } catch (err) {
        RemoteLogger.warn(`watch handoff unsubscribe failed: ${WatchHandoffStore.errorText(err)}`);
      }
    }
    this.listener = undefined;
    this.syncListener = undefined;
    if (this.deviceManager) {
      try {
        distributedDeviceManager.releaseDeviceManager(this.deviceManager);
      } catch (err) {
        RemoteLogger.warn(`watch handoff release device manager failed: ${WatchHandoffStore.errorText(err)}`);
      }
      this.deviceManager = undefined;
    }
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
      // S1, and it has to be: a store only replicates to a device whose security
      // level is at least its data label, and a watch is an SL1 device — SL1
      // accepts S1 and nothing else. At S2 the sync was refused by policy before
      // it was ever attempted, which is indistinguishable from a dead link: no
      // `dataChange` here, and an empty RecentError on both devices.
      //
      // S1 is honest for what actually rides this store rather than a shortcut
      // around the rule. The master key is never here in the clear — the request
      // carries the watch's ephemeral public key, and the response carries a
      // blob sealed to it that only the watch can open. If plaintext credentials
      // needed to cross, the answer would be a second store, not a lower label.
      securityLevel: distributedKVStore.SecurityLevel.S1
    };
    try {
      this.kvStore = await this.kvManager.getKVStore<distributedKVStore.SingleKVStore>(STORE_ID, options);
      await this.kvStore.enableSync(true);
    } catch (err) {
      throw OhosError.wrap('watch handoff open store', err);
    }
    // The only place a refused or failed replication reports itself. Without it
    // a sync the platform dropped is indistinguishable from a watch that never
    // asked.
    this.syncListener = (results: Array<[string, number]>): void => {
      results.forEach((result: [string, number]) => {
        RemoteLogger.info(`watch handoff sync complete device=${result[0].substring(0, 8)} code=${result[1]}`);
      });
    };
    this.kvStore.on('syncComplete', this.syncListener);
    return this.kvStore;
  }

  private static errorText(err: Object): string {
    return err instanceof Error ? err.message : JSON.stringify(err);
  }
}
