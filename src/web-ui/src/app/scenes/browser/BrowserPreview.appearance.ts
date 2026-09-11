import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const browserPreviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'browser-preview',
  parts: [{ id: 'image', propertyProfile: 'layout', visualRole: 'content' }],
};
