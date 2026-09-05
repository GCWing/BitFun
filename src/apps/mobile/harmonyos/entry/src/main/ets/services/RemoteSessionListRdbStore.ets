import { relationalStore } from '@kit.ArkData';
import { RecentWorkspaceEntry, RemoteSession } from '../model/RemoteModels';
import { Encoding } from './Encoding';
import { OhosError } from './OhosError';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { RemoteSessionListSlice, RemoteSessionListStore } from './RemoteSessionListCache';

const STORE_CONFIG: relationalStore.StoreConfig = {
  name: HarmonyUpgradeIdentityContract.REMOTE_SESSIONS_DATABASE,
  // Session titles are as revealing as the transcripts themselves, so this
  // matches the level the transcript store uses.
  securityLevel: relationalStore.SecurityLevel.S2
};

/** Bumped whenever a stored row stops meaning what it used to; see the chat store. */
const SCHEMA_VERSION: number = 2;
const SCHEMA_VERSION_KEY: string = 'schema_version';
const LAST_DEVICE_KEY: string = 'last_device_key';

const CREATE_META_SQL =
  'CREATE TABLE IF NOT EXISTS remote_session_meta (' +
  'key TEXT PRIMARY KEY, value TEXT NOT NULL)';
// One row per device: the list is small, is always replaced as a whole, and is
// only ever read in full, so there is nothing for a per-session row to buy.
const CREATE_LIST_SQL =
  'CREATE TABLE IF NOT EXISTS remote_session_list (' +
  'device_key TEXT PRIMARY KEY, payload TEXT NOT NULL, has_more INTEGER NOT NULL, ' +
  'updated_at INTEGER NOT NULL)';
const CREATE_WORKSPACES_SQL =
  'CREATE TABLE IF NOT EXISTS remote_device_workspaces (' +
  'device_key TEXT PRIMARY KEY, payload TEXT NOT NULL, updated_at INTEGER NOT NULL)';

/** Session lists on disk, keyed by the desktop that owns them. */
export class RemoteSessionListRdbStore implements RemoteSessionListStore {
  private store?: relationalStore.RdbStore;

  async init(context: Context): Promise<void> {
    try {
      const store = await relationalStore.getRdbStore(context, STORE_CONFIG);
      await store.executeSql(CREATE_META_SQL);
      if (await RemoteSessionListRdbStore.storedValue(store, SCHEMA_VERSION_KEY) !== `${SCHEMA_VERSION}`) {
        await store.executeSql('DROP TABLE IF EXISTS remote_session_list');
        await store.executeSql('DROP TABLE IF EXISTS remote_device_workspaces');
        await store.executeSql(
          'DELETE FROM remote_session_meta WHERE key = ?',
          [LAST_DEVICE_KEY]
        );
        await store.executeSql(
          'INSERT OR REPLACE INTO remote_session_meta (key, value) VALUES (?, ?)',
          [SCHEMA_VERSION_KEY, `${SCHEMA_VERSION}`]
        );
      }
      await store.executeSql(CREATE_LIST_SQL);
      await store.executeSql(CREATE_WORKSPACES_SQL);
      this.store = store;
    } catch (err) {
      throw OhosError.wrap('remote session list store init', err);
    }
  }

  async loadList(deviceKey: string): Promise<RemoteSessionListSlice> {
    if (deviceKey.length === 0) {
      return RemoteSessionListRdbStore.empty();
    }
    try {
      const rows = await this.requireStore().querySql(
        'SELECT payload, has_more FROM remote_session_list WHERE device_key = ?',
        [deviceKey]
      );
      try {
        if (!rows.goToNextRow()) {
          return RemoteSessionListRdbStore.empty();
        }
        return {
          sessions: Encoding.parseJsonObject<RemoteSession[]>(rows.getString(rows.getColumnIndex('payload'))),
          hasMore: rows.getLong(rows.getColumnIndex('has_more')) === 1
        };
      } finally {
        RemoteSessionListRdbStore.closeResult(rows);
      }
    } catch (err) {
      throw OhosError.wrap('remote session list load', err);
    }
  }

