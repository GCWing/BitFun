import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const ecosystemCompatibilityAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'ecosystem-compatibility',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace' },
    { id: 'sidebar', propertyProfile: 'layout', visualRole: 'panel' },
    { id: 'productList', visualRole: 'content' },
    { id: 'main', propertyProfile: 'layout', visualRole: 'continuous-surface' },
    { id: 'header', visualRole: 'toolbar' },
    { id: 'content', visualRole: 'content' },
  ],
};
