import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const currentSessionTitleAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'current-session-title',
  parts: [
    { id: 'root', visualRole: 'toolbar', continuityGroup: 'session-header' },
    { id: 'title', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'create', propertyProfile: 'control', visualRole: 'control' },
  ],
};
