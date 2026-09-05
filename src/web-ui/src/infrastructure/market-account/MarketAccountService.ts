import {
  miniAppMarketAPI,
  type DesktopAuthStart,
  type MarketMe,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';

export type MarketAccountStatus = 'loading' | 'signed-out' | 'signed-in' | 'authorizing';

export interface MarketAccountSnapshot {
  resolved: boolean;
  status: MarketAccountStatus;
  me: MarketMe | null;
  lastError?: MarketAccountError;
}

export type MarketAccountErrorCode = 'cancelled' | 'expired' | 'failed';

export class MarketAccountError extends Error {
  constructor(
    public readonly code: MarketAccountErrorCode,
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = 'MarketAccountError';
  }
}

export interface MarketAccountApi {
  me(): Promise<MarketMe | null>;
  authStart(): Promise<DesktopAuthStart>;
  authPoll(transaction: DesktopAuthStart): Promise<'pending' | 'authorized' | 'expired'>;
  logout(): Promise<void>;
  onAccountChanged?(handler: () => void): () => void;
}

export interface MarketAccountChangedEvent {
  kind: 'identity-changed';
  eventId: string;
  sourceId: string;
}

export interface MarketAccountSyncPort {
  publish(event: MarketAccountChangedEvent): void | Promise<void>;
  subscribe(listener: (event: MarketAccountChangedEvent) => void): () => void;
  dispose?(): void;
}

const noopSyncPort: MarketAccountSyncPort = {
  publish: () => undefined,
  subscribe: () => () => undefined,
};

function isMarketAccountChangedEvent(value: unknown): value is MarketAccountChangedEvent {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const event = value as Record<string, unknown>;
  return event.kind === 'identity-changed'
    && typeof event.eventId === 'string'
    && typeof event.sourceId === 'string';
}

class BroadcastMarketAccountSyncPort implements MarketAccountSyncPort {
  private readonly listeners = new Set<(event: MarketAccountChangedEvent) => void>();

  constructor(private readonly channel: BroadcastChannel) {
    channel.addEventListener('message', this.handleMessage);
  }

  publish(event: MarketAccountChangedEvent): void {
    this.channel.postMessage(event);
  }

  subscribe(listener: (event: MarketAccountChangedEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  dispose(): void {
    this.channel.removeEventListener('message', this.handleMessage);
    this.channel.close();
    this.listeners.clear();
  }

  private readonly handleMessage = (message: MessageEvent<unknown>): void => {
    const event = message.data;
    if (!isMarketAccountChangedEvent(event)) return;
    this.listeners.forEach(listener => listener(event));
  };
}

export function createMarketAccountSyncPort(): MarketAccountSyncPort {
  if (typeof BroadcastChannel === 'undefined') return noopSyncPort;
  try {
    return new BroadcastMarketAccountSyncPort(new BroadcastChannel('openbitfun-market-account'));
  } catch {
    return noopSyncPort;
  }
}

export interface MarketAccountServiceDependencies {
  api: MarketAccountApi;
  openExternal(url: string): Promise<void>;
  syncPort: MarketAccountSyncPort;
  now(): number;
  sleep(milliseconds: number): Promise<void>;
  sourceId: string;
}

function createId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const defaultDependencies = (): MarketAccountServiceDependencies => ({
  api: miniAppMarketAPI,
  openExternal: url => systemAPI.openExternal(url),
  syncPort: createMarketAccountSyncPort(),
  now: () => Date.now(),
  sleep: milliseconds => new Promise(resolve => globalThis.setTimeout(resolve, milliseconds)),
  sourceId: createId(),
});

export class MarketAccountService {
  private snapshot: MarketAccountSnapshot = Object.freeze({
    resolved: false,
    status: 'loading',
    me: null,
  });
  private readonly listeners = new Set<(snapshot: MarketAccountSnapshot) => void>();
  private initializePromise: Promise<void> | null = null;
  private refreshPromise: Promise<MarketMe | null> | null = null;
  private authPromise: Promise<MarketMe> | null = null;
  private authGeneration = 0;
  private stopSync: (() => void) | null = null;
  private stopNativeAccountEvents: (() => void) | null = null;
  private observingHost = false;

  constructor(private readonly dependencies: MarketAccountServiceDependencies = defaultDependencies()) {}

  getSnapshot(): MarketAccountSnapshot {
    return this.snapshot;
  }

  subscribe(listener: (snapshot: MarketAccountSnapshot) => void): () => void {
    this.listeners.add(listener);
    void this.initialize();
    return () => this.listeners.delete(listener);
  }

  initialize(): Promise<void> {
    if (this.initializePromise) return this.initializePromise;
    this.stopSync = this.dependencies.syncPort.subscribe(event => {
      if (event.sourceId === this.dependencies.sourceId) return;
      void this.refresh(false).catch(() => undefined);
    });
    this.stopNativeAccountEvents = this.dependencies.api.onAccountChanged?.(() => {
      void this.refresh(false).catch(() => undefined);
    }) ?? null;
    this.attachHostObservation();
    this.initializePromise = this.refresh()
      .then(() => undefined)
      .catch(error => {
        this.setSnapshot({
          resolved: true,
          status: 'signed-out',
          me: null,
          lastError: asMarketAccountError(error),
        });
      });
    return this.initializePromise;
  }

  refresh(broadcastChange = true): Promise<MarketMe | null> {
    if (this.refreshPromise) return this.refreshPromise;
    const previous = this.snapshot;
    this.refreshPromise = this.dependencies.api.me()
      .then(me => {
        this.setSnapshot({
          resolved: true,
          status: me ? 'signed-in' : this.snapshot.status === 'authorizing'
            ? 'authorizing'
            : 'signed-out',
          me,
        });
        if (broadcastChange
          && previous.resolved
          && identityKey(previous.me) !== identityKey(me)) {
          void this.publishIdentityChanged();
        }
        return me;
      })
      .finally(() => {
        this.refreshPromise = null;
      });
    return this.refreshPromise;
  }

  signIn(): Promise<MarketMe> {
    if (this.snapshot.me) return Promise.resolve(this.snapshot.me);
    if (this.authPromise) return this.authPromise;

    const generation = ++this.authGeneration;
    this.setSnapshot({
      ...this.snapshot,
      resolved: true,
      status: 'authorizing',
      lastError: undefined,
    });
    const operation = this.runSignIn(generation)
      .catch(error => {
        const failure = asMarketAccountError(error);
        if (generation === this.authGeneration) {
          this.setSnapshot({
            resolved: true,
            status: this.snapshot.me ? 'signed-in' : 'signed-out',
            me: this.snapshot.me,
            lastError: failure.code === 'cancelled' ? undefined : failure,
          });
        }
        throw failure;
      })
      .finally(() => {
        if (this.authPromise === operation) this.authPromise = null;
      });
    this.authPromise = operation;
    return operation;
  }

  cancelSignIn(): void {
    if (this.snapshot.status !== 'authorizing') return;
    this.authGeneration += 1;
    this.setSnapshot({
      resolved: true,
      status: this.snapshot.me ? 'signed-in' : 'signed-out',
      me: this.snapshot.me,
    });
  }

  async logout(): Promise<void> {
    this.cancelSignIn();
    await this.dependencies.api.logout();
    this.setSnapshot({ resolved: true, status: 'signed-out', me: null });
    await this.publishIdentityChanged();
  }

  dispose(): void {
    this.authGeneration += 1;
    this.stopSync?.();
    this.stopSync = null;
    this.stopNativeAccountEvents?.();
    this.stopNativeAccountEvents = null;
    this.dependencies.syncPort.dispose?.();
    this.detachHostObservation();
    this.listeners.clear();
  }

  private async runSignIn(generation: number): Promise<MarketMe> {
    const transaction = await this.dependencies.api.authStart();
    this.ensureCurrentAuth(generation);
    await this.dependencies.openExternal(transaction.authorizationUrl);
    const deadline = transaction.expiresAt * 1000;
    while (this.dependencies.now() < deadline) {
      await this.dependencies.sleep(Math.max(1, transaction.pollIntervalSeconds) * 1000);
      this.ensureCurrentAuth(generation);
      const status = await this.dependencies.api.authPoll(transaction);
      this.ensureCurrentAuth(generation);
      if (status === 'expired') break;
      if (status !== 'authorized') continue;

      const me = await this.dependencies.api.me();
      this.ensureCurrentAuth(generation);
      if (!me) {
        throw new MarketAccountError('failed', 'The market authorized GitHub but returned no account.');
      }
      this.setSnapshot({ resolved: true, status: 'signed-in', me });
      await this.publishIdentityChanged();
      return me;
    }
    throw new MarketAccountError('expired', 'The GitHub authorization expired.');
  }

  private ensureCurrentAuth(generation: number): void {
    if (generation !== this.authGeneration) {
      throw new MarketAccountError('cancelled', 'The GitHub authorization was cancelled.');
    }
  }

  private async publishIdentityChanged(): Promise<void> {
    await this.dependencies.syncPort.publish({
      kind: 'identity-changed',
      eventId: createId(),
      sourceId: this.dependencies.sourceId,
    });
  }

  private setSnapshot(snapshot: MarketAccountSnapshot): void {
    this.snapshot = Object.freeze(snapshot);
    this.listeners.forEach(listener => listener(this.snapshot));
  }

  private readonly handleHostFocus = (): void => {
    void this.refresh().catch(() => undefined);
  };

  private readonly handleVisibilityChange = (): void => {
    if (typeof document === 'undefined' || document.visibilityState === 'visible') {
      this.handleHostFocus();
    }
  };

  private attachHostObservation(): void {
    if (this.observingHost) return;
    this.observingHost = true;
    if (typeof window !== 'undefined') window.addEventListener('focus', this.handleHostFocus);
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', this.handleVisibilityChange);
    }
  }

  private detachHostObservation(): void {
    if (!this.observingHost) return;
    this.observingHost = false;
    if (typeof window !== 'undefined') window.removeEventListener('focus', this.handleHostFocus);
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.handleVisibilityChange);
    }
  }
}

function asMarketAccountError(error: unknown): MarketAccountError {
  if (error instanceof MarketAccountError) return error;
  return new MarketAccountError(
    'failed',
    error instanceof Error ? error.message : String(error),
    error,
  );
}

function identityKey(me: MarketMe | null): string {
  return me ? `${me.user.githubId}:${me.user.login}` : '';
}

export const marketAccountService = new MarketAccountService();
