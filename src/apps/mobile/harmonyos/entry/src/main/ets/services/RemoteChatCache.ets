import { ChatMessage } from '../model/RemoteModels';
import { RemoteLogger } from './RemoteLogger';

/**
 * One session's cached transcript, as it was last written.
 *
 * `lastMessageId` is what makes an append safe: the desktop hands out message
 * tails by index (`poll_session` skips `known_msg_count` entries), so a cached
 * prefix that no longer matches the desktop's would silently splice two
 * different transcripts together.
 */
export interface RemoteChatCacheSlice {
  messages: ChatMessage[];
  lastMessageId: string;
}

/** Storage behind [RemoteChatCache]; see `RemoteChatLocalRdbStore`. */
export interface RemoteChatCacheStore {
  init(context: Context): Promise<void>;
  loadSlice(deviceKey: string, sessionId: string): Promise<RemoteChatCacheSlice>;
  appendMessages(
    deviceKey: string,
    sessionId: string,
    startSeq: number,
    messages: ChatMessage[],
    lastMessageId: string
  ): Promise<void>;
  replaceMessages(
    deviceKey: string,
    sessionId: string,
    messages: ChatMessage[],
    lastMessageId: string
  ): Promise<void>;
  deleteSession(deviceKey: string, sessionId: string): Promise<void>;
  pruneSessions(deviceKey: string, keepMostRecent: number): Promise<void>;
  clearAll(): Promise<void>;
}

/**
 * How many sessions per desktop keep a cached transcript.
 *
 * The list a phone actually revisits is short, and every session past that is
 * disk spent on a transcript the user will re-fetch anyway. Past the budget the
 * least recently opened transcript goes first, and opening counts as reading —
 * so a session the user keeps coming back to outlives any number of one-off
 * ones opened around it.
 */
const CACHED_SESSIONS_PER_DEVICE: number = 20;

/**
 * How many transcripts stay in memory in front of the disk ones.
 *
 * Three covers what the phone actually alternates between — the session being
 * worked in, the one before it, and whatever was glanced at in between — and
 * costs nothing on disk. Past that the read off disk is already fast enough
 * that holding more would be spending memory to save nothing.
 */
const RESIDENT_SESSIONS: number = 3;

/**
 * On-disk transcripts, so reopening a session renders before the network answers.
 *
 * Everything here is reconstructible from the desktop, which is what lets every
 * method swallow its own failures: a cache that cannot be read or written makes
 * the app as slow as it was before the cache existed, and nothing worse. The
 * one thing it must never do is serve a transcript that is missing messages out
 * of the middle, so any doubt about a stored prefix is resolved by reporting a
 * miss.
 *
 * Entries are scoped per desktop: `session_id` is issued by the desktop that
 * owns the session, so two desktops on one account can hand out the same id for
 * different conversations.
 */
export class RemoteChatCache {
  private readonly store: RemoteChatCacheStore;
  private readonly deviceKeyProvider: () => string;
  private ready: boolean = false;
  // Write cursor for the session on screen, so a poll tick costs one append
  // rather than a read followed by an append.
  private trackedScope: string = '';
  private trackedCount: number = 0;
  private trackedLastMessageId: string = '';
  // Transcripts held in memory, most recently used last.
  //
  // Only ever populated from a disk read that succeeded or a disk write that
  // succeeded, so a resident copy can never claim more messages than the store
  // actually holds — which is what lets a resident hit set the append cursor
  // instead of forcing the next sync to rewrite the session.
  private readonly resident: Map<string, RemoteChatCacheSlice> = new Map<string, RemoteChatCacheSlice>();
  private residentOrder: string[] = [];

  constructor(store: RemoteChatCacheStore, deviceKeyProvider: () => string) {
    this.store = store;
    this.deviceKeyProvider = deviceKeyProvider;
  }

  async init(context: Context): Promise<void> {
    try {
      await this.store.init(context);
      this.ready = true;
    } catch (err) {
      RemoteLogger.error(`remote chat cache unavailable: ${RemoteChatCache.reason(err)}`);
    }
  }

  /** The cached transcript for [sessionId], or an empty list on any miss. */
  async load(sessionId: string): Promise<ChatMessage[]> {
    const scope = this.scope(sessionId);
    if (scope.length === 0) {
      return [];
    }
    const resident = this.resident.get(scope);
    if (resident !== undefined) {
      this.touchResident(scope);
      this.track(scope, resident.messages.length, resident.lastMessageId);
      RemoteLogger.info(`remote chat cache resident hit count=${resident.messages.length}`);
      return resident.messages.slice();
    }
    try {
      const slice = await this.store.loadSlice(this.deviceKeyProvider(), sessionId);
      this.track(scope, slice.messages.length, slice.lastMessageId);
      if (slice.messages.length > 0) {
        this.remember(scope, slice.messages, slice.lastMessageId);
      }
      return slice.messages;
    } catch (err) {
      this.forgetCursor();
      this.evictResident(scope);
      RemoteLogger.error(`remote chat cache read failed: ${RemoteChatCache.reason(err)}`);
      return [];
    }
  }

