import type { GlobalSearchProvider } from './types';
import { GLOBAL_SEARCH_PROVIDERS } from './providers';

type RegistryListener = () => void;

/**
 * Runtime contribution boundary for product features and installed extensions.
 * Providers keep ownership of querying and activation targets; the shell owns
 * only orchestration, cancellation, ranking, and presentation.
 */
export class GlobalSearchRegistry {
  private providers = new Map<string, GlobalSearchProvider>();
  private listeners = new Set<RegistryListener>();
  private snapshot: readonly GlobalSearchProvider[] = [];

  constructor(initialProviders: readonly GlobalSearchProvider[] = []) {
    initialProviders.forEach((provider) => this.add(provider));
  }

  register(provider: GlobalSearchProvider): () => void {
    if (this.providers.has(provider.id)) {
      throw new Error(`Global search provider already registered: ${provider.id}`);
    }
    this.add(provider);
    this.publish();
    let registered = true;
    return () => {
      if (!registered) return;
      registered = false;
      this.providers.delete(provider.id);
      this.publish();
    };
  }

  getSnapshot = (): readonly GlobalSearchProvider[] => this.snapshot;

  subscribe = (listener: RegistryListener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private add(provider: GlobalSearchProvider): void {
    if (!provider.id.trim()) {
      throw new Error('Global search providers require a stable id');
    }
    if (this.providers.has(provider.id)) {
      throw new Error(`Global search provider already registered: ${provider.id}`);
    }
    this.providers.set(provider.id, provider);
    this.snapshot = [...this.providers.values()];
  }

  private publish(): void {
    this.snapshot = [...this.providers.values()];
    this.listeners.forEach((listener) => listener());
  }
}

export const globalSearchRegistry = new GlobalSearchRegistry(GLOBAL_SEARCH_PROVIDERS);
