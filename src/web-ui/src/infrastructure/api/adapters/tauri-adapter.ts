 

import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { ITransportAdapter, type TransportRequestTiming } from './base';
import { createLogger } from '@/shared/utils/logger';
import { routeSurfaceEvent } from '@/infrastructure/peer-device/deviceSurfaceRouting';
import { surfaceIdForDevice } from '@/infrastructure/peer-device/deviceSurface';
import { routeRuntimeSessionEvent } from '@/infrastructure/peer-device/runtimeSessionEventGate';
import { sanitizeErrorForLog } from '../logSanitizer';

const log = createLogger('TauriAdapter');

interface SharedTauriEventListener {
  subscriptions: Set<TauriEventSubscription>;
  unlistenFn: UnlistenFn | null;
  registrationPromise: Promise<void> | null;
  closed: boolean;
}

interface TauriEventSubscription {
  owner: TauriTransportAdapter;
  event: string;
  callback: (data: unknown) => void;
  listener: SharedTauriEventListener;
  active: boolean;
}

// The Tauri event bus is window-wide, while Peer Device Mode keeps multiple
// transport adapters alive. Route each native event once before fan-out so a
// positioned Session event cannot consume its cursor once per adapter.
const sharedTauriEventListeners = new Map<string, SharedTauriEventListener>();

function closeSharedTauriEventListener(
  event: string,
  shared: SharedTauriEventListener,
): void {
  if (shared.closed) {
    return;
  }
  shared.closed = true;
  if (sharedTauriEventListeners.get(event) === shared) {
    sharedTauriEventListeners.delete(event);
  }
  if (shared.unlistenFn) {
    try {
      shared.unlistenFn();
    } catch (error) {
      log.error('Error while unlistening', sanitizeErrorForLog(error));
    }
    shared.unlistenFn = null;
  }
}

function registerSharedTauriEventListener(
  event: string,
  shared: SharedTauriEventListener,
): Promise<void> {
  const registration = listen<unknown>(event, (e) => {
    if (shared.closed) {
      return;
    }

    // Capture the logical listeners that owned the event when it arrived. A
    // Session read may hold delivery after accepting the write; those owners
    // must still paint it when released, while later subscribers must not
    // receive the native event retroactively.
    const subscriptions = [...shared.subscriptions];

    // Peer devices stay attached while the UI renders another device, so
    // several product event streams share this bus. Only the rendered device
    // surface may reach product listeners.
    const route = routeSurfaceEvent(event, e.payload);
    if (!route.deliver) {
      return;
    }

    routeRuntimeSessionEvent(
      surfaceIdForDevice(route.sourceDeviceId),
      event,
      route.payload,
      payload => {
        for (const subscription of subscriptions) {
          try {
            subscription.callback(payload);
          } catch (error) {
            log.error('Error in event listener callback', {
              event,
              error: sanitizeErrorForLog(error),
            });
          }
        }
      },
    );
  }).then(fn => {
    if (shared.closed || sharedTauriEventListeners.get(event) !== shared) {
      fn();
    } else {
      shared.unlistenFn = fn;
    }
  }).catch(error => {
    log.error('Failed to listen event', { event, error: sanitizeErrorForLog(error) });
    if (sharedTauriEventListeners.get(event) === shared) {
      sharedTauriEventListeners.delete(event);
    }
    shared.closed = true;
  }).finally(() => {
    if (shared.registrationPromise === registration) {
      shared.registrationPromise = null;
    }
  });
  shared.registrationPromise = registration;
  return registration;
}

export function isExpectedTauriRequestError(action: string, params: unknown, error: unknown): boolean {
  if (action !== 'get_config') {
    return false;
  }

  const request = (params as { request?: unknown } | undefined)?.request;
  if (!request || typeof request !== 'object') {
    return false;
  }

  if (!(request as Record<string, unknown>).skipRetryOnNotFound) {
    return false;
  }

  const errorMessage = error instanceof Error ? error.message : String(error);
  const normalized = errorMessage.toLowerCase();
  return normalized.includes('not found') && normalized.includes('config path');
}

export class TauriTransportAdapter implements ITransportAdapter {
  private connected: boolean = false;
  private invokeFn: ((action: string, params?: any) => Promise<any>) | null = null;
  private initPromise: Promise<void> | null = null;
  private listenerRegistrationPromises = new Set<Promise<void>>();
  private eventSubscriptions = new Set<TauriEventSubscription>();

  supportsSearchStreamEvents(): boolean {
    return true;
  }

