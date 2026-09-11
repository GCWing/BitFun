import { generateRequestId, type CloudAccountSession } from './CloudAccountClient';
import { deserializeCloudAccountSession, type StoredCloudAccountSession } from './CloudAccountSessionStore';
import { fromB64, generateKeyPair, toB64 } from './E2EEncryption';
import { migrateMobileNavigationController } from './MobileNavigationStore';
import { pairingRelayUrl } from './pairingLink';

const DATABASE = 'openbitfun-mobile-account';
const ACCOUNTS = 'accounts';
const CHANGE_CHANNEL = 'openbitfun.mobile.account.changed';
const LEGACY_SESSION_KEY = 'openbitfun.mobile.account_session.v2';

interface BrowserAccountRecord {
  version: 1;
  relayUrl: string;
  controllerDeviceId: string;
  privateKey: string;
  revision: number;
  session: { token: string; userId: string } | null;
  lastChange?: 'signed-out' | 'expired';
}

export interface BrowserAccountSnapshot {
  controllerDeviceId: string;
  privateKey: Uint8Array;
  revision: number;
  session: CloudAccountSession | null;
  lastChange?: 'signed-out' | 'expired';
}

export class BrowserAccountStorageError extends Error {
  constructor(readonly reason: 'unavailable' | 'invalid') {
    super(reason === 'invalid' ? 'Stored browser identity is unreadable.' : 'Browser storage is unavailable.');
    this.name = 'BrowserAccountStorageError';
  }
}

export class BrowserAccountChangedError extends Error {
  constructor() { super('Browser account changed during sign-in.'); this.name = 'BrowserAccountChangedError'; }
}

export function releaseBrowserAccount(snapshot: BrowserAccountSnapshot): void {
  snapshot.privateKey.fill(0);
  snapshot.session?.masterKey.fill(0);
}

function validateRecord(value: unknown, relayUrl: string): BrowserAccountRecord {
  const record = value as BrowserAccountRecord | null;
  try {
    if (!record || record.version !== 1 || record.relayUrl !== relayUrl
      || typeof record.controllerDeviceId !== 'string'
      || !/^[A-Za-z0-9_.-]{1,128}$/.test(record.controllerDeviceId)
      || typeof record.privateKey !== 'string'
      || !Number.isSafeInteger(record.revision) || record.revision < 0
      || (record.session !== null && (!record.session
        || typeof record.session.token !== 'string' || !record.session.token.trim()
        || typeof record.session.userId !== 'string' || !record.session.userId.trim()))) {
      throw new Error('Invalid record');
    }
    const key = fromB64(record.privateKey);
    const validKey = key.length === 32;
    key.fill(0);
    if (!validKey) throw new Error('Invalid key');
    return record;
  } catch { throw new BrowserAccountStorageError('invalid'); }
}

function snapshot(record: BrowserAccountRecord): BrowserAccountSnapshot {
  const privateKey = fromB64(record.privateKey);
  return {
    controllerDeviceId: record.controllerDeviceId,
    privateKey,
    revision: record.revision,
    session: record.session ? { ...record.session, masterKey: privateKey.slice() } : null,
    lastChange: record.lastChange,
  };
}

/**
 * One encrypted-RPC identity and account per browser profile + Relay endpoint.
 * IndexedDB read/write transactions serialize competing tabs even on LAN HTTP,
 * where Web Locks are unavailable. Navigation remains in each tab's own store.
 */
export class BrowserAccountStore {
  readonly relayUrl: string;
  private database: Promise<IDBDatabase> | null = null;
  private listeners = new Set<() => void>();
  private channel: BroadcastChannel | null = null;

  constructor(relayUrl: string) {
    const normalized = pairingRelayUrl(relayUrl);
    if (!normalized) throw new Error('Invalid Relay URL');
    this.relayUrl = normalized;
  }

