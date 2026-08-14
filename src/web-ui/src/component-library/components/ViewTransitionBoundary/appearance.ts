import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

/**
 * Lifecycle-only surface. It deliberately exposes no paint properties; the
 * current and outgoing child trees continue to own their visual Appearance.
 */
export const viewTransitionBoundaryAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'view-transition-boundary',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface' },
    { id: 'view', visualRole: 'content' },
  ],
};
