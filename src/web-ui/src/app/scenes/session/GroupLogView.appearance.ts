import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupLogViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-log-view',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'body', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'emptyState', visualRole: 'content' },
  ],
};