  // Lazy initialize Tauri API
  private async ensureInitialized() {
    if (this.invokeFn) return;

    if (this.initPromise) {
      await this.initPromise;
      return;
    }

    this.initPromise = this.doInitialize();
    await this.initPromise;
  }

  private async doInitialize() {
    try {
      // Check if Tauri API is available
      if (typeof window !== 'undefined' && !('__TAURI__' in window)) {
        log.warn('Tauri API not available, running in non-Tauri environment');
        this.invokeFn = async () => {
          throw new Error('Tauri API is not available. Make sure you are running in a Tauri environment.');
        };
        return;
      }

      const tauriApi = await import('@tauri-apps/api/core');
      this.invokeFn = tauriApi.invoke;
      log.debug('Tauri API initialized successfully');
    } catch (error) {
      log.error('Failed to load Tauri API', error);
      this.invokeFn = async () => {
        throw new Error('Failed to load Tauri API: ' + (error instanceof Error ? error.message : 'Unknown error'));
      };
    }
  }

  async connect(): Promise<void> {
    this.connected = true;
  }

  async request<T>(action: string, params?: any, timing?: TransportRequestTiming): Promise<T> {
    const transportStartedAt = nowMs();
    if (!this.connected) {
      await this.connect();
    }

    const adapterInitStartedAt = nowMs();
    await this.ensureInitialized();
    if (timing) {
      timing.adapterInitDurationMs = elapsedMs(adapterInitStartedAt);
    }

    try {
      if (!this.invokeFn) {
        throw new Error('Tauri invoke function not initialized');
      }
      const invokeStartedAt = nowMs();
      try {
        const result = params !== undefined
          ? await this.invokeFn(action, params)
          : await this.invokeFn(action);
        if (timing) {
          timing.invokeDurationMs = elapsedMs(invokeStartedAt);
          timing.transportDurationMs = elapsedMs(transportStartedAt);
        }

        return result as T;
      } catch (error) {
        if (timing) {
          timing.invokeDurationMs = elapsedMs(invokeStartedAt);
        }
        throw error;
      }
    } catch (error) {
      if (timing) {
        timing.transportDurationMs = elapsedMs(transportStartedAt);
      }
      if (!isExpectedTauriRequestError(action, params, error)) {
        log.error('Request failed', { action, error: sanitizeErrorForLog(error) });
      }
      throw error;
    }
  }

  listen<T>(event: string, callback: (data: T) => void): () => void {
    let shared = sharedTauriEventListeners.get(event);
    let needsRegistration = false;
    if (!shared) {
      shared = {
        subscriptions: new Set(),
        unlistenFn: null,
        registrationPromise: null,
        closed: false,
      };
      sharedTauriEventListeners.set(event, shared);
      needsRegistration = true;
    }

    const subscription: TauriEventSubscription = {
      owner: this,
      event,
      callback: callback as (data: unknown) => void,
      listener: shared,
      active: true,
    };
    shared.subscriptions.add(subscription);
    this.eventSubscriptions.add(subscription);

    const registration = needsRegistration
      ? registerSharedTauriEventListener(event, shared)
      : shared.registrationPromise;
    if (registration) {
      this.trackListenerRegistration(registration);
    }

    return () => this.removeEventSubscription(subscription);
  }

  private trackListenerRegistration(registration: Promise<void>): void {
    this.listenerRegistrationPromises.add(registration);
    void registration.finally(() => {
      this.listenerRegistrationPromises.delete(registration);
    });
  }

  private removeEventSubscription(subscription: TauriEventSubscription): void {
    if (!subscription.active || subscription.owner !== this) {
      return;
    }
    subscription.active = false;
    subscription.listener.subscriptions.delete(subscription);
    this.eventSubscriptions.delete(subscription);
    if (subscription.listener.subscriptions.size === 0) {
      closeSharedTauriEventListener(subscription.event, subscription.listener);
    }
  }

  async disconnect(): Promise<void> {
    for (const subscription of [...this.eventSubscriptions]) {
      this.removeEventSubscription(subscription);
    }
    this.connected = false;
  }

  isConnected(): boolean {
    return this.connected;
  }

  async waitForListenerRegistrations(): Promise<void> {
    while (this.listenerRegistrationPromises.size > 0) {
      // Listener registration is async, and settling one registration can reveal
      // another registration issued in the same initialization wave.
      await Promise.allSettled(Array.from(this.listenerRegistrationPromises));
    }
  }
}
