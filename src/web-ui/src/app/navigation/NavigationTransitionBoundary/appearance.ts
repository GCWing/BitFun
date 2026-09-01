import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

/**
 * Lifecycle-only surface. Child views retain ownership of their visual
 * Appearance while this boundary owns the navigation transition layers.
 */
export const navigationTransitionBoundaryAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'navigation-transition-boundary',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface' },
    { id: 'layer', visualRole: 'content' },
  ],
};