  private open(): Promise<IDBDatabase> {
    if (this.database) return this.database;
    this.database = new Promise<IDBDatabase>((resolve, reject) => {
      let request: IDBOpenDBRequest;
      try { request = window.indexedDB.open(DATABASE, 1); }
      catch { reject(new BrowserAccountStorageError('unavailable')); return; }
      let settled = false;
      const fail = () => {
        settled = true;
        reject(new BrowserAccountStorageError('unavailable'));
      };
      request.onblocked = fail;
      request.onerror = fail;
      request.onupgradeneeded = () => request.result.createObjectStore(ACCOUNTS, { keyPath: 'relayUrl' });
      request.onsuccess = () => {
        const db = request.result;
        if (settled) { db.close(); return; }
        db.onversionchange = () => { db.close(); this.database = null; };
        resolve(db);
      };
    }).catch(error => { this.database = null; throw error; });
    return this.database;
  }

  private async transaction(
    mode: IDBTransactionMode,
    update: (current: BrowserAccountRecord | undefined) => BrowserAccountRecord | undefined,
  ): Promise<BrowserAccountRecord | undefined> {
    const db = await this.open();
    let changed = false;
    const result = await new Promise<BrowserAccountRecord | undefined>((resolve, reject) => {
      let tx: IDBTransaction;
      try { tx = db.transaction(ACCOUNTS, mode); }
      catch { reject(new BrowserAccountStorageError('unavailable')); return; }
      let next: BrowserAccountRecord | undefined;
      let failure: unknown;
      tx.oncomplete = () => resolve(next);
      tx.onabort = tx.onerror = () => reject(failure ?? new BrowserAccountStorageError('unavailable'));
      const store = tx.objectStore(ACCOUNTS);
      const request = store.get(this.relayUrl);
      request.onsuccess = () => {
        try {
          const current = request.result === undefined ? undefined : validateRecord(request.result, this.relayUrl);
          next = update(current);
          if (mode === 'readwrite' && next !== current && next) {
            store.put(validateRecord(next, this.relayUrl));
            changed = true;
          }
        } catch (error) { failure = error; tx.abort(); }
      };
    });
    if (changed) this.publish();
    return result;
  }

  private legacySession(): StoredCloudAccountSession | null {
    try {
      const raw = window.sessionStorage.getItem(LEGACY_SESSION_KEY);
      if (!raw) return null;
      const legacy = deserializeCloudAccountSession(raw);
      if (legacy?.relayUrl === this.relayUrl) return legacy;
      legacy?.session.masterKey.fill(0);
    } catch { /* Legacy storage is optional; never reset unreadable records. */ }
    return null;
  }

  private retireLegacy(legacy: StoredCloudAccountSession | null, current: BrowserAccountRecord): void {
    if (!legacy) return;
    if (current.session?.userId === legacy.session.userId) {
      migrateMobileNavigationController(legacy.session.userId, this.relayUrl,
        legacy.controllerDeviceId, current.controllerDeviceId);
    }
    try {
      // Only retire a successfully read legacy credential after the shared
      // transaction commits. A retained signed-out record prevents resurrection
      // when another, previously suspended legacy tab later loads this build.
      window.sessionStorage.removeItem(LEGACY_SESSION_KEY);
      window.sessionStorage.removeItem(`openbitfun.mobile.device_key:${this.relayUrl}:${legacy.controllerDeviceId}`);
    } catch { /* A shared record is authoritative even if legacy cleanup fails. */ }
  }

