 

import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { ITransportAdapter } from './base';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TauriAdapter');

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

  async request<T>(action: string, params?: any): Promise<T> {
    if (!this.connected) {
      await this.connect();
    }

    await this.ensureInitialized();

    try {
      if (!this.invokeFn) {
        throw new Error('Tauri invoke function not initialized');
      }
      const result = params !== undefined
        ? await this.invokeFn(action, params)
        : await this.invokeFn(action);

      return result as T;
    } catch (error) {
      log.error('Request failed', { action, error });
      throw error;
    }
  }

  listen<T>(event: string, callback: (data: T) => void): () => void {
    let unlistenFn: UnlistenFn | null = null;
    let isUnlistened = false;

    listen<T>(event, (e) => {
      if (!isUnlistened) {
        callback(e.payload);
      }
    }).then(fn => {
      if (isUnlistened) {
        fn();
      } else {
        unlistenFn = fn;
        this.unlistenFunctions.push(fn);
      }
    }).catch(error => {
      log.error('Failed to listen event', { event, error });
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
        log.error('Error while unlistening', error);
      }
    });
    this.unlistenFunctions = [];
    this.connected = false;
  }

  isConnected(): boolean {
    return this.connected;
  }
}


