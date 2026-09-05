import { relationalStore } from '@kit.ArkData';
import { ChatMessage } from '../model/RemoteModels';
import { Encoding } from './Encoding';
import { OhosError } from './OhosError';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { RemoteChatCacheSlice, RemoteChatCacheStore } from './RemoteChatCache';

const STORE_CONFIG: relationalStore.StoreConfig = {
  name: HarmonyUpgradeIdentityContract.REMOTE_CHAT_DATABASE,
  // A step above the S1 the general-chat store uses: these transcripts carry
  // the user's source, file paths and terminal output from a machine they own.
  securityLevel: relationalStore.SecurityLevel.S2
};

/**
 * Bumped whenever a stored row stops meaning what it used to.
 *
 * A cache is reconstructible by definition, so a mismatch drops the tables and
 * refills them from the desktop instead of paying for a migration.
 */
// Version 1 could retain an assistant row whose id was stable while its final
// body became durable. The cache is reconstructible, so invalidate those rows
// once instead of carrying potentially hollow completed replies forward.
const SCHEMA_VERSION: number = 2;
const SCHEMA_VERSION_KEY: string = 'schema_version';

const CREATE_META_SQL =
  'CREATE TABLE IF NOT EXISTS remote_chat_meta (' +
  'key TEXT PRIMARY KEY, value TEXT NOT NULL)';
const CREATE_SESSIONS_SQL =
  'CREATE TABLE IF NOT EXISTS remote_chat_sessions (' +
  'device_key TEXT NOT NULL, session_id TEXT NOT NULL, message_count INTEGER NOT NULL, ' +
  'last_message_id TEXT NOT NULL, updated_at INTEGER NOT NULL, ' +
  'PRIMARY KEY (device_key, session_id))';
// `seq` is the message's index in the desktop's own list, not an arbitrary sort
// key: `poll_session` returns the tail after `known_msg_count` entries, so the
// two have to count the same things in the same order.
const CREATE_MESSAGES_SQL =
  'CREATE TABLE IF NOT EXISTS remote_chat_messages (' +
  'device_key TEXT NOT NULL, session_id TEXT NOT NULL, seq INTEGER NOT NULL, payload TEXT NOT NULL, ' +
  'PRIMARY KEY (device_key, session_id, seq))';

/** Remote transcripts on disk, keyed by the desktop that owns the session. */
export class RemoteChatLocalRdbStore implements RemoteChatCacheStore {
  private store?: relationalStore.RdbStore;

  async init(context: Context): Promise<void> {
    try {
      const store = await relationalStore.getRdbStore(context, STORE_CONFIG);
      await store.executeSql(CREATE_META_SQL);
      if (await RemoteChatLocalRdbStore.storedSchemaVersion(store) !== SCHEMA_VERSION) {
        await store.executeSql('DROP TABLE IF EXISTS remote_chat_messages');
        await store.executeSql('DROP TABLE IF EXISTS remote_chat_sessions');
        await store.executeSql(
          'INSERT OR REPLACE INTO remote_chat_meta (key, value) VALUES (?, ?)',
          [SCHEMA_VERSION_KEY, `${SCHEMA_VERSION}`]
        );
      }
      await store.executeSql(CREATE_SESSIONS_SQL);
      await store.executeSql(CREATE_MESSAGES_SQL);
      this.store = store;
    } catch (err) {
      throw OhosError.wrap('remote chat store init', err);
    }
  }

  async loadSlice(deviceKey: string, sessionId: string): Promise<RemoteChatCacheSlice> {
    try {
      const store = this.requireStore();
      let expectedCount = 0;
      let lastMessageId = '';
      const header = await store.querySql(
        'SELECT message_count, last_message_id FROM remote_chat_sessions ' +
        'WHERE device_key = ? AND session_id = ?',
        [deviceKey, sessionId]
      );
      try {
        if (header.goToNextRow()) {
          expectedCount = header.getLong(header.getColumnIndex('message_count'));
          lastMessageId = header.getString(header.getColumnIndex('last_message_id'));
        }
      } finally {
        RemoteChatLocalRdbStore.closeResult(header);
      }
      if (expectedCount <= 0) {
        return { messages: [], lastMessageId: '' };
      }

      const messages: ChatMessage[] = [];
      const rows = await store.querySql(
        'SELECT payload FROM remote_chat_messages ' +
        'WHERE device_key = ? AND session_id = ? AND seq < ? ORDER BY seq ASC',
        [deviceKey, sessionId, expectedCount]
      );
      try {
        const payloadColumn = rows.getColumnIndex('payload');
        while (rows.goToNextRow()) {
          messages.push(Encoding.parseJsonObject<ChatMessage>(rows.getString(payloadColumn)));
        }
      } finally {
        RemoteChatLocalRdbStore.closeResult(rows);
      }
      // Fewer rows than the header claims means a write was interrupted partway.
      // Reporting a miss costs one fetch; reporting the rows would show the user
      // a transcript with messages missing out of the middle.
      if (messages.length !== expectedCount) {
        return { messages: [], lastMessageId: '' };
      }
      await this.touchSession(deviceKey, sessionId);
      return { messages, lastMessageId };
    } catch (err) {
      throw OhosError.wrap('remote chat load slice', err);
    }
  }