  async read(): Promise<BrowserAccountSnapshot> {
    const legacy = this.legacySession();
    try {
      let current = await this.transaction('readonly', value => value);
      if (!current || (current.revision === 0 && legacy)) {
        let candidate: BrowserAccountRecord;
        if (legacy) {
          candidate = { version: 1, relayUrl: this.relayUrl,
            controllerDeviceId: legacy.controllerDeviceId, privateKey: toB64(legacy.session.masterKey),
            revision: 1, session: { token: legacy.session.token, userId: legacy.session.userId } };
        } else {
          const keys = await generateKeyPair();
          candidate = { version: 1, relayUrl: this.relayUrl,
            controllerDeviceId: generateRequestId(), privateKey: toB64(keys.privateKey),
            revision: 0, session: null };
          keys.privateKey.fill(0);
        }
        current = await this.transaction('readwrite', value => {
          if (!value || (value.revision === 0 && !value.session && legacy)) return candidate;
          return value;
        });
      }
      if (!current) throw new BrowserAccountStorageError('unavailable');
      this.retireLegacy(legacy, current);
      return snapshot(current);
    } finally { legacy?.session.masterKey.fill(0); }
  }

  async saveSession(
    expected: BrowserAccountSnapshot, session: CloudAccountSession, isCurrent: () => boolean = () => true,
  ): Promise<BrowserAccountSnapshot> {
    const current = await this.transaction('readwrite', value => {
      if (!isCurrent() || !value || value.revision !== expected.revision || value.controllerDeviceId !== expected.controllerDeviceId) {
        throw new BrowserAccountChangedError();
      }
      if (value.privateKey !== toB64(session.masterKey)) throw new BrowserAccountStorageError('invalid');
      return { ...value, revision: value.revision + 1, lastChange: undefined,
        session: { token: session.token, userId: session.userId } };
    });
    return snapshot(current!);
  }

  /** Clear only the observed account, never a replacement login from another tab. */
  async clearSession(token: string, reason: 'signed-out' | 'expired'): Promise<void> {
    await this.transaction('readwrite', current => {
      if (!current || current.session?.token !== token) return current;
      return { ...current, revision: current.revision + 1, session: null, lastChange: reason };
    });
  }

  private emit = () => { for (const listener of this.listeners) listener(); };
  private onMessage = (event: MessageEvent) => {
    if (event.data?.relayUrl === this.relayUrl) this.emit();
  };
  private onStorage = (event: StorageEvent) => {
    if (event.key !== CHANGE_CHANNEL || !event.newValue) return;
    try { if (JSON.parse(event.newValue).relayUrl === this.relayUrl) this.emit(); }
    catch { /* Notifications are hints; credentials are read from the database. */ }
  };
  private onVisible = () => { if (document.visibilityState === 'visible') this.emit(); };

  private publish(): void {
    this.emit();
    const message = { relayUrl: this.relayUrl, nonce: generateRequestId() };
    try {
      const channel = this.channel ?? new BroadcastChannel(CHANGE_CHANNEL);
      channel.postMessage(message);
      if (channel !== this.channel) channel.close();
    } catch { /* Storage events also work in browsers without BroadcastChannel. */ }
    try { window.localStorage.setItem(CHANGE_CHANNEL, JSON.stringify(message)); }
    catch { /* No secrets in notifications; focus/resume also rechecks the record. */ }
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    if (this.listeners.size === 1) {
      try {
        this.channel = new BroadcastChannel(CHANGE_CHANNEL);
        this.channel.addEventListener('message', this.onMessage);
      } catch { /* Use storage and lifecycle events as a fallback. */ }
      window.addEventListener('storage', this.onStorage);
      window.addEventListener('focus', this.emit);
      window.addEventListener('pageshow', this.emit);
      document.addEventListener('visibilitychange', this.onVisible);
    }
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size) return;
      this.channel?.close(); this.channel = null;
      window.removeEventListener('storage', this.onStorage);
      window.removeEventListener('focus', this.emit);
      window.removeEventListener('pageshow', this.emit);
      document.removeEventListener('visibilitychange', this.onVisible);
    };
  }
}

const stores = new Map<string, BrowserAccountStore>();
export function getBrowserAccountStore(relayUrl: string): BrowserAccountStore {
  const normalized = pairingRelayUrl(relayUrl);
  if (!normalized) throw new Error('Invalid Relay URL');
  let store = stores.get(normalized);
  if (!store) { store = new BrowserAccountStore(normalized); stores.set(normalized, store); }
  return store;
}
