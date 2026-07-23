/**
 * Recommendation provider registry
 */

import { IRecommendationProvider, RecommendationProviderFactory } from './types';
class RecommendationRegistry {
  private static instance: RecommendationRegistry;
  private providers: Map<string, IRecommendationProvider> = new Map();
  private factories: Map<string, RecommendationProviderFactory> = new Map();

  private constructor() {}

  static getInstance(): RecommendationRegistry {
    if (!RecommendationRegistry.instance) {
      RecommendationRegistry.instance = new RecommendationRegistry();
    }
    return RecommendationRegistry.instance;
  }

  registerFactory(id: string, factory: RecommendationProviderFactory): void {
    this.factories.set(id, factory);
  }

  registerProvider(provider: IRecommendationProvider): void {
    this.providers.set(provider.id, provider);
  }

  unregisterProvider(id: string): void {
    const provider = this.providers.get(id);
    if (provider?.dispose) {
      provider.dispose();
    }
    this.providers.delete(id);
  }

  getProvider(id: string): IRecommendationProvider | undefined {
    // Create instance on demand when a factory exists
    if (!this.providers.has(id) && this.factories.has(id)) {
      const factory = this.factories.get(id)!;
      const provider = factory();
      this.providers.set(id, provider);
    }
    return this.providers.get(id);
  }

  getAllProviders(): IRecommendationProvider[] {
    const providers = Array.from(this.providers.values());
    return providers.sort((a, b) => b.priority - a.priority);
  }

  dispose(): void {
    this.providers.forEach(provider => {
      if (provider.dispose) {
        provider.dispose();
      }
    });
    this.providers.clear();
    this.factories.clear();
  }
}

export const recommendationRegistry = RecommendationRegistry.getInstance();