  /**
   * Persists an authoritative transcript returned by `get_session_messages` or
   * `message_snapshot`.
   *
   * Message ids and counts are ordering facts, not content revisions: a turn
   * keeps the same assistant id while its final text, status and tool results
   * become durable. Therefore a full snapshot always replaces the cached rows,
   * even when its ids and length match what is already stored.
   */
  async replaceSnapshot(sessionId: string, persistedMessages: ChatMessage[]): Promise<void> {
    const scope = this.scope(sessionId);
    if (scope.length === 0) {
      return;
    }
    const lastMessageId = persistedMessages.length > 0
      ? persistedMessages[persistedMessages.length - 1].id
      : '';
    try {
      await this.store.replaceMessages(
        this.deviceKeyProvider(),
        sessionId,
        persistedMessages,
        lastMessageId
      );
      await this.store.pruneSessions(this.deviceKeyProvider(), CACHED_SESSIONS_PER_DEVICE);
      this.track(scope, persistedMessages.length, lastMessageId);
      if (persistedMessages.length > 0) {
        this.remember(scope, persistedMessages, lastMessageId);
      } else {
        this.evictResident(scope);
      }
    } catch (err) {
      this.handleWriteFailure(scope, err);
    }
  }

  /**
   * Persists the timeline after an additive poll delta was merged into it.
   *
   * This path may append because the caller did not receive an authoritative
   * replacement. A prefix mismatch still rewrites the session to avoid splicing
   * two different histories together.
   */
  async syncIncremental(sessionId: string, persistedMessages: ChatMessage[]): Promise<void> {
    const scope = this.scope(sessionId);
    if (scope.length === 0 || persistedMessages.length === 0) {
      return;
    }
    const lastMessageId = persistedMessages[persistedMessages.length - 1].id;
    try {
      if (this.canAppendTo(scope, persistedMessages)) {
        if (persistedMessages.length === this.trackedCount) {
          return;
        }
        await this.store.appendMessages(
          this.deviceKeyProvider(),
          sessionId,
          this.trackedCount,
          persistedMessages.slice(this.trackedCount),
          lastMessageId
        );
      } else {
        await this.store.replaceMessages(
          this.deviceKeyProvider(),
          sessionId,
          persistedMessages,
          lastMessageId
        );
        // Only a rewrite can bring a session into the cache that was not
        // already there, so this is the one branch eviction has anything to do:
        // a poll tick appending to a stored session leaves the count where it
        // was, and paying for a prune on every tick would be pure overhead.
        await this.store.pruneSessions(this.deviceKeyProvider(), CACHED_SESSIONS_PER_DEVICE);
      }
      this.track(scope, persistedMessages.length, lastMessageId);
      this.remember(scope, persistedMessages, lastMessageId);
    } catch (err) {
      this.handleWriteFailure(scope, err);
    }
  }

  /** Drops one session, for a delete or a transcript the desktop rewrote. */
  async forget(sessionId: string): Promise<void> {
    const scope = this.scope(sessionId);
    if (scope.length === 0) {
      return;
    }
    if (scope === this.trackedScope) {
      this.forgetCursor();
    }
    this.evictResident(scope);
    try {
      await this.store.deleteSession(this.deviceKeyProvider(), sessionId);
    } catch (err) {
      RemoteLogger.error(`remote chat cache evict failed: ${RemoteChatCache.reason(err)}`);
    }
  }

  /**
   * Drops every cached transcript, for signing out.
   *
   * Transcripts belong to the account that was signed in; leaving them on disk
   * for the next account to read is the one failure mode worth being loud about.
   */
  async clear(): Promise<void> {
    this.forgetCursor();
    this.resident.clear();
    this.residentOrder = [];
    if (!this.ready) {
      return;
    }
    try {
      await this.store.clearAll();
    } catch (err) {
      RemoteLogger.error(`remote chat cache clear failed: ${RemoteChatCache.reason(err)}`);
    }
  }

  private canAppendTo(scope: string, persistedMessages: ChatMessage[]): boolean {
    return this.trackedScope === scope &&
      this.trackedCount > 0 &&
      this.trackedCount <= persistedMessages.length &&
      persistedMessages[this.trackedCount - 1].id === this.trackedLastMessageId;
  }

  private track(scope: string, count: number, lastMessageId: string): void {
    this.trackedScope = scope;
    this.trackedCount = count;
    this.trackedLastMessageId = lastMessageId;
  }

  private forgetCursor(): void {
    this.trackedScope = '';
    this.trackedCount = 0;
    this.trackedLastMessageId = '';
  }

  /** Takes a copy: the caller's array outlives this call and the timeline edits it. */
  private remember(scope: string, messages: ChatMessage[], lastMessageId: string): void {
    this.resident.set(scope, { messages: messages.slice(), lastMessageId });
    this.touchResident(scope);
    while (this.residentOrder.length > RESIDENT_SESSIONS) {
      const oldest = this.residentOrder.shift();
      if (oldest !== undefined) {
        this.resident.delete(oldest);
      }
    }
  }

  private touchResident(scope: string): void {
    this.residentOrder = this.residentOrder.filter((entry: string): boolean => entry !== scope);
    this.residentOrder.push(scope);
  }

  private evictResident(scope: string): void {
    this.resident.delete(scope);
    this.residentOrder = this.residentOrder.filter((entry: string): boolean => entry !== scope);
  }

  private handleWriteFailure(scope: string, err: Object): void {
    // Dropping the cursor turns the next incremental sync into a rewrite, which
    // is the only safe assumption once a write outcome is unknown. The resident
    // copy goes with it because the store may have changed underneath it.
    this.forgetCursor();
    this.evictResident(scope);
    RemoteLogger.error(`remote chat cache write failed: ${RemoteChatCache.reason(err)}`);
  }

  /** Cache key for a session, or '' when there is nothing safe to key on. */
  private scope(sessionId: string): string {
    const deviceKey = this.deviceKeyProvider().trim();
    if (!this.ready || deviceKey.length === 0 || sessionId.length === 0) {
      return '';
    }
    return `${deviceKey}::${sessionId}`;
  }

  private static reason(err: Object): string {
    return err instanceof Error ? err.message : `${err}`;
  }
}
