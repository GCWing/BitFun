/**
 * Pending queue module
 *
 * Frontend-side message queue used while a session's current dialog turn is
 * still running. Items are kept here (NOT submitted to the backend scheduler)
 * until the session returns to IDLE, at which point the head item is drained
 * via the regular `sendMessage` path. Users may also pop an item early through
 * the "send now" action which triggers `agentAPI.steerDialogTurn`, injecting
 * the message mid-turn.
 *
 * State is persisted per session so the queue survives a page refresh.
 */

import { createLogger } from '@/shared/utils/logger';
import type { QueuedMessage } from '../../types/flow-chat';

const log = createLogger('PendingQueueModule');

const STORAGE_PREFIX = 'flowChat.pendingQueue.';
const MAX_QUEUE_DEPTH = 20;

export interface EnqueueInput {
  sessionId: string;
  content: string;
  displayMessage?: string;
  agentType?: string;
  imageContexts?: unknown[];
  imageDisplayData?: unknown[];
  /**
   * How many times this content has already been auto-restored from a failed
   * dialog turn. Items with `retryCount > 0` are treated as "failed-recovery"
   * entries: the auto-drain listener will skip them so the user must explicitly
   * confirm (edit / send now / delete). Defaults to 0.
   */
  retryCount?: number;
  /**
   * Initial item status. Used by the failed-recovery path to mark an item as
   * `'failed'` from the start so the UI shows the correct visual.
   */
  initialStatus?: QueuedMessage['status'];
}

export type PendingQueueListener = (sessionId: string, items: QueuedMessage[]) => void;

class PendingQueueManager {
  private static _instance: PendingQueueManager | null = null;
  private queues = new Map<string, QueuedMessage[]>();
  private listeners = new Set<PendingQueueListener>();
  private hydrated = false;

  static getInstance(): PendingQueueManager {
    if (!PendingQueueManager._instance) {
      PendingQueueManager._instance = new PendingQueueManager();
    }
    return PendingQueueManager._instance;
  }

  private constructor() {
    this.hydrateFromStorage();
  }

  /** Lazily load all per-session queues from localStorage on first construction. */
  private hydrateFromStorage(): void {
    if (this.hydrated) return;
    this.hydrated = true;
    if (typeof window === 'undefined' || !window.localStorage) return;
    try {
      for (let i = 0; i < window.localStorage.length; i++) {
        const key = window.localStorage.key(i);
        if (!key || !key.startsWith(STORAGE_PREFIX)) continue;
        const sessionId = key.slice(STORAGE_PREFIX.length);
        const raw = window.localStorage.getItem(key);
        if (!raw) continue;
        try {
          const parsed = JSON.parse(raw) as QueuedMessage[];
          if (Array.isArray(parsed) && parsed.length > 0) {
            this.queues.set(sessionId, parsed);
          }
        } catch (err) {
          log.warn('Failed to parse persisted queue, dropping', { sessionId, err });
          window.localStorage.removeItem(key);
        }
      }
    } catch (err) {
      log.warn('Queue hydration failed', err);
    }
  }

  private persist(sessionId: string): void {
    if (typeof window === 'undefined' || !window.localStorage) return;
    const items = this.queues.get(sessionId);
    const key = STORAGE_PREFIX + sessionId;
    try {
      if (!items || items.length === 0) {
        window.localStorage.removeItem(key);
      } else {
        window.localStorage.setItem(key, JSON.stringify(items));
      }
    } catch (err) {
      log.warn('Queue persistence failed', { sessionId, err });
    }
  }

  list(sessionId: string): QueuedMessage[] {
    return this.queues.get(sessionId) ?? [];
  }

  isFull(sessionId: string): boolean {
    return this.list(sessionId).length >= MAX_QUEUE_DEPTH;
  }

  enqueue(input: EnqueueInput): QueuedMessage {
    const items = this.queues.get(input.sessionId) ?? [];
    if (items.length >= MAX_QUEUE_DEPTH) {
      throw new Error(`Pending queue is full (max ${MAX_QUEUE_DEPTH})`);
    }
    const item: QueuedMessage = {
      id: `pending_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      sessionId: input.sessionId,
      content: input.content,
      displayMessage: input.displayMessage,
      timestamp: Date.now(),
      status: input.initialStatus ?? 'queued',
      retryCount: input.retryCount ?? 0,
      agentType: input.agentType,
      imageContexts: input.imageContexts,
      imageDisplayData: input.imageDisplayData,
    };
    items.push(item);
    this.queues.set(input.sessionId, items);
    this.persist(input.sessionId);
    this.notify(input.sessionId);
    return item;
  }

  update(
    sessionId: string,
    id: string,
    patch: Partial<Pick<QueuedMessage, 'content' | 'displayMessage'>>,
  ): boolean {
    const items = this.queues.get(sessionId);
    if (!items) return false;
    const idx = items.findIndex(item => item.id === id);
    if (idx === -1) return false;
    items[idx] = { ...items[idx], ...patch, timestamp: Date.now() };
    this.persist(sessionId);
    this.notify(sessionId);
    return true;
  }

  remove(sessionId: string, id: string): boolean {
    const items = this.queues.get(sessionId);
    if (!items) return false;
    const next = items.filter(item => item.id !== id);
    if (next.length === items.length) return false;
    if (next.length === 0) {
      this.queues.delete(sessionId);
    } else {
      this.queues.set(sessionId, next);
    }
    this.persist(sessionId);
    this.notify(sessionId);
    return true;
  }

  setStatus(sessionId: string, id: string, status: QueuedMessage['status']): void {
    const items = this.queues.get(sessionId);
    if (!items) return;
    const idx = items.findIndex(item => item.id === id);
    if (idx === -1) return;
    items[idx] = { ...items[idx], status };
    this.persist(sessionId);
    this.notify(sessionId);
  }

  /** Pop and return the head item (FIFO). */
  consumeNext(sessionId: string): QueuedMessage | undefined {
    const items = this.queues.get(sessionId);
    if (!items || items.length === 0) return undefined;
    const [head, ...rest] = items;
    if (rest.length === 0) {
      this.queues.delete(sessionId);
    } else {
      this.queues.set(sessionId, rest);
    }
    this.persist(sessionId);
    this.notify(sessionId);
    return head;
  }

  clear(sessionId: string): void {
    if (!this.queues.has(sessionId)) return;
    this.queues.delete(sessionId);
    this.persist(sessionId);
    this.notify(sessionId);
  }

  subscribe(listener: PendingQueueListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify(sessionId: string): void {
    const snapshot = this.list(sessionId).slice();
    this.listeners.forEach(listener => {
      try {
        listener(sessionId, snapshot);
      } catch (err) {
        log.error('Pending queue listener error', { sessionId, err });
      }
    });
  }
}

export const pendingQueueManager = PendingQueueManager.getInstance();
