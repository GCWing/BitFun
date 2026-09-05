import { relationalStore } from '@kit.ArkData';
import { ChatMessage, RemoteSession } from '../../model/RemoteModels';
import { Encoding } from '../Encoding';
import { OhosError } from '../OhosError';
import { HarmonyUpgradeIdentityContract } from '../HarmonyUpgradeIdentityContract';
import { GeneralChatLocalStore } from './GeneralChatPort';

const STORE_CONFIG: relationalStore.StoreConfig = {
  name: HarmonyUpgradeIdentityContract.GENERAL_CHAT_DATABASE,
  securityLevel: relationalStore.SecurityLevel.S1
};

const CREATE_SESSIONS_SQL =
  'CREATE TABLE IF NOT EXISTS general_chat_sessions (' +
  'id TEXT PRIMARY KEY, updated_at TEXT NOT NULL, payload TEXT NOT NULL)';
const CREATE_MESSAGES_SQL =
  'CREATE TABLE IF NOT EXISTS general_chat_messages (' +
  'id TEXT PRIMARY KEY, session_id TEXT NOT NULL, sort_order INTEGER NOT NULL, payload TEXT NOT NULL)';
const CREATE_DRAFTS_SQL =
  'CREATE TABLE IF NOT EXISTS general_chat_drafts (' +
  'draft_id TEXT PRIMARY KEY, text TEXT NOT NULL, updated_at TEXT NOT NULL)';

export class GeneralChatLocalRdbStore implements GeneralChatLocalStore {
  private store?: relationalStore.RdbStore;

  async init(context: Context): Promise<void> {
    try {
      this.store = await relationalStore.getRdbStore(context, STORE_CONFIG);
      await this.store.executeSql(CREATE_SESSIONS_SQL);
      await this.store.executeSql(CREATE_MESSAGES_SQL);
      await this.store.executeSql(CREATE_DRAFTS_SQL);
      await this.store.executeSql(
        'CREATE INDEX IF NOT EXISTS idx_general_chat_messages_session ' +
        'ON general_chat_messages(session_id, sort_order)'
      );
    } catch (err) {
      throw OhosError.wrap('general chat store init', err);
    }
  }

  async listSessions(): Promise<RemoteSession[]> {
    try {
      const store = this.requireStore();
      const result = await store.querySql(
        'SELECT payload FROM general_chat_sessions ORDER BY updated_at DESC'
      );
      const sessions: RemoteSession[] = [];
      try {
        const payloadColumn = result.getColumnIndex('payload');
        while (result.goToNextRow()) {
          sessions.push(Encoding.parseJsonObject<RemoteSession>(result.getString(payloadColumn)));
        }
      } finally {
        GeneralChatLocalRdbStore.closeResult(result);
      }
      return sessions;
    } catch (err) {
      throw OhosError.wrap('general chat list sessions', err);
    }
  }

  async loadMessages(sessionId: string): Promise<ChatMessage[]> {
    try {
      const store = this.requireStore();
      const result = await store.querySql(
        'SELECT payload FROM general_chat_messages WHERE session_id = ? ORDER BY sort_order ASC',
        [sessionId]
      );
      const messages: ChatMessage[] = [];
      try {
        const payloadColumn = result.getColumnIndex('payload');
        while (result.goToNextRow()) {
          messages.push(Encoding.parseJsonObject<ChatMessage>(result.getString(payloadColumn)));
        }
      } finally {
        GeneralChatLocalRdbStore.closeResult(result);
      }
      return messages;
    } catch (err) {
      throw OhosError.wrap('general chat load messages', err);
    }
  }

  async upsertSession(session: RemoteSession): Promise<void> {
    try {
      const values: relationalStore.ValuesBucket = {
        id: session.id,
        updated_at: session.updatedAt,
        payload: JSON.stringify(session)
      };
      await this.requireStore().insert(
        'general_chat_sessions',
        values,
        relationalStore.ConflictResolution.ON_CONFLICT_REPLACE
      );
    } catch (err) {
      throw OhosError.wrap('general chat upsert session', err);
    }
  }

  async replaceMessages(sessionId: string, messages: ChatMessage[]): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql('DELETE FROM general_chat_messages WHERE session_id = ?', [sessionId]);
      for (let index = 0; index < messages.length; index++) {
        const message = messages[index];
        const values: relationalStore.ValuesBucket = {
          id: message.id,
          session_id: sessionId,
          sort_order: index,
          payload: JSON.stringify(message)
        };
        await store.insert(
          'general_chat_messages',
          values,
          relationalStore.ConflictResolution.ON_CONFLICT_REPLACE
        );
      }
    } catch (err) {
      throw OhosError.wrap('general chat replace messages', err);
    }
  }

  async deleteSession(sessionId: string): Promise<void> {
    try {
      const store = this.requireStore();
      await store.executeSql('DELETE FROM general_chat_messages WHERE session_id = ?', [sessionId]);
      await store.executeSql('DELETE FROM general_chat_drafts WHERE draft_id = ?', [sessionId]);
      await store.executeSql('DELETE FROM general_chat_sessions WHERE id = ?', [sessionId]);
    } catch (err) {
      throw OhosError.wrap('general chat delete session', err);
    }
  }

  async loadDraft(draftId: string): Promise<string> {
    try {
      const result = await this.requireStore().querySql(
        'SELECT text FROM general_chat_drafts WHERE draft_id = ?',
        [draftId]
      );
      try {
        if (!result.goToNextRow()) {
          return '';
        }
        return result.getString(result.getColumnIndex('text'));
      } finally {
        GeneralChatLocalRdbStore.closeResult(result);
      }
    } catch (err) {
      throw OhosError.wrap('general chat load draft', err);
    }
  }

  async saveDraft(draftId: string, text: string): Promise<void> {
    try {
      const values: relationalStore.ValuesBucket = {
        draft_id: draftId,
        text,
        updated_at: new Date().toISOString()
      };
      await this.requireStore().insert(
        'general_chat_drafts',
        values,
        relationalStore.ConflictResolution.ON_CONFLICT_REPLACE
      );
    } catch (err) {
      throw OhosError.wrap('general chat save draft', err);
    }
  }

  async deleteDraft(draftId: string): Promise<void> {
    try {
      await this.requireStore().executeSql(
        'DELETE FROM general_chat_drafts WHERE draft_id = ?',
        [draftId]
      );
    } catch (err) {
      throw OhosError.wrap('general chat delete draft', err);
    }
  }

  private static closeResult(result: relationalStore.ResultSet): void {
    try {
      result.close();
    } catch (err) {
      throw OhosError.wrap('general chat close cursor', err);
    }
  }

  private requireStore(): relationalStore.RdbStore {
    if (!this.store) {
      throw new Error('General chat local store is not initialized.');
    }
    return this.store;
  }
}