  async loadLastList(): Promise<RemoteSessionListSlice> {
    const deviceKey = await RemoteSessionListRdbStore.storedValue(this.requireStore(), LAST_DEVICE_KEY);
    return await this.loadList(deviceKey);
  }

  async saveList(deviceKey: string, sessions: RemoteSession[], hasMore: boolean): Promise<void> {
    await this.writeList(deviceKey, sessions, hasMore, true);
  }

  async saveObservedList(deviceKey: string, sessions: RemoteSession[], hasMore: boolean): Promise<void> {
    await this.writeList(deviceKey, sessions, hasMore, false);
  }

  private async writeList(
    deviceKey: string,
    sessions: RemoteSession[],
    hasMore: boolean,
    rememberAsLast: boolean
  ): Promise<void> {
    if (deviceKey.length === 0) {
      return;
    }
    try {
      const store = this.requireStore();
      const values: relationalStore.ValuesBucket = {
        device_key: deviceKey,
        payload: JSON.stringify(sessions),
        has_more: hasMore ? 1 : 0,
        updated_at: Date.now()
      };
      await store.insert('remote_session_list', values, relationalStore.ConflictResolution.ON_CONFLICT_REPLACE);
      if (rememberAsLast) {
        // Written after the list, so a pointer never names a row that is not there.
        await store.executeSql(
          'INSERT OR REPLACE INTO remote_session_meta (key, value) VALUES (?, ?)',
          [LAST_DEVICE_KEY, deviceKey]
        );
      }
    } catch (err) {
      throw OhosError.wrap('remote session list save', err);
    }
  }

  async loadWorkspaces(deviceKey: string): Promise<RecentWorkspaceEntry[]> {
    if (deviceKey.length === 0) {
      return [];
    }
    try {
      const rows = await this.requireStore().querySql(
        'SELECT payload FROM remote_device_workspaces WHERE device_key = ?',
        [deviceKey]
      );
      try {
        if (!rows.goToNextRow()) {
          return [];
        }
        return Encoding.parseJsonObject<RecentWorkspaceEntry[]>(
          rows.getString(rows.getColumnIndex('payload'))
        );
      } finally {
        RemoteSessionListRdbStore.closeResult(rows);
      }
    } catch (err) {
      throw OhosError.wrap('remote device workspace load', err);
    }
  }

  async saveWorkspaces(deviceKey: string, workspaces: RecentWorkspaceEntry[]): Promise<void> {
    if (deviceKey.length === 0) {
      return;
    }
    try {
      const values: relationalStore.ValuesBucket = {
        device_key: deviceKey,
        payload: JSON.stringify(workspaces),
        updated_at: Date.now()
      };
      await this.requireStore().insert(
        'remote_device_workspaces',
        values,
        relationalStore.ConflictResolution.ON_CONFLICT_REPLACE
      );
    } catch (err) {
      throw OhosError.wrap('remote device workspace save', err);
    }
  }

  async clearAll(): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql('DELETE FROM remote_session_list');
      await store.executeSql('DELETE FROM remote_device_workspaces');
      await store.executeSql('DELETE FROM remote_session_meta WHERE key = ?', [LAST_DEVICE_KEY]);
    } catch (err) {
      throw OhosError.wrap('remote session list clear', err);
    }
  }

  private static async storedValue(store: relationalStore.RdbStore, key: string): Promise<string> {
    try {
      const rows = await store.querySql(
        'SELECT value FROM remote_session_meta WHERE key = ?',
        [key]
      );
      try {
        if (!rows.goToNextRow()) {
          return '';
        }
        return rows.getString(rows.getColumnIndex('value'));
      } finally {
        RemoteSessionListRdbStore.closeResult(rows);
      }
    } catch (err) {
      throw OhosError.wrap('remote session list stored value', err);
    }
  }

  private static closeResult(result: relationalStore.ResultSet): void {
    try {
      result.close();
    } catch (err) {
      throw OhosError.wrap('remote session list close cursor', err);
    }
  }

  private static empty(): RemoteSessionListSlice {
    return { sessions: [], hasMore: false };
  }

  private requireStore(): relationalStore.RdbStore {
    if (!this.store) {
      throw new Error('Remote session list local store is not initialized.');
    }
    return this.store;
  }
}
