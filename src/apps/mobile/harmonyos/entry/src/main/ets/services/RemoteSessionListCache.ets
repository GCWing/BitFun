import { RecentWorkspaceEntry, RemoteSession } from '../model/RemoteModels';
import { RemoteLogger } from './RemoteLogger';
import { RemoteResponseMapper } from './RemoteResponseMapper';

/** A device's session list as it was last shown. */
export interface RemoteSessionListSlice {
  sessions: RemoteSession[];
  hasMore: boolean;
}

/** Storage behind [RemoteSessionListCache]; see `RemoteSessionListRdbStore`. */
export interface RemoteSessionListStore {
  init(context: Context): Promise<void>;
  loadList(deviceKey: string): Promise<RemoteSessionListSlice>;
  /** The list for whichever device was written last, for a cold start. */
  loadLastList(): Promise<RemoteSessionListSlice>;
  saveList(deviceKey: string, sessions: RemoteSession[], hasMore: boolean): Promise<void>;
  /** Same row as [saveList], but does not move the cold-start last-device pointer. */
  saveObservedList(deviceKey: string, sessions: RemoteSession[], hasMore: boolean): Promise<void>;
  loadWorkspaces(deviceKey: string): Promise<RecentWorkspaceEntry[]>;
  saveWorkspaces(deviceKey: string, workspaces: RecentWorkspaceEntry[]): Promise<void>;
  clearAll(): Promise<void>;
}

/**
 * How many sessions per desktop are worth keeping.
 *
 * The list exists to fill the first screen while the connection settles, and
 * the desktop replaces it wholesale a moment later, so storing past what the
 * user can scroll to in that moment buys nothing.
 */
const CACHED_SESSIONS_PER_DEVICE: number = 60;

/**
 * The session list on disk, so Remote Home has something to show before the
 * desktop answers.
 *
 * Same bargain as `RemoteChatCache`: every failure is swallowed, because a
 * cache that cannot be read leaves the app exactly as slow as it was without
 * one. What is served here is a snapshot that may already be stale — titles
 * change, sessions get deleted — which is safe only because opening a session
 * goes through the desktop anyway and the live list overwrites this one as soon
 * as the connection is up.
 */
export class RemoteSessionListCache {
  private readonly store: RemoteSessionListStore;
  private readonly deviceKeyProvider: () => string;
  private ready: boolean = false;
  // Signature of the last list written, so the repeated `onSessions` callbacks
  // behind one refresh cost one write rather than one write each.
  private writtenScope: string = '';
  private writtenSignature: string = '';

  constructor(store: RemoteSessionListStore, deviceKeyProvider: () => string) {
    this.store = store;
    this.deviceKeyProvider = deviceKeyProvider;
  }

  async init(context: Context): Promise<void> {
    try {
      await this.store.init(context);
      this.ready = true;
    } catch (err) {
      RemoteLogger.error(`remote session list cache unavailable: ${RemoteSessionListCache.reason(err)}`);
    }
  }

  /**
   * The list for the device the phone last talked to.
   *
   * Used at startup, where the device to key on is not known yet: identity is
   * still being restored and the auto-reconnect that follows targets exactly
   * the device this list came from.
   */
  async restoreLast(): Promise<RemoteSessionListSlice> {
    if (!this.ready) {
      return RemoteSessionListCache.empty();
    }
    try {
      return RemoteSessionListCache.visible(await this.store.loadLastList());
    } catch (err) {
      RemoteLogger.error(`remote session list cache read failed: ${RemoteSessionListCache.reason(err)}`);
      return RemoteSessionListCache.empty();
    }
  }

  /** The list stored for the device currently being controlled. */
  async load(): Promise<RemoteSessionListSlice> {
    const deviceKey = this.scope();
    if (deviceKey.length === 0) {
      return RemoteSessionListCache.empty();
    }
    try {
      return RemoteSessionListCache.visible(await this.store.loadList(deviceKey));
    } catch (err) {
      RemoteLogger.error(`remote session list cache read failed: ${RemoteSessionListCache.reason(err)}`);
      return RemoteSessionListCache.empty();
    }
  }

  /**
   * Stores the list now on screen.
   *
   * An empty list is never written: disconnecting clears the list through the
   * same path a real refresh takes, and a user who steps away from a desktop
   * should still find their sessions there on the next launch.
   */
  async save(sessions: RemoteSession[], hasMore: boolean): Promise<void> {
    const deviceKey = this.scope();
    const visibleSessions = RemoteResponseMapper.visibleSessions(sessions);
    if (deviceKey.length === 0 || visibleSessions.length === 0) {
      return;
    }
    const kept = visibleSessions.slice(0, CACHED_SESSIONS_PER_DEVICE);
    const keptHasMore = hasMore || kept.length < visibleSessions.length;
    const signature = RemoteSessionListCache.signature(kept, keptHasMore);
    if (this.writtenScope === deviceKey && this.writtenSignature === signature) {
      return;
    }
    try {
      await this.store.saveList(deviceKey, kept, keptHasMore);
      this.writtenScope = deviceKey;
      this.writtenSignature = signature;
    } catch (err) {
      this.forgetSignature();
      RemoteLogger.error(`remote session list cache write failed: ${RemoteSessionListCache.reason(err)}`);
    }
  }

  /** Drops every stored list, for signing out. */
  async clear(): Promise<void> {
    this.forgetSignature();
    if (!this.ready) {
      return;
    }
    try {
      await this.store.clearAll();
    } catch (err) {
      RemoteLogger.error(`remote session list cache clear failed: ${RemoteSessionListCache.reason(err)}`);
    }
  }

  private forgetSignature(): void {
    this.writtenScope = '';
    this.writtenSignature = '';
  }

  /** The device to key on, or '' when there is nothing safe to key on. */
  private scope(): string {
    const deviceKey = this.deviceKeyProvider().trim();
    if (!this.ready || deviceKey.length === 0) {
      return '';
    }
    return deviceKey;
  }

  /**
   * Everything the list renders from, so a stored list is rewritten when a
   * title changes but not when the same list is published twice.
   */
  private static signature(sessions: RemoteSession[], hasMore: boolean): string {
    const parts = sessions.map((item: RemoteSession): string => {
      return `${item.id}${item.title}${item.status}${item.updatedAt}${item.messageCount}`;
    });
    return `${hasMore ? '1' : '0'}${parts.join('')}`;
  }

  private static empty(): RemoteSessionListSlice {
    return { sessions: [], hasMore: false };
  }

  private static visible(slice: RemoteSessionListSlice): RemoteSessionListSlice {
    return {
      sessions: RemoteResponseMapper.visibleSessions(slice.sessions),
      hasMore: slice.hasMore
    };
  }

  private static reason(err: Object): string {
    return err instanceof Error ? err.message : `${err}`;
  }
}