  async appendMessages(
    deviceKey: string,
    sessionId: string,
    startSeq: number,
    messages: ChatMessage[],
    lastMessageId: string
  ): Promise<void> {
    if (messages.length === 0) {
      return;
    }
    try {
      const store = this.requireStore();
      // Clearing the range first makes a retried append idempotent, which is what
      // lets the write run without a transaction around it.
      await store.executeSql(
        'DELETE FROM remote_chat_messages WHERE device_key = ? AND session_id = ? AND seq >= ?',
        [deviceKey, sessionId, startSeq]
      );
      await store.batchInsert(
        'remote_chat_messages',
        RemoteChatLocalRdbStore.rows(deviceKey, sessionId, startSeq, messages)
      );
      await this.writeHeader(deviceKey, sessionId, startSeq + messages.length, lastMessageId);
    } catch (err) {
      throw OhosError.wrap('remote chat append messages', err);
    }
  }

  async replaceMessages(
    deviceKey: string,
    sessionId: string,
    messages: ChatMessage[],
    lastMessageId: string
  ): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql(
        'DELETE FROM remote_chat_messages WHERE device_key = ? AND session_id = ?',
        [deviceKey, sessionId]
      );
      if (messages.length > 0) {
        await store.batchInsert(
          'remote_chat_messages',
          RemoteChatLocalRdbStore.rows(deviceKey, sessionId, 0, messages)
        );
      }
      await this.writeHeader(deviceKey, sessionId, messages.length, lastMessageId);
    } catch (err) {
      throw OhosError.wrap('remote chat replace messages', err);
    }
  }

  async deleteSession(deviceKey: string, sessionId: string): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql(
        'DELETE FROM remote_chat_messages WHERE device_key = ? AND session_id = ?',
        [deviceKey, sessionId]
      );
      await store.executeSql(
        'DELETE FROM remote_chat_sessions WHERE device_key = ? AND session_id = ?',
        [deviceKey, sessionId]
      );
    } catch (err) {
      throw OhosError.wrap('remote chat delete session', err);
    }
  }

  async pruneSessions(deviceKey: string, keepMostRecent: number): Promise<void> {
    try {
      const store = this.requireStore();
      const stale =
        'SELECT session_id FROM remote_chat_sessions WHERE device_key = ? ' +
        'ORDER BY updated_at DESC LIMIT -1 OFFSET ?';
      await store.executeSql(
        `DELETE FROM remote_chat_messages WHERE device_key = ? AND session_id IN (${stale})`,
        [deviceKey, deviceKey, keepMostRecent]
      );
      await store.executeSql(
        `DELETE FROM remote_chat_sessions WHERE device_key = ? AND session_id IN (${stale})`,
        [deviceKey, deviceKey, keepMostRecent]
      );
    } catch (err) {
      throw OhosError.wrap('remote chat prune sessions', err);
    }
  }

  async clearAll(): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql('DELETE FROM remote_chat_messages');
      await store.executeSql('DELETE FROM remote_chat_sessions');
    } catch (err) {
      throw OhosError.wrap('remote chat clear all', err);
    }
  }

  /** Keeps [pruneSessions] evicting the sessions the user stopped opening. */
  private async touchSession(deviceKey: string, sessionId: string): Promise<void> {
    try {
      await this.requireStore().executeSql(
        'UPDATE remote_chat_sessions SET updated_at = ? WHERE device_key = ? AND session_id = ?',
        [Date.now(), deviceKey, sessionId]
      );
    } catch (err) {
      throw OhosError.wrap('remote chat touch session', err);
    }
  }

  private async writeHeader(
    deviceKey: string,
    sessionId: string,
    messageCount: number,
    lastMessageId: string
  ): Promise<void> {
    const values: relationalStore.ValuesBucket = {
      device_key: deviceKey,
      session_id: sessionId,
      message_count: messageCount,
      last_message_id: lastMessageId,
      updated_at: Date.now()
    };
    try {
      await this.requireStore().insert(
        'remote_chat_sessions',
        values,
        relationalStore.ConflictResolution.ON_CONFLICT_REPLACE
      );
    } catch (err) {
      throw OhosError.wrap('remote chat write header', err);
    }
  }

  private static rows(
    deviceKey: string,
    sessionId: string,
    startSeq: number,
    messages: ChatMessage[]
  ): relationalStore.ValuesBucket[] {
    return messages.map((message: ChatMessage, index: number): relationalStore.ValuesBucket => ({
      device_key: deviceKey,
      session_id: sessionId,
      seq: startSeq + index,
      payload: JSON.stringify(message)
    }));
  }

  private static async storedSchemaVersion(store: relationalStore.RdbStore): Promise<number> {
    try {
      const result = await store.querySql(
        'SELECT value FROM remote_chat_meta WHERE key = ?',
        [SCHEMA_VERSION_KEY]
      );
      try {
        if (!result.goToNextRow()) {
          return 0;
        }
        return Number.parseInt(result.getString(result.getColumnIndex('value')), 10) || 0;
      } finally {
        RemoteChatLocalRdbStore.closeResult(result);
      }
    } catch (err) {
      throw OhosError.wrap('remote chat schema version', err);
    }
  }

  private static closeResult(result: relationalStore.ResultSet): void {
    try {
      result.close();
    } catch (err) {
      throw OhosError.wrap('remote chat close cursor', err);
    }
  }

  private requireStore(): relationalStore.RdbStore {
    if (!this.store) {
      throw new Error('Remote chat local store is not initialized.');
    }
    return this.store;
  }
}
