 

import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { ITransportAdapter, type TransportRequestTiming } from './base';
import { createLogger } from '@/shared/utils/logger';
import { sanitizeErrorForLog } from '../logSanitizer';

const log = createLogger('TauriAdapter');

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
  private unlistenFunctions: UnlistenFn[] = [];
  private connected: boolean = false;
  private invokeFn: ((action: string, params?: any) => Promise<any>) | null = null;
  private initPromise: Promise<void> | null = null;

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
        timing.transportDurationMs = elapsedMs(transportStartedAt);
      }
      if (!isExpectedTauriRequestError(action, params, error)) {
        log.error('Request failed', { action, error: sanitizeErrorForLog(error) });
      }
      throw error;
    }
  }

  listen<T>(event: string, callback: (data: T) => void): () => void {
    let unlistenFn: UnlistenFn | null = null;
    let isUnlistened = false;

    listen<T>(event, (e) => {
      if (!isUnlistened) {
        try {
          callback(e.payload);
        } catch (error) {
      log.error('Error in event listener callback', { event, error: sanitizeErrorForLog(error) });
        }
      }
    }).then(fn => {
      if (isUnlistened) {
        fn();
      } else {
        unlistenFn = fn;
        this.unlistenFunctions.push(fn);
      }
    }).catch(error => {
      log.error('Failed to listen event', { event, error: sanitizeErrorForLog(error) });
    });

    return () => {
      isUnlistened = true;
      if (unlistenFn) {
        unlistenFn();
        const index = this.unlistenFunctions.indexOf(unlistenFn);
        if (index > -1) {
          this.unlistenFunctions.splice(index, 1);
        }
      }
    };
  }

  async disconnect(): Promise<void> {
    this.unlistenFunctions.forEach(fn => {
      try {
        fn();
      } catch (error) {
        log.error('Error while unlistening', sanitizeErrorForLog(error));
      }
    });
    this.unlistenFunctions = [];
    this.connected = false;
  }

  isConnected(): boolean {
    return this.connected;
  }
}


