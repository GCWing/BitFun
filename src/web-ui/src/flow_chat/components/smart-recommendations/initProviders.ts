/**
 * Initialize recommendation providers
 */

import { recommendationRegistry } from './RecommendationRegistry';

let initialized = false;

export function initRecommendationProviders(): void {
  if (initialized) {
    return;
  }

  // Register additional providers here in the future
  // recommendationRegistry.registerProvider(new CodeQualityRecommendationProvider());
  // recommendationRegistry.registerProvider(new TestRecommendationProvider());
  // recommendationRegistry.registerProvider(new DocumentationRecommendationProvider());

  initialized = true;
}

export function cleanupRecommendationProviders(): void {
  if (!initialized) {
    return;
  }

  recommendationRegistry.dispose();
  initialized = false;
}

